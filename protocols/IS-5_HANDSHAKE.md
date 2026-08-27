# IS-5 — THE HANDSHAKE

**Status:** specified. The frame is implemented in `datum/src/session.rs`
and its vector generated at §7; the session that would send it is not
built.

A substrate other meshes connect to cannot work without a way for two
peers to say what they speak. `IS-2` had none.

## 1. It is a declaration, not a negotiation

**Ruled, and it follows from the lab's `decide/node-identity.md`.** A negotiation
settles on terms both accept, which needs a party to hold the settled
terms and a way to be bound by them. There is no issuer here and no
authority to ask.

So each peer **states what it holds** and neither agrees to anything:

```
peer A  ──  I implement these revisions, hold these ranges,
            and will accept records up to this size   ──▶  peer B
peer B  ──  and I these                               ──▶  peer A
```

There is no round trip, no agreed subset, and no failure to agree. Each
side simply knows what the other can read.

This is the same shape as the position handshake one layer up: a peer
**exhibits** rather than presents a credential, and nothing in the
exchange refers to who it is.

## 2. A peer that speaks less is limited, never refused

the lab's `decide/node-identity.md` and `IS-2` §5 both rule that a linking mesh is
not a distinguished peer. It forwards frames it does not own, by length,
and needs no version for them at all.

So a peer implementing fewer revisions is **limited to what it can
read** and is not turned away. Refusing it would break the property that
makes a mesh-of-meshes possible.

The only thing a declaration can cause is a sender choosing not to send
something the receiver has not claimed to read.

## 3. What it carries

```
hello   = LE32(revisions) ‖ revision…
        ‖ LE32(ranges) ‖ (LE32(low) ‖ LE32(high))…
        ‖ LE32(max_record)
        ‖ [uplink]                              IS-5/2, optional

revision = LE16(len) ‖ utf8
text     = LE16(len) ‖ utf8

uplink   = text                                 the sender's chain
         ‖ LE32(len) ‖ digest
         ‖ LE32(count) ‖ (text ‖ LE64(height))…
```

- **revisions** — which protocol documents the peer implements, as
  identifiers it chose. `IS-1/1` and so on. Opaque to the reader beyond
  equality: a revision it does not recognise is one it cannot rely on,
  which is all it needs to know.
- **ranges** — the registry grants the peer holds, from `IS-3` §5. A
  receiver learns which tags the sender may originate, and therefore
  which of its own it should not expect back.
- **max_record** — the largest record value this peer will accept,
  from `IS-2` §7.3.
- **uplink** — `IS-5/2`. Who the sender is *as a chain*, and what it
  has seen. §3.1.

The frame is **tag 64**, the first value in the range `IS-3` §5 grants
`isthmus`.

## 3.1 The uplink, and why it had to be on the wire

A chain's name is **not in its stored bytes**. It is context the acts
are read in, on the same footing as the layout, so a party reading a
chain off disk learns the history and not who kept it.

That made the substrate downstream-only. A peer could attach, be
deeded, and have its frames recognised — all downward — and no chain
could record having observed it, because the act that records such an
observation names its target and there was no name to write.

- **chain** — what the sender's chain calls itself. This is the string
  another chain's anchor cites.
- **digest** — the sender's own chain, at its own height, digested. The
  height it covers is the sender's own entry in the frontier, so there
  is one place a height is stated and no pair to disagree.
- **frontier** — every chain the sender has observed and how much of
  each, its own included. Heights are **counts**: height 0 and absent
  are the same fact, so a frontier never carries a zero.

**Which function produced the digest is not specified here, and that is
deliberate.** Picking one would be this document choosing a security
property for every integrator. The verifier takes the function in.

A receiver may mint **one** observation from a declaration: over the
sender's own chain, at the declared height, with the declared digest.
It may **not** mint observations over the other chains the frontier
names. Those are the sender's observations, and recording them would
mean asserting *"I observed that chain"* on the strength of somebody
saying they did. The frontier is still worth carrying: ordering the two
peers against each other is a different power from observing a third.

**Two peers are ordered by comparing frontiers, and the comparison is
partial.** Neither ahead nor behind is *concurrent* — both acted on
what they knew and neither is wrong. That verdict must be kept distinct
from *one of you declared no uplink*, which is not concurrency but
unaddressability. Collapsing them files an unreachable peer as a rival.

### 3.1.1 Sending it is opt-in, and the reason

An `IS-5/1` reader refuses trailing bytes — correctly, since a
declaration with bytes left over is not that declaration — so an
`IS-5/2` declaration **is refused by an `IS-5/1` peer**.

This is survivable only because the block is never sent by default. A
peer that does not opt in emits the `IS-5/1` bytes exactly, so the
incompatibility is chosen by the side that wants upstream, and never
acquired as a side effect of a chain having a name.

### 3.1.2 The one downgrade

The block is optional **and last**, so it can be cut off: a declaration
truncated at the byte where the uplink begins decodes as a complete,
valid declaration by an anonymous peer. No arrangement of an optional
trailing field avoids this.

What is bounded is the cost. That is the **only** cut that survives,
and what survives is strictly less — never a *different* uplink, only
an absent one. It is not a framing hazard, because the value is
length-framed by the record around it and a party that can shorten the
value can rewrite it entirely. It **is** a downgrade an active party
can perform, and its observable consequence is that the sender reads as
unaddressable, which is why that verdict is kept distinct from
concurrent above.

## 4. The bound, and why smaller is the dangerous direction

`IS-2` §7.3 sets `MAX_RECORD = 1 << 20` as the default.

- A peer may declare a **larger** `max_record`. A sender may then use
  it, because the receiver said it would accept it.
- A peer may declare a **smaller** one, and a sender that has heard the
  declaration must respect it.
- A peer that has heard **no** declaration uses the default.

The failure this prevents: a reader refusing at a ceiling its sender
does not know about. That is the silent-misread failure the wire's own
header warns of, one level up — the sender is not wrong, the reader is
not wrong, and the record vanishes anyway.

A declaration is therefore **required before a peer may enforce a
smaller bound than the default.** Enforcing one silently is the defect.

## 5. Revisions identify, they do not order

A revision is compared for equality and nothing else. There is no
newer-than.

`NS-3` §2.4 already rules the same for a corpus — *"a corpus without one
names a moving target"* — and `IS-4` §3 carries it for the observer. A
protocol is a corpus in this respect: two peers on different revisions
disagree about what a frame means, and **neither is wrong.**

Ordering revisions would let a peer decide it is ahead and act on the
difference, which is the authority this protocol does not have.

## 6. What a hello may not do

1. **May not be required.** A session with no hello runs on the
   defaults. A substrate that refuses the silent is not a substrate.
2. **May not be believed.** A peer declaring a range it does not hold
   has lied about nothing important: the receiver still re-derives every
   claim, and a grant is a claim on numbers rather than on meaning.
3. **May not be a round trip.** Each side sends; neither waits. A
   handshake that blocks is a coordinator with extra steps.
4. **May not change the framing.** It is an ordinary record under
   `IS-1` §1, so socket bytes still do not differ from file bytes —
   refusal 1 holds.

## 7. Test vectors

```
V13  hello: one revision "IS-1/1", one range 64–79, max_record 1<<20

value   01000000 0600 49532d312f31 01000000 40000000 4f000000 00001000
        └──┬───┘ └─┬┘ └─────┬────┘ └──┬───┘ └───┬──┘ └───┬──┘ └───┬──┘
         1 rev   len 6   "IS-1/1"   1 range   low 64  high 79  1 MiB

framed  401c00000001000000060049532d312f3101000000400000004f00000000001000
```

```
V14  hello IS-5/2: one revision "IS-5/2", one range 64–79,
     max_record 1<<20, uplink for chain "datum" at height 14
     with one anchor to "strand" at 6

value   01000000 0600 49532d352f32
        01000000 4000000000000000 4f00000000000000
        00001000
        0500 646174756d                            chain "datum"
        08000000 0102030405060708                  digest, 8 bytes
        02000000                                   2 frontier entries
        0500 646174756d 0e00000000000000           datum   at 14
        0600 737472616e64 0600000000000000         strand  at 6

framed  405a0000000100000006004953 2d352f3201000000400000000000
        00004f0000000000000000001000 0500646174756d08000000010203
        0405060708020000000500646174756d0e00000000000000
        0600737472616e640600000000000000
```

The digest is a **literal**, eight bytes counting up. This document
specifies the framing of a digest and not which function produced it
(§3.1), so a vector that computed one would be the document quietly
picking a hash for every integrator.

The frontier carries `datum` at 14 and `strand` at 6 — the sender's own
height and one anchor to another chain. Entries are in name order, and
a height of zero is never present, because absent and zero are the same
fact.

Generated by `datum/tests/vectors.rs::the_hello_vector` and
`::the_uplink_vector`, asserted against this hex, exactly as `IS-1`
§9's twelve are.

An earlier draft of this section carried a **hand-written** hex and
flagged the real one as owed in the same breath. Hand-typing the bytes
was wrong three times across this work — twice in `IS-1` §9 and once
here — which is the whole argument for generating them.

## 8. Round-trip and refusal

`a_hello_round_trips_and_refuses_a_partial_one` holds four properties:

- a hello round-trips
- an `IS-5/1` declaration truncated at **any** offset decodes to
  nothing. A truncated declaration is not a partial one, because a peer
  acting on half of it would act on terms the sender never stated
- trailing bytes are not this declaration
- no declaration heard means the **default**, not zero

**This section said "any offset" without qualification and `IS-5/2`
made that false.** An `IS-5/2` declaration has exactly one surviving
cut — §3.1.2 — and the correction is recorded here rather than made
quietly, because a document that keeps a claim wider than the code is
the failure this whole series of vectors exists to prevent. The bounded
form is held by `isthmus/tests/sphere_laws.rs::s7_the_declaration_
round_trips_and_absent_is_not_empty`, which asserts that the surviving
cut is that one and that what survives is strictly less.

## 9. What this does not settle

- **Whether a peer should re-declare.** A session that runs long enough
  for a peer's grants to change has no way to say so, and adding one
  risks a peer changing terms mid-session.
- **The verdict frame.** A receiver that cannot read a sender's records
  says nothing about it today. Whether it should is `IS-4` §8's open
  question about verdicts.
- **Which digest function.** §3.1 frames a digest and names no function
  on purpose. Two peers that digest differently will disagree about a
  history they agree on, and nothing here detects that — it reads as a
  false anchor.
- **How a receiver learns a chain name it was not told.** The uplink
  carries the sender's own name, so a chain named only inside somebody
  else's frontier is a string with nothing behind it.
- **Whether `datum::session::Hello` should follow.** `datum` carries a
  second, independent implementation of this document, and it
  implements `IS-5/1` — it has no uplink block. Measured, not assumed:
  `datum/tests/vectors.rs` generates V13 from that implementation and
  V14 from `isthmus`'s. Two implementations at different revisions is
  the disagreement revision strings exist to make visible, and it is
  named here rather than closed by making one call the other.

## 10. Revisions

| revision | change |
|---|---|
| `IS-5/1` | the declaration: revisions, ranges, `max_record` |
| `IS-5/2` | §3.1 the uplink block — a peer says which chain it is and what it has seen. Optional, opt-in, and refused by an `IS-5/1` reader when sent; §3.1.2 states its one downgrade |
