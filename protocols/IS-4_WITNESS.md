# IS-4 — THE WITNESS

**Status:** §5 frame + roles **ENFORCED** (`IS-4/1`, 2026-08-28) —
`isthmus::witness` (the frame, refuse-not-repair on arm and revision),
`datum::witnessing` (the watcher, held to all four §6 prohibitions;
the court's witness log; tag 84 in the court's own grant), the fourth
`plumbd` role, `tests/witness_frame.rs` + `tests/witnessing.rs`.
§8 stands: the verdict frame remains unsettled — watchers return
reports above the substrate, not on the wire. Grounded in the lab's
`measure/witnesses.md`, which surveyed every witness-shaped type
across the three trees.

## 1. Three roles, and none of them is optional

A witness alone is not a checkable claim. Measured across all five
witness-shaped types in the environment: **every one of them needs
something besides the witness** — a reference relation, a corpus, a
path, a record, a task-and-program.

That missing thing has a name on both sides already, and the roles are
three:

```
OBSERVER   holds the subject, at the depth it has standing to see
WITNESS    the claim, and the derivation that reached it
WATCHER    re-derives, and returns a verdict. Holds nothing.
```

## 2. Why the observer is not the watcher

**Because an observer is depth-bounded and a verdict must not be.**

netstratum, `lens/src/observer.rs`:

> *the entity cannot see deeper than its capacity allows, regardless of
> pressure — the effective limit is `min(requested, κ)`. That makes
> level-of-detail a **property of the observer**.*

xylarium says the same one tag over — tag 13 is a *facet grant, how deep
a peer may read*.

An observer that also verified would emit verdicts bounded by its own
capacity, and it would emit them **silently**: a shallow observer
returning *verified* on a claim it could not fully see is not lying, it
is answering a smaller question and saying nothing about the
difference.

Separating them makes the watcher **pure and total** — no state, no I/O,
no standing — and puts the messy question of *what exists, at what
depth, as of when* entirely in the observer.

## 3. Why the witness is not the observer

**Naming is not carrying.** The witness names its observer; it does not
ship it. A corpus is fetched, a record is consulted, a reference is
already held.

This is not an optimisation. `NS-1`'s own registry records the reason:

> *ARC-AGI-2 has amended published tasks, so two peers on different
> corpus revisions disagree about which tasks exist and neither is
> wrong. **A verdict that cannot say which corpus it was reached against
> cannot be compared with another.***

So the observer is identified, and the identity is part of the claim.
strand already carries this as tag 18 — *pouw: corpus revision, which
revision of the public corpus a verifier holds*. That tag is the
observer's identity on the wire, and it existed before this document
named the role.

## 4. The two arms

From `measure/witnesses.md`. A witness **must say which arm it is**,
because the watcher's budget differs by an order of magnitude and it has
to know before it starts.

| arm | checking costs | buys |
|---|---|---|
| **succinct** | less than producing | economy. `lith::Witness` is `O(m)`, *"no reduction, no division"* |
| **replay** | what producing costs | tamper-evidence, and the index of the first diverged link |

netstratum's certificate is the replay arm and says so: `verify_work`
*"RE-FORGES the task and RE-APPLIES the program (THE REPLAY-COMPLETE LAW
— an intact chain over a non-solution is not a solution). Finding is
search; checking is `O(program length)`."*

A replay witness is not a lesser witness. It is a different purchase,
and a category that collapsed the two would let a watcher think it had a
cheap check when it had an expensive one.

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
- `observer.revision` — required, never defaulted. `NS-3` §2.4 already
  rules that *a corpus without one names a moving target*, and a
  protocol is a corpus in this respect.
- `observer.depth` — the depth the claim was reached at. A watcher
  reading deeper than the witness was taken at is not checking the same
  claim.
- `derivation` — the grantee's, and opaque here. `IS-3` §5.2: what a
  value means inside a granted range is the grantee's.

The frame sits in the granting range of whoever owns the claim, not in a
range of its own. A witness about a `lith` relation is a `lith` frame.

## 6. What a watcher may not do

1. **May not observe.** If it needs the subject it is handed it, or it
   refuses. A watcher that fetches has standing, and standing is the
   observer's.
2. **May not repair.** The shared ratio rule already sets this at the
   byte level — *refuse, do not silently reduce* — and it holds one
   level up. A watcher that fixes a witness has produced a different
   claim and verified that instead.
3. **May not require canonical form.** `lith::Witness` is *"fixed only
   up to a constant per connected component, so a relation has many
   valid witnesses and every one of them verifies."* Equality of
   witnesses is not the test; verification is. This is the same gauge
   freedom `G` records.
4. **May not return a bare verdict.** It returns the verdict *and the
   observer it was reached against*, or the answer cannot be compared
   with another watcher's.

## 7. What is an instance today, and what is not

| | arm | instance? |
|---|---|---|
| `lith::Witness` | succinct | **yes** — constructible, `O(m)` check, names its reference |
| netstratum `SolveCertificate` | replay | **yes** — constructible via `seal`, names task and program |
| `strand::receipt::Receipt` | — | it is a *verdict*, not a witness. A watcher's output |
| `arbor::Receipt` | — | verdict. Cost unmeasured, recorded as unknown |
| `strand::pouw::Witness` | succinct | **no.** `Frame` and `Keying` are private, so it cannot be constructed outside strand — measured in `tests/mining.rs` |

The last row is the one that matters for the substrate. A format can be
agreed; an unconstructible type cannot be implemented against, and no
amount of specification substitutes. That is task #28 and it is
intelligence-lane's call.

## 8. What this does not settle

- **The verdict frame.** Watchers return verdicts and this document
  specifies only the witness. `strand` tag 17 is *pouw: verification
  report* and nothing decodes it.
- **Whether an observer can be wrong.** Two observers at the same
  revision must agree, or the revision is not an identity. Unmeasured.
- **`arbor::Receipt`'s arm.** Located, cost not read. `IS-4` should not
  assume it is succinct because it looks like `lith`'s.
