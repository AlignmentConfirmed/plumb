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
cargo test            # the whole suite; touches nothing outside this directory
cargo run -p plumb-sdk --example join
```

## The protocols

The wire is specified independently of the code, with conformance
vectors an implementation in any language can be checked against:

- [`protocols/`](protocols/) — IS-1 (wire) through IS-6 (chain)
- [`conformance/`](conformance/) — the vectors, the manifest, and a
  Python reference reader
- [`INTEGRATING.md`](INTEGRATING.md) — the entry point for implementers
- [`decide/`](decide/) — design decisions and research tracks:
  the signature layer (ratified design), topological cryptography
  (prospective), x402 payment rails (prospective)

## What this is, honestly

A complete, tested **model** of a settlement layer: the wire, the
court, the economics, ~270 tests. It is not yet a deployable network.
The status discipline lives in `PROOF_ECONOMY.md` §5: nothing here
claims to be built unless a test can fail over it.

Kernel-edge measurements — the suites that verify live domain engines
against this reference — live in the lab, a separate repository that
depends on these crates the way any outsider would. This workspace
reaches nothing outside its own directory.

## Known gaps

Published with these gaps **stated, not hidden**. An earlier publishing
policy held release until they closed; that hold was **waived by a
recorded decision (2026-08-27)** on the grounds that transparency about
what is verified versus pending builds more trust than delay. The gaps:

1. **No cryptographic identity.** No signatures exist; the chain's
   digest field is opaque and no digest family is wired in.
   "Independent parties" is structural in transit but unenforceable at
   the edges — one party can present as many. The signature layer is
   designed (`decide/signatures.md`), not built.
2. **No transport.** The substrate is a library, not a daemon. IS-2 §6
   session freshness is specified open.
3. **IS-4 witness role** is specified, not built.
4. **Tag-51 relation frame carries no shape.** A hexagon crosses as a
   five-simplex. Known wrong; owed as a frame revision. Do not build
   on tag 51 as published.
5. **No independent reader yet.** Every specification gap found so far
   (four) was found by the author re-reading. If you implement IS-1
   from the documents and the conformance vectors alone, what you trip
   over is exactly the feedback this project wants — file it.
