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
| **UC1** | Declared-complex codec: incidence + boundary operators + exact weights as data, no geometry structs | **DONE** — `assay::complex`, `tests/complex_laws.rs (mod declared)`: hexagon ≠ five-simplex as bytes |
| **UC2** | Fixed evaluator in assay: submitted chain closes against a declared complex, exact arithmetic, fuel-bounded | **DONE** — ∂∂=0 admitted, closure checked, `OpenBoundary` names the leaking cell, `FuelExhausted` refuses |
| **UC3** | Domain-3 claim body: complex reference + witness; content-addressed `work_id` | **DONE** — `DOMAIN_DECLARED=3` through `WorkBody`; replay refuses across transports; multi-axial credit per declared dimension |
| **UC4** | Domain registration on chain, bound to a tag grant; courts resolve tag → definition from chain state | **DONE** — `Act::Declare` (IS-6/5), `Ledger::declaration_of`, `datum::domains::verify_registered`; a definition lapses with its grant; registration is not trust (bad definitions refuse at judgment) |
| **UC5** | Shape re-expressed as declared complex; verdict equivalence vs compiled domain 2 | **DONE** — `complex::from_shape`, `tests/complex_laws.rs (mod shape_equivalence)` over the constructible corpus; charges survive exactly |
| **UC6** | Fuel/size bounds priced as board axes | **DONE** — metered verify returns spent; `FuelExhausted { budget }` names the price; `domains::fuel_budget` reads it off a board price axis |

Registration authority (ruled here): publishing a domain definition
requires holding the tag grant it binds to — the same authorization
surface as everything else, no new gatekeeper.

---

## 2b · Domain 4 — homological proof calculus (SQ, the Squier track)

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
| **SQ4** | Conjecture space: open 1-boundary posted as priced space (with X1 `query_id` when it lands) | an unproven target is visibly open, funded space |
| **SQ5** | Sublet lemma market: nested conjecture spaces funded from the parent bounty through the moon cascade | a two-solver proof settles from two independently-paid lemmas |
| **SQ6** | Confluence 3-cells: critical-pair closure (the Squier syzygies) verified as $\partial\partial = 0$ one dimension up | two derivations of one lemma verifiably commute |

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
