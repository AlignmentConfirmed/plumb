# plumb

plumb is an epistemic settlement layer: a system that prices, verifies,
and settles claims. A claim enters the system unsettled and remains so
until it is verified — by re-derivation, or by convergence among
independent parties. Once verified, it settles: it is credited on a
ledger, recorded, and protected against duplicate payment. A full
description is given in [`PROOF_ECONOMY.md`](PROOF_ECONOMY.md).

## Workspace

Three crates:

| crate | dependencies | role |
|---|---|---|
| [`crates/isthmus`](crates/isthmus) | none | the substrate: transports claims without interpreting them |
| [`crates/assay`](crates/assay) | none | verification: multi-axial convergence |
| [`crates/datum`](crates/datum) | not imported by other crates | the court: prices work, verifies, settles, and maintains the chain |

isthmus transports claims without interpreting them; datum interprets
claims without transporting them. The two roles are kept separate: a
single party performing both could act as an arbiter.

```bash
cargo test               # runs the full test suite
cargo run -p plumb-sdk --example join
scripts/proofnet.sh      # single-machine run: genesis, signed claims, a
                         # registered domain, federation, kill/resume
scripts/simnet.sh start  # standing local network on 9xxx ports: three
                         # federating courts, a carrier, signed clients
```

## Protocols

The wire language is Plumbline: messages are linear byte sequences whose
meaning is multi-dimensional ([`PLUMBLINE.md`](PLUMBLINE.md)). It is
specified independently of the implementation, with conformance vectors
that an implementation in any language can be checked against.

- [`PLUMBLINE.md`](PLUMBLINE.md) — the wire language
- [`POWPP.md`](POWPP.md) — what the engine verifies and how credit is
  assigned
- [`protocols/`](protocols/) — IS-1 (wire) through IS-6 (chain)
- [`conformance/`](conformance/) — the vectors, the manifest, and a
  Python reference reader
- [`INTEGRATING.md`](INTEGRATING.md) — guide for implementers
- [`ROADMAP.md`](ROADMAP.md) — development roadmap
- [`IMPLEMENTATION.md`](IMPLEMENTATION.md) — rulings, task ladders, and
  open questions

## Status

plumb is a tested model of a settlement layer, covering the wire
protocol, the court, and the settlement economics, with approximately
300 tests. It is not a deployed network. A component is documented as
built only when a test covers it (`PROOF_ECONOMY.md` §5).

Kernel-edge measurements — the suites that verify live domain engines
against this reference — are maintained in a separate repository that
depends on these crates.

## Scope

Implemented:

- Ed25519-over-BLAKE3 signatures (scheme `0x01`); keys bind to grants on
  the chain (`Act::Bind`, IS-6/4). With `require_signatures = true`, a
  court rejects forged, stale, and unbound presentations.
- Live registration (`plumbd join`): a key with no genesis-time bind
  joins a running court and registers without a restart.
- Signed, fresh sessions over TCP (IS-2/2 session challenge); durable
  court federation; optional channel encryption (`tls = true`, IS-6/6:
  a chain-pinned certificate). Admission limits (connection caps,
  handshake deadline) bound unauthenticated connections.
- IS-4 witness (IS-4/1): the frame, the court's witness log, and a
  watcher bound by four prohibitions.
- Tag-51 shaped relations (IS-1/5): a relation carries its polytopal
  shape as a declared-complex definition.

Not yet implemented:

- Signature enforcement and live registration are enabled per deployment
  by config flag, not by default.
- Transports other than TCP; NAT traversal; peer discovery.
- The IS-4 verdict frame (§8).
- An independent implementation from the specification and conformance
  vectors.
