//! THE WIRE — every TCP/session measurement in one binary: the
//! node seam, signature admission, the IS-2/2 challenge, the
//! carrier, registered-tag judgment, and the witness record.
//! One binary instead of six: each tests/*.rs links the whole
//! crate, and the link is the build's real cost.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod common;

mod noded {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, RwLock};
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
        let book = Arc::new(RwLock::new(RewardBook::new()));

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
                market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(&listener, &layout, &Arc::new(Mutex::new(ledger)), &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
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
                let guard = book.read().expect("book");
                if guard.seen().len() == 1 && guard.act_len() == 1 {
                    break; // one work_id, one credit act: the copy refused
                }
            }
            assert!(
                Instant::now() < deadline,
                "court never credited: book has {} work_ids",
                book.read().expect("book").seen().len()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }


}

mod admission {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, RwLock};
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
        let book = Arc::new(RwLock::new(RewardBook::new()));

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "test-court".into(),
                    bound: BOUND,
                    enforce: true,
                market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(&listener, &layout, &Arc::new(Mutex::new(ledger)), &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
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
            book.read().expect("book").act_len(),
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
        while book.read().expect("book").act_len() != 1 {
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
        assert_eq!(book.read().expect("book").act_len(), 1, "stranger earned nothing");
    }
}

mod session_freshness {


    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, RwLock};
    use std::time::Duration;

    use datum::admission;
    use datum::plumbd::{self, SessionRules};
    use datum::reward::RewardBook;
    use isthmus::deed::Act;
    use isthmus::hello::Hello;
    use isthmus::layout::Layout;

    use super::common::BOUND;

    use super::common::edge_with;

    fn court(key: &sig::Keypair) -> (std::net::SocketAddr, Arc<RwLock<RewardBook>>) {
        let mut ledger = edge_with("court");
        ledger.record(Act::Bind {
            holder: "solver-a".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: key.public().to_vec(),
            from_epoch: 0,
            until_epoch: u64::MAX,
        });
        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (layout, book2) = (Layout::founding(), Arc::clone(&book));
        std::thread::spawn(move || {
            let rules = SessionRules {
                holder: "court".into(),
                bound: BOUND,
                enforce: true,
                market: None,
            register: false,
            chain_path: None,
            max_total_connections: 0,
            max_connections_per_ip: 0,
            handshake_deadline: None,
            connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
            tls: None,
            };
            let _ = plumbd::serve(&listener, &layout, &Arc::new(Mutex::new(ledger)), &rules, &book2, &Arc::new(Mutex::new(Vec::new())), |_| {});
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
            book.read().expect("book").act_len(),
            0,
            "a replayed session earns nothing: its answer covers a dead token"
        );
    }

}

mod carrier {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, RwLock};
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
        let book = Arc::new(RwLock::new(RewardBook::new()));
        let court_listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let court_addr = court_listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: true,
                market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(&court_listener, &layout, &Arc::new(Mutex::new(ledger)), &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
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
                    None,
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
        while book.read().expect("book").act_len() != 1 {
            assert!(
                Instant::now() < deadline,
                "the claim never credited through the carrier"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn repeated_relays_through_a_market_court_never_silently_drop_a_record() {
        // A market court sends an UNCONDITIONAL announcement right
        // after the challenge — the carrier never reads it (it only
        // ever reads the challenge itself before forwarding
        // whatever the client sends). That leaves the carrier's OWN
        // receive buffer non-empty at the moment it closes its
        // upstream leg, which is exactly the condition that makes an
        // OS send RST instead of FIN — and a RST at a record
        // boundary elsewhere reads as a graceful departure, silently
        // dropping whatever was still in flight. Every one of these
        // relays reproduces that condition; none may drop a record.
        let layout = Layout::founding();
        let key = sig::Keypair::from_seed([6u8; 32]);
        let court_key = sig::Keypair::from_seed([7u8; 32]);

        let mut court_ledger = edge_with("court");
        court_ledger.record(Act::Bind {
            holder: "solver-b".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: key.public().to_vec(),
            from_epoch: 0,
            until_epoch: u64::MAX,
        });

        let query = datum::query::Query {
            poser: "court".into(),
            shape: vec![2, 3],
            domain_tag: isthmus::work::SHAPE_CLAIM_TAG,
            guarantee: datum::query::Guarantee::Rederivation,
            statement: datum::domains::demo_theta_universe().encode(),
        };
        let market = std::sync::Arc::new(plumbd::MarketPost {
            bounty: datum::bounty::Bounty {
                query_id: query.query_id(),
                max_fuel: 200,
                max_bytes: 4_000,
                base: 1_000,
                per_saved_fuel: 10,
                per_saved_byte: 3,
            },
            query,
            court: "court".into(),
            key: court_key,
        });

        let book = Arc::new(RwLock::new(RewardBook::new()));
        let court_listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let court_addr = court_listener.local_addr().expect("addr");
        {
            let (layout, ledger, book, market) =
                (layout.clone(), court_ledger.clone(), Arc::clone(&book), Arc::clone(&market));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: true,
                    market: Some(market),
                    register: false,
                    chain_path: None,
                    max_total_connections: 0,
                    max_connections_per_ip: 0,
                    handshake_deadline: None,
                    connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                    tls: None,
                };
                let _ = plumbd::serve(
                    &court_listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

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
                    None,
                    |_| {},
                );
            });
        }

        // Fresh (non-market) work every round, relayed through the
        // carrier — the market's announcement is left unread EVERY
        // time. A fixed universe size (9), varying charge only, keeps
        // every claim's body comfortably inside the bounty's byte
        // budget while still being distinct content each round.
        const ROUNDS: i64 = 30;
        for lap in 1..=ROUNDS {
            let body = datum::domains::demo_cycle_claim_charged(9, lap, 0).encode();
            let mut envelope = Vec::new();
            isthmus::work::put_shape_claim(&body, &mut envelope).expect("frames");
            plumbd::produce_signed(
                carrier_addr,
                &layout,
                &edge_with("solver-b"),
                "solver-b",
                BOUND,
                std::slice::from_ref(&envelope),
                &key,
            )
            .expect("client attaches to the carrier");
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let credited = book.read().expect("book").act_len();
            if credited == ROUNDS as usize {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "only {credited}/{ROUNDS} relayed claims credited — a record was silently dropped"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

mod registered_wire {


    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, RwLock};
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

        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (Layout::founding(), ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: false,
                market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
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
                let guard = book.read().expect("book");
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
            book.read().expect("book").act_len(),
            1,
            "the wrong universe earned nothing on the registered tag"
        );
    }
}

mod witnessing {


    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, RwLock};
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
        let book = Arc::new(RwLock::new(RewardBook::new()));
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
                market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(&listener, &layout, &Arc::new(Mutex::new(ledger)), &rules, &book, &log, |_| {});
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
    fn the_replay_arm_re_derives() {
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

mod native_market {
    //! The whole x402 loop with ZERO HTTP: the question announced on
    //! the session (tag 85), the answer as an ordinary attested
    //! claim, the signed receipt back on the wire (tag 81), verified
    //! offline against the chain. The gateway edge exists only for
    //! payers who cannot speak Plumbline; a native solver never
    //! touches it.

    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, RwLock};

    use datum::bounty::Bounty;
    use datum::plumbd::{self, MarketPost, SessionRules};
    use datum::query::{Guarantee, Query};
    use datum::reward::RewardBook;
    use isthmus::layout::Layout;

    use super::common::{await_true, bind, cycle_envelope, edge_with, BOUND};

    #[test]
    fn the_whole_market_loop_natively_no_http_anywhere() {
        // One key is one party: the solver and the court each bind
        // their OWN (the first draft shared a key, and holder_of_key
        // rightly resolved it to only one of them).
        let key = sig::Keypair::from_seed([6u8; 32]);
        let court_key = sig::Keypair::from_seed([7u8; 32]);
        let mut ledger = edge_with("court-a");
        bind(&mut ledger, "solver-a", &key, 0, u64::MAX);
        bind(&mut ledger, "court-a", &court_key, 0, u64::MAX);
        let chain = ledger.clone();

        let query = Query {
            poser: "court-a".into(),
            shape: vec![2, 3],
            domain_tag: 82,
            guarantee: Guarantee::Rederivation,
            statement: datum::domains::demo_theta_universe().encode(),
        };
        let market = MarketPost {
            bounty: Bounty {
                query_id: query.query_id(),
                max_fuel: 200,
                max_bytes: 400,
                base: 1_000,
                per_saved_fuel: 10,
                per_saved_byte: 3,
            },
            query,
            court: "court-a".into(),
            key: sig::Keypair::from_seed([7u8; 32]),
        };

        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (Layout::founding(), ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court-a".into(),
                    bound: BOUND,
                    enforce: true,
                    market: Some(Arc::new(market)),
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

        // The solver hears the question, answers leanly, and takes
        // the receipt — one session, one wire, no HTTP.
        let answer = assay::complex::DeclaredClaim {
            transport: 1,
            complex: datum::domains::demo_theta_universe(),
            dim: 1,
            witness: vec![(0, assay::whole(1)), (1, assay::whole(-1))],
        }
        .encode();
        let (heard, signed) = plumbd::solve_market(
            addr,
            &Layout::founding(),
            &edge_with("solver-a"),
            "solver-a",
            BOUND,
            &answer,
            &key,
            None,
        )
        .expect("the native loop closes");

        assert_eq!(heard.guarantee, Guarantee::Rederivation, "declared on the wire too");
        datum::receipt::verify(&signed, &chain).expect("offline verification, same as ever");
        assert_eq!(signed.receipt.axes, vec![2, 3]);
        assert_eq!(signed.receipt.query_id, heard.query_id());
    }

    #[test]
    fn ordinary_traffic_still_credits_when_the_posted_market_is_a_conjecture() {
        // credit_value's "not an answer to the question" fallthrough
        // originally only named NotThePosersUniverse — the plain-
        // universe market's shape of "this isn't even trying to
        // answer." A CONJECTURE market's equivalent is NotAProof /
        // NotDeclared, and until those were added a court running any
        // conjecture-shaped market (P5's real corpus, live) refused
        // EVERY ordinary claim that ever crossed it — the market
        // question was the only thing that could ever settle.
        let key = sig::Keypair::from_seed([8u8; 32]);
        let court_key = sig::Keypair::from_seed([9u8; 32]);
        let mut ledger = edge_with("court-a");
        bind(&mut ledger, "producer-a", &key, 0, u64::MAX);
        bind(&mut ledger, "court-a", &court_key, 0, u64::MAX);

        let (_, conjecture) = datum::corpus::dihedral_conjecture().expect("compiles");
        let query = Query {
            poser: "court-a".into(),
            shape: vec![2, 3],
            domain_tag: isthmus::work::SHAPE_CLAIM_TAG,
            guarantee: Guarantee::Rederivation,
            statement: conjecture.encode(),
        };
        let market = MarketPost {
            bounty: Bounty {
                query_id: query.query_id(),
                max_fuel: 2_000,
                max_bytes: 10_000,
                base: 1_000,
                per_saved_fuel: 10,
                per_saved_byte: 3,
            },
            query,
            court: "court-a".into(),
            key: court_key,
        };

        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (Layout::founding(), ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court-a".into(),
                    bound: BOUND,
                    enforce: true,
                    market: Some(Arc::new(market)),
                    register: false,
                    chain_path: None,
                    max_total_connections: 0,
                    max_connections_per_ip: 0,
                    handshake_deadline: None,
                    connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                    tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

        // Plain background work — never an attempt at the posted
        // theorem — sent while a CONJECTURE market is live.
        let envelope = cycle_envelope(11);
        plumbd::produce_signed(
            addr,
            &Layout::founding(),
            &edge_with("producer-a"),
            "producer-a",
            BOUND,
            std::slice::from_ref(&envelope),
            &key,
        )
        .expect("attaches");

        await_true("ordinary work still credits under a conjecture market", || {
            book.read().expect("book").act_len() == 1
        });
    }
}

mod registered_calculus {
    //! SQ3's done-when: a machine-checkable derivation in a CHAIN-
    //! REGISTERED calculus settles end to end — announced by Declare,
    //! judged by the session, credited by the book. The court was
    //! never compiled to know the sorting monoid; the chain taught it.

    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, RwLock};

    use datum::plumbd::{self, SessionRules};
    use datum::reward::RewardBook;
    use isthmus::deed::Act;
    use isthmus::hello::Hello;
    use isthmus::layout::Layout;

    use super::common::{await_true, edge_with, BOUND};

    #[test]
    fn a_derivation_in_a_chain_taught_calculus_settles_over_the_wire() {
        let compiled = assay::rewrite::Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![(vec![b'b', b'a'], vec![b'a', b'b'])],
        }
        .compile(3)
        .expect("compiles");

        // The chain: "logician" holds a range and registers the
        // compiled calculus on its low tag.
        let mut ledger = edge_with("court");
        ledger.issue("logician", 16).expect("room");
        let tag = ledger
            .deeds()
            .into_iter()
            .find(|d| d.live && d.holder == "logician")
            .expect("issued")
            .low();
        ledger.record(Act::Declare {
            holder: "logician".into(),
            tag,
            definition: compiled.complex.encode(),
        });

        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (Layout::founding(), ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: false,
                    market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

        // The derivation bba → bab → abb, framed under the calculus
        // tag and sent raw over the session.
        let bba = compiled.word(b"bba").expect("axiom");
        let bab = compiled.word(b"bab").expect("mid");
        let abb = compiled.word(b"abb").expect("theorem");
        let body = assay::complex::ProofClaim {
            transport: 1,
            complex: compiled.complex.clone(),
            dim: 1,
            target: compiled.target(bba, abb).expect("target"),
            witness: compiled.derive(&[bba, bab, abb]).expect("licensed"),
            deps: Vec::new(),
        }
        .encode();

        let layout = Layout::founding();
        let ours = Hello::of(&ledger, "logician", BOUND as u32);
        let mut stream = TcpStream::connect(addr).expect("connect");
        plumbd::send_hello(&mut stream, &layout, tag, &ours).expect("hello");
        let mut buf = Vec::new();
        let _court = plumbd::read_hello(&mut stream, &mut buf, &layout, &ours, BOUND).expect("court");
        let _challenge = plumbd::read_record(&mut stream, &mut buf, &layout, BOUND)
            .expect("read")
            .expect("challenge");
        let mut wire = Vec::new();
        isthmus::frame::put_frame(&layout, tag, &body, &mut wire).expect("frames");
        stream.write_all(&wire).expect("sends");
        drop(stream);

        await_true("the derivation settled", || {
            book.read().expect("book").act_len() == 1
        });
    }
}

mod session_watcher {
    //! R4: the watcher, live in the session. A witness about a
    //! subject the session itself carried gets re-derived on the
    //! spot — and a witness about a BROKEN claim becomes a dispute
    //! on the record, not a rumor.

    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, RwLock};

    use datum::plumbd::{self, SessionRules, SessionReport};
    use datum::reward::RewardBook;
    use datum::witnessing;
    use isthmus::hello::Hello;
    use isthmus::layout::Layout;
    use isthmus::witness::{Arm, Observer, Witness};

    use super::common::{await_true, cycle_envelope, edge_with, BOUND};

    fn witness_about(envelope: &[u8]) -> Witness {
        Witness {
            arm: Arm::Replay,
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
    fn the_session_watches_what_it_carried_and_disputes_the_broken() {
        let ledger = edge_with("court");
        let book = Arc::new(RwLock::new(RewardBook::new()));
        let reports: Arc<Mutex<Vec<SessionReport>>> = Arc::new(Mutex::new(Vec::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book, reports) =
                (Layout::founding(), ledger.clone(), Arc::clone(&book), Arc::clone(&reports));
            std::thread::spawn(move || {
                let rules = SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: false,
                    market: None,
                register: false,
                chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    move |report| reports.lock().expect("reports").push(report.clone()),
                );
            });
        }

        // One session: a good claim, a broken claim, and a witness
        // about each.
        let good = cycle_envelope(11);
        let broken = {
            let mut open = datum::domains::demo_cycle_claim(9, 0);
            open.witness.pop();
            let body = open.encode();
            let mut wire = Vec::new();
            isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
            wire
        };

        let layout = Layout::founding();
        let solver = edge_with("solver");
        let ours = Hello::of(&solver, "solver", BOUND as u32);
        let mut stream = TcpStream::connect(addr).expect("connect");
        plumbd::send_hello(&mut stream, &layout, plumbd::hello_tag(&solver, "solver"), &ours)
            .expect("hello");
        let mut buf = Vec::new();
        let _court = plumbd::read_hello(&mut stream, &mut buf, &layout, &ours, BOUND).expect("court");
        let _challenge = plumbd::read_record(&mut stream, &mut buf, &layout, BOUND)
            .expect("read")
            .expect("challenge");

        for envelope in [&good, &broken] {
            stream.write_all(envelope).expect("sends");
            let mut wire = Vec::new();
            isthmus::frame::put_frame(
                &layout,
                witnessing::WITNESS_TAG,
                &witness_about(envelope).encode(),
                &mut wire,
            )
            .expect("frames");
            stream.write_all(&wire).expect("sends");
        }
        drop(stream);

        await_true("the session reported", || !reports.lock().expect("r").is_empty());
        let report = reports.lock().expect("r").first().cloned().expect("one");
        assert_eq!(report.witnessed, 2, "both witnesses on the record");
        assert_eq!(report.watched, 2, "both subjects crossed this session — both watched");
        assert_eq!(
            report.disputed, 1,
            "the broken claim's witness re-derived FALSE: a dispute, not a rumor"
        );
    }

    #[test]
    fn a_stranger_registers_live_and_is_credited_in_the_same_run() {
        // A total stranger: OS entropy, an identity this chain has
        // never heard of — no genesis edit, no restart. That is the
        // whole claim of P2, tested against the real thing rather
        // than a fixture seed.
        let stranger = sig::Keypair::generate().expect("os entropy");

        let layout = Layout::founding();
        let court_ledger = edge_with("court");
        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: true,
                    market: None,
                    register: true,
                    chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

        let envelope = cycle_envelope(21);
        let outcome = plumbd::register_and_produce(
            addr,
            &layout,
            "stranger",
            BOUND,
            &stranger,
            &envelope,
            None,
        )
        .expect("one connection: register, then send, no restart in between");
        assert!(outcome.high >= outcome.low, "a real deed came back");
        assert_eq!(outcome.from_epoch, 0);

        await_true("the stranger's claim credited", || {
            let guard = book.read().expect("book");
            guard.act_len() == 1
        });
    }

    #[test]
    fn registering_a_name_or_a_key_already_on_the_chain_refuses() {
        let layout = Layout::founding();
        let mut court_ledger = edge_with("court");
        let incumbent = sig::Keypair::from_seed([21u8; 32]);
        super::common::bind(&mut court_ledger, "incumbent", &incumbent, 0, u64::MAX);
        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        {
            let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
            std::thread::spawn(move || {
                let rules = plumbd::SessionRules {
                    holder: "court".into(),
                    bound: BOUND,
                    enforce: true,
                    market: None,
                    register: true,
                    chain_path: None,
                max_total_connections: 0,
                max_connections_per_ip: 0,
                handshake_deadline: None,
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: None,
                };
                let _ = plumbd::serve(
                    &listener,
                    &layout,
                    &Arc::new(Mutex::new(ledger)),
                    &rules,
                    &book,
                    &Arc::new(Mutex::new(Vec::new())),
                    |_| {},
                );
            });
        }

        // A fresh key, but asking for a name the chain already holds.
        let envelope = cycle_envelope(22);
        let name_taken = sig::Keypair::generate().expect("os entropy");
        let by_name = plumbd::register_and_produce(
            addr,
            &layout,
            "incumbent",
            BOUND,
            &name_taken,
            &envelope,
            None,
        );
        assert!(
            matches!(by_name, Err(plumbd::NodeBroken::Unsatisfiable)),
            "a taken holder name refuses — the court sends no ack"
        );

        // The incumbent's OWN key, but a fresh name — the key is
        // already someone.
        let by_key = plumbd::register_and_produce(
            addr,
            &layout,
            "someone-else",
            BOUND,
            &incumbent,
            &envelope,
            None,
        );
        assert!(
            matches!(by_key, Err(plumbd::NodeBroken::Unsatisfiable)),
            "an already-bound key refuses under a new name too"
        );
    }
}

mod walls {
    //! P3: the admission wall, checked and charged BEFORE a thread is
    //! ever spawned — not a session-layer rule, so it gets its own
    //! module rather than living beside signature admission.

    use std::io::Read;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, RwLock};
    use std::time::Duration;

    use datum::plumbd::{self, ConnectionCounts, SessionRules};
    use datum::reward::RewardBook;
    use isthmus::deed::Ledger;
    use isthmus::layout::Layout;

    use super::common::{await_true, edge_with, BOUND};

    fn walled_court(rules: SessionRules) -> (std::net::SocketAddr, Ledger) {
        let layout = Layout::founding();
        let ledger = edge_with("court");
        let book = Arc::new(RwLock::new(RewardBook::new()));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let court_ledger = ledger.clone();
        std::thread::spawn(move || {
            let _ = plumbd::serve(
                &listener,
                &layout,
                &Arc::new(Mutex::new(court_ledger)),
                &rules,
                &book,
                &Arc::new(Mutex::new(Vec::new())),
                |_| {},
            );
        });
        (addr, ledger)
    }

    #[test]
    fn the_total_cap_drops_a_connection_with_no_thread_and_no_bytes() {
        let connections = Arc::new(Mutex::new(ConnectionCounts::default()));
        let rules = SessionRules {
            holder: "court".into(),
            bound: BOUND,
            enforce: false,
            market: None,
            register: false,
            chain_path: None,
            max_total_connections: 1,
            max_connections_per_ip: 0,
            handshake_deadline: None,
            connections: Arc::clone(&connections),
        tls: None,
        };
        let (addr, _ledger) = walled_court(rules);

        // The first connection occupies the only slot — it sends no
        // declaration, so the court's read_hello holds it forever.
        let _holder = TcpStream::connect(addr).expect("first connects");
        await_true("the wall's one slot is charged", || {
            connections.lock().expect("counts").total() == 1
        });

        // Accepted at the TCP level, then dropped: no thread, no
        // bytes. A held-but-unadmitted session would still be silent
        // right now — the distinguishing fact is HOW FAST it closes.
        let mut second = TcpStream::connect(addr).expect("second connects at TCP level");
        second
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("timeout");
        let mut buf = [0u8; 1];
        assert_eq!(
            second.read(&mut buf).ok(),
            Some(0),
            "the wall closes it immediately — EOF, not a stalled handshake"
        );
    }

    #[test]
    fn the_per_ip_cap_bites_before_the_total_cap_would() {
        let connections = Arc::new(Mutex::new(ConnectionCounts::default()));
        let rules = SessionRules {
            holder: "court".into(),
            bound: BOUND,
            enforce: false,
            market: None,
            register: false,
            chain_path: None,
            max_total_connections: 5, // plenty of ROOM overall
            max_connections_per_ip: 1, // but only one from any single peer
            handshake_deadline: None,
            connections: Arc::clone(&connections),
        tls: None,
        };
        let (addr, _ledger) = walled_court(rules);

        let _holder = TcpStream::connect(addr).expect("first connects");
        await_true("the per-ip slot is charged", || {
            connections.lock().expect("counts").total() == 1
        });

        let mut second = TcpStream::connect(addr).expect("second connects at TCP level");
        second
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("timeout");
        let mut buf = [0u8; 1];
        assert_eq!(
            second.read(&mut buf).ok(),
            Some(0),
            "same IP, second connection — the per-ip wall refuses even with total room to spare"
        );
    }

    #[test]
    fn the_handshake_deadline_releases_a_silent_connections_slot() {
        let connections = Arc::new(Mutex::new(ConnectionCounts::default()));
        let rules = SessionRules {
            holder: "court".into(),
            bound: BOUND,
            enforce: false,
            market: None,
            register: false,
            chain_path: None,
            max_total_connections: 1,
            max_connections_per_ip: 0,
            handshake_deadline: Some(Duration::from_millis(200)),
            connections: Arc::clone(&connections),
        tls: None,
        };
        let (addr, _ledger) = walled_court(rules);

        // A connection that never sends its declaration — under the
        // OLD unbounded read this thread, and its slot, are held
        // forever. The deadline is the only thing that frees it.
        let _silent = TcpStream::connect(addr).expect("connects");
        await_true("the slot is charged", || {
            connections.lock().expect("counts").total() == 1
        });
        await_true("the deadline fires and the slot is released", || {
            connections.lock().expect("counts").total() == 0
        });
    }
}
