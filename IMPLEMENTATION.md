# IMPLEMENTATION — the program of record

**This document supersedes and retires `decide/`.** Every ruling that
directory held is restated here as implementation direction; the full
deliberations live in git history (`decide/*.md`, removed 2026-08-27).
One rule carried over: a claim is not done until a named test can fail
over it.

Task IDs (S, UC, N, P, B, X) are tracked on the working task board and
mirrored in [`ROADMAP.md`](ROADMAP.md).

---

## 1 · The signature layer (S) — COMPLETE (S1–S7, 2026-08-27)

**Rulings:** Ed25519 keys; BLAKE3 digests; grants bind
`key × tag-range × epoch window` as a chain fact. Crypto lives in the
`sig` leaf — the substrate stays dependency-free.

**Wire facts (built):**

- Envelope hash = BLAKE3 over the **whole frame** `tag ‖ LE32(len) ‖
  value` — a signature moved to another record forges even on equal
  payloads.
- Attestation record value = `scheme(u8) ‖ signer(32) ‖ sig(64)`,
  fixed width, traveling **beside** the envelope under a granted tag.
  Scheme `0x01` = Ed25519/BLAKE3; unknown schemes are named refusals
  (the agility seam a successor scheme enters through — §8).
- `Act::Bind` (chain tag 10, IS-6/4): `text(holder) ‖ u8(scheme) ‖
  blob(key) ‖ LE64(from_epoch) ‖ LE64(until_epoch)`. Last bind wins;
  rotation is an append; a bind covers no ground; unbound holders
  read as legacy. Strict predicate: `sdk::grant::authorizes_presenter`.

**Enforcement (built — `datum::admission`, `tests/wire.rs (mod admission)`):**

| ID | task | status |
|---|---|---|
| **S4** | Court refuses forged / stale / unbound; identity resolved key→holder from chain bindings (rotation respected: a superseded key is history, not authority) | **DONE** |
| **S5** | Admission is payload-blind — it hashes envelope bytes and reads chain state, never the value — so a carrier may run it and remain a carrier | **DONE** |
| **S6** | `admission::anchor_digest` — BLAKE3 at the court edge; the wire stays digest-agnostic | **DONE** |
| **S7** | Unknown scheme is a named refusal at the court seam | **DONE** |

On the wire: an attestation record (tag 83) follows its envelope; an
enforcing court (`require_signatures = true`) holds each envelope for
its attestation and refuses orphans. `plumbd::produce_signed` is the
producer half; the daemon takes a 64-hex-char `seed`.

Epoch source of truth: the reward book's `EpochOpened`/`EpochClosed`
acts. Freshness thereby becomes a chain fact, not a transport secret —
which is most of §3.

---

## 2 · The universal checker (UC) — COMPLETE (UC1–UC6, 2026-08-27)

**Ruling (2026-08-27): adopt now, ahead of the beta network.** The
binary is the invariant proof-checker; domains circulate as data. If
the engine recompiled to learn a concept, the system would fail as an
epistemic substrate.

**Fixed engine (all that stays compiled):** boundary closure over
*declared* complexes (∂∂ = 0, per-axis conservation), exact-rational /
integer reduction, bounded deterministic evaluation. Already true and
kept: no floats anywhere (enforced by source-reading tests),
refuse-not-repair rationals, skip-unknown carriage.

**What moves to data:** geometry as incidence matrices, boundary
operators, and exact weights in the claim payload or a chain-registered
definition. The compiled Shape judgment (`Shape::admit`) is the legacy
path — the tag-51 defect (a hexagon crossing as a five-simplex) is
what assumed-not-declared geometry costs.

**Guardrails:** (1) an axiom pack is code by another name — every
declared evaluation is fuel- and size-bounded, deterministic, total;
(2) declared ≠ trusted — a registered domain is a vocabulary, not a
truth; claims still settle by re-derivation/convergence; (3) the
compiled path stays until UC5's verdict-equivalence suite is green.

| ID | task | done when |
|---|---|---|
| **UC1** | Declared-complex codec: incidence + boundary operators + exact weights as data, no geometry structs | **DONE** — `assay::complex`, `tests/engine_laws.rs (mod complex_laws)`: hexagon ≠ five-simplex as bytes |
| **UC2** | Fixed evaluator in assay: submitted chain closes against a declared complex, exact arithmetic, fuel-bounded | **DONE** — ∂∂=0 admitted, closure checked, `OpenBoundary` names the leaking cell, `FuelExhausted` refuses |
| **UC3** | Domain-3 claim body: complex reference + witness; content-addressed `work_id` | **DONE** — `DOMAIN_DECLARED=3` through `WorkBody`; replay refuses across transports; multi-axial credit per declared dimension |
| **UC4** | Domain registration on chain, bound to a tag grant; courts resolve tag → definition from chain state | **DONE** — `Act::Declare` (IS-6/5), `Ledger::declaration_of`, `datum::domains::verify_registered`; a definition lapses with its grant; registration is not trust (bad definitions refuse at judgment) |
| **UC5** | Shape re-expressed as declared complex; verdict equivalence vs compiled domain 2 | **DONE** — `complex::from_shape`, `tests/engine_laws.rs (mod complex_laws)` over the constructible corpus; charges survive exactly |
| **UC6** | Fuel/size bounds priced as board axes | **DONE** — metered verify returns spent; `FuelExhausted { budget }` names the price; `domains::fuel_budget` reads it off a board price axis |

Registration authority (ruled here): publishing a domain definition
requires holding the tag grant it binds to — the same authorization
surface as everything else, no new gatekeeper.

---

## 2b · Domain 4 — homological proof calculus — COMPLETE (SQ1–SQ6, 2026-08-28)

**Docketed 2026-08-27.** Logical deduction as boundary annihilation:
Squier showed string-rewriting systems carry inherent homology
(critical branchings generate the module of 3-syzygies), so the
network needs no bespoke proof checker per discipline — only a domain
where statements are cells and the SAME fixed evaluator computes
$\partial c$ over the wire.

**The dimension table:**

| dim | homological meaning | logical meaning |
|---|---|---|
| 0-cells | generators | base constants, variables, atomic types |
| 1-cells | strings / paths | well-formed formulas, propositions |
| 2-cells | rewriting rules | directed inference steps ($A \wedge (A \to B) \Rightarrow B$) |
| 3-cells | confluences / homotopies | proof equivalences — two derivations of one lemma commute |

A conjecture posts to the board as an **open 1-boundary**: a target
proposition lacking a path from the axioms. A proof is a chain $c$
with $\partial c = \text{target} - \text{axioms}$ — watertight
means no missing premises and no dangling conclusions. Settled lemmas
are cited by `work_id` (the T2 memoization cache): the verifier
trusts them as fixed infrastructure because the ledger already
guarantees they closed, and never re-pays their compute. Oversized
proofs fragment through the **sublet lemma market**: a midway
proposition posts as a nested conjecture space funded from the parent
bounty via the moon cascade. The infinite corpus emerges here — every
settled lemma is a permanent content-addressed word future proofs
cite, and the economy recursively builds its own axiomatic base by
decentralized matrix multiplication.

**Corrections applied to the docket, per ratified rulings:**
1. Digests are **BLAKE3**, not SHA3 (the ruled digest family, §1).
2. The calculus registers as an **IS-3 tag grant + `Act::Declare`
   definition** (u64 tag, IS-6/5), not a bespoke 4-byte tag field.
3. Bounty escrow is **X-track work** (settlement receipts, §6) — SQ
   verification must not block on payment rails.

| ID | task | done when |
|---|---|---|
| **SQ1** | Prescribed-boundary evaluation: $\partial c = z$ (relative closure), fuel-metered | **DONE** — `closes_to`, `ProofClaim` (domain 4), `BoundaryMismatch{cell}` names missing premises and dangling conclusions; $z = \varnothing$ recovers plain closure |
| **SQ2** | Dependency citation: settled `work_id`s as trusted infrastructure | **DONE** — citations answer to the book: `UnsettledDependency{work_id}` names the missing lemma; citation order is settlement order; citing lemmas costs zero verification fuel; citations are part of proof identity |
| **SQ3** | The registered calculus | **DONE** — `assay::rewrite`: a presentation compiles into a polygraph where the ONLY 1-cells are rule-licensed rewrites, so an illegal inference fails to EXIST as a cell rather than being refused (the soundness bar, met by construction and swept programmatically over the whole universe); derivations are SQ1 proof claims (∂c = theorem − axiom); `verify_registered` accepts them; measured end to end — the sorting monoid ⟨a,b∣ba→ab⟩ registered by `Act::Declare`, the derivation bba→bab→abb settling over a real socket at a court never compiled to know it. Completeness is bounded and stated (words ≤ max_len) |
| **SQ4** | Conjecture spaces | **DONE** — `Conjecture` (universe + pinned target) as the query statement: a proof of a different theorem refuses as `NotThePosedTheorem`, a cycle cannot answer a conjecture, the posed theorem settles with its rebate |
| **SQ5** | The lemma market | **DONE** — cited lemmas CONTRIBUTE their settled boundaries, read from their content addresses (the ledger stores structure, never a summary that could drift): the composite closes iff ∂(witness) = target − Σ(cited targets); two solvers split a theorem, each paid on their own bounty; the cache is measured (citing < re-deriving); a freeloader citing the lemma without the remainder refuses — lemmas contribute boundaries, not absolution |
| **SQ6** | Confluence | **DONE** — `with_confluences` compiles one 2-cell per diamond (two branches rejoining in one step); the baba diamond commutes BY CERTIFICATE: the two derivations' difference is filled by a compiled square, verified as a proof claim one dimension up by the same evaluator as everything else |

---

## 3 · Session freshness (N2) — CLOSED (IS-2/2, 2026-08-27)

Ruled shape: freshness is layered, not invented per transport.

1. **Work replay is already structural** (`work_id`) — never blocked
   on transport freshness (standing discipline).
2. **Presentation freshness is a chain fact**: an attestation presents
   inside its bind's epoch window (§1/S4).
3. **Session freshness (the remaining hole)**: the declaration
   exchange gains a per-session token; a session replays only inside
   one epoch, and a replayed declaration with a stale token or closed
   epoch refuses. Written as **IS-2/2** with vectors; implemented in
   `plumbd`'s session loop.

| ID | task | done when |
|---|---|---|
| **N2** | IS-2/2: the session challenge | **DONE** — entropy token after the declaration; first attestation answers its exact frame bytes or the session never goes live; replayed session measured dead; carriers relay the challenge verbatim (freshness survives carriage like the signature does) |

---

## 4 · Protocol completion (P)

| ID | task | done when |
|---|---|---|
| **P1** | Tag-51 shape revision | **DONE (IS-1/5)** — shaped closures carry a declared-complex definition (UC1 codec, one shape encoding in the system); vertex-map law (dim-0 cells = arity); `def_len = 0` is explicit shape-unknown legacy; vectors V17–V19 generated by an independent implementation and pinned cross-codec |
| **P2** | IS-4 witness | **DONE (IS-4/1)** — `isthmus::witness` frame (arm refused not guessed; revision required never defaulted); `datum::witnessing` watcher obeying all four §6 prohibitions (wrong subject refuses; replay arm re-derives in full — an intact chain over a non-solution is not a solution; reports carry their observer); court witness log under tag 84; fourth plumbd role; measured over TCP |

---

## 5 · Beta network (B) — B0 PASSED; the gate to public is open

| ID | task | done when |
|---|---|---|
| **B0** | **Local proofnet** — the whole economy on one machine | **DONE** — `scripts/proofnet.sh`: genesis (issue + bind + declare) → enforcing court federating to a second → signed declared-domain claim credits once, its copy refuses → `kill -9` → resume → still refuses. 9/9 checks, PASS |
| **B0+** | **simnet** — a standing local network until alpha/beta goes global: three courts in a federation ring (9501–9503/9601–9603), a payload-blind carrier (9701), two signed clients producing fresh n-cycle work on timers. `scripts/simnet.sh start\|stop\|status\|logs\|reset`; state under `~/.plumb/simnet` | **RUNNING** |
| **B1** | Testnet genesis: public founding chain, seed nodes, grant flow (request → `Issue` + `Bind` on chain → tester's plumbd attaches). Resets allowed and stated | a stranger's plumbd holds a granted range |
| **B2** | Onboarding kit: QUICKSTART, Docker image, per-role configs, BETA.md, issue templates (bug / spec-gap / independent-reader finding), CONTRIBUTING.md | tester online in 15 minutes, measured with a real tester |
| **B3** | CI + pinned-tag consumption | **DONE** — `.github/workflows/ci.yml` (test + clippy on an empty runner — the clean-clone proof by construction; no fmt gate, decided: house style is hand-set, the denies live in the manifests) and `release.yml` (suite must stand, then the tag publishes); the `tag-consumption` job builds a stranger's kernel against the exact commit via git dependency, no checkout |

---

## 6 · x402 payment rails (X) — COMPLETE; HTTP QUARANTINED at the edge

**The quarantine ruling (2026-08-28):** the native protocol is not
downgraded to HTTP — HTTP is confined to the border, for payers who
cannot speak Plumbline, and the confinement is a law with a test:

- **The native market loop (tags 85/81):** a court with a posted
  market ANNOUNCES its query on the session (tag 85); the solver
  answers with an ordinary attested claim; the signed receipt returns
  on the same wire (tag 81, the receipt tag doing its named job).
  The whole x402 loop with zero HTTP — `plumbd::solve_market`,
  measured end to end including offline receipt verification.
- **The eviction:** `datum::x402` no longer exists; every byte of
  HTTP lives in the gateway EDGE BINARY only, with its tests inside
  it. No library a node links contains any of it.
- **The pin:** `tests/court_laws.rs (mod http_quarantine)` scans
  every library source for HTTP markers and refuses escape — and
  separately asserts the edge still HOLDS them, so a broken scanner
  cannot read as a clean one.

**The division of labor:** x402 carries fiat-pegged liquidity (USDC
on **Base** — OQ3 — over HTTP, EIP-3009 escrow); plumb carries the truth (money moves only
when the claim settles); the signature layer binds payout to the
granted solver.

**Flow:** agent's request → server posts the problem to the board as
priced space and answers `402` with escrow instructions naming a
**plumb settlement event** as the release condition → solvers race,
sign claims, envelopes cross carriers unread → court verifies
(re-derivation or convergence) and settlement emits a **signed,
chain-anchored receipt** → the facilitator verifies the receipt and
executes the transfer.

**Trust boundaries, stated:** the facilitator stays custodial (the
receipt makes misbehavior provable, not impossible); payment-chain
finality is external (the two ledgers anchor, never merge); the
challenge body must say which guarantee the price buys —
re-derivation or convergence, which is not correctness.

**Architecture rule:** the gateway is a **kernel-class edge** attached
through the SDK. The court never learns HTTP; the substrate never
learns USDC.

| ID | task | done when |
|---|---|---|
| **X1** | `query_id` | **DONE** — `datum::query`: a question's name is BLAKE3 of what the POSER fixes (shape, domain tag, guarantee, statement) and never the funding — a question keeps its name while the pot fills; the guarantee is declared, never defaulted (X5's law seeded: unpriced guarantee refuses) |
| **X2** | Settlement receipt | **DONE** — `datum::receipt`: signed, epoch-stamped, verified against CHAIN STATE ALONE (signature over exact bytes → key resolves to bound holder → holder is the named court → epoch inside the window); tampered, unbound, misnamed, and stale each refuse by name |
| **X3** | The gateway | **DONE** — `datum::x402` + the `gateway` binary (the court operator's HTTP face, like plumbd, so the crate laws hold by construction): GET /query answers 402 with the declared challenge, POST /answer settles the yield-rebate bounty and returns a signed receipt; EIP-3009 calldata ASSEMBLED for the facilitator (`TransferAuthorization` — the gateway never signs, never executes); no HTTP/JSON dependency spent. Live Base execution awaits the chosen facilitator counterparty (OQ3 residue, external) |
| **X4** | Facilitator vector | **DONE** — V20 (receipt ‖ attestation) + V21 (the chain): a self-contained two-file check with the recipe in the conformance MANIFEST; regenerated by test, never hand-typed |
| **X5** | The declared guarantee | **DONE** — the challenge JSON carries `guarantee` from the query (declared, never implied); an unpriced guarantee byte refuses at decode; measured over a live socket |

---

## 6b · The optimization market (OPT) — docketed 2026-08-27

**Compression priced as a commodity distinct from discovery.** The
board settles the FIRST valid closure to unblock the network (finding
the shortest homologous chain is NP-hard; nobody waits for perfect),
and keeps an economic door open for every subsequent improvement —
without touching T2, because a tighter chain is different structure
and therefore new work by content address.

**O1 — the yield rebate (at discovery).** On a demand-posed space the
payout is a function of measured efficiency:

$$
\text{payout} = \text{base}
+ (\text{max\_fuel} - \text{spent\_fuel}) \cdot r_f
+ (\text{max\_bytes} - \text{spent\_bytes}) \cdot r_b
$$

Sound because both quantities are **consensus facts**: metered fuel is
a deterministic function of witness structure (every court re-derives
the identical number) and canonical byte length is canonical. Escrow
is bounded at `base + max·rates`; the residue refunds the poser.

Two hard rules: **units are fuel and bytes, never cpu cycles or
memory** (machine facts are unverifiable by a federation), and
**rebates bind only to demand-posed spaces** — on self-posed work a
rebate is free money; the strand corpus already recorded why: *a node
authoring its own task solves it for free*.

**O2 — the refinement bounty.** A standing bounty targets a settled
`work_id`: exhibit a chain closing the SAME boundary (SQ1
`closes_to`) at a strict improvement threshold (≥ N% less fuel/bytes
— the anti-dust rule). Because the network cites lemmas by content
address, making a heavily-cited lemma cheaper is a public good worth
a standing price.

**O3 — equivalence by append, never rewrite.** Settlement of a
refinement APPENDS an equivalence act (`old ≈ new`, with measured
savings) to the record. Old proofs keep citing the old id — their
meaning is frozen, as history must be; new work selects the cheap id
because the chain advertises it; courts accept either id in
dependency checks by reading the equivalence fold. The cheap path
wins by selection, not decree.

**O4 — the homology certificate (quality tier).** A refinement
claiming genuine homology — not just same boundary — exhibits the
filling chain $h$ with $\partial h = c - c'$, verified like
everything else. This is SQ6's confluence cell wearing its economic
hat.

| ID | task | done when |
|---|---|---|
| **O1** | Yield rebate | **DONE** — `datum::bounty`: payout = base + saved-fuel·rate + saved-bytes·rate, escrow-bounded; the poser's-universe gate refuses self-posed answers however well they close; over-budget refuses with the price named (fuel AND bytes); measured on the theta universe: the lean cycle out-earns the fat one on the same bounty; T2 untouched |
| **O2** | Refinement bounty | **DONE** — `settle_refinement`: the original's cost is RE-DERIVED FROM ITS CONTENT ADDRESS (a work_id is the structure; the ledger stores no cost table); a ≥N% leaner same-universe chain settles; zero improvement dies at the threshold before the book is consulted; refusals name every number |
| **O3** | Equivalence by append | **DONE** — `RewardAct::Equivalent` (old ≈ new + measured savings), grow-only and deduplicated, in the durable store (tag 4) and the federation merge; `refinements_of` advertises the cheap articulation; old citations frozen, both ids remain settled work; gossip creates no value, equivalences included |
| **O4** | Homology certificate | **DONE** — the certificate IS a proof claim: dim+1, prescribed boundary = original − refined, verified by the SQ1 evaluator; wrong difference and unfilling fillings each refuse by name; `homologous` is a verified fact on the settlement, never a belief |

Sequenced behind **X2** (receipts) for payout; O1's mechanics and all
verification pieces can start now. Tracked with SQ and X on the
working board.

---

## 6c · The kernel (K) — SCOPED 2026-08-28, ratified 2026-08-28

**The validation gap it closes:** every producer on the net today
replays fixtures it was handed — clients walk cycle families, the
solver answers with a hard-coded lean chain. Nothing DERIVES. A
kernel — a simplified convergence engine in the netstratum lineage —
is a producer that finds answers it was not given: posed a conjecture
in a universe it has never seen, it derives a chain closing the
target by traversing the declared complex's OWN licensed 1-cells —
"search" is the wrong word for this, imported from a lineage (generic
tree/heuristic exploration over an unstructured space) this is not:
the complex already licenses which single steps exist as cells (SQ3),
so what a kernel does is walk that licensed step-graph to a
prescribed boundary, not search an open-ended possibility space.
Production is derivation; verification stays the court's; the
asymmetry POWPP §4 describes finally has both sides running on one
machine.

| ID | task | done when |
|---|---|---|
| **K1** | Portable market vocabulary out of the court: `Query` / `Conjecture` / `Receipt`+verify move to a leaf a kernel may import. Ratified: the SDK may import the LEAVES (isthmus, assay, sig) — leaves are laws, not the court; "sdk never imports datum" stays its law | **DONE** — `sdk::query`/`sdk::receipt`; `datum` re-exports both under their original names (zero call-site edits anywhere in the workspace); `receipt::issue` decoupled from the court's `Credit` bookkeeping type (takes work_id bytes + axes units); the chain-key→holder lookup moved to `sdk::grant`, re-exported from `datum::admission`; sdk depends on assay+sig for real now |
| **K2** | The derivation core: given a declared universe and a target boundary, FIND a witness chain — bounded traversal of the complex's own licensed 1-cells (iterative deepening over the step-graph, never a heuristic search of an unstructured space), with a derivation budget distinct from verification fuel (production may be expensive; checking stays cheap) | **DONE** — `sdk::derivation::derive`: reads the step-graph directly out of `ops[0]` (never `Compiled`'s word/step tables — those are the poser's own scaffolding, not wire-portable), iterative-deepening DFS with cycle-safe backtracking, a `budget: u64` distinct from `assay::complex`'s verification fuel; proven against the reference dihedral conjecture (`bab = aa`) from the wire-portable complex alone; budget enforcement confirmed by falsification (disabling the spend check makes the exhaustion test fail) |
| **K3** | The kernel daemon (`crates/kernel`, `plumb-kernel`): attach via sdk, hear query announcements (tag 85), decode the conjecture, derive, submit the attested proof, take the receipt (tag 81) — looping, native Plumbline, no HTTP | one binary joins any court and earns by deriving |
| **K4** | Composed localhost validation: the kernel joins the simnet; courts pose conjectures over registered calculi; the kernel finds derivations by traversal and the books fill with DERIVED work | `simnet.sh status` shows kernel earnings; the whole economy — courts, carrier, clients, kernel, solver, witness, gateway — on one machine |

---

## 6d · Production readiness (PR) — closing every fixture gap between the simnet and a public net

**The audit that opened this section (2026-08-28):** genesis, chain
bootstrap, and signature-based admission were already real — no
theatre there. But five things were fixture-only and would have to be
genuinely built before a stranger could download a binary, generate
their own key, and earn: key generation, live (post-genesis)
registration, network admission bounds, transport confidentiality,
and real (non-`demo_*`) corpus content. Tracked as tasks #18, #29–33.

| ID | task | done when |
|---|---|---|
| **PR1** (#29) | Real key generation: `plumbd keygen <path>` draws from OS entropy (`sig::Keypair::generate`), writes only the seed at mode 0600, refuses to overwrite. `seed_file =` added to `plumbd`'s and `gateway`'s config parsers (`resolve_seed`) so every signing role can be pointed at a keygen-made file instead of pasting a private seed inline. `scripts/simnet.sh` now calls `plumbd keygen` for every signing party (`client-1`, `client-2`, `court-a`, `solver-1`, `witness-1`) and reads genesis `bind =` lines back out of the generated files — **zero hardcoded seeds remain anywhere in the live path.** | DONE 2026-08-28 — `crates/sig/src/lib.rs::generate_draws_fresh_entropy_and_seed_restores_it`; `crates/datum/src/bin/plumbd.rs (mod tests)::keygen_draws_real_entropy_and_the_saved_seed_restores_the_same_identity`, `::keygen_refuses_to_overwrite_an_existing_identity`, `::resolve_seed_prefers_a_keygen_file_over_inline_hex`; verified live — a clean `simnet.sh reset && simnet.sh start` generates 5 real identities, binds them into genesis, and the solver/witness one-shots succeed on first boot |
| **PR2** (#18) | Live registration + one-command `join`: a running court accepts a register request from an unbound key and appends `Act::Bind`+`Act::Issue` to its OWN live ledger — no restart, no re-genesis. Folds with keygen into `plumbd join <court-address>`. | DONE 2026-08-28 — `crates/datum/src/registration.rs` (proof of possession over the session's own challenge, ledger-level `bind_live`, atomic chain persistence); `Ledger` behind `Arc<Mutex<_>>` in `court_session`/`serve` so a bind lands mid-session; `plumbd::register_and_produce` + `role = join` (self-keygens if the identity file is missing, registers, sends a proof-of-life claim, all on one connection); tests: `crates/datum/src/registration.rs (mod tests)` (proof-of-possession, bind_live uniqueness, wire round-trip) + `crates/datum/tests/wire.rs (mod session_watcher)::a_stranger_registers_live_and_is_credited_in_the_same_run` and `::registering_a_name_or_a_key_already_on_the_chain_refuses`; verified live — `simnet.sh`'s `join-1` generates a fresh identity, registers against court-b, and is credited on first boot with zero genesis edits, and refuses cleanly (bounded, no hang) on a replayed restart |
| **PR3** (#30) | Admission walls: per-IP + total connection caps, handshake deadline, so an unbound/hostile connection cannot hold a thread/socket indefinitely on a public IP | DONE 2026-08-28 — `crates/datum/src/plumbd.rs`: `ConnectionCounts` (shared, checked and CHARGED at accept — before a thread is ever spawned), `SessionRules::{max_total_connections, max_connections_per_ip, handshake_deadline}`; the handshake deadline is set before `read_hello` and lifted the moment a real declaration arrives, so a working session's later idle gaps are never this wall's concern; `plumbd` binary ships REAL non-zero defaults (256 total / 16 per-IP / 10s handshake) — a bare court has a wall without being told to; `0` opts a given wall out explicitly. Tests: `crates/datum/tests/wire.rs (mod walls)::the_total_cap_drops_a_connection_with_no_thread_and_no_bytes`, `::the_per_ip_cap_bites_before_the_total_cap_would`, `::the_handshake_deadline_releases_a_silent_connections_slot`; verified live — simnet's courts run under the real defaults with zero session failures |
| **PR4** (#31) | Transport encryption: TLS (rustls) under the wire framing — attestations authenticate content, nothing today encrypts the channel | DONE 2026-08-28 — `isthmus::deed::Act::Certify` (`IS-6/6`, chain tag 12): a holder's cert fingerprint, recorded and resolved like a bind (`Ledger::fingerprint_of`); `crates/datum/src/tls.rs`: `generate_identity` (fresh self-signed Ed25519 cert via rcgen), `FingerprintVerifier` (a custom `rustls::client::danger::ServerCertVerifier` that checks a presented cert's BLAKE3 fingerprint against the chain's `Certify` fact — no CA, no hostname match — while still verifying the TLS handshake signature for real via `rustls-webpki`'s `EndEntityCert::verify_signature`, so chain-of-trust is what's substituted, not cryptography); `ReadWrite` trait (`Read + Write + Send`) lets `court_session`/`serve` and every client-initiating function (`produce_inner`, `carrier_session`, `solve_market`, `witness_to`, `register_and_produce`) run over a plain `TcpStream` or a `rustls::StreamOwned` interchangeably; `SessionRules::tls` wraps EVERY inbound connection on a court's listener uniformly (claims, registration, market answers, witness records all cross the same accept loop); `plumbd tls-cert <holder> <cert> <key>` generates an identity; `role = court` with `tls = true` certifies itself on its own live ledger at startup (no round trip needed — it already holds the ledger); a client-facing role dials over TLS automatically when `court =` names a holder with a live `Certify` on its loaded chain, plaintext otherwise. Tests: `crates/datum/src/tls.rs (mod tests)` (real entropy, fingerprint match/mismatch, config construction); `crates/isthmus/tests/deed_suite.rs` (Certify in the exhaustive tag round-trip). Verified live — court-b in the simnet runs TLS ON with a real generated cert, certifies itself, and every client (client-2, witness-1, join-1) dials it over genuine TLS with zero session failures; found and fixed along the way: a TLS peer that drops the connection without a close_notify alert surfaces as `UnexpectedEof` (not `ConnectionReset`) — extended the existing departure-vs-failure distinction in `read_record` to cover it. #32-33 (real corpus, binary packaging) remain. |
| **PR5** (#32) | Real corpus: at least one genuinely sourced problem live-posted, `demo_*` fixtures confined to `#[cfg(test)]` only | DONE 2026-08-28 — `crates/datum/src/corpus.rs`: a complete (confluent, terminating) rewriting presentation of the dihedral group of order 6 (≅ S₃) — a cited textbook structure (Book & Otto, *String-Rewriting Systems*), not invented for this project. The live market poses `bab = aa`, a genuine instance of the defining relation `bab⁻¹ = a⁻¹`; `plumbd`'s `market = dihedral` and the gateway's x402 bounty both pose the SAME conjecture (identical query_id), and the solver/gateway answer with a real 2-step derivation (`bab → aabb → aa`). Bounty sizing measured, not guessed (~464 fuel, ~8.7KB for the real proof, vs. the old theta fixture's 200/400 — the market silently refused everything until this was fixed). Tests: `crates/datum/src/corpus.rs (mod tests)` (six normal forms = \|D₃\|, the real derivation verifies, a false instance is distinguishably different); `crates/datum/tests/court_laws.rs (mod live_corpus)` — a source scan (in the `http_quarantine` tradition) asserting neither binary's live path prices `demo_theta_universe` and that `corpus::dihedral_conjecture` is what's actually wired in. `demo_hexagon_*`/`demo_cycle_*` remain as honestly-labeled synthetic TRAFFIC generators (a client's fresh-work-every-round, a witness's demo subject, a join's proof-of-life claim) — not banned, since inventing "real" content for pure liveness traffic would just be relabeling, not adding value; what had to stop being a fixture was the priced QUESTION itself. Verified live — solver-1's receipt shows axes `[127, 227]`, the real universe's cell counts, and the gateway's `/query` returns the identical query_id. |
| **PR6** (#33) | CI builds and attaches prebuilt `plumbd`/`gateway` binaries to the GitHub Release — "download and run" becomes literal | DONE 2026-08-28 — `.github/workflows/release.yml`: a `binaries` job, matrixed over linux (`x86_64-unknown-linux-gnu`), mac (`aarch64-apple-darwin`), and windows (`x86_64-pc-windows-msvc`), each on its OS's own NATIVE runner (no cross-compilation, so rustls's `ring` backend and everything else in the dependency tree builds for real on every target) — `cargo build --release --locked --bin plumbd --bin gateway`, packaged (`tar.gz` / `.zip`) and attached to the tag's release via `gh release upload`. Verified as far as this session can: `cargo build --release --locked --bin plumbd --bin gateway --target x86_64-unknown-linux-gnu` builds clean locally and the packaged binary runs; the mac/windows legs and the actual `gh release upload` step are unverified pending a real tagged release (creating one is a visible, public action outside this session's scope to trigger unprompted). |

---

## 6e · Carrier upstream close race (found during PR1–6's own live verification)

**The bug:** a carrier drops its upstream `TcpStream` to the court the
instant the client disconnects, without confirming the court consumed
every relayed record. A market-enabled court sends an UNCONDITIONAL
announcement right after the challenge, which the carrier never reads
(it only reads the challenge itself before switching to a
write-only forward loop) — so the carrier's OWN receive buffer is
never empty at the moment it closes. Closing a socket with unread
data in its own receive queue makes the OS send RST instead of a
clean FIN, and `read_record`'s existing RST/`UnexpectedEof`-at-a-
boundary leniency (added for a genuine departure case, see PR4) reads
that RST as a graceful departure — silently discarding whatever
records were still in flight, with no credit and no refusal logged.
Reproduces intermittently on a long-running carrier relay, not
reliably on a fresh boot, which is what let it hide through PR1–6's
own live checks.

**The fix:** `carrier_session` (`crates/datum/src/plumbd.rs`) no
longer drops its upstream connection immediately. After relaying
every client record it now calls `shutdown_write()` (a new
`ReadWrite` trait method — a raw TCP half-close for `TcpStream`, a
TLS `close_notify` + flush for a `rustls::StreamOwned` upstream leg)
to tell the court "no more input, but I may still want to read,"
then drains (bounded, so a peer that never stops sending cannot hang
the thread forever) whatever the court still sends before the final
drop — so the carrier's receive buffer is empty and the eventual
close is a clean FIN on both sides. `ReadWrite` moved from a blanket
impl to three explicit ones (`TcpStream`, `StreamOwned<ClientConnection, _>`,
`StreamOwned<ServerConnection, _>`, plus a forwarding impl for
`Box<T>`) since the half-close/close_notify behavior genuinely
differs per transport.

DONE 2026-08-28 — test: `crates/datum/tests/wire.rs (mod carrier)
::repeated_relays_through_a_market_court_never_silently_drop_a_record`
— 30 rounds of client→carrier→market-court relay, asserting all 30
credit. Falsified before committing: reverting only the fix (keeping
the test) drops it to 9/30 credited, confirming the test catches the
exact bug; with the fix, 30/30. Verified live on the simnet
afterward, consistent with the regression test.

**A second, deeper bug this same verification pass surfaced:**
`credit_value`'s "not an answer to the question" fallthrough only
named `AnswerRefused::NotThePosersUniverse` (the plain-universe
market's shape of "not even trying to answer"). PR5 switched
court-a's live market to a CONJECTURE (`bab = aa`), whose equivalent
cases — `NotAProof`, `NotDeclared` — hit the generic `Err(_)` arm
instead and were refused outright. Every plain claim court-a ever
saw (all of client-1's carrier-relayed background traffic) was
refused from the moment PR5 landed; the carrier fix above only made
this VISIBLE (it had been silently masquerading as "empty" sessions
until then). Fixed by folding `NotAProof`/`NotDeclared` into the same
fallthrough as `NotThePosersUniverse`. A related, more general
instance of the SAME close-with-unread-data race (not just the
carrier) was also found and fixed: `produce_inner`, `witness_to`, and
`register_and_produce` all dropped their connection without draining
a court's unsolicited market announcement — `finish_politely`
(shutdown_write + bounded drain) is now shared by all four dialing
functions and `carrier_session`. Test:
`crates/datum/tests/wire.rs (mod native_market)
::ordinary_traffic_still_credits_when_the_posted_market_is_a_conjecture`.
Verified live: court-a and court-b both settled to 20/20 credited
over their last 20 sessions; a direct count comparison showed
client-1's sends == carrier-1's relayed sessions == court-a's carrier
credits, exactly, with the solver's own direct credit accounted for
separately — zero drops, zero silent refusals. The O1 yield-rebate
payout formula was independently hand-computed against the real
dihedral proof's measured fuel (464) and bytes (8,731) and matched
the library exactly (20,167) — the arithmetic is correct as
implemented, though note it is not currently ledgered anywhere
durable (`Answer.payout` is computed and used to size the immediate
reply, but no receipt or book field records the number after that
session ends — a separate, real gap from what this pass was fixing).

---

## 7 · Topological signatures — research track, scheme ≥ 0x02

Held at research until the cryptanalytic bar is met; enters through
the scheme byte with no wire break. Summary of the program (full
exposition in git history):

- **Primitives:** braid-group conjugacy (Artin `B_n`), homological
  obstruction on chain complexes (identity as a homology class,
  hardness ≈ shortest homologous chain / lattice problems), discrete
  gauge holonomy (identity = what survives your own gauge moves).
- **Pipeline:** private deformation signs by transport
  (`Σ = w·x_M·w⁻¹` or cochain sum); verification is invariant
  equality / boundary closure — SNF over ℤ, Garside normal forms.
- **Requirements:** unique canonical forms before verification
  (malleability otherwise), size honesty (hundreds of bytes to kB per
  witness), deterministic hash-to-cycle.
- **The bar for leaving research:** concrete parameter set,
  canonical-form specification with test vectors, and **published
  independent cryptanalysis** — braid crypto's history (Dehornoy
  reduction, Garside/super-summit analysis, Lawrence–Krammer,
  length-based attacks) is assumed available to the attacker. Until
  then, scheme `0x01` signs everything that ships.

---

## 8 · Operator rulings (all questions closed 2026-08-27)

| OQ | ruling |
|---|---|
| **OQ1** | **Deferred behind local proof.** Genesis custody and seed hosting are decided only after the full loop is proven on the operator's own machine (B0, §5). Nothing network-public before that |
| **OQ2** | **No crates.io for the beta.** Not needed: beta kernels and testers depend by **pinned git tag** (`git = "https://github.com/AlignmentConfirmed/plumb", tag = "v0.x"`), which cargo supports natively. crates.io is discoverability, not capability — revisit at public launch, last |
| **OQ3** | **Base.** The gateway targets USDC on Base via EIP-3009; facilitator selection happens when X3 starts. No Solana work until Base settles claims end-to-end |
| **OQ4** | **Twitter + website, operator-run** (with agentic amplification). Repo carries the artifacts (BETA.md, templates); the call itself is the operator's channel. Sequenced after B0 like everything public |

**Sequencing law these rulings share: prove locally first.** B0 — the
local proofnet — precedes every public step (genesis, recruitment,
any registry publication).
