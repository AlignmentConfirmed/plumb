# plumb

A plumb line is the surveyor's instrument for finding true vertical —
the reference a measurement is trusted against. That is this project's
job, for claims.

**plumb is an epistemic settlement layer** — a market where claims are
priced, verified, and settled like trades. A claim arrives unsettled:
asserted, unpaid, contestable. It is verified by re-derivation or by
convergence among independent parties, and then it **settles** —
credited on a ledger, recorded, and protected against ever being paid
twice. The long-form description is [`PROOF_ECONOMY.md`](PROOF_ECONOMY.md).

## The workspace

Three crates, one law each:

| crate | law | role |
|---|---|---|
| [`crates/isthmus`](crates/isthmus) | imports **nothing** | the substrate — carries claims it cannot read |
| [`crates/assay`](crates/assay) | imports **nothing** | the verification physics — multi-axial convergence |
| [`crates/datum`](crates/datum) | imported by **nothing** | the court — prices space, verifies, settles, keeps the chain |

The division of ignorance is the security model: isthmus moves things
without understanding them; datum understands things without moving
them. A party that could do both would be an arbiter, and this design
refuses arbiters.

```bash
cargo test               # the whole suite; touches nothing outside this directory
cargo run -p plumb-sdk --example join
scripts/proofnet.sh      # the whole economy on one machine: genesis, signed
                         # claims, a registered domain, federation, kill/resume
scripts/simnet.sh start  # a standing local network on 9xxx ports: three
                         # federating courts, a carrier, looping signed clients
```

## The protocols

The wire language is called **Plumbline** — utterances are linear
bytes; meanings live in spaces ([`PLUMBLINE.md`](PLUMBLINE.md)). It is
specified independently of the code, with conformance vectors an
implementation in any language can be checked against:

- [`PLUMBLINE.md`](PLUMBLINE.md) — the language, and how it is
  multi-dimensional
- [`POWPP.md`](POWPP.md) — the physics, the proofs, and the
  economics: what the engine verifies and why credit is earned
- [`protocols/`](protocols/) — IS-1 (wire) through IS-6 (chain)
- [`conformance/`](conformance/) — the vectors, the manifest, and a
  Python reference reader
- [`INTEGRATING.md`](INTEGRATING.md) — the entry point for implementers
- [`ROADMAP.md`](ROADMAP.md) — from tested model to live beta network
- [`IMPLEMENTATION.md`](IMPLEMENTATION.md) — the program of record:
  every ruling (signatures, universal checker, freshness, x402,
  topological track) with its task ladder and open operator questions

## What this is

A complete, tested **model** of a settlement layer: the wire, the
court, the economics, ~300 tests. It is not yet a deployable network.
The status discipline lives in `PROOF_ECONOMY.md` §5: nothing here
claims to be built unless a test can fail over it.

Kernel-edge measurements — the suites that verify live domain engines
against this reference — live in the lab, a separate repository that
depends on these crates the way any outsider would. This workspace
reaches nothing outside its own directory.

## Known gaps

plumb is a tested model, not a deployed network. What stands, and what
does not, yet:

1. **Cryptographic identity — built, opt-in.** Ed25519 over BLAKE3
   envelopes (scheme 0x01), with keys bound to grants on the chain
   (`Act::Bind`, IS-6/4). Courts set to `require_signatures = true`
   reject forged, stale, and unbound presentations (S1–S7). A party
   with no genesis-time bind can join a running court and register one
   (`plumbd join`, P2) without a restart or a hand-edited config.
   Enforcement and live registration are config flags today; a testnet
   genesis would make them the default.
2. **Transport — built, TCP only.** `plumbd` runs signed, fresh
   sessions over TCP (IS-2/2 session challenge: a replayed session's
   answer covers a dead token), courts federate durably, and the
   channel can run encrypted (`tls = true`, IS-6/6 — a chain-pinned
   certificate, no CA, since a court has no DNS name to answer to).
   Admission limits (connection caps, a handshake deadline) bound what
   an unauthenticated connection can hold. TLS is opt-in per court, TCP
   is the only transport, and there is no NAT traversal or peer
   discovery — peers are configured, not found.
3. **IS-4 witness — built (IS-4/1).** The frame (arm ‖ observer ‖
   subject ‖ derivation), the court's witness log, and a watcher bound
   by four prohibitions: it may not observe, repair, canonicalize, or
   answer bare. The verdict *frame* is deliberately left open (§8):
   reports live above the substrate.
4. **Tag-51 — closed (IS-1/5).** The closure carries its shape as a
   declared-complex definition: a hexagon and a five-simplex over the
   same six orbs are distinct bytes (vectors V17/V18), and the legacy
   grain is explicit "shape unknown," never an inferred simplex.
5. **No independent implementation yet.** Every specification gap found
   so far surfaced in review. If you implement IS-1 from the documents
   and the conformance vectors alone, the places you trip over are the
   feedback this project most wants — please file them.
