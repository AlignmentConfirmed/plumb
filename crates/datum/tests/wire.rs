//! THE WIRE — every TCP/session measurement in one binary: the
//! node seam, signature admission, the IS-2/2 challenge, the
//! carrier, registered-tag judgment, and the witness record.
//! One binary instead of six: each tests/*.rs links the whole
//! crate, and the link is the build's real cost.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod common;

mod noded {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use datum::plumbd;
    use datum::reward::RewardBook;
    
    use isthmus::layout::Layout;

    use super::common::BOUND;

    use super::common::edge_with;

    use super::common::shape_triangle_envelope as triangle_envelope;

    #[test]
    fn a_claim_crosses_tcp_and_credits_once() {
        let layout = Layout::founding();
        let court_ledger = edge_with("test-court");
        let book = Arc::new(Mutex::new(RewardBook::new()));

        // The court, on an ephemeral port.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "test-court".into(),
                    bound: BOUND,
                    enforce: false,
                };
                let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
            });
        }

        // The producer: attach, send the same claim twice.
        let producer_ledger = edge_with("test-producer");
        let envelope = triangle_envelope();
        let sent = plumbd::produce(
            addr,
            &layout,
            &producer_ledger,
            "test-producer",
            BOUND,
            &[envelope.clone(), envelope],
        )
        .expect("attach and send");
        assert_eq!(sent, 2);

        // The court credits the work once; the copy is replay. Poll — the
        // session runs on the serve thread.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            {
                let guard = book.lock().expect("book");
                if guard.seen().len() == 1 && guard.act_len() == 1 {
                    break; // one work_id, one credit act: the copy refused
                }
            }
            assert!(
                Instant::now() < deadline,
                "court never credited: book has {} work_ids",
                book.lock().expect("book").seen().len()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }


}

mod admission {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use datum::admission::{self, AdmissionRefused};
    use datum::plumbd;
    use datum::reward::RewardBook;
    use isthmus::deed::{Act, Ledger};
    use isthmus::layout::Layout;

    use super::common::BOUND;

    use super::common::edge_with;

    use super::common::bind;

    use super::common::shape_triangle_envelope as triangle_envelope;

    // ── S4: the three refusals, and the acceptance ──────────────────────

    #[test]
    fn a_bound_presenter_in_window_admits_and_names_its_holder() {
        let key = sig::Keypair::from_seed([1u8; 32]);
        let mut court = edge_with("court");
        bind(&mut court, "solver-a", &key, 0, 10);
        let envelope = triangle_envelope();
        let att = key.attest(&envelope).encode();
        assert_eq!(
            admission::admit(&court, 5, &envelope, &att),
            Ok("solver-a".to_string())
        );
    }

    #[test]
    fn forged_stale_and_unbound_each_refuse_by_name() {
        let key = sig::Keypair::from_seed([1u8; 32]);
        let other = sig::Keypair::from_seed([2u8; 32]);
        let mut court = edge_with("court");
        bind(&mut court, "solver-a", &key, 3, 9);
        let envelope = triangle_envelope();

        // Forged: right key, wrong bytes.
        let att = key.attest(b"different bytes entirely").encode();
        assert_eq!(
            admission::admit(&court, 5, &envelope, &att),
            Err(AdmissionRefused::Forged)
        );

        // Unbound: a valid signature by a key no holder bound.
        let att = other.attest(&envelope).encode();
        assert_eq!(
            admission::admit(&court, 5, &envelope, &att),
            Err(AdmissionRefused::Unbound)
        );

        // Stale: bound key, epoch outside the window.
        let att = key.attest(&envelope).encode();
        assert_eq!(
            admission::admit(&court, 10, &envelope, &att),
            Err(AdmissionRefused::Stale {
                epoch: 10,
                window: (3, 9)
            })
        );
    }

    #[test]
    fn a_rotated_away_key_no_longer_admits() {
        let old = sig::Keypair::from_seed([1u8; 32]);
        let new = sig::Keypair::from_seed([2u8; 32]);
        let mut court = edge_with("court");
        bind(&mut court, "solver-a", &old, 0, 100);
        bind(&mut court, "solver-a", &new, 0, 100); // rotation appends

        let envelope = triangle_envelope();
        let att = old.attest(&envelope).encode();
        assert_eq!(
            admission::admit(&court, 5, &envelope, &att),
            Err(AdmissionRefused::Unbound),
            "the superseded key is history, not authority"
        );
        let att = new.attest(&envelope).encode();
        assert!(admission::admit(&court, 5, &envelope, &att).is_ok());
    }

    // ── S5: payload-blind — the carrier's operation ─────────────────────

    #[test]
    fn admission_never_judges_the_payload() {
        // The envelope's body is garbage no court could credit. Admission
        // still passes: it binds a presenter to bytes, it does not judge
        // claims. That separation is what lets a CARRIER run this check.
        let key = sig::Keypair::from_seed([3u8; 32]);
        let mut court = edge_with("court");
        bind(&mut court, "solver-a", &key, 0, 10);

        let mut garbage = Vec::new();
        isthmus::work::put_claim(b"not a decodable claim body", &mut garbage).expect("frames");
        let att = key.attest(&garbage).encode();
        assert!(
            admission::admit(&court, 1, &garbage, &att).is_ok(),
            "admission is identity, not verification"
        );
    }

    // ── S6: the anchor digest, chosen at the court edge ─────────────────

    #[test]
    fn anchors_digest_with_blake3_and_recompute_matches() {
        let ledger = edge_with("north");
        let prefix = isthmus::deed::chain::encode(ledger.acts());
        let digest = admission::anchor_digest(&prefix);
        assert_eq!(digest.len(), 32);
        assert_eq!(
            digest,
            admission::anchor_digest(&prefix),
            "reproducible by any party holding the same prefix"
        );

        // A stranger anchors this chain and a reader re-derives the digest.
        let mut south = Ledger::new(Layout::founding());
        south.record(Act::Anchor {
            chain: "north".into(),
            height: ledger.acts().len() as u64,
            digest: digest.clone(),
            witnessed: "admission tests".into(),
        });
        let recorded = south.acts().iter().find_map(|a| match a {
            Act::Anchor { digest, .. } => Some(digest.clone()),
            _ => None,
        });
        assert_eq!(recorded, Some(digest));
    }

    // ── S7: unknown schemes are named, end to end ───────────────────────

    #[test]
    fn an_unknown_scheme_is_a_named_refusal_at_the_court_seam() {
        let key = sig::Keypair::from_seed([4u8; 32]);
        let mut court = edge_with("court");
        bind(&mut court, "solver-a", &key, 0, 10);
        let envelope = triangle_envelope();
        let mut att = key.attest(&envelope);
        att.scheme = 0x02; // the topological seam, not yet spoken
        assert_eq!(
            admission::admit(&court, 5, &envelope, &att.encode()),
            Err(AdmissionRefused::UnknownScheme(0x02))
        );
    }

    // ── the whole seam over real TCP ────────────────────────────────────

    #[test]
    fn an_enforcing_court_credits_signed_and_refuses_unsigned() {
        let layout = Layout::founding();
        let solver_key = sig::Keypair::from_seed([7u8; 32]);
        let mut court_ledger = edge_with("test-court");
        bind(&mut court_ledger, "solver-a", &solver_key, 0, u64::MAX);
        let book = Arc::new(Mutex::new(RewardBook::new()));

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "test-court".into(),
                    bound: BOUND,
                    enforce: true,
                };
                let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
            });
        }

        let producer_ledger = edge_with("solver-a");
        let envelope = triangle_envelope();

        // Unsigned: the enforcing court credits nothing.
        plumbd::produce(
            addr,
            &layout,
            &producer_ledger,
            "solver-a",
            BOUND,
            std::slice::from_ref(&envelope),
        )
        .expect("session runs");
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            book.lock().expect("book").act_len(),
            0,
            "an unsigned envelope earns nothing under enforcement"
        );

        // Signed by the bound key: credited.
        plumbd::produce_signed(
            addr,
            &layout,
            &producer_ledger,
            "solver-a",
            BOUND,
            std::slice::from_ref(&envelope),
            &solver_key,
        )
        .expect("session runs");
        let deadline = Instant::now() + Duration::from_secs(5);
        while book.lock().expect("book").act_len() != 1 {
            assert!(Instant::now() < deadline, "signed claim never credited");
            std::thread::sleep(Duration::from_millis(20));
        }

        // Signed by an unbound key: refused, book unchanged.
        let stranger = sig::Keypair::from_seed([9u8; 32]);
        plumbd::produce_signed(
            addr,
            &layout,
            &producer_ledger,
            "solver-a",
            BOUND,
            &[triangle_envelope()],
            &stranger,
        )
        .expect("session runs");
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(book.lock().expect("book").act_len(), 1, "stranger earned nothing");
    }
}

mod session_freshness {


    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use datum::admission;
    use datum::plumbd::{self, SessionRules};
    use datum::reward::RewardBook;
    use isthmus::deed::Act;
    use isthmus::hello::Hello;
    use isthmus::layout::Layout;

    use super::common::BOUND;

    use super::common::edge_with;

    fn court(key: &sig::Keypair) -> (std::net::SocketAddr, Arc<Mutex<RewardBook>>) {
        let mut ledger = edge_with("court");
        ledger.record(Act::Bind {
            holder: "solver-a".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: key.public().to_vec(),
            from_epoch: 0,
            until_epoch: u64::MAX,
        });
        let book = Arc::new(Mutex::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (layout, book2) = (Layout::founding(), Arc::clone(&book));
        std::thread::spawn(move || {
            let rules = SessionRules {
                holder: "court".into(),
                bound: BOUND,
                enforce: true,
            };
            let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book2, &Arc::new(Mutex::new(Vec::new())), |_| {});
        });
        (addr, book)
    }

    use super::common::cycle_envelope as envelope;


    #[test]
    fn a_replayed_session_answer_never_goes_live() {
        let key = sig::Keypair::from_seed([2u8; 32]);
        let (addr, book) = court(&key);
        let layout = Layout::founding();
        let ledger = edge_with("solver-a");
        let ours = Hello::of(&ledger, "solver-a", BOUND as u32);

        // Session 1 — legitimate: capture the challenge and answer it.
        let mut s1 = TcpStream::connect(addr).expect("connect");
        plumbd::send_hello(&mut s1, &layout, plumbd::hello_tag(&ledger, "solver-a"), &ours)
            .expect("hello");
        let mut buf = Vec::new();
        let _court = plumbd::read_hello(&mut s1, &mut buf, &layout, &ours, BOUND).expect("court");
        let (_t, challenge1) = plumbd::read_record(&mut s1, &mut buf, &layout, BOUND)
            .expect("read")
            .expect("challenge");
        // The RECORDED answer an attacker captures off the wire:
        let recorded_answer = key.attest(&challenge1).encode();
        drop(s1); // session 1 abandoned

        // Session 2 — the replay: same recorded answer, NEW challenge.
        let mut s2 = TcpStream::connect(addr).expect("connect");
        plumbd::send_hello(&mut s2, &layout, plumbd::hello_tag(&ledger, "solver-a"), &ours)
            .expect("hello");
        let mut buf = Vec::new();
        let _court = plumbd::read_hello(&mut s2, &mut buf, &layout, &ours, BOUND).expect("court");
        let (_t, challenge2) = plumbd::read_record(&mut s2, &mut buf, &layout, BOUND)
            .expect("read")
            .expect("challenge");
        assert_ne!(challenge1, challenge2, "the token never repeats");

        let mut wire = Vec::new();
        isthmus::frame::put_frame(&layout, admission::ATTESTATION_TAG, &recorded_answer, &mut wire)
            .expect("frame");
        s2.write_all(&wire).expect("replayed answer sent");

        // Signed work follows — but the session never went live, so it
        // refuses before any envelope is even held.
        let env = envelope(13);
        s2.write_all(&env).expect("envelope");
        let att = key.attest(&env).encode();
        let mut wire = Vec::new();
        isthmus::frame::put_frame(&layout, admission::ATTESTATION_TAG, &att, &mut wire)
            .expect("frame");
        s2.write_all(&wire).expect("attestation");
        drop(s2);

        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(
            book.lock().expect("book").act_len(),
            0,
            "a replayed session earns nothing: its answer covers a dead token"
        );
    }

}

mod carrier {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use datum::plumbd;
    use datum::reward::RewardBook;
    use isthmus::deed::Act;
    use isthmus::layout::Layout;

    use super::common::BOUND;

    use super::common::edge_with;

    #[test]
    fn a_signed_claim_credits_through_an_unreading_carrier() {
        let layout = Layout::founding();
        let key = sig::Keypair::from_seed([5u8; 32]);

        // The enforcing court, with the solver's key bound.
        let mut court_ledger = edge_with("court");
        court_ledger.record(Act::Bind {
            holder: "solver-a".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: key.public().to_vec(),
            from_epoch: 0,
            until_epoch: u64::MAX,
        });
        let book = Arc::new(Mutex::new(RewardBook::new()));
        let court_listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let court_addr = court_listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: true,
                };
                let _ = plumbd::serve(&court_listener, &layout, &ledger, &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
            });
        }

        // The carrier between them.
        let carrier_listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let carrier_addr = carrier_listener.local_addr().expect("addr");
        {
            let (layout, ledger) = (layout.clone(), edge_with("carrier"));
            std::thread::spawn(move || {
                let _ = plumbd::carry(
                    &carrier_listener,
                    &layout,
                    &ledger,
                    "carrier",
                    BOUND,
                    court_addr.to_string(),
                    |_| {},
                );
            });
        }

        // The client sends THROUGH the carrier.
        let body = datum::domains::demo_cycle_claim(9, 0).encode();
        let mut envelope = Vec::new();
        isthmus::work::put_shape_claim(&body, &mut envelope).expect("frames");
        plumbd::produce_signed(
            carrier_addr,
            &layout,
            &edge_with("solver-a"),
            "solver-a",
            BOUND,
            std::slice::from_ref(&envelope),
            &key,
        )
        .expect("client attaches to the carrier");

        let deadline = Instant::now() + Duration::from_secs(5);
        while book.lock().expect("book").act_len() != 1 {
            assert!(
                Instant::now() < deadline,
                "the claim never credited through the carrier"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

mod registered_wire {


    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use datum::plumbd::{self, SessionRules};
    use datum::reward::RewardBook;
    use isthmus::deed::{Act, Ledger};
    use isthmus::hello::Hello;
    use isthmus::layout::Layout;

    use super::common::BOUND;

    #[test]
    fn a_registered_tag_is_judged_on_the_wire_by_the_chain_taught_definition() {
        // The chain: "geometer" holds a range and registered the hexagon
        // universe on its low tag.
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger.issue("court", 16).expect("room");
        ledger.issue("geometer", 16).expect("room");
        let tag = ledger
            .deeds()
            .into_iter()
            .find(|d| d.live && d.holder == "geometer")
            .expect("issued")
            .low();
        ledger.record(Act::Declare {
            holder: "geometer".into(),
            tag,
            definition: datum::domains::demo_hexagon_universe().encode(),
        });

        let book = Arc::new(Mutex::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (Layout::founding(), ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: false,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &ledger,
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

        // A raw session: hello, read hello + challenge, then two records
        // under the REGISTERED tag — the right universe, and a wrong one.
        let layout = Layout::founding();
        let ours = Hello::of(&ledger, "geometer", BOUND as u32);
        let mut stream = TcpStream::connect(addr).expect("connect");
        plumbd::send_hello(&mut stream, &layout, tag, &ours).expect("hello");
        let mut buf = Vec::new();
        let _court = plumbd::read_hello(&mut stream, &mut buf, &layout, &ours, BOUND).expect("court");
        let _challenge = plumbd::read_record(&mut stream, &mut buf, &layout, BOUND)
            .expect("read")
            .expect("challenge");

        // The right universe: the hexagon claim, framed under the tag.
        let good = datum::domains::demo_hexagon_claim(1).encode();
        let mut wire = Vec::new();
        isthmus::frame::put_frame(&layout, tag, &good, &mut wire).expect("frames");
        stream.write_all(&wire).expect("sends");

        // The wrong universe: a triangle — closes beautifully, but it is
        // not what the chain registered for this tag.
        let bad = datum::domains::demo_cycle_claim(3, 1).encode();
        let mut wire = Vec::new();
        isthmus::frame::put_frame(&layout, tag, &bad, &mut wire).expect("frames");
        stream.write_all(&wire).expect("sends");
        drop(stream);

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            {
                let guard = book.lock().expect("book");
                if guard.act_len() == 1 {
                    break; // the hexagon credited; the triangle refused
                }
            }
            assert!(
                Instant::now() < deadline,
                "the registered-tag path never judged the claim"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            book.lock().expect("book").act_len(),
            1,
            "the wrong universe earned nothing on the registered tag"
        );
    }
}

mod witnessing {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use datum::plumbd::{self, SessionRules, WitnessLog};
    use datum::reward::RewardBook;
    use datum::witnessing::{self, WatcherRefused};
    
    use isthmus::layout::Layout;
    use isthmus::witness::{Arm, Observer, Witness};

    use super::common::BOUND;

    use super::common::edge_with;

    use super::common::cycle_envelope as envelope;

    fn witness_about(envelope: &[u8], arm: Arm) -> Witness {
        Witness {
            arm,
            observer: Observer {
                kind: 1,
                identity: [3u8; 32],
                revision: "IS-6/5".into(),
                depth: 0,
            },
            subject: witnessing::subject_of(envelope),
            derivation: Vec::new(),
        }
    }

    #[test]
    fn a_witness_goes_on_the_record_over_tcp() {
        let ledger = edge_with("court");
        let book = Arc::new(Mutex::new(RewardBook::new()));
        let log: WitnessLog = Arc::new(Mutex::new(Vec::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book, log) =
                (Layout::founding(), ledger.clone(), Arc::clone(&book), Arc::clone(&log));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: false,
                };
                let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book, &log, |_| {});
            });
        }

        let subject = envelope(21);
        let statement = witness_about(&subject, Arm::Succinct);
        let sent = plumbd::witness_to(
            addr,
            &Layout::founding(),
            &edge_with("watcher-1"),
            "watcher-1",
            BOUND,
            std::slice::from_ref(&statement),
            None,
        )
        .expect("witness session");
        assert_eq!(sent, 1);

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            {
                let guard = log.lock().expect("log");
                if guard.len() == 1 {
                    assert_eq!(guard.first(), Some(&statement), "kept verbatim, unjudged");
                    break;
                }
            }
            assert!(Instant::now() < deadline, "witness never reached the record");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn the_watcher_is_handed_its_subject_or_refuses() {
        let subject = envelope(23);
        let statement = witness_about(&subject, Arm::Replay);

        // §6.1: handed the WRONG subject, it refuses — it may not observe
        // (fetch the right one) and may not repair (pretend this matches).
        let wrong = envelope(25);
        assert_eq!(
            witnessing::watch(&statement, &wrong),
            Err(WatcherRefused::NotTheSubject)
        );

        // Handed the right subject, the replay arm re-derives in full and
        // the report is never bare: it carries the observer (§6.4).
        let report = witnessing::watch(&statement, &subject).expect("answers");
        assert!(report.verified, "the 23-cycle closes on re-derivation");
        assert_eq!(report.observer, statement.observer);
    }

    #[test]
    fn the_replay_arm_actually_re_derives() {
        // A witness about a BROKEN claim: the digest matches (the witness
        // honestly names the broken bytes), but replay re-derivation says
        // the claim does not close. Verdict false, never a refusal — the
        // watcher answered the question it was asked.
        let mut open_claim = datum::domains::demo_cycle_claim(9, 0);
        open_claim.witness.pop();
        let body = open_claim.encode();
        let mut subject = Vec::new();
        isthmus::work::put_shape_claim(&body, &mut subject).expect("frames");

        let statement = witness_about(&subject, Arm::Replay);
        let report = witnessing::watch(&statement, &subject).expect("answers");
        assert!(
            !report.verified,
            "an intact chain over a non-solution is not a solution"
        );
        assert_eq!(report.observer, statement.observer, "still not bare");
    }
}
