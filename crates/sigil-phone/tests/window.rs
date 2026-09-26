//! The wake window against a real exchange, with a loopback distributor
//! standing in for the push service -- exactly the arrangement SIP-45 says a
//! desktop test client may use until a phone exists. This is the phone.
//!
//! What is proved: a window with nothing new says nothing (the negative
//! control); a message posted while the phone held no stream reaches the
//! distributor as `wake`; the next window says it, once, after it is on
//! disk; a busy channel is one notification; the quiet setting names
//! nothing; and a call rings.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ed25519_dalek::SigningKey;
use sigil_chat::session::{self, Cmd};
use sigil_net::Endpoint;
use sigil_phone::{Distributor, Fake, Outcome, Privacy, Registered, Step, Window};
use sqexd::config::FileConfig;
use sqnr_core::{PubKey, SoftwareSigner};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn server_in(dir: &Path) -> Endpoint {
    let key_path = dir.join("host_key");
    let (server_sk, _) = squic::generate_keypair();
    std::fs::write(&key_path, hex::encode(server_sk.to_bytes())).unwrap();
    let config_toml = format!(
        "listen = \"127.0.0.1:0\"\nkey_file = {:?}\nstate_file = {:?}\nadmins = []\n\
         welcome_channel = \"\"\nlimits = {{ posts = [0, 0], signals = [0, 0], joins = [0, 0], creates = [0, 0], uploads = [0, 0] }}\nwake_loopback = true\n",
        key_path.to_string_lossy(),
        dir.join("sqex.state").to_string_lossy(),
    );
    let config_path = dir.join("sqexd.toml");
    std::fs::write(&config_path, &config_toml).unwrap();
    let file: FileConfig = toml::from_str(&config_toml).unwrap();
    let config = file.resolve().unwrap();
    let (signing_key, _pub) =
        squic::load_keypair(&std::fs::read_to_string(&config.key_file).unwrap()).unwrap();
    let bound = sqexd::bind(config, Some(config_path), signing_key)
        .await
        .unwrap();
    let addr: SocketAddr = bound.local_addr;
    let server = PubKey::new(bound.public_key.to_bytes());
    tokio::spawn(async move {
        let _ = sqexd::serve(bound).await;
    });
    Endpoint {
        address: addr,
        server,
    }
}

fn signer(b: u8) -> (SoftwareSigner, PubKey) {
    let sk = SigningKey::from_bytes(&[b; 32]);
    let public = PubKey::new(sk.verifying_key().to_bytes());
    (SoftwareSigner::new(sk), public)
}

/// A push distributor, as far as an exchange can tell: a POST on loopback,
/// remembered.
struct DistributorStub {
    url: String,
    bodies: Arc<Mutex<Vec<Vec<u8>>>>,
}

async fn distributor() -> DistributorStub {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let bodies: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let seen = Arc::clone(&bodies);
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let seen = Arc::clone(&seen);
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let body = loop {
                    let n = match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => break None,
                        Ok(n) => n,
                    };
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..end]).to_string();
                        let len: usize = head
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        while buf.len() < end + 4 + len {
                            let n = match sock.read(&mut tmp).await {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            };
                            buf.extend_from_slice(&tmp[..n]);
                        }
                        break Some(buf[end + 4..].to_vec());
                    }
                };
                if let Some(body) = body {
                    seen.lock().unwrap().push(body);
                    let _ = sock
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await;
                }
            });
        }
    });
    DistributorStub {
        url: format!("http://127.0.0.1:{port}/up/phone"),
        bodies,
    }
}

/// The exchange's own request counter, from `/status`. Reading it is itself
/// a request, which the caller subtracts.
async fn requests_served(endpoint: Endpoint) -> u64 {
    let mut probe = sqnr::Client::connect(endpoint.address, endpoint.server.as_bytes())
        .await
        .unwrap();
    let (code, body) = probe.get("/status").await.unwrap();
    assert_eq!(code, 200);
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    status["requests"]
        .as_u64()
        .expect("status reports requests")
}

async fn until<F: FnMut() -> bool>(mut f: F, secs: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

fn window(endpoint: Endpoint, signer: SoftwareSigner, store: &Path, d: &DistributorStub) -> Window {
    let mut w = Window::new(endpoint, signer);
    w.store_at = Some(store.to_path_buf());
    w.distributor = Some(Distributor {
        url: d.url.clone(),
        ttl_secs: 3600,
    });
    w.budget = Duration::from_secs(20);
    w.settle = Duration::from_millis(600);
    w
}

fn position(out: &Outcome, step: &Step) -> Option<usize> {
    out.steps.iter().position(|s| s == step)
}

#[tokio::test]
async fn a_phone_is_woken_for_a_message_and_says_it_once_after_writing_it() {
    if let Ok(filter) = std::env::var("SIGIL_PHONE_LOG") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .try_init();
    }
    let dir = tempfile::tempdir().unwrap();
    let endpoint = server_in(dir.path()).await;
    let push = distributor().await;
    let phone_store = dir.path().join("phone.db");
    let (phone_signer, phone_id) = signer(1);
    let seed = phone_signer.seed();
    let (friend_signer, friend_id) = signer(2);

    // Window 1: the phone's first look. Nothing has happened, so nothing is
    // said -- the negative control for everything below.
    let fake = Fake::default();
    let out =
        sigil_phone::window::run(window(endpoint, signer(1).0, &phone_store, &push), &fake).await;
    assert_eq!(out.trouble, None, "{out:?}");
    assert!(out.connected && out.synced && !out.ran_out, "{out:?}");
    assert_eq!(out.registered, Some(Registered::Kept), "{out:?}");
    assert_eq!(out.notified, 0, "nothing new, nothing said: {out:?}");
    assert!(fake.notifications().is_empty());
    assert_eq!(out.steps.last(), Some(&Step::Closed));

    // Somebody writes while the phone holds no stream.
    let friend = session::start(
        endpoint,
        friend_signer,
        Some(dir.path().join("friend.db")),
        || {},
    );
    assert!(
        until(|| friend.state().me == Some(friend_id), 15).await,
        "{:?}",
        friend.state().trouble
    );
    friend.send(Cmd::OpenDm(phone_id));
    assert!(
        until(|| friend.state().open.is_some(), 15).await,
        "{:?}",
        friend.state().trouble
    );
    friend.send(Cmd::Send("hello phone".into()));
    assert!(
        until(
            || friend
                .state()
                .lines
                .iter()
                .any(|l| l.text.contains("hello phone")),
            15
        )
        .await,
        "the friend's message should post: {:?}",
        friend.state().trouble
    );

    // SIP-45: the exchange posts `wake` to the distributor the phone left.
    assert!(
        until(|| !push.bodies.lock().unwrap().is_empty(), 10).await,
        "the distributor should have been woken"
    );
    assert!(push.bodies.lock().unwrap().iter().all(|b| b == b"wake"));

    // Window 2: woken, the phone connects, writes, then says. This is the
    // window that first hears of the conversation, so it cannot be caught
    // up (SIP-52 names what the store holds); window 4 below is measured.
    let before = requests_served(endpoint).await;
    let fake = Fake::default();
    let out =
        sigil_phone::window::run(window(endpoint, signer(1).0, &phone_store, &push), &fake).await;
    let cost = requests_served(endpoint).await - before - 2;
    eprintln!("a wake window that first hears of a conversation cost {cost} requests");
    assert_eq!(out.trouble, None, "{out:?}");
    assert_eq!(out.arrivals, 1, "{out:?}");
    assert_eq!(out.notified, 1, "{out:?}");
    let said = fake.notifications();
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].body, "hello phone");
    assert_eq!(said[0].count, 1);
    let channel = said[0]
        .channel
        .expect("a direct message names its conversation");
    // The order the SIP exists for: said after synced, closed last.
    let synced = position(&out, &Step::Synced).expect("synced");
    let spoke = position(
        &out,
        &Step::Said {
            notified: 1,
            rang: 0,
        },
    )
    .expect("said");
    assert!(synced < spoke, "{:?}", out.steps);
    assert_eq!(out.steps.last(), Some(&Step::Closed));

    // And it is on disk: the window's own copy, which is the only copy that
    // will ever open. Read back with the lock free, which `Closed` promises.
    let mut store = sqex_chat::store::Store::open(&seed, Some(&phone_store)).unwrap();
    store.scope_to(&endpoint.server).unwrap();
    let held = store.entries_after(&channel, 0, 16).unwrap();
    assert!(
        !held.is_empty(),
        "the message should be in the phone's store"
    );

    // Window 3: nothing new since. Said once means said once.
    let fake = Fake::default();
    let out =
        sigil_phone::window::run(window(endpoint, signer(1).0, &phone_store, &push), &fake).await;
    assert_eq!(out.trouble, None, "{out:?}");
    assert_eq!(out.notified, 0, "already said: {out:?}");

    // Three more: one notification, naming the newest, counting the rest.
    for text in ["one", "two", "three"] {
        friend.send(Cmd::Send(text.into()));
        assert!(until(|| friend.state().lines.iter().any(|l| l.text == text), 15).await);
    }
    // Counted at the exchange, the only place the cost of a window is
    // visible: every request a window makes is one the radio carries. The
    // conversation is held now, so it is named in the SIP-52 catch-up and
    // the three messages and any key arrive in that one round trip.
    let before = requests_served(endpoint).await;
    let fake = Fake::default();
    let out =
        sigil_phone::window::run(window(endpoint, signer(1).0, &phone_store, &push), &fake).await;
    let cost = requests_served(endpoint).await - before - 2;
    eprintln!(
        "a wake window with one held conversation and three new messages cost {cost} requests"
    );
    // Measured on 2026-09-17 with `SIGIL_PHONE_LOG=sqexd=debug` listing them:
    // 15 -- prekey/count, catchup, info ×2, home, device/list ×2,
    // wake/register, events, mine, info, peers, beacon beat and read,
    // device/list, one fetch. The window that first heard of the
    // conversation, above, cost 21 without a catch-up to name it in. The
    // bound is the measurement with room for a bookkeeping request or two;
    // a fetch and a key collection per channel creeping back would cross it.
    assert!(
        cost <= 20,
        "a window cost {cost} requests, over the bound of 20"
    );
    assert_eq!(out.arrivals, 3, "{out:?}");
    let said = fake.notifications();
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].count, 3);
    assert!(said[0].body.starts_with("three"), "{}", said[0].body);
    let channel = said[0]
        .channel
        .expect("a notification names its conversation");

    // Under the quiet setting, neither who nor where.
    friend.send(Cmd::Send("four".into()));
    assert!(until(|| friend.state().lines.iter().any(|l| l.text == "four"), 15).await);
    let fake = Fake::default();
    let mut quiet = window(endpoint, signer(1).0, &phone_store, &push);
    quiet.privacy = Privacy::FactOnly;
    let out = sigil_phone::window::run(quiet, &fake).await;
    assert_eq!(out.notified, 1, "{out:?}");
    let said = fake.notifications();
    assert_eq!(said[0].channel, None);
    assert_eq!(said[0].body, "A new message");
    assert!(!said[0].body.contains("four"));

    // **Muted: the same conversation wakes nothing.** The running client
    // has always checked this; the wake window did not, so a conversation
    // somebody muted was silent while they were looking and woke them while
    // they were not. The mute is keyed by exchange *and* channel, so this
    // also pins that the window looks it up under the string the client
    // muted with -- empty for the identity's default exchange, and a
    // mismatch there would fail silently, which is the worst way for a mute
    // to fail.
    friend.send(Cmd::Send("five".into()));
    assert!(until(|| friend.state().lines.iter().any(|l| l.text == "five"), 15).await);
    let fake = Fake::default();
    let mut muted = window(endpoint, signer(1).0, &phone_store, &push);
    muted.quiet.set_muted("", &channel, true);
    let out = sigil_phone::window::run(muted, &fake).await;
    // The precondition, or this proves nothing: the message *did* arrive
    // and was written; a window that fetched nothing would also notify
    // nothing.
    assert_eq!(out.arrivals, 1, "the message should still arrive: {out:?}");
    assert_eq!(
        out.notified, 0,
        "a muted conversation woke the phone: {out:?}"
    );
    assert!(
        fake.notifications().is_empty(),
        "{:?}",
        fake.notifications()
    );

    // A call rings.
    friend.send(Cmd::Call { direct: false });
    assert!(
        until(|| friend.ringing().iter().any(|r| r.mine), 15).await,
        "the friend should be calling: {:?}",
        friend.state().trouble
    );
    let fake = Fake::default();
    let out =
        sigil_phone::window::run(window(endpoint, signer(1).0, &phone_store, &push), &fake).await;
    assert_eq!(out.rang, 1, "{out:?}");
    let rang = fake.rings();
    assert_eq!(rang[0].from, friend_id);
    assert!(rang[0].direct);
    let _ = friend.close();
}
