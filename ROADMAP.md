# ROADMAP — from tested model to live beta network

Where v0.1.0 stands: the wire, the physics, the court, and the SDK are
built and tested (280 tests), and court-to-court TCP federation
already exists in-process (`datum::court_live` — XDCT snapshot
exchange with replay-safe merge). What does not exist yet is a node
you can leave running, an identity a grant can be held to, and the
onboarding path for someone who is not us.

Each item ends in a test or a shipped artifact, per this repository's
discipline. IDs are defined in `IMPLEMENTATION.md` (S = signatures,
UC = universal checker, X = x402).

## Phase A — trust (the unpoured floor)

| ID | task | done when |
|---|---|---|
| **S1–S2** | Signature leaf crate: Ed25519 + BLAKE3 envelope hash, scheme byte `0x01`; signature/pubkey as tagged records | round-trip test; unsigned-era reader forwards signed traffic whole |
| **S3** | Grant deeds carry `holder_key` (IS-3/3, IS-6/4) | vectors; unbound grants read as legacy |
| **S4–S7** | Court refuses forged / stale / unbound; carrier admission proven payload-blind; BLAKE3 anchors; unknown scheme refused | refusal tests; Known Gap #1 closed |

Crypto dependencies enter here as a sibling leaf — `isthmus` stays
dependency-free.

## Phase B — the node

| ID | task | done when |
|---|---|---|
| **N1** | `plumbd`: node daemon speaking the isthmus wire over TCP — declaration first, then the four verdicts drive the loop; roles (producer / verifier / carrier) by config | two processes attach, a shape claim crosses, the verifier credits it |
| **N2** | IS-2 §6 session freshness (the one OPEN section) as IS-2/2 | vectors; replayed session refused |
| **N3** | Durable court service: periodic snapshots (`court_store`), federation peering (`court_live`) with reconnect | replay refusal holds across real hosts; a killed node resumes from snapshot |

## Phase C — protocol completion

| ID | task | done when |
|---|---|---|
| **P1** | Tag-51 revision: the relation frame carries its polytopal shape (IS-1/3) | shaped-relation vectors; Known Gap #4 closed |
| **P2** | IS-4 witness: observer / witness / watcher productized, fourth `plumbd` role | vectors; Known Gap #3 closed |

## Phase C+ — the universal checker (blocks beta)

The engine becomes the invariant: domains circulate as data, the
binary never recompiles to learn a discipline. Scheduled ahead of the
beta so testers onboard onto the declared-domain engine, not the
legacy compiled path. Full ladder: `IMPLEMENTATION.md`.

| ID | task | done when |
|---|---|---|
| **UC1** | declared-complex codec (incidence + boundary + exact weights as data) | a hexagon and a five-simplex are distinct bytes |
| **UC2** | fixed evaluator: closure against a declared complex, fuel-bounded | closure accepts; non-closure and fuel exhaustion refuse, named |
| **UC3** | domain-3 claim body, content-addressed work_id | replay refuses across transports |
| **UC4** | domain registration on chain; court resolves tag → definition | a node learns a discipline from the chain alone |
| **UC5** | Shape re-expressed as declared complex; verdict equivalence | equivalence suite green; tag-51 defect class closed |
| **UC6** | fuel/size bounds priced on the board | over-budget refuses with the price named |

## Phase D — beta network (blocked on UC1–UC4)

| ID | task | done when |
|---|---|---|
| **B0** | Local proofnet: the whole economy on one machine — signed, declared-domain, federated, kill/resume | one script stands it up; the full loop settles locally |
| **B1** | Testnet genesis: public founding chain, seed nodes, grant issuance flow (request → deed on chain → attach) | a stranger's `plumbd` holds a granted range |
| **B2** | Onboarding kit: QUICKSTART, Docker image, per-role configs, `BETA.md`, issue templates (bug / spec-gap / independent-reader finding) | tester online in 15 minutes, measured with a real tester |
| **B3** | CI (test + clippy + clean-clone job); consumers depend by pinned git tag — no crates.io until public launch (OQ2) | a kernel builds against a tag with no local checkout |

Testnet resets are allowed and will be announced — it is a beta.

## Phase E — after the network stands

- **SQ1–SQ6** — Domain 4, the homological proof calculus
  (`IMPLEMENTATION.md` §2b): scientific proofs as boundary
  annihilation over polygraphs; the sublet lemma market; the
  self-building corpus. SQ1–SQ2 built; SQ3 is the research
  centerpiece; SQ4–SQ5 join the X track.
- **O1–O4** — the optimization market (`IMPLEMENTATION.md` §6b):
  compression priced as a commodity distinct from discovery — yield
  rebates on unspent fuel/bytes at discovery, standing refinement
  bounties on settled work, equivalence by append (never rewrite),
  and the homology certificate as the quality tier. T2 untouched:
  a tighter chain is new work by content address.

- **X1–X5** — x402 payment rails (`IMPLEMENTATION.md`),
  gated behind the settlement receipts S1–S7 make possible.
- **Topological signatures** — held at PROSPECTIVE
  (`IMPLEMENTATION.md`) until the cryptanalytic bar
  is met; enters as scheme ≥ `0x02` through the agility seam, no wire
  break.
- **Kernel repoints** — external kernels move onto the published
  crates; the crossing suites reopen in the lab.

## How to help right now

The highest-leverage contribution needs no Rust: **implement IS-1 from
`protocols/` and `conformance/` alone** and file every ambiguity you
hit. Four gaps were found by the author re-reading; the first
independent reading closes Known Gap #5 and will find things we
cannot.
