# The Proof Economy

**An epistemic settlement layer** — a market where claims are priced,
verified, and settled like trades.

In finance, settlement is the moment a trade becomes final: obligations
discharged, ownership actually transferred. This system does the same
thing for **claims of useful work**. A claim arrives unsettled —
asserted, unpaid, contestable. It is verified by re-derivation or by
convergence among independent parties, and then it **settles**: credited
on a ledger, recorded, and protected against ever being paid twice.

Blockchains settle ownership. Prediction markets price beliefs but rely
on outside oracles. Peer review verifies but has no ledger and no
economics. This layer settles *the claims themselves*, natively.

---

## 1 · The two components

The economy is two parts with a deliberate division of ignorance:

### isthmus — the substrate (moves claims without understanding them)

A dependency-free wire library. It defines:

- **The record** — `tag ‖ length ‖ value`. The length prefix is what
  lets a carrier forward a frame it cannot read.
- **The four verdicts** — accept, refuse, skip, wait. *Skip* (a frame
  you will never own) and *wait* (a frame that has not finished
  arriving) are kept distinct, which is what lets strangers share a
  wire without a coordinator.
- **The registry** — tag ranges granted to vocabularies. A granted
  range is yours until you retire it; a retired tag is never reissued;
  the frozen band is never grantable. This is the authorization
  surface (see §4).
- **Node roles** — producer, verifier, carrier. Roles are
  capabilities, not ranks; nothing distinguishes a sender from a
  receiver, so there is no coordinator to capture.
- **Claim envelopes** — opaque frames (tags 80–82) that carry proof
  bytes the substrate never inspects.

Isthmus enforces exactly one epistemics: *carriers cannot read what
they carry.* Independence of parties in transit is structural, not
promised.

### datum — the court (understands claims without moving them)

The settlement engine. It defines:

- **The board** — priced, multi-axial space. Posting a question is
  opening space; the price is the bounty. Space is n-dimensional on
  purpose: an answer can be required to close on several independent
  axes (correct, bounded, reproducible, cited) before it earns
  anything.
- **Work identity** — `work_id` is derived from the *structure* of the
  work, never from a nonce. The same answer resubmitted — or copied —
  is the same work, and is refused as replay. This is what makes a
  knowledge economy possible: credit cannot be farmed by re-uploading
  known results.
- **The reward book** — credit ledgered against work identity, with
  multi-axis cover: claims that do not close on every axis earn
  nothing.
- **Settlement** — `enact_if_funded`: a claim settles only when the
  space it lands on is funded and every axis converges. Settlement
  writes the chain; the chain is the record of what was measured
  rather than assumed.
- **The chain** — founding acts, blocks as well-formed act batches,
  vertical anchors across chains, and sphere-merge economics for when
  two independently-grown ledgers meet.

### How they compose

```
producer                    carriers                    court
   │  claim in an envelope     │                          │
   ├──────────────────────────▶│  forwarded unread        │
   │                           ├─────────────────────────▶│
   │                           │                          │ verify by
   │                           │                          │ re-derivation
   │                           │            credit settles│ or convergence
   │◀──────────────────────────┴──────────────────────────┤
   │                                       on the ledger  │
```

Isthmus moves things without understanding them; datum understands
things without moving them. A party that can do both — read a proof
*and* carry it — would be an arbiter, and the design refuses arbiters.

---

## 2 · The lifecycle of a claim

1. **Space opens.** A question, task, or frontier is priced on the
   board — per axis, with a bounty.
2. **Work happens elsewhere.** A kernel (any attached domain engine)
   produces a candidate: a proof, a construction, a solution.
3. **The claim travels.** Wrapped in an opaque envelope, it crosses the
   substrate. No carrier can read it; no carrier can front-run it.
4. **The court verifies.** For checkable domains, verification is
   re-derivation: the court reruns the work and the claim either closes
   or it does not. For open domains, verification is convergence:
   credit requires independent claims with matching structure from
   parties who could not see each other's work in transit.
5. **Settlement.** If the space is funded and every axis closes, the
   claim settles: credit is written, the work identity is recorded, and
   any future copy of the same work is replay.

---

## 3 · What is checkable, and what is not

The layer's honest guarantee is **verified-or-convergent, not true.**

- **Checkable domains** (mathematics, program verification, data
  transformation, retrieval with provenance): re-derivation gives
  answers that arrive with proof. Here the economy is at full strength.
- **Open domains** (judgment, taste, strategy): re-derivation is
  impossible; convergence is the only tool, and convergent is not the
  same as correct — independent parties can share a bias. The layer
  prices and settles such claims but does not pretend to more certainty
  than the mechanism provides.

Any public statement about this system should preserve that
distinction. Overclaiming is the one failure the settlement metaphor
cannot survive.

---

## 4 · Attaching a kernel (the authorization model)

The economy is a substrate, not a club — but attachment is authorized,
and the authorization is *on the ledger*, not in a config file:

1. **A grant.** The kernel's vocabulary receives a tag range from the
   registry. Grants are recorded as deeds on the chain; the frozen band
   is never granted; a retired range is never reissued.
2. **A declaration.** The kernel declares itself over the handshake —
   which revisions it speaks. Revisions compare for equality and are
   never ordered: two peers on different revisions disagree about what
   a frame means, and neither is wrong.
3. **An on-ramp.** The kernel's domain dialect is translated at its own
   edge into a portable claim the court can verify. The substrate never
   learns the dialect; the court never learns the kernel.

The SDK surface a kernel needs (see `decide/consolidation.md` for the
crate plan):

| operation | what it does |
|---|---|
| `attach` | handshake + revision declaration |
| `grant` | obtain / verify a tag-range deed |
| `submit` | wrap a portable claim in an envelope (tags 80–82) |
| `survey` | read priced space on the board |
| `settle` | present a credit stack against funded space |
| `credit` | query the reward book by work identity |

---

## 5 · Status — what is built and what is not

Built and enforced by tests today:

- The wire, the four verdicts, skip-unknown, exact rationals, refusals
- The registry, the handshake, the chain codec, conformance vectors
- The board, the reward book (work_id-primary), funded settlement,
  block production, vertical anchors, sphere-merge economics
- Producer / verifier / carrier roles; opaque claim envelopes

Not built, and required before any public deployment:

- **Cryptographic identity, enforcement half.** Primitives exist
  (Ed25519 over BLAKE3 envelopes) and the chain binds keys to grants
  (`Act::Bind`); courts and carriers do not yet refuse unsigned or
  mis-signed envelopes. Until they do, one party can present as many.
- **Transport.** The substrate is a library, not a daemon. Sessions,
  freshness, and anti-replay at the transport level are specified as
  open, not implemented.
- **The witness role** (IS-4) is specified, not built.

The honest description of the current system: a complete, tested
*model* of an epistemic settlement layer, with the trust layer still to
be poured underneath it.
