# The Proof Economy

An epistemic settlement layer: a system that prices, verifies, and
settles claims of useful work.

Settlement is the point at which a claim becomes final: credited on a
ledger and recorded. A claim is submitted unsettled — asserted, unpaid,
contestable. It is verified by re-derivation or by convergence among
independent parties, and then it settles: credited on a ledger,
recorded, and protected against being paid twice.

Blockchains settle ownership. Prediction markets price beliefs and rely
on external oracles. Peer review verifies without a ledger or
economics. This layer settles the claims themselves.

---

## 1 · The two components

The economy has two parts with a separation of function:

### isthmus — the substrate (moves claims without interpreting them)

A dependency-free wire library. It defines:

- **The record** — `tag ‖ length ‖ value`. The length prefix lets a
  carrier forward a frame it does not interpret.
- **The four verdicts** — accept, refuse, skip, wait. *Skip* (a frame a
  node will never own) and *wait* (a frame that has not finished
  arriving) are distinct, which lets independent parties share a wire
  without a coordinator.
- **The registry** — tag ranges granted to vocabularies. A granted
  range is held until retired; a retired tag is never reissued; the
  frozen band is never grantable. This is the authorization surface
  (see §4).
- **Node roles** — producer, verifier, carrier. Roles are capabilities,
  not ranks; senders and receivers are not distinguished, so there is
  no coordinator role.
- **Claim envelopes** — opaque frames (tags 80–82) carrying proof bytes
  that the substrate does not inspect.

Isthmus enforces one epistemic property: carriers do not interpret what
they carry. Independence of parties in transit is structural.

### datum — the court (interprets claims without moving them)

The settlement engine. It defines:

- **The board** — priced, multi-axial space. Posting a question opens
  space; the price is the bounty. Space is n-dimensional: an answer can
  be required to close on several independent axes (correct, bounded,
  reproducible, cited) before it earns anything.
- **Work identity** — `work_id` is derived from the structure of the
  work, not from a nonce. The same answer resubmitted or copied is the
  same work, and is refused as replay. Credit cannot be obtained by
  re-uploading known results.
- **The reward book** — credit ledgered against work identity, with
  multi-axis cover: claims that do not close on every axis earn nothing.
- **Settlement** — `enact_if_funded`: a claim settles only when the
  space it lands on is funded and every axis converges. Settlement
  writes the chain; the chain records what was measured.
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

Isthmus moves claims without interpreting them; datum interprets claims
without moving them. A party that both reads a proof and carries it
would be an arbiter; the design separates these two functions so that
no single party holds both.

---

## 2 · The lifecycle of a claim

1. **Space opens.** A question, task, or frontier is priced on the
   board — per axis, with a bounty.
2. **Work happens elsewhere.** A kernel (any attached domain engine)
   produces a candidate: a proof, a construction, a solution.
3. **The claim travels.** Wrapped in an opaque envelope, it crosses the
   substrate. Carriers do not read it and cannot front-run it.
4. **The court verifies.** For checkable domains, verification is
   re-derivation: the court reruns the work and the claim either closes
   or it does not. For open domains, verification is convergence:
   credit requires independent claims with matching structure from
   parties that could not see each other's work in transit.
5. **Settlement.** If the space is funded and every axis closes, the
   claim settles: credit is written, the work identity is recorded, and
   any future copy of the same work is replay.

---

## 3 · What is checkable, and what is not

The layer's guarantee is **verified-or-convergent, not true.**

- **Checkable domains** (mathematics, program verification, data
  transformation, retrieval with provenance): re-derivation produces
  answers that arrive with proof. The economy operates at full strength
  here.
- **Open domains** (judgment, taste, strategy): re-derivation is not
  possible; convergence is the only available mechanism, and convergent
  is not the same as correct — independent parties can share a bias. The
  layer prices and settles such claims to the level of certainty the
  mechanism provides.

Public statements about this system preserve this distinction.

---

## 4 · Attaching a kernel (the authorization model)

Attachment is authorized, and the authorization is recorded on the
ledger rather than in a config file:

1. **A grant.** The kernel's vocabulary receives a tag range from the
   registry. Grants are recorded as deeds on the chain; the frozen band
   is never granted; a retired range is never reissued.
2. **A declaration.** The kernel declares itself over the handshake —
   which revisions it speaks. Revisions compare for equality and are
   never ordered: two peers on different revisions disagree about what a
   frame means, and neither is authoritative.
3. **An on-ramp.** The kernel's domain dialect is translated at its own
   edge into a portable claim the court can verify. The substrate does
   not process the dialect; the court does not process the kernel.

The SDK surface a kernel uses:

| operation | what it does |
|---|---|
| `attach` | handshake + revision declaration |
| `grant` | obtain / verify a tag-range deed |
| `submit` | wrap a portable claim in an envelope (tags 80–82) |
| `survey` | read priced space on the board |
| `settle` | present a credit stack against funded space |
| `credit` | query the reward book by work identity |

---

## 5 · Status

Built and enforced by tests:

- The wire, the four verdicts, skip-unknown, exact rationals, refusals
- The registry, the handshake, the chain codec, conformance vectors
- The board, the reward book (work_id-primary), funded settlement,
  block production, vertical anchors, sphere-merge economics
- Producer / verifier / carrier roles; opaque claim envelopes

- **Cryptographic identity.** Ed25519 over BLAKE3 envelopes (scheme
  `0x01`), keys bound to grants on the chain (`Act::Bind`, IS-6/4). With
  `require_signatures = true`, a court refuses forged, stale, and unbound
  presentations.
- **Transport.** The `plumbd` daemon runs signed, fresh sessions over
  TCP (IS-2/2 session challenge), durable court federation, and optional
  chain-pinned TLS (IS-6/6). Admission limits (connection caps, handshake
  deadline) bound unauthenticated connections.
- **Live registration** (`plumbd join`): a key with no genesis-time bind
  joins a running court and registers without a restart.
- **The IS-4 witness** (IS-4/1): the frame, the court's witness log, and
  the watcher bound by four prohibitions.

Enabled per deployment, or not yet implemented:

- Signature enforcement and live registration are config flags, not
  defaults.
- TCP is the only transport; there is no NAT traversal or peer discovery.
- The IS-4 verdict frame (§8).

The current system is a tested model of an epistemic settlement layer.
It is not yet a deployed network.
