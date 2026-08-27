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

**Enforcement (built — `datum::admission`, `tests/admission.rs`):**

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

## 2 · The universal checker (UC) — ruled, blocks beta

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
| **UC1** | Declared-complex codec: incidence + boundary operators + exact weights as data, no geometry structs | **DONE** — `assay::complex`, `tests/declared.rs`: hexagon ≠ five-simplex as bytes |
| **UC2** | Fixed evaluator in assay: submitted chain closes against a declared complex, exact arithmetic, fuel-bounded | **DONE** — ∂∂=0 admitted, closure checked, `OpenBoundary` names the leaking cell, `FuelExhausted` refuses |
| **UC3** | Domain-3 claim body: complex reference + witness; content-addressed `work_id` | **DONE** — `DOMAIN_DECLARED=3` through `WorkBody`; replay refuses across transports; multi-axial credit per declared dimension |
| **UC4** | Domain registration on chain, bound to a tag grant; courts resolve tag → definition from chain state | a node learns a discipline from the chain alone, no rebuild |
| **UC5** | Shape re-expressed as declared complex; verdict equivalence vs compiled domain 2 | equivalence suite green; tag-51 class closed |
| **UC6** | Fuel/size bounds priced as board axes | over-budget refuses with the price named |

Registration authority (ruled here): publishing a domain definition
requires holding the tag grant it binds to — the same authorization
surface as everything else, no new gatekeeper.

---

## 3 · Session freshness (N2) — IS-2 §6, the one OPEN protocol section

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
| **N2** | IS-2/2: session token + epoch acceptance; plumbd implements | vectors; replayed session refused |

---

## 4 · Protocol completion (P)

| ID | task | done when |
|---|---|---|
| **P1** | Tag-51 relation frame revision (IS-1/3): the relation carries its polytopal shape. Coordinated with UC1 — the declared-complex codec is the natural payload, so P1 should not invent a second shape encoding | shaped-relation vectors; old frame decodes as legacy |
| **P2** | IS-4 witness: observer / witness / watcher productized; fourth `plumbd` role; court-side witness log | vectors; witnesses attest to what crossed |

---

## 5 · Beta network (B) — blocked on UC1–UC4

| ID | task | done when |
|---|---|---|
| **B0** | **Local proofnet** — the whole economy on one machine: federated courts with snapshots, signed claims enforced (S4–S7), declared-domain verification (UC), producer → carrier → court → settle, kill/resume | one script stands the net up and the full loop settles, locally |
| **B1** | Testnet genesis: public founding chain, seed nodes, grant flow (request → `Issue` + `Bind` on chain → tester's plumbd attaches). Resets allowed and stated | a stranger's plumbd holds a granted range |
| **B2** | Onboarding kit: QUICKSTART, Docker image, per-role configs, BETA.md, issue templates (bug / spec-gap / independent-reader finding), CONTRIBUTING.md | tester online in 15 minutes, measured with a real tester |
| **B3** | CI (test + clippy + clean-clone); beta consumers depend by **pinned git tag** (OQ2: no crates.io until public launch) | a kernel builds against a tag with no local checkout |

---

## 6 · x402 payment rails (X) — prospective, behind S4–S7

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
| **X1** | `query_id`: board entries addressable from outside | naming test |
| **X2** | Settlement receipt act: signed, epoch-stamped, verifiable without a court | receipt round-trip + external-verify test |
| **X3** | Gateway crate skeleton (SDK-attached): 402 encode, receipt → EIP-3009 call | gateway attaches and holds a grant |
| **X4** | Facilitator conformance vector: receipt bytes + expected verdict | vector any language can check |
| **X5** | Challenge declares its guarantee (re-derivation vs convergence) | unpriced guarantee refused |

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
