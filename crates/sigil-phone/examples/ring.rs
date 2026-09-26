//! A peer that *places* a call, so a phone can be made to ring without a
//! microphone being opened anywhere.
//!
//! **Why this exists.** [`answer`](../answer.rs) is the far end for a call the
//! phone places; this is the far end for a call the phone *receives*, which is
//! a different set of screens: the ring notification, the ring card, the two
//! answers on it, and everything that happens when one is pressed.
//!
//! **Nothing here opens a microphone, on either side.** `Cmd::Call` posts the
//! SIP-36 invitation and sets the ring state and stops — carrying audio is a
//! separate step a client takes when the call is answered (sigil's own
//! `join_call` is the only thing that does it). So this rings and holds, and
//! the phone at the other end rings without having opened anything. A phone
//! that is only ringing has no audio session: the mode stays `MODE_NORMAL`
//! and no `CallService` is started, which was measured on a handset.
//!
//! That makes the whole refusal path — the notification, the card, Decline,
//! and the ring going back up when a refusal cannot go out — checkable with
//! nothing listening in the room.
//!
//! **It will not ring anybody it was not told to, in full.** A label is an
//! assertion and a prefix is an invitation to a fencepost; this takes the
//! peer's whole key, matches it exactly, and prints who it matched. With no
//! `--yes` it stops there, which is the mode to run it in first.
//!
//! ```text
//! # list what this identity can reach, and ring nobody
//! cargo run -p sigil-phone --example ring -- ~/.sqnr/identity-3 trunk.exchange
//!
//! # ring exactly one of them, for 30 seconds
//! cargo run -p sigil-phone --example ring -- ~/.sqnr/identity-3 trunk.exchange \
//!     38a5UzWXkEabHzPD4hdiCfT9eAortS6uhDy9Pcu9LPsv --yes 30
//! ```
//!
//! An example rather than a test: it talks to a real exchange with a real
//! identity, which is never something CI should do.

use std::time::{Duration, Instant};

use sigil_chat::session::{self, Cmd, LinkState};
use sigil_net::Dial;

fn main() {
    // **Without this the library says nothing.** Every `tracing::` call in
    // sigil-chat, sigil-net and sqex-chat goes to a subscriber that is not
    // installed, so this example printed byte-for-byte the same output with
    // `RUST_LOG=sigil_chat=debug` set as without it. That is what a broken
    // instrument looks like, and an afternoon of diagnosing a call that rang
    // out and never arrived was spent reading its silence as evidence.
    //
    // `warn` by default, so an ordinary run stays as quiet as it was: what
    // this example says for itself is its interface, and the library's log
    // is what somebody asks for when that is not enough.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let args: Vec<String> = std::env::args().collect();
    let Some(identity) = args.get(1) else {
        eprintln!("usage: ring <identity-file> [exchange] [peer-key] [--yes] [ring-secs]");
        eprintln!("  the identity must be one with no passphrase");
        eprintln!("  with no peer-key it lists what this identity can reach and rings nobody");
        std::process::exit(2);
    };
    let exchange = args.get(2).cloned().unwrap_or_default();
    let want = args.get(3).cloned().unwrap_or_default();
    // **Off by default.** Listing is safe; ringing makes a phone in somebody
    // else's room make a noise, and the difference between the two should not
    // be the order of two arguments.
    let go = args.iter().any(|a| a == "--yes");
    let ring_for = Duration::from_secs(
        args.iter()
            .skip(4)
            .find_map(|s| s.parse().ok())
            .unwrap_or(30),
    );

    let signer = match sqnr::identity::load(std::path::Path::new(identity), None) {
        Ok(signer) => signer,
        Err(why) => {
            eprintln!("{identity}: {why}");
            eprintln!("(an encrypted identity needs a passphrase, which this will not ask for)");
            std::process::exit(1);
        }
    };
    // Its own store, so this never takes the lock off a client that is
    // already running as the same identity.
    // **One store per identity, not one per run.** This was keyed by the pid,
    // so every invocation minted a fresh store -- and a fresh store publishes
    // a fresh batch of SIP-23 one-time prekeys for the account, then throws
    // the private halves away with the temp directory. `MAX_PUBLISH` is 64
    // and `MAX_STORED` is 128, so the *second* run fills the pool and the
    // third is refused `pool_full`, which this example reported as "never
    // synced, or nothing to reach".
    //
    // The worse half is what happens before it fills: a sender takes one of
    // the orphaned prekeys and seals its invitation to a key no living client
    // holds the other half of. The call rings out and is never heard -- which
    // is precisely the fault these examples exist to rule out, and they were
    // causing it.
    //
    // The lock this path was avoiding is the *user's own* client, and a path
    // under the temp directory is already clear of that; keying it by the
    // identity keeps two different identities apart without minting a pool
    // each time.
    let store = std::env::temp_dir().join(format!(
        "sigil-{}-{}",
        "ring",
        std::path::Path::new(identity)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("identity")
    ));
    let layers = if exchange.is_empty() {
        sigil_net::discovery::layers(
            sigil_net::discovery::nothing_explicit(),
            &sqnr::config::Config::load(),
            Some(std::path::Path::new(identity)),
        )
    } else {
        vec![sigil_net::Layer {
            server: Some(exchange.clone()),
            ..Default::default()
        }]
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(async move {
        let mut handle = session::start(Dial::Discover(layers), signer, Some(store), || {});
        println!(
            "connecting to {}…",
            if exchange.is_empty() {
                "the default exchange"
            } else {
                &exchange
            }
        );
        let up = Instant::now() + Duration::from_secs(30);
        while Instant::now() < up {
            let state = handle.state();
            if state.link == LinkState::Up {
                break;
            }
            if let Some(trouble) = state.trouble {
                println!("trouble: {trouble}");
            }
            let _ = tokio::time::timeout(Duration::from_millis(200), handle.changed()).await;
        }
        if handle.state().link != LinkState::Up {
            println!("the link never came up");
            return;
        }

        // Wait for the conversation list, then say what it is. A call is
        // placed into whatever is *open*, so a conversation this session has
        // not learned about yet cannot be called.
        let settle = Instant::now() + Duration::from_secs(15);
        let mut known = Vec::new();
        let mut synced = false;
        while Instant::now() < settle {
            let state = handle.state();
            if state.synced {
                synced = true;
                known = state.conversations.clone();
                break;
            }
            let _ = tokio::time::timeout(Duration::from_millis(200), handle.changed()).await;
        }
        // **Which of the two, and what the session said about it.** This read
        // "never synced, or nothing to reach" for both, which are not the
        // same fault and do not have the same fix: one is the exchange not
        // answering and the other is an identity with nobody to ring. An
        // afternoon was spent reading the first as the second. `trouble` is
        // the session's own word for why and was being thrown away.
        if !synced {
            match handle.state().trouble {
                Some(why) => {
                    println!("the link is up but it never synced in 15s — {why}");
                    // **A refusal somebody can act on.** `pool_full` means
                    // this account has SIP-23's `MAX_STORED` one-time prekeys
                    // at the exchange and cannot publish more. The library
                    // already fixes that by itself -- an empty local pool
                    // makes it send `Clear` and resume above what was there --
                    // but `Clear` is capped at `MAX_CLEAR` in an hour, and
                    // past that the refusal falls in with "this exchange has
                    // no Clear at all" and the stale prekeys stay. So the
                    // answer really is to wait, and an afternoon went on not
                    // knowing that.
                    if why.contains("pool_full") {
                        println!();
                        println!(
                            "  that is SIP-23's pool at its ceiling. This client clears it \
                             and starts again on its own,"
                        );
                        println!(
                            "  but Clear is capped at 4 in an hour — so if several runs \
                             have just been made as this"
                        );
                        println!(
                            "  identity, give it the rest of the hour and try once more. \
                             Nothing needs to be deleted."
                        );
                    }
                }
                None => println!("the link is up but it never synced in 15s, and said no reason"),
            }
            return;
        }
        if known.is_empty() {
            println!("synced, and this identity has no conversation to ring");
            return;
        }
        println!("{} conversation(s) known:", known.len());
        for c in &known {
            match c.peer {
                Some(peer) => println!("  {peer}  dm with {}", c.label),
                None => println!("  {:44}  channel {}", "", c.label),
            }
        }

        if want.is_empty() {
            println!();
            println!("no peer key given, so nothing was rung.");
            println!("pass one of the keys above and --yes to ring it.");
            return;
        }
        // **Exactly, or not at all.** Matching a prefix or a label is how a
        // test call reaches somebody it was never aimed at.
        let Some(target) = known
            .iter()
            .find(|c| c.peer.is_some_and(|p| p.to_string() == want))
        else {
            println!();
            println!("no conversation here has the peer {want}.");
            println!("(the whole key, exactly as printed above — a prefix is not accepted)");
            return;
        };
        let peer = target.peer.expect("filtered on it");
        println!();
        println!(
            "this will ring {peer} — \"{}\" — for {ring_for:?}",
            target.label
        );
        if !go {
            println!("nothing was rung: pass --yes to actually place the call.");
            return;
        }

        // The call goes into whatever is open, so open it first and wait for
        // the session to agree that it is.
        handle.send(Cmd::Show(target.channel));
        let opened = Instant::now() + Duration::from_secs(10);
        while Instant::now() < opened {
            if handle.state().open == Some(target.channel) {
                break;
            }
            let _ = tokio::time::timeout(Duration::from_millis(200), handle.changed()).await;
        }
        if handle.state().open != Some(target.channel) {
            println!("the conversation never opened, so the call would have gone nowhere");
            return;
        }

        println!("ringing…");
        // Relayed rather than direct: an introduction (SIP-25) would hand this
        // machine's address to the phone, which a throwaway test caller has no
        // business doing, and the ring is identical either way.
        handle.send(Cmd::Call { direct: false });

        // Say what the far end does with it, which is the point of the whole
        // exercise: answered, refused, or rung out.
        let until = Instant::now() + ring_for;
        let mut said = String::new();
        while Instant::now() < until {
            let mine = handle
                .ringing()
                .into_iter()
                .find(|r| r.mine && r.channel == target.channel);
            let now = match &mine {
                Some(r) if r.answered => "answered".to_string(),
                Some(_) => "ringing".to_string(),
                None => "no longer ringing (refused, or taken down)".to_string(),
            };
            if now != said {
                println!("  {now}");
                said = now;
            }
            if mine.is_none() && !said.is_empty() {
                break;
            }
            let _ = tokio::time::timeout(Duration::from_millis(200), handle.changed()).await;
        }

        println!("giving up on it");
        if let Some(r) = handle
            .ringing()
            .into_iter()
            .find(|r| r.mine && r.channel == target.channel)
        {
            handle.send(Cmd::Hangup {
                channel: r.channel,
                seq: r.seq,
                seconds: 0,
            });
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let closing = handle.close();
        let gone = Instant::now() + Duration::from_secs(5);
        while !closing.is_finished() && Instant::now() < gone {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        println!("done");
    });
}
