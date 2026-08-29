# ARCHITECTURE — the code map

This document maps the codebase so a reader — a developer or an agent —
can find where each responsibility lives without tracing functions from
scratch. It names the real crates, modules, and public types.

It complements the other documents:

- [`README.md`](README.md) — what the system is, and how to run it.
- [`protocols/`](protocols/) (IS-1…IS-6) — the wire and chain formats.
- [`POWPP.md`](POWPP.md) / [`PROOF_ECONOMY.md`](PROOF_ECONOMY.md) — what
  the engine verifies and how credit is assigned.
- [`IMPLEMENTATION.md`](IMPLEMENTATION.md) — the ruling ledger and task
  ladders.

## Crate topology and the dependency law

Six crates. Each may only import the crates below it; the direction is
the security model, and it is enforced by tests
(`crates/assay/tests/isolation.rs`,
`crates/isthmus/tests/no_path_dependencies.rs`).

| crate | imports | role |
|---|---|---|
| `isthmus` | nothing | the substrate: the wire, the chain, deeds. Carries claims without interpreting them. |
| `assay` | nothing | the convergence engine: verification physics (flux, shape, homology). |
| `sig` | nothing | identity: Ed25519 over BLAKE3 (scheme `0x01`). |
| `sdk` | isthmus + assay + sig | the kernel attach surface (leaf-importable). |
| `kernel` | sdk + isthmus + assay + sig | a derivation-finding producer. |
| `datum` | isthmus + assay + sig + sdk | the court: prices, verifies, settles, keeps the chain. |

`isthmus` moves claims without reading them; `datum` reads claims
without moving them. A party that did both would be an arbiter, and the
dependency law forbids it structurally.

## Reading order

1. This document, then `README.md`.
2. **Wire and data model** — `isthmus`: `frame`, `ratio`, `deed`,
   `session`, `hello`, `work`.
3. **Verification physics** — `assay`: `flux`, `shape`, `complex`,
   `homology`, `snf`.
4. **The court** — `datum`: `ledger`, `board`, `settle`, `reward`,
   `plumbd`.
5. **Identity and attach** — `sig`, then `sdk`.

## isthmus — the substrate

The dependency-free wire library: framing, the exact rational, the
chain, deeds, sessions, and opaque claim envelopes.

| module | responsibility | key types / entry points |
|---|---|---|
| `frame.rs` | the record and every refusal a reader may produce | `Record`, `Malformed`, `encode`, `decode`, `take_frame` |
| `ratio.rs` | the exact rational codec and its strictness | `put_ratio`, `take_ratio` |
| `layout.rs` | the record header as a structure, not a number | `Layout`, `Field`, `Width` |
| `session.rs` | telling *not yet arrived* from *never* | `Session`, `Reader`, `Verdict`, `whole_records` |
| `hello.rs` | the declaration (IS-5) | `Hello`, `Uplink` |
| `deed.rs` | the chain: acts, deeds, the ledger, well-formedness | `Act`, `Deed`, `Ledger`, `Standing`, `Flaw`, `Axis`, `Frontier` |
| `sphere.rs` | multi-chain linkage across ledgers | sphere / frontier merge |
| `work.rs` | opaque claim envelopes (tags 80–82) | `Envelope`, `Claim`, `is_work_tag`, `put_claim`, `put_shape_claim`, `put_receipt` |
| `witness.rs` | the IS-4 witness frame | `Witness`, `Observer`, `Arm` |
| `node.rs` | node roles and the carrier step | `Role`, `Step`, `CarrierOut`, `carrier_step` |

## assay — the convergence engine

The verification physics. Imports nothing, reads no framing tag: a
kernel asks `assay` whether a claim closes; `assay` does not know
kernels exist.

| module | responsibility | key types / entry points |
|---|---|---|
| `flux.rs` | the oriented boundary and its divergence (PoWC) | `Boundary`, `Facet`, `Orientation` |
| `shape.rs` | the shape domain — orbs, edges, charges (PoUW) | `Shape`, `Edge`, `ShapeClaim` |
| `complex.rs` | the declared complex and the fixed evaluator (universal checker) | `DeclaredComplex`, `Compiled`, `DeclaredClaim`, `from_shape` |
| `homology.rs` | `H_k = ker ∂_k / im ∂_{k+1}`; exact and fast legs | `Betti`, `BettiFast`, `betti`, `betti_fast` |
| `snf.rs` | the integer boundary calculus — invariants and filling chains | `Boundary`, `invariant_factors`, `rank`, `rank_mod`, `solve` |
| `simplex.rs` | exact-rational two-phase simplex | `minimize_l1`, `minimize_forward` |
| `rewrite.rs` | a rewriting calculus compiled into a polygraph | `RewriteBroken` |
| `extent.rs` | per-axis coordinates, one component per axis | `Extent` |
| `work.rs` | portable multi-axial work claims (boundary domain) | `WorkBody`, `Claim`, `WorkId`, `assess` |
| `freshness.rs` | work-identity freshness — credit once per structure | `OnceCredit` |
| `exact_codec.rs` | exact rational bytes shared across claim kinds | `put_exact`, `take_exact` |
| `credit_event.rs` | the settlement-event leaf type | `CreditEvent`, `ClaimClasses` |

## sig — the identity physics

| module | responsibility | key types / entry points |
|---|---|---|
| `lib.rs` | Ed25519 over BLAKE3, scheme `0x01` | `Keypair`, `Attestation`, `envelope_hash`, `session_token` |

## sdk — the kernel attach surface

Leaf-importable: what a kernel needs to declare itself, obtain a grant,
wrap a claim, and settle a receipt.

| module | responsibility | key types / entry points |
|---|---|---|
| `attach.rs` | declare and agree over the handshake (IS-5) | `Agreement`, `attach`, `agree` |
| `grant.rs` | authorization as a ledger fact (IS-3) | `grant`, `authorizes` |
| `submit.rs` | wrap portable claims in highway envelopes | `submit`, `claim`, `shape` |
| `query.rs` | a demand-posed problem, addressable from outside (X1) | `Query`, `Conjecture`, `open` |
| `receipt.rs` | the settlement receipt external rails settle against (X2) | `Receipt`, `SignedReceipt`, `verify` |
| `derivation.rs` | the derivation core — find a witness not given (K2) | `derive` |

## datum — the court

The authority: prices space, verifies claims, settles credit, keeps the
chain, and runs the node daemon. Grouped by concern.

### Chain and registry

| module | responsibility | key types |
|---|---|---|
| `ledger.rs` | the founding edge's chain, held here | `Ledger` re-export |
| `registry.rs` | the tag registry, counted from prose tables | `Registry`, `State` |
| `domains.rs` | registered domains resolved from the chain; fuel bounds (UC4/UC6) | domain resolution |
| `block.rs` | block production — append settlement acts | `BlockRefused` |

### Pricing and settlement

| module | responsibility | key types |
|---|---|---|
| `board.rs` | applications, the survey, the price, the docket | `Ask`, the board |
| `negotiation.rs` | settlement positions and folds over multi-axial space | `Proposal` |
| `settle.rs` | credit useful work against deed-priced multi-axis space | `court_settle` |
| `reward.rs` | multi-axial credit, primary by `work_id` | `RewardBook`, `RewardAct` |
| `extent.rs` | per-axis space, one component per axis | `Extent` |
| `merge.rs` | sphere merge: bulk, residual carry, PoUW/PoWC admit, payout | `MergeSettle` |
| `bounty.rs` | the yield rebate for efficient work at discovery (O1) | yield accounting |

### Convergence (Phase 6)

| module | responsibility | key types |
|---|---|---|
| `section.rs` | the convergent settlement section (valued per grade); durable codec, committed anchor, cross-node merge | `Section`, `GradeShape`, `SectionBroken` |
| `geometry.rs` | the bridge from verification geometry (`assay`) to scheduling | `graded_torsion`, `grade_shapes`, `claim_grades` |
| `sched.rs` | local admission scheduling, kept separate from settlement | `ResourceGovernor`, `Turned` |

### Identity and enforcement

| module | responsibility | key types |
|---|---|---|
| `admission.rs` | court and carrier enforcement of signatures (S4–S7) | signature admission |
| `registration.rs` | live registration — a running court binds a fresh key (P2) | `bind_live`, `RegisterOutcome` |
| `tls.rs` | transport confidentiality (IS-6/6) | chain-pinned TLS |
| `hygiene.rs` | transport replay hygiene, secondary to work identity | wire hygiene |
| `witnessing.rs` | the IS-4 witness log and the watcher's law | witness log |
| `escrow.rs` | balance accounting over the chain's stake acts (IS-6/7) | `Balance`, `Accounting` |

### Daemon and services

| module | responsibility | key types / entry points |
|---|---|---|
| `plumbd.rs` | the node daemon: sessions over TCP, serve, produce | `serve`, `serve_with_snapshot`, `court_session`, `produce`, `carrier_session`, `witness_to`, `solve_market` |
| `court_service.rs` | the durable court — snapshots that survive a kill (N3) | durable service |
| `court_store.rs` | the durable `RewardBook` across restart | store codec |
| `court_live.rs` | multi-host federation | live federation |
| `bin/gateway.rs` | the court's x402 HTTP face (X3) | `Gateway` |
| `onramp.rs` | tollway → superhighway on-ramp for useful work | shape → envelope |
| `corpus.rs` | the live corpus a court prices (P5) | the priced question |

Binaries live in `crates/datum/src/bin/` (`plumbd.rs`, `gateway.rs`).
Functions prefixed `demo_*` are synthetic fixtures for tests and
liveness traffic, not the live priced work — that is `corpus.rs`.

## kernel — the producer daemon

| module | responsibility |
|---|---|
| `main.rs` | the kernel daemon: a derivation-finding producer (K3) |

## A claim's lifecycle through the code

1. **Derive.** A kernel finds a witness it was not given —
   `sdk::derivation::derive`, backed by `assay` (`shape`, `complex`,
   `simplex`).
2. **Wrap.** The claim is enveloped for the highway —
   `sdk::submit` → `isthmus::work::Envelope` (tags 80–82).
3. **Cross the wire.** Framed and sequenced —
   `isthmus::frame`, `isthmus::session`; forwarded unread by a carrier
   (`isthmus::node::carrier_step`).
4. **Receive.** The court runs the session —
   `datum::plumbd::court_session`.
5. **Verify.** Re-derivation — `assay::work::assess` and the
   `assay::complex` evaluator.
6. **Price and settle.** Against deed-priced space —
   `datum::board`, `datum::settle::court_settle`.
7. **Credit.** Recorded by work identity —
   `datum::reward::RewardBook`.
8. **Chain.** Written as acts — `isthmus::deed::Act`,
   `datum::ledger`, `datum::block`.
9. **Converge.** The settled claim grows the section —
   `datum::section::Section` via `datum::geometry::claim_grades`.

## Where to go for a given question

| question | code | spec |
|---|---|---|
| How is a record framed? | `isthmus/frame.rs`, `ratio.rs` | IS-1 |
| How does verification work? | `assay/flux.rs`, `shape.rs`, `complex.rs`, `homology.rs` | POWPP |
| Where is credit recorded? | `datum/reward.rs`, `settle.rs` | POWPP |
| How does the chain work? | `isthmus/deed.rs`, `datum/ledger.rs`, `block.rs` | IS-6 |
| Signatures and identity? | `sig/`, `datum/admission.rs`, `registration.rs` | IS-6/4 |
| Sessions and freshness? | `isthmus/session.rs`, `datum/hygiene.rs` | IS-2 |
| The tag registry? | `datum/registry.rs`, `isthmus/deed.rs` | IS-3 |
| The witness / watcher? | `isthmus/witness.rs`, `datum/witnessing.rs` | IS-4 |
| Running a node? | `datum/plumbd.rs`, `bin/plumbd.rs`, `scripts/simnet.sh` | — |
| Convergence / the section? | `datum/section.rs`, `geometry.rs` | IMPLEMENTATION §6h |
| Scheduling / load? | `datum/sched.rs` | — |
| Escrow / stake? | `datum/escrow.rs` | IS-6/7 |
| Federation? | `datum/court_live.rs`, `court_service.rs` | — |
| x402 / HTTP? | `datum/bin/gateway.rs`, `sdk/receipt.rs` | — |

## Conventions

- **Refusals are typed and total.** Failure modes are enums named
  `*Broken`, `*Refused`, `Malformed`, or `Flaw`; a conforming path
  refuses and names the reason, and never silently repairs.
- **Values are multi-axial.** Space and credit are an `Extent` — one
  exact-rational component per axis — never collapsed to a scalar.
- **Identity is structural.** `work_id` is derived from the structure of
  the work, not a nonce; the same work resubmitted is replay.
- **Conformance vectors are generated, not hand-written.** Byte-exact
  vectors are produced by the codec and asserted against the hex a
  document carries, so a document and its implementation cannot drift.
- **The dependency law is tested**, not merely documented: `assay` and
  `isthmus` fail a test if a mesh, a kernel, or any path dependency
  enters their manifests or imports.

## Keeping this map current

Each component above maps to one module. When a task changes a
component's behaviour or contract, update that module's `//!` header and
the corresponding row here in the same change, so the code and its
documentation close together. A new component adds a row to the relevant
crate table and a one-line `//!` header to its module.
