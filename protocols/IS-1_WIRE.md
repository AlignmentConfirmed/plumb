# IS-1 — WIRE

**Revision: `IS-1/2`.** Compared for equality, never ordered — see
`decide/publishing.md`. §12 records what moved.

**Status: BUILT.** `crates/isthmus` implements §1 through §4, and §9's
frame and rational vectors, as a crate with **no path dependencies**. An
outside title adds one line to its manifest and needs neither kernel
tree on disk.

§7's two frames — the relation and the manifold — are **not** in that
crate and will not be. They name kernel types, so implementing them
there would put a kernel dependency inside the crate whose whole claim
is that it has none. They stay in `datum`, which is allowed to know
about kernels.

§9 still publishes byte-exact vectors, so an implementation can also be
written against this document alone — the standard `NS-1` §3 sets, and
the standard that found three gaps in this document and then a fourth.

## 1. The record

```
tag u8 ‖ LE32(length) ‖ value
```

Measured identical in both projects:

- netstratum, `NS-1_DOP.md:47` — `tag u8 ‖ LE32(length) ‖ value`
- xylarium, `strand/src/wire.rs:490` — `put_frame(tag, value)` pushes
  the tag byte, the little-endian 32-bit length, then the value.

The length prefix is load-bearing and is the whole reason the mesh can
be opaque. `strand/src/wire.rs:550`:

> *"The length prefix is what makes this possible without understanding
> the value."*

## 2. Unknown tags

A reader that does not know a tag skips the record and does not fail.

- netstratum states it as conformance rule (c), `NS-1_DOP.md:166`.
- xylarium implements it as `skip_unknown`, `strand/src/wire.rs:552`.

This is what lets a mesh forward a frame it does not own, and what lets
a mesh linking into this one forward a frame neither of them owns.

## 3. Exact rational encoding

```
sign u8 ‖ LE32(len) ‖ numerator magnitude BE ‖ LE32(len) ‖ denominator magnitude BE
```

Sign byte is 0 for non-negative and 1 for negative. Magnitudes carry no
leading zeros. Zero's magnitude is the empty string.

Measured identical in shape on both sides. See
`measure/wire-shape-shared.md`.

**`isthmus` does not implement this.** A ratio inside a frame value is
written and read by whoever owns the tag. The mesh reads tag and length
only. See `decide/siblings.md` for why this is forced rather than
chosen.

It is recorded here because it is the shared rule both grantees must
follow, not because the mesh performs it.

## 4. Strictness

The shared rule is the stricter of the two decoders. **Every refusal a
conforming reader may produce is here** — an implementer who handles
only some of these has a reader that accepts what this one refuses.

| input | rule |
|---|---|
| zero denominator | refuse |
| **magnitude with a leading zero byte** | refuse |
| non-reduced | refuse — do not silently reduce |
| **`0/n` for any `n` other than 1** | refuse. Zero's canonical denominator is 1 |
| sign byte not 0 or 1 | refuse |
| declared length exceeds what the record holds | refuse — `IS-2` §7.4 |
| bytes left over after a value is read | refuse |
| a nested record carrying an unexpected tag | refuse |
| **zero carrying a negative sign** | refuse — **new in `IS-1/2`**, see below |

§3 says magnitudes *carry* no leading zeros, which states what an
encoder does. **It does not follow that a decoder refuses them**, and
the two rows in bold were both found by a second implementation reading
this document and diverging from the reference:

```
0/5              document: accept        reference: refuse NotReduced
leading zero 01/2  document: accept      reference: refuse LeadingZero
```

`measure/second-implementation.md` carries the run. A rule stated about
the writer is not a rule about the reader, and this table now states
both.

### 4.1 Negative zero — the ninth row, and the only one both ancestors miss

`01 ‖ LE32(0) ‖ ‖ LE32(1) ‖ 01`. Sign negative, numerator empty,
denominator one.

**Both ancestors accept it**, which is what separates this row from the
eight above: those are cases where the ancestors differ from each other
and the stricter one was adopted. Here neither refuses, so no amount of
reading the two implementations produces the rule.

It decodes to zero and nothing downstream misbehaves. That is precisely
the defect:

> two byte strings for one value, where every other such pair in this
> table refuses.

`0/5` refuses because zero's canonical denominator is 1. `2/4` refuses
because a value has one spelling. `-0/1` is a second spelling of `0/1`
by the same argument. Everything above this layer takes an address over
bytes, so a second spelling is a second address.

Found by writing `isthmus` — a third implementation, in a typed
language, which cannot decline to say what `sign = 1` composed with
`numerator = 0` means. The Python reader, following this document's
prose sequentially, reads the sign, reads the magnitudes, and never has
cause to consider the pair. `measure/third-implementation.md` carries
the run.

`isthmus` refuses it and declares `IS-1/2`. `strand` accepts it and
implements `IS-1/1`. They disagree about one byte string, and that
disagreement is visible in the handshake rather than hidden — which is
the entire reason `IS-5` carries revision strings.
`propose/negative-zero.md` is the patch that closes it.

netstratum refuses the zero denominator, silently reduces rather than
refusing, and accepts any sign byte that is not 1. Adopting this rule
tightens netstratum's acceptance and changes nothing either project
emits. See `measure/ratio-strictness.md` for the measurement and the
both-ways test owed before it lands.

## 5. The carrier

A carrier moves opaque bytes and decides nothing about them. It has no
method that can look inside a frame.

Reached independently by both projects — netstratum at
`NS-5_MESH.md:253` (*"every adapter is logic-free"*), xylarium at
`strand/src/carrier.rs`. Refusals 4 and 19.

A stream has no message boundaries and a file does, so a socket
delivery can stop mid-record. The partial tail is held by the session,
and where a record ends is answered by the frame format. The adapter
still parses nothing. An envelope was refused for this — refusal 1.

## 6. Settled since drafting

- **Tag width.** 56 claimed, 200 free. A constraint to plan against, not
  a blocker. `IS-3` §5 carries the grant table.
- **Transport security.** The protocol carries none; the adapter may. A
  `Carrier` moving TLS bytes is still a `Carrier`, and encryption below
  the frame keeps socket bytes identical to file bytes.
  `decide/transport-security.md`.
- **Node identity.** None. A peer is what it can demonstrate.
  `decide/node-identity.md`.

Neither of the last two answers **freshness**: a replayed frame is
valid and re-derives correctly. That is `IS-2` §6.

## 7. Frames that cite kernel types

Two frames name a kernel type and therefore carry a dependency on it.
Both were understated when first drafted, because `lith` gained
relations of arbitrary arity and then wired them into `Manifold`. See
`measure/arity-and-the-wire.md`.

### 7.1 The relation frame

`Support` is **the 1-skeleton**, not the relation. A manifold may carry
cells above its edges, and a cell relates three or more orbs at once.

A frame carrying only the 1-skeleton is not wrong, it is partial — and
partial in the direction that reads as complete. A peer decoding it
alone sees a ring where the sender's own Betti vector says the face is
filled. That is the disagreement `lith` removed inside the kernel at
`dc66dc6`, reproduced across the wire.

**LANDED**, `strand/src/wire.rs` tag 51, as *closures — the grains above
a manifold's edges*. A `Grain` is a sorted deduplicated orb list:

```
grain     = LE32(arity) ‖ LE32(orb)…
closures  = LE32(count) ‖ grain…
```

It **follows** the manifold rather than sitting inside it, and that is
forced rather than chosen: `read_nested` calls `finish()`, so a trailing
section inside tag 5's value is `Malformed::TrailingBytes` on every
existing reader. As its own record it skips whole under §2, which is
what makes carrying a closure additive instead of a flag day.

Vectors at §9.4.

### 7.2 The manifold frame

```
manifold = LE64(tick) ‖ LE32(orbs) ‖ orb… ‖ frame(1)
orb      = extent(room) ‖ extent(capacity) ‖ extent(energy)
extent   = LE32(components) ‖ ratio…
```

**`IS-1/4`: capacity and energy are extents, not scalars.** They were
single ratios; `lith` `e105b6f` made them multi-component, because a
scalar capacity is one number met or refused and poles do not exchange
— a holder short on one pole and long on another is not insolvent, it
is a party with something to trade. A frame carrying the scalar could
not express that, so it carried a reading that had already stopped
being true.

Nothing sums across components. Each is read against its own pole; the
old scalar reading is component zero, and every 1-D value crosses
byte-identically to how it crossed before.

`D` is **not** on this wire. It is `reduce(support).depth()` and the
support is carried, so sending it would be sending a quantity the
receiver re-derives — a declared operand where a measured one exists.

Vector at §9.5.

**This layout was missing until a second implementation went looking for
it.** §9.5 published the bytes and no section said how to produce them,
so a reader could check a frame it could not write. See
`measure/second-implementation.md`.

A manifold frame that omits `cells` decodes to a manifold with none —
**a valid state and the wrong one.** It stands, and its depth reads off
the 1-skeleton.

This is worse than a decode failure. Refusing is recoverable; silently
producing a state that passes is not.

The repair is additive and matches the kernel's own door:
`Manifold::new` leaves cells empty, so a frame without a trailing cells
section decodes to a cell-free manifold, which is truthful for a sender
that has none.

### 7.3 The rule this produces

**A frame that names a kernel type carries a dependency on that type.**
Pinning the kernel's commit does not catch it — `lith`'s head moved
three times during the session that drafted this document, and only one
of those moves touched a cited type.

`cargo test` pins structure per named type for exactly this reason.

## 8. What the mesh gains from the arity model

`Complex::depths()` returns the Betti vector; `Complex::torsion(k)`
returns torsion coefficients. Both are topological invariants: they do
not move with a frame, a basis, or a charge.

That makes them the strongest FAR candidates measured so far — stronger
than anything in `measure/tuniversal.md`, because they are invariant by
construction rather than by argument. Whether netstratum can produce a
Betti vector at all is unmeasured and decides whether this is a shared
invariant or one kernel's.

## 9. Test vectors

**The exact bytes.** Any independent implementation of §1 and §3 can
check itself against these without reading a line of the source, which
is the standard `NS-1` §3 sets and the reason this section exists.

Every vector is produced by calling the codec in
`datum/tests/vectors.rs` and asserted against the hex below. If the codec changes, that test
fails and this section is known-stale rather than quietly wrong.

Hex, lowercase, no separators. Read `LE32` as four bytes little-endian.

### 9.1 The frame

```
V1   tag 1, empty value
     0100000000
     └┬┘ └───┬──┘
      1   len 0

V2   tag 51, value AA BB
     3302000000aabb
```

### 9.2 The exact rational

`sign u8 ‖ LE32(len) ‖ numerator BE ‖ LE32(len) ‖ denominator BE`

```
V3   0            00000000000100000001
                  ││ └──┬──┘ └──┬──┘ └┘
                  │  len 0    len 1   1
                  sign 0, and zero's magnitude is EMPTY

V4   1            0001000000010100000001
V5   −3           0101000000030100000001
V6   7/5          0001000000070100000005
V7   256/255      0002000000010001000000ff
                    └──┬──┘ └┬┘        └┘
                    len 2  0x0100     0xff
```

V3 is the one to implement against first: zero's magnitude is the empty
string, not a zero byte. V7 is the one that catches a fixed-width
assumption.

### 9.3 The relation — tag 1

`LE32(orbs) ‖ LE32(count) ‖ (LE32(i) ‖ LE32(j) ‖ ratio)…`

```
V8   3 orbs, triangle (0,1)=1 (1,2)=1 (0,2)=2
     01410000000300000003000000000000000100000000010000000101000000010100000002000000000100000001010000000100000000020000000001000000020100000001
```

### 9.4 The closures — tag 51

`LE32(count) ‖ (LE32(arity) ‖ LE32(orb)…)…`

```
V9   one grain over orbs 0 1 2
     33140000000100000003000000000000000100000002000000

V10  no grains
     330400000000000000
```

V10 matters: a manifold with no closures still sends the frame, and it
is four bytes of value. A reader that treats an absent frame and an
empty one as the same thing is right today and should not rely on it.

### 9.5 The manifold — tag 5

```
V11  tick 0, one orb (extent [1], capacity [1], energy [0]), empty relation
     055b00000000000000000000000100000001000000000100000001010000000102000000000100000001010000000100010000000101000000010200000000010000000101000000010000000000010000000101080000000100000000000000
```

**Regenerated at `IS-1/4`** — never hand-typed; the codec produces
these bytes and `tests/vectors.rs` asserts them, so the two cannot
drift. Hand-typing this section has been wrong three times.

The trailing `08` frame is the nested tag-1 relation. `D` is **not** on
this wire — it is `reduce(support).depth()` and the support is carried,
so sending it would be sending a quantity the receiver re-derives.

### 9.6 An unknown tag

```
V12  tag 200, four arbitrary bytes
     c804000000deadbeef
```

A conforming reader **skips this whole record** and continues. It is
`1 + 4 + 4 = 9` bytes. This is the vector that proves §2, and it is the
one a linking mesh depends on.

## 10. Conformance

An implementation conforms when it produces the stated verdict for every
case in `conformance/MANIFEST.md`. Fourteen cases as `.bin` files, with
four verdicts:

| verdict | meaning |
|---|---|
| `accept` | decodes to a whole record |
| `refuse` | malformed; a reader that accepts has a defect |
| `recognised` | **new in `IS-1/3`** — the shape is confirmed and the payload is not read. See below |
| `skip` | not a record this reader owns — step over it whole and continue |
| `wait` | a record has begun and not finished; hold, and neither accept nor refuse |

**The difference between `skip` and `wait` is where most readers go
wrong**, and it is the difference between a record you will never own
and one that has not finished arriving. Conflating them either drops
data or stalls.

### 10.1 Recognised — between skip and accept

A record whose tag is **deeded on the court** but not owned by this
reader is `recognised`: the carrier confirms the spatial boundaries of
the bytes — length-framed whole, tag held by a named holder — and
treats the payload as entirely opaque. The whole record is delivered
unfractured to the kernel behind the seam, which applies the deed's
law on receipt or discards the unreadable geometry. No payload byte is
evaluated at the carrier; the only consultation beyond the header is
the court's fold.

This is the mechanism by which two differing kernels share a mesh
without forcing mutual comprehension, and it is grammar's own ceiling
speaking on the wire: *the shape is known and the meaning is not.*

**Requires a court.** A reader without one lawfully degrades
`recognised` to `skip` — forwarding instead of delivering, which loses
economy and never correctness. The conformance corpus therefore stays
four-verdict for courtless readers; a fifth-verdict reader differs
from a fourth only in where a recognised record is handed, never in
what is accepted or refused.

The corpus is generated and checked against the live codec by
`datum/tests/conformance.rs`, so the bytes and the verdicts cannot drift
from what the reference does.

## 11. What this document does not cover

The session and its sequencing — `IS-2`. The registry and its grants —
`IS-3`. The witness category — `IS-4`. The declaration — `IS-5`.

## 12. Revisions

Compared for **equality and never ordered**. There is no newer-than:
ordering would let a peer decide it is ahead and act on the difference,
which is authority this substrate does not have. Two peers on different
revisions disagree about what a frame means and neither is wrong.

| revision | what moved |
|---|---|
| `IS-1/1` | first statement. §4 carried eight refusals |
| `IS-1/2` | §4 gains **negative zero**, a ninth refusal neither ancestor produces. §4.1 |
| `IS-1/3` | §10 gains **recognised**, the fifth verdict — court-dependent, degrading to `skip` without one. §10.1 |
| `IS-1/4` | §7.2 orbs carry **extents, not scalars**: capacity and energy are multi-component. V11 regenerated |

A revision is bumped when the bytes a conforming reader must accept or
refuse change, and **not** for prose, a new measurement, or a correction
that does not move the wire. Adding a refusal moves it: a reader on
`IS-1/1` accepts a byte string a reader on `IS-1/2` rejects.

Every other change in `IS-1/2` — the status header, §12 itself — is
prose and would not have bumped it alone.

