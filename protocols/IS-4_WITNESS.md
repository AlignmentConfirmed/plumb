# IS-4 — THE WITNESS

**Status:** the §5 frame and roles are implemented (`IS-4/1`):
`isthmus::witness` (the frame, refuse-not-repair on arm and revision),
`datum::witnessing` (the watcher held to the four §6 constraints, the
court's witness log, tag 84 in the court's own grant), and the fourth
`plumbd` role. Tests: `tests/wire_suite.rs (mod witness_frame)`,
`tests/wire.rs (mod witnessing)`. The §8 verdict frame is unsettled:
watchers return reports above the substrate, not on the wire.

## 1. Three roles

A witness alone is not a checkable claim: verifying it needs something
besides the witness — a reference relation, a corpus, a path, a record,
a task-and-program. The roles are three:

```
OBSERVER   holds the subject, at the depth it has standing to see
WITNESS    the claim, and the derivation that reached it
WATCHER    re-derives, and returns a verdict. Holds nothing.
```

## 2. The observer is not the watcher

An observer is depth-bounded: it cannot see deeper than its capacity
allows, so level-of-detail is a property of the observer (tag 13 is a
facet grant — how deep a peer may read). An observer that also verified
would emit verdicts bounded by its own capacity, and emit them silently:
a shallow observer returning *verified* on a claim it could not fully
see is answering a smaller question and saying nothing about the
difference.

Separating them makes the watcher pure and total — no state, no I/O, no
standing — and puts the question of what exists, at what depth, as of
when, entirely in the observer.

## 3. The witness names the observer, it does not carry it

The witness names its observer; it does not ship it. A corpus is
fetched, a record consulted, a reference already held. The observer's
identity is part of the claim: two peers on different corpus revisions
disagree about which subject exists, so a verdict that cannot say which
observer it was reached against cannot be compared with another. The
corpus revision is the observer's identity on the wire (tag 18).

## 4. The two arms

A witness must state which arm it is, because the watcher's budget
differs by an order of magnitude and it must know before it starts.

| arm | checking costs | buys |
|---|---|---|
| **succinct** | less than producing | economy: an `O(m)` check |
| **replay** | what producing costs | tamper-evidence, and the index of the first diverged link |

A replay witness is not a lesser witness — it is a different purchase. A
category that collapsed the two would let a watcher think it had a cheap
check when it had an expensive one.

## 5. The frame

```
witness  = arm u8 ‖ observer ‖ subject ‖ derivation
observer = kind u8 ‖ identity[32] ‖ revision utf8 ‖ depth u8
subject  = identity[32]
```

- `arm` — 0 succinct, 1 replay. Refuse any other value; this is the
  budget and a guess at it is not recoverable.
- `observer.identity` — what to consult. A corpus digest, a record
  address, a reference cell.
- `observer.revision` — required, never defaulted: a corpus without a
  revision names a moving target, and a protocol is a corpus in this
  respect.
- `observer.depth` — the depth the claim was reached at. A watcher
  reading deeper than the witness was taken at is not checking the same
  claim.
- `derivation` — the grantee's, and opaque here (`IS-3` §5.2: what a
  value means inside a granted range is the grantee's).

The frame sits in the granting range of whoever owns the claim, not in a
range of its own: a witness about a kernel's relation is that kernel's
frame.

## 6. The watcher's constraints

1. **Observes nothing.** If it needs the subject it is handed it, or it
   refuses. A watcher that fetches has standing, and standing is the
   observer's.
2. **Repairs nothing.** The shared ratio rule sets this at the byte
   level — refuse, do not silently reduce — and it holds one level up. A
   watcher that fixes a witness has produced a different claim and
   verified that instead.
3. **Requires no canonical form.** A relation has many valid witnesses
   (fixed only up to a constant per connected component), and every one
   verifies; equality of witnesses is not the test, verification is.
   This is the gauge freedom `G` records.
4. **Returns no bare verdict.** It returns the verdict and the observer
   it was reached against, or the answer cannot be compared with another
   watcher's.

## 7. Constructibility

A format can be agreed; an unconstructible type cannot be implemented
against, and no amount of specification substitutes. A witness type must
be constructible outside the crate that defines it.

## 8. Open items

- **The verdict frame.** Watchers return verdicts; this document
  specifies only the witness.
- **Whether an observer can be wrong.** Two observers at the same
  revision must agree, or the revision is not an identity. Unmeasured.
