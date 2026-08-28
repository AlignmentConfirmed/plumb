# IS-2 — SESSION

**Status:** §7 (**never vs not yet**) is **ENFORCED** in
`isthmus::session` / `datum::session`. §6 freshness: §6.0 session challenge
is **OPEN** — and is **not** the primary identity key for useful work
(see `FOUNDATION.md` §5 Wave B / the lab's `decide/powpp-alignment.md`).

## 1. What a session is

A sequence of records in one direction, and the same in the other.
Nothing in the sequence marks which side is which — refusals 4 and 19,
and the lab's `decide/siblings.md`. There is no coordinator, no server, and no
distinguished linking mesh.

A session is not a connection. A file carrier and a socket carrier run
the same session; the difference is only how bytes arrive.

## 2. Record boundaries

A stream has no message boundaries and a file does, so a delivery can
stop mid-record. An envelope was refused for this — refusal 1 — because
an envelope would make the socket's bytes differ from the file's.

The frame's own length prefix answers it instead.

```rust
// strand/src/wire.rs:594
pub fn whole_records(bytes: &[u8]) -> usize {
    let mut consumed = 0;
    while consumed < bytes.len() {
        match take_frame(rest) {
            Ok((_, _, taken)) => consumed += taken,
            Err(_) => break,
        }
    }
    consumed
}
```

```rust
// strand/src/session.rs:607
pub fn feed(&mut self, arrived: &[u8]) -> Vec<u8> {
    self.held.extend_from_slice(arrived);
    let whole = wire::whole_records(&self.held);
    let ready = self.held[..whole].to_vec();
    self.held = self.held[whole..].to_vec();
    ready
}
```

The partial tail is held. The adapter parses nothing. Per
the lab's `decide/wire-framing.md`, the session **sequences** records and never
reframes them: a record read from a socket and the same record read from
a file are the same bytes.

## 3. Outcomes

The ancestor's `Heard` vocabulary carries the kernel's reasons whole and
never a status:

```
Authorized(Latched)      a peer latched on, and the cell it stood on
Refused(Refusal)         it did not, with the kernel's reasons
Receipt(Receipt)         a presented path, re-derived
PathRefused{step,rifts}  a path this node presented tore, with every rift
```

> *A session that reported "refused" and dropped the reason would undo*
> the whole design. Reasons travel; statuses do not.

## 4. Reattachment

`continuity.rs`. A node that detaches, plays and returns holds a
*different* relation, so it has a different address — observation only
grows a universe. It presents what it was, and the mesh checks that what
it is now **contains** it.

The ancestor states the limit plainly and this document does not soften
it:

> *It proves continuity of structure. It does not prove identity of
> operator. ... a returning node's claim is checkable by anyone and
> **forgeable by anyone who was watching**. Making it unforgeable is the
> Sybil anchor and it is not solved here.*

## 5. Linking meshes

A mesh linking into `isthmus` runs an ordinary session. It demonstrates
a position or it does not, exactly as a kernel does, and it forwards
frames it does not own by length. Nothing in this document treats it
differently, which is the lab's `decide/node-identity.md` holding.

## 6. Freshness — ruled, and split by frame kind (H7)

### 6.0 IS-2/2 — the session challenge (CLOSED 2026-08-27)

The one OPEN hole is closed, at the layer it belonged to. After its
declaration, a court emits a **session challenge**: one record whose
value is eight bytes of operating-system entropy, framed under the
court's own tag. Under signature enforcement, the FIRST attestation on
the session must verify over the challenge's **exact frame bytes** by
a chain-bound key; until it does, the session is not live and every
work record refuses.

What this buys, precisely: a **replayed session dies**. The recorded
answer an attacker captures covers a token the court never issues
again — the token never repeats, so the old signature binds dead
bytes. What it deliberately does not buy: work replay protection
(that was never the session's job — §6.1 stands; `work_id` is the
primary identity) or payload inspection (a carrier relays the
challenge verbatim, and the freshness survives carriage for the same
reason the signature does: the answer binds bytes, not routes).

Lenient courts emit the challenge and do not demand the answer; an
unsigned peer reads past it. Enforced by `datum::plumbd`
(`tests/wire.rs (mod session_freshness)`, including the replayed-session and
through-carrier measurements).

**Session still detects no replay.** That is deliberate (§6.1).

**Authority may apply secondary wire hygiene** to **effectful** payloads
(`datum::hygiene::WireHygiene`): exact byte identity only, after peel.
Primary useful-work identity remains `work_id` on the reward book.
Neither is a session sequence number.

Ancestor measurement (historical):

```
grep -rniE 'replay|freshness|nonce|seen before|duplicate' strand/src/*.rs
```

returns only unrelated `RELATION_DUPLICATED` and `MISMATCH_DUPLICATED`
constants. There is no sequence number, no window, no seen-set.

the lab's `decide/transport-security.md` and the lab's `decide/node-identity.md` both
concluded that replay is not answered where it was being looked for:

- Tampering degrades to refusal because nothing above believes an
  unre-derived claim. **A replayed frame is not tampered** — it is a
  valid frame, and it re-derives correctly, because it did before.
- Identity would not have caught it either. A named peer replaying its
  own valid frame is still a named peer sending a valid frame.

### 6.1 RULED — freshness is a property of the frame, not the session

The session owes **no sequence number, no window and no seen-set.**

A frame is one of two things, and replay means something different to
each:

| | replayed | why |
|---|---|---|
| a **claim** | changes nothing | nothing is believed on presentation. The receiver re-derives, and re-deriving a claim that was true gives the same answer |
| an **effect** | must be idempotent | applying it twice must equal applying it once, or capacity moves twice |

So the rule is not *detect the replay*. It is:

> **A frame that has an effect must be idempotent under replay, either
> naturally or by carrying an identity the receiver dedups on.**

### 6.2 The aperture does NOT do this — corrected

A first version of this section claimed the aperture was already
idempotent, on the strength of `settle` opening with

```rust
let mut deals: Vec<Deal> = legs.iter().map(|leg| leg.deal).collect();
deals.sort_unstable();
deals.dedup();
```

**That was a misreading.** `dedup()` deduplicates the deal *list*, so
each deal is enumerated once. The totals then sum **every** matching
leg:

```rust
.fold(zero(), |sum, leg| sum + leg.amount.clone())
```

So a replayed pair doubles both sides. Measured:

```
once      Balanced { deal 7, amount 5 }
replayed  Balanced { deal 7, amount 10 }
```

**And it stays `Balanced`.** Both sides double equally, conservation
holds, nothing refuses. The replay is invisible *because it preserves
the invariant that would have caught it* — the worst shape a defect
takes.

`datum/tests/freshness.rs::a_replayed_effect_is_not_idempotent_and_the_replay_is_invisible`
holds this, and is written so that fixing the defect fails the test
rather than passing it silently.

The ruling in §6.1 stands — a claim is replay-safe and an effect must be
idempotent. What was wrong was the belief that any effect already **is**.

### 6.3 Why this satisfies the constraints a sequence would have strained

- **No issuer.** An identity on a frame is not a credential; nobody
  vouches for a deal number.
- **The carrier decides nothing.** Dedup is the grantee's, inside a
  granted frame. The mesh never reads it.
- **Socket bytes do not differ from file bytes.** The identity is
  already inside the value.

A per-session sequence would have failed the first constraint the moment
sessions had to be bound to peers, and there is no identity to bind them
to.

### 6.4 What is owed

**The aperture itself**, first. An identity on the frame is not enough;
the *reader* must dedup on it, and `settle` sums instead. The fix is in
`settle`, not on the wire — legs carrying one deal and one direction
from one party are one leg.

Then **an audit of which granted frames have effect, and whether each
carries an identity a reader actually dedups on.** These are candidates
and are unchecked:

```
tag  3   authorization          latching — idempotent naturally?
tag  8   composed authorization    ditto
tag 10   reattachment           value follows a node
tag 13   facet grant            grants depth
```

An effectful frame without an identity and without natural idempotence
is a defect, and this list is the shape of the search rather than its
result.

One thing the party field *does* catch: `claimants` deduplicates parties
per side and reports `Contested` when two claim one side. So a **third
party** replaying a leg is visible. The original party replaying its own
is not.

### 6.5 The search, run once — the chain's acts

§6.4 describes the shape of an audit. This is the first one completed,
and it found a defect.

**The chain's acts are effects.** Every one of them moves the fold, and
a chain is precisely where an effect applied twice moves capacity
twice. They were not in §6.4's candidate list because `IS-6` did not
exist when it was written; the codec had been live since the founding
with its format specified nowhere.

All eight acts, replayed:

| act | replayed | verdict |
|---|---|---|
| `Encumber`, `EncumberBox` | changes nothing | idempotent naturally — an observation recorded twice is one observation |
| `Retire` | changes nothing | idempotent naturally |
| `Anchor` | changes nothing | idempotent naturally — a frontier takes the larger height, so a second report of the same height is absorbed |
| `Issue`, `IssueBox` | second is **refused** | not idempotent, but `well_formed` rejects the chain, so the effect never lands |
| `Cede` | second is **refused** | ditto — the ceder no longer holds the slab |
| `Open` | **doubled the axis count and multiplied the volume, and was accepted** | **the defect** |

`Open` was the only act that was both non-idempotent *and* well-formed.
Replaying it opened a second axis of the same name, so the space grew
for free and nothing said so.

**Fixed by the second of §6.1's two remedies, not by a third one.**
`Act::Open` already carries `axis`, and that name is the identity a
receiver dedups on — the fold now ignores an `Open` whose axis is
already open. No sequence number, no window, no seen-set; §6.1's rule
was already sufficient and had simply never been applied here.

One case the ruling does not reach: the same axis name with a
**different extent**. That is not a replay but two irreconcilable
declarations about one direction, and deduping keeps the first, which
silently discards the second. `IS-6` §8 names it as a flaw rather than
folding it away — see `Flaw::AxisRedeclared`.

Held by `isthmus/tests/deed_suite.rs (mod replay_laws)`, as **one law over every act**
rather than one test per act, with a coverage gate asserting the act
table and the codec's tags are the same set. A ninth act arriving
without a row fails `r2` rather than passing silently.

## 7. RESOLVED — the silent stall, and the bound that separates it

### 7.1 The defect

`whole_records` stops at the first record it cannot take and `feed`
keeps the remainder in `held`. A header declaring more value than can
ever arrive therefore sits at the head forever: every later `feed`
appends, re-parses from the same offset, fails identically, and returns
nothing.

**The session stalls and reports nothing.** `pending()` grows without
bound. Both projects otherwise hold *refuse, never guess*, and stalling
is neither.

### 7.2 Why it could not be fixed by looking harder

**Unsatisfiable and not-yet-arrived are the same observation without a
bound.** A header declaring 4 GiB and one declaring 900 bytes are both
*more than has arrived*, and a reader with no ceiling can only wait for
either.

The bound has to come from somewhere other than the frame, because the
frame is what is under suspicion.

### 7.3 The bound

```
MAX_RECORD = 1 << 20        one mebibyte of value
```

**Measured.** The largest record across 4127 stored netstratum
chronicle records is 585 bytes of value; this clears it by a factor of
roughly 1790. `measure/record-bound.md` carries the count.

Stated with the ratio rather than as a round number: a bound without
headroom refuses real traffic, and a bound without a measurement behind
it is a guess that gets argued about later.

A peer may agree a **larger** bound in the handshake. It may not agree a
smaller one silently — a reader refusing at a ceiling its sender does
not know about is the silent-misread failure one level up. That is §5's
concern and is owed to the handshake.

### 7.4 The rule

Decidable from the header alone, so it costs the same whatever the
buffer holds:

```
len > MAX_RECORD        REFUSE   no arrival can satisfy this
buffer < 5              WAIT     the header is incomplete
buffer < 5 + len        WAIT     the value is incomplete
otherwise               TAKE
```

A stream that hits a bad head returns **the whole prefix that was good**
alongside the refusal. What already parsed is not lost to what follows
it.

### 7.5 The unbounded buffer closes with it

A header declaring more than the bound refuses **at the header, before
any value is held**. So a conforming session's held bytes never exceed
one maximal record.

That was the second half of this defect and it needed no separate rule.

### 7.6 Gated both ways

`datum/tests/conformance.rs::the_session_rule_separates_never_from_not_yet`

| | |
|---|---|
| never | `u32::MAX` declared → refuse |
| not yet | 900 declared, 10 arrived → wait, however long it has waited |
| the bound exactly | permitted → wait |
| one past the bound | refuse. Exact, not approximate |
| good prefix, bad head | the prefix is returned **and** the refusal named |

Conformance cases `09`, `15` and `16`. A single case at the bound would
have proved nothing about the other side of it.

### 7.7 Not landed

This is `datum`'s reference rule in `src/session.rs`. `strand::wire::whole_records`
and `session::Inbox` still stall, and changing them is a proposal rather
than something applied from here.

## 8. What this document does not cover

The wire record and the exact rational rule — `IS-1`. The registry and
its grants — `IS-3`.
