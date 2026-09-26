//! A peer that answers, so a call can be *connected* without a person.
//!
//! **Why this exists.** The Android audio session — `MODE_IN_COMMUNICATION`,
//! the focus request, the route — engages when a call connects, not when one
//! rings out (measured on a handset: ringing out leaves the mode
//! `MODE_NORMAL` and starts no `CallService`). So nothing about it could be
//! verified without a far end, and the only far end to hand was a person.
//!
//! This is the far end. It connects as an identity, waits for a ring that is
//! not its own, answers it, holds the call for a while and hangs up, saying
//! what it did. Point a phone at it and the phone reaches a connected call.
//!
//! **Revoke the phone's microphone first** if nobody is in the room to
//! consent to it being open:
//!
//! ```text
//! adb shell pm revoke org.squic.sigil android.permission.RECORD_AUDIO
//! cargo run -p sigil-phone --example answer -- ~/.sqnr/identity-3 trunk.exchange 20
//! adb shell pm grant  org.squic.sigil android.permission.RECORD_AUDIO
//! ```
//!
//! An example rather than a test: it talks to a real exchange with a real
//! identity, which is never something CI should do.

use std::time::{Duration, Instant};

use sigil_chat::session::{self, Cmd, LinkState};
use sigil_net::Dial;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(identity) = args.get(1) else {
        eprintln!("usage: answer <identity-file> [exchange] [hold-secs] [wait-before-answering]");
        eprintln!("  the identity must be one with no passphrase");
        std::process::exit(2);
    };
    let exchange = args.get(2).cloned().unwrap_or_default();
    let hold = Duration::from_secs(args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20));
    // **Time to mute the calling phone before this answers.** A connected
    // call opens the caller's microphone; muting it first means the phone
    // sends comfort noise (SIP-15) and the room it is in goes nowhere. The
    // mute is a control on the call card, so there has to be a gap between
    // the ring arriving here and this taking it.
    let wait_first = Duration::from_secs(args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0));

    let signer = match sqnr::identity::load(std::path::Path::new(identity), None) {
        Ok(signer) => signer,
        Err(why) => {
            eprintln!("{identity}: {why}");
            eprintln!("(an encrypted identity needs a passphrase, which this will not ask for)");
            std::process::exit(1);
        }
    };
    // A second one for the media call: `session::start` takes the first, and
    // a signer is cheap to expand from the same file -- cheaper than keeping
    // a seed in a second place.
    let call_signer = match sqnr::identity::load(std::path::Path::new(identity), None) {
        Ok(signer) => signer,
        Err(why) => {
            eprintln!("{identity}: {why}");
            std::process::exit(1);
        }
    };
    // Its own store, so this never takes the lock off a client that is
    // already running as the same identity.
    let store = std::env::temp_dir().join(format!("sigil-answer-{}", std::process::id()));
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
        // **What it can see, not just that it is up.** A ring is a signal on
        // a channel the session is watching, and a client with an empty
        // store may not yet know it has a conversation with the caller --
        // in which case it will sit here saying nothing while a phone rings
        // out at the other end. Say the count, so that case is visible.
        let mut said_what = false;
        let settle = Instant::now() + Duration::from_secs(15);
        while Instant::now() < settle {
            let state = handle.state();
            if state.synced && !said_what {
                said_what = true;
                println!(
                    "synced: {} conversation(s) known, {} ringing",
                    state.conversations.len(),
                    state.ringing.len()
                );
                // Named, because "it synced" does not say whether the
                // conversation the caller is ringing is one of them.
                for c in &state.conversations {
                    println!(
                        "  {} {}",
                        match c.peer {
                            Some(peer) => format!("dm with {peer}"),
                            None => "channel".to_string(),
                        },
                        c.label
                    );
                }
            }
            let _ = tokio::time::timeout(Duration::from_millis(200), handle.changed()).await;
        }
        if !said_what {
            println!("never synced — it will not see a ring on a channel it is not watching");
        }
        println!("waiting for a ring — call this identity now.");

        let until = Instant::now() + Duration::from_secs(120);
        let mut answered = None;
        while Instant::now() < until && answered.is_none() {
            if let Some(ring) = handle.ringing().into_iter().find(|r| !r.mine) {
                if !wait_first.is_zero() {
                    println!(
                        "ringing from {} — waiting {wait_first:?} before answering",
                        ring.from
                    );
                    tokio::time::sleep(wait_first).await;
                }
                println!("answering");
                handle.send(Cmd::Answer {
                    channel: ring.channel,
                    seq: ring.seq,
                });
                answered = Some(ring);
            }
            let _ = tokio::time::timeout(Duration::from_millis(200), handle.changed()).await;
        }
        let Some(ring) = answered else {
            println!("nothing rang in two minutes");
            return;
        };

        // **Answering is a signal; it is not a call.** A session signals the
        // answer and nothing more -- carrying audio is a separate step, and
        // `session` has no `CallOpts` anywhere in it because it never carries
        // any. So this example answered the ring and left the phone talking
        // into a call with nobody on the other end of the *media*: connected,
        // and deaf, which is the state the card draws "nothing is coming
        // through from the other side" for. Enough to bring the phone's audio
        // session up, and not a call.
        //
        // This is the other half. The same two steps sigil's own `join_call`
        // takes, on the connection this session already holds.
        //
        // **A tone in and nothing out**, so this machine opens neither a
        // microphone nor a speaker: the far end of a test call has no room to
        // listen to and nothing to play. The phone's own microphone still
        // opens -- that is the thing being tested and there is no way around
        // it -- so somebody has to be in the room with it.
        let room = sigil_net::RoomId::new(ring.secret);
        let call = sigil_net::spawn_dm_call(
            sigil_net::Dial::On(handle.connection()),
            call_signer,
            ring.from,
            room,
            // Relayed: an introduction (SIP-25) would hand this machine's
            // address to the phone, and a throwaway test peer has no business
            // doing that.
            false,
            sigil_net::CallOpts {
                source: sigil_net::Source::Tone,
                sink: sigil_net::Sink::Null,
                ..sigil_net::CallOpts::default()
            },
            || {},
        );

        println!("answered. holding for {hold:?} — read the phone's audio state now.");
        // Say what the media is doing, not just that time passed: a call that
        // never reached `Live` is the failure this example exists to catch,
        // and it looks exactly like a working one from a `sleep`.
        let until = Instant::now() + hold;
        let mut said = String::new();
        while Instant::now() < until {
            let state = call.state();
            let now = format!(
                "{:?}{}",
                state.phase,
                if state.deaf { " (deaf)" } else { "" }
            );
            if now != said {
                println!("  media: {now}");
                said = now;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        println!("hanging up");
        call.hang_up();
        handle.send(Cmd::Hangup {
            channel: ring.channel,
            seq: ring.seq,
            seconds: hold.as_secs() as u32,
        });
        tokio::time::sleep(Duration::from_secs(2)).await;
        let closing = handle.close();
        let gone = Instant::now() + Duration::from_secs(5);
        while !closing.is_finished() && Instant::now() < gone {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        println!("done");
    });
}
