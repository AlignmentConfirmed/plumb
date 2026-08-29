# IS-1 — WIRE

**Revision: `IS-1/2`.** Compared for equality, never ordered. §12
records what moved.

**Status: implemented.** `crates/isthmus` implements §1 through §4 and
§9's frame and rational vectors, with no path dependencies.

§7's two frames — the relation and the manifold — are not in `isthmus`.
They name kernel types, and implementing them there would place a kernel
dependency inside a crate that has none; they are implemented in
`datum`.

§9 publishes byte-exact vectors, so an implementation can be written
against this document alone.

## 1. The record

```
tag u8 ‖ LE32(length) ‖ value
```

The length prefix lets a reader forward or skip a record without
interpreting its value.

## 2. Unknown tags

A reader that does not know a tag skips the record and does not fail.
This lets a mesh forward a frame it does not own, including a frame
owned by neither of two linked meshes.

## 3. Exact rational encoding

```
sign u8 ‖ LE32(len) ‖ numerator magnitude BE ‖ LE32(len) ‖ denominator magnitude BE
```

Sign byte is 0 for non-negative and 1 for negative. Magnitudes carry no
leading zeros. Zero's magnitude is the empty string.

`isthmus` does not implement this: a ratio inside a frame value is
written and read by whoever owns the tag, and the mesh reads tag and
length only. The encoding is specified here because it is the shared
rule every grantee follows.

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

§3 states what an encoder does: magnitudes carry no leading zeros. It
does not follow automatically that a decoder refuses them, so the two
rows in bold state the decoder rule explicitly. A reading of §3 alone
accepts byte strings the reference refuses:

```
0/5                accept under §3 alone    refuse: NotReduced
leading zero 01/2  accept under §3 alone    refuse: LeadingZero
```

A rule stated about the writer is not automatically a rule about the
reader; this table states both.

### 4.1 Negative zero

`01 ‖ LE32(0) ‖ ‖ LE32(1) ‖ 01`. Sign negative, numerator empty,
denominator one. It decodes to zero.

A value has one spelling. `0/5` refuses because zero's canonical
denominator is 1; `2/4` refuses because a value has one spelling; `-0/1`
is a second spelling of `0/1` by the same argument. Everything above
this layer addresses values by their bytes, so a second spelling is a
second address for one value.

A reader on `IS-1/1` accepts this byte string; a reader on `IS-1/2`
refuses it. The two revisions disagree about one byte string, and the
disagreement is visible in the handshake, which is why `IS-5` carries
revision strings.

## 5. The carrier

A carrier moves opaque bytes and decides nothing about them. It has no
method that can look inside a frame.

A stream has no message boundaries and a file does, so a socket
delivery can stop mid-record. The partial tail is held by the session,
and where a record ends is answered by the frame format; the adapter
parses nothing.

## 6. Related concerns

- **Tag width.** 56 tags claimed, 200 free. `IS-3` §5 carries the grant
  table.
- **Transport security.** The wire carries none; the adapter may. A
  `Carrier` moving TLS bytes is still a `Carrier`, and encryption below
  the frame keeps socket bytes identical to file bytes.
- **Node identity.** The wire carries none; identity is bound at the
  court (`Act::Bind`, `IS-6` §4).
- **Freshness.** The wire does not deduplicate a replayed frame; the
  `IS-2` §6 session challenge provides freshness.

## 7. Frames that cite kernel types

Two frames name a kernel type and therefore carry a dependency on it.

### 7.1 The relation frame

`Support` is **the 1-skeleton**, not the relation. A manifold may carry
cells above its edges, and a cell relates three or more orbs at once.

A frame carrying only the 1-skeleton is not wrong, it is partial: a peer
decoding it alone sees a ring where the sender's own Betti vector reports
a filled face.

Tag 51 carries closures — the grains above a manifold's edges. A `Grain`
is a sorted deduplicated orb list:

```
grain     = LE32(arity) ‖ LE32(orb)…
closures  = LE32(count) ‖ grain…
```

It follows the manifold rather than sitting inside it: `read_nested`
calls `finish()`, so a trailing section inside tag 5's value is
`Malformed::TrailingBytes` on every existing reader. As its own record
it skips whole under §2, which makes carrying a closure additive.

Vectors at §9.4.

### 7.1.1 IS-1/5 — the closure carries its shape

The grain above names which orbs a cell spans, not how they connect.
Six orbs in a ring and six orbs fully joined are the same grain — a
hexagon indistinguishable from a five-simplex
(`defect_tag51_carries_no_shape`). `IS-1/5` closes the class:

```
closures = LE32(count) ‖ shaped…
shaped   = LE32(arity) ‖ LE32(orb)… ‖ LE32(def_len) ‖ definition
```

`definition` is a **declared-complex definition** — the same canonical
codec the universal checker evaluates (`plumb-assay::complex`,
`Act::Declare`): cell counts per dimension, then sparse boundary
operators with exact-rational coefficients. One shape encoding in the
whole system, on purpose. Two laws:

1. **The vertex map**: the definition's dimension-0 cell count MUST
   equal the arity; local 0-cell $k$ names the $k$-th orb in the list.
2. **Legacy is explicit, never inferred**: `def_len = 0` is the old
   information content — *shape unknown*. A reader must not infer a
   simplex, a ring, or anything else from an empty definition; it is
   the frame saying "I did not say."

An `IS-1/4` closure body is not an `IS-1/5` body (the revision moves
because the bytes move; revisions compare for equality, never order).
Vectors at §9.4.1.

### 7.2 The manifold frame

```
manifold = LE64(tick) ‖ LE32(orbs) ‖ orb… ‖ frame(1)
orb      = extent(room) ‖ extent(capacity) ‖ extent(energy)
extent   = LE32(components) ‖ ratio…
```

**`IS-1/4`: capacity and energy are extents, not scalars.** They were
single ratios and are now multi-component: a scalar capacity is one
number met or refused, but poles do not exchange — a holder short on one
pole and long on another is solvent, with something to trade. A scalar
frame cannot express that.

Nothing sums across components. Each is read against its own pole; the
old scalar reading is component zero, and every 1-D value crosses
byte-identically to how it crossed before.

`D` is **not** on this wire. It is `reduce(support).depth()` and the
support is carried, so sending it would be sending a quantity the
receiver re-derives — a declared operand where a measured one exists.

Vector at §9.5.

A manifold frame that omits `cells` decodes to a manifold with none: a
valid state. `Manifold::new` leaves cells empty, so a frame without a
trailing cells section decodes to a cell-free manifold, which is correct
for a sender that has none. Its depth reads off the 1-skeleton.

### 7.3 The rule

A frame that names a kernel type carries a dependency on that type.
`cargo test` pins structure per named type, so a change to a cited type
is caught.

## 8. Topological invariants from the arity model

`Complex::depths()` returns the Betti vector; `Complex::torsion(k)`
returns torsion coefficients. Both are topological invariants: they do
not move with a frame, a basis, or a charge.

## 9. Test vectors

**The exact bytes.** Any independent implementation of §1 and §3 can
check itself against these without reading the source.

Every vector is produced by the codec in `datum/tests/wire_suite.rs
(mod vectors)` and asserted against the hex below; if the codec changes,
that test fails and this section cannot silently drift.

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
                  sign 0, and zero's magnitude is empty

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

V10: a manifold with no closures still sends the frame — four bytes of
value. A reader must not treat an absent frame and an empty one as the
same.

#### 9.4.1 The shaped closures — `IS-1/5`

Whole frames, in `conformance/`. **Generated by an independent
implementation** (python, from this document) and pinned against the
engine's own codec in `tests/market.rs (mod shaped_closure)` — the two cannot
drift without one of them refusing.

```
V17  one shaped grain, orbs 0–5, definition = the 6-cycle (hexagon)
     conformance/17-shaped-closure-hexagon.bin   (285 bytes)
V18  one shaped grain, SAME orbs 0–5, definition = K6 (five-simplex
     1-skeleton)
     conformance/18-shaped-closure-simplex.bin   (627 bytes)
V19  one legacy grain, orbs 0–5, def_len = 0 — shape unknown
     conformance/19-shaped-closure-legacy.bin    (41 bytes)
```

V17 ≠ V18 with identical orb lists is the defect class closed at the
byte level: the incidence is on the wire, not assumed by the reader.

### 9.5 The manifold — tag 5

```
V11  tick 0, one orb (extent [1], capacity [1], energy [0]), empty relation
     055b00000000000000000000000100000001000000000100000001010000000102000000000100000001010000000100010000000101000000010200000000010000000101000000010000000000010000000101080000000100000000000000
```

**Regenerated at `IS-1/4`.** The codec produces these bytes and
`tests/wire_suite.rs (mod vectors)` asserts them, so they cannot drift.

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

`skip` and `wait` differ: `skip` is a record this reader will never own;
`wait` is one that has not finished arriving. Conflating them drops data
or stalls.

### 10.1 Recognised — between skip and accept

A record whose tag is **deeded on the court** but not owned by this
reader is `recognised`: the carrier confirms the spatial boundaries of
the bytes — length-framed whole, tag held by a named holder — and
treats the payload as entirely opaque. The whole record is delivered
unfractured to the kernel behind the seam, which applies the deed's
law on receipt or discards the unreadable geometry. No payload byte is
evaluated at the carrier; the only consultation beyond the header is
the court's fold.

This lets two differing kernels share a mesh without mutual
comprehension: the shape is known and the meaning is not.

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
revisions disagree about what a frame means, and neither is authoritative.

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

