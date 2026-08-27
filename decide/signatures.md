# DECIDED — the signature layer (design; not yet built)

**Status: DESIGN RATIFIED 2026-08-27. Nothing below is ENFORCED until
its task line names a test.**

This is the unpoured first floor. Without it, "independent parties" is
structural in transit but unenforceable at the edges, and a grant on
the chain names a party nothing can hold to the name.

## The three rulings

### 1 · Key scheme: Ed25519

The standard for modern distributed consensus: fast, side-channel
resistant, and free of ECDSA's malleability traps. A solver's identity
is an Ed25519 public key; nothing else about the solver is identity.

### 2 · Digest family: BLAKE3

`assay` verifies by deterministic re-derivation and `work_id` is
content-addressed, so the digest sits on the hot path of every claim.
BLAKE3's throughput is the reason; its tree structure is a bonus
(future: incremental digests over large claim bodies).

Two seams take the digest, and both were left deliberately open in the
existing code — this decision closes them **at the edge, not in the
wire**:

- `isthmus::deed` anchors carry `digest: Vec<u8>`, uninterpreted.
  BLAKE3 is what an edge writes there. The wire still does not name a
  digest function; the *court* does.
- The envelope hash a presenter signs (below) is BLAKE3 of the whole
  opaque frame: `tag ‖ LE32(len) ‖ value`.

### 3 · Grant binding

A signed grant on the ledger cryptographically binds three things:

```
solver_pubkey  ×  tag-range  ×  epoch window
```

- **Issuance.** A grant deed (IS-3) gains a `holder_key` field: the
  Ed25519 public key of the grantee. The deed enters the chain like
  any act; the chain remains the one authorization surface — no
  allowlist in source, no certificate authority.
- **Presentation.** To present under a grant, the presenter signs the
  **envelope hash** — BLAKE3 of the opaque frame bytes. The signature
  travels beside the envelope, never inside it.
- **Verification.** A checker resolves the envelope's tag to the
  active grant on chain, takes its `holder_key`, and verifies the
  signature over the envelope hash. **The payload is never read.**
  Signature checking is not claim inspection: a carrier may refuse an
  unsigned or mis-signed envelope at admission without becoming an
  arbiter, because the check touches only bytes the carrier already
  owns (the frame envelope) and public chain state.
- **Epoch window.** A grant is valid for an explicit epoch interval
  (the reward book's epoch acts already exist: `EpochOpened` /
  `EpochClosed`). A signature presented outside its grant's window is
  refused as stale, which is the anti-replay story IS-2 §6 left open —
  freshness becomes a chain fact, not a transport secret.

## What this does not change

- The wire format. Signature and pubkey travel as new tagged records
  in the granted band; skip-unknown means an unsigned-era reader
  forwards them whole.
- The arbiter refusal. Verifiers and the court verify **claims**;
  anyone may verify **signatures**. The first requires reading the
  payload; the second forbids it.
- The digest field's opacity in `isthmus`. The crate still computes no
  digest and names none — the court and the SDK do.

## Scheme agility

The signature record carries a one-byte scheme tag. `0x01 = Ed25519 /
BLAKE3` is the baseline. The seam exists so a successor scheme — see
[`topological-cryptography.md`](topological-cryptography.md) — can be
admitted by a chain act rather than a wire break. Revisions compare
for equality, never order; two peers on different schemes disagree
about what a signature means, and neither is wrong.

The settlement receipt this layer makes possible is what external
payment rails settle against — see
[`x402-integration.md`](x402-integration.md) for the HTTP/stablecoin
flow (PROSPECTIVE; depends on S1–S7).

## Task list

| ID | task | done when |
|---|---|---|
| **S1** | `sig` module (Ed25519 keys, BLAKE3 envelope hash) in the SDK or its own crate | sign/verify round-trip test |
| **S2** | Signature + pubkey as tagged records; skip-unknown proven | old reader forwards signed traffic whole |
| **S3** | Grant deed carries `holder_key`; chain codec revision | IS-3/IS-6 revision bump; vectors |
| **S4** | Court refuses claims whose envelope signature fails or whose grant is outside its epoch window | refusal tests: forged, stale, unbound |
| **S5** | Carrier admission check (envelope-only) shown payload-blind | test: carrier verifies without decoding |
| **S6** | Anchor digests written as BLAKE3 at the court edge | cross-chain anchor round-trip |
| **S7** | Scheme tag `0x01` + refusal of unknown schemes | unknown-scheme refusal test |
