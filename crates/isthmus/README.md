# isthmus

**Superhighway substrate** — the up-level wire every mesh and kernel can
link into. Domain meshes (`strand`, netstratum mesh, …) are tollways:
kernel-specific on-ramps. This crate is general: all content, all
kernels, connectivity without payload inspection.

**Dimensionality:** deed space is **n-axial polytopal** (`Act::Open`),
not 2-D. Multi-chain structure is a **sphere of frontiers**
(`sphere` module) — hypersphere envelope of observations. Master
equation: datum the lab's `decide/linkage-estates.md`.

Independent nodes import this crate as **producer**, **verifier**, or
**carrier** (`node`); POW++ claim envelopes are opaque frames (`work`).
**datum** is the authority (deeds, rewards); this crate is the issuer.

```
                 your title ──┐
netstratum mesh ──────────────┼── isthmus ── exits: kernels
xylarium strand ──────────────┘
```

```toml
[dependencies]
isthmus = "0.1"
```

**That is the whole integration cost.** No kernel tree on disk, no
workspace to join, no build script. `tests/no_path_dependencies.rs`
reads this crate's own manifest and fails if that ever stops being true.

## What it is

`IS-1` (the record, the exact rational, the refusals), `IS-2` §7 (the
session rule), `IS-3` (the registry) and `IS-5` (the declaration).

Revisions implemented: `IS-1/2`, `IS-2/1`, `IS-3/1`, `IS-5/1`.

## What it is not

It is not a kernel and it names no kernel type.

`IS-1` §7 specifies two frames — the relation and the manifold — that
cite kernel types. They are **absent here on purpose**: implementing
them would put a kernel dependency inside the crate whose entire claim
is that it has none. This crate carries the framing they travel in and
hands you their bytes without reading them.

## The minimum

Four things, in this order.

```rust
use isthmus::{read, Verdict};
use isthmus::registry::isthmus_owns;
use isthmus::session::MAX_RECORD;

match read(&buffer, MAX_RECORD, isthmus_owns) {
    Verdict::Accept              => { /* decode it */ }
    Verdict::Skip { tag, whole } => { /* forward those bytes, keep going */ }
    Verdict::Wait                => { /* hold; more is coming */ }
    Verdict::Refuse(why)         => { /* say why, and drop the peer */ }
}
```

**1. The record.** `tag u8 ‖ LE32(length) ‖ value`. The length prefix is
load-bearing — it is what lets a carrier move a frame it cannot read.

**2. Skip what you do not own.** Not an optimisation and not optional.
It is the property that lets a mesh link to a mesh: you *will* receive
frames belonging to kernels you have never heard of, and dropping the
connection because of one is the failure the whole design avoids.

**3. The four verdicts.** `accept`, `refuse`, `skip`, `wait`.

**`skip` and `wait` are where implementations go wrong.** One is a
record you will never own; the other has not finished arriving.
Conflating them either drops data or stalls forever.

**4. Tell *never* from *not yet*.**

```text
len > bound        REFUSE   no arrival can satisfy this
buffer < 5         WAIT     the header is incomplete
buffer < 5 + len   WAIT     the value is incomplete
otherwise          TAKE
```

The bound is what makes the first line decidable. Without one those two
cases are the same observation and a reader can only wait — which is how
a session stalls while reporting nothing.

`MAX_RECORD` is `1 << 20`, measured rather than picked: the largest
record across 4127 stored records is 585 bytes of value.

## The exact rational

```text
sign u8 ‖ LE32(len) ‖ numerator BE ‖ LE32(len) ‖ denominator BE
```

Sign is 0 or 1. Magnitudes are big-endian with no leading zeros.
**Zero's magnitude is the empty string**, not a zero byte — that is the
rule most first implementations get wrong.

There is no floating point on this wire and no rounding mode to agree
on. Two peers that disagree about a value disagree about bytes, which is
checkable; two peers that agree to within an epsilon have agreed to
nothing a third can verify.

### Refuse, never repair

`Malformed` carries **every** refusal `IS-1` §4 permits and no others. A
reader that handles only some of them accepts what this one refuses.

```
zero denominator          leading zero byte in a magnitude
non-reduced               0/n for any n other than 1
sign byte not 0 or 1      length exceeding the record
bytes after a value       a nested record with the wrong tag
negative zero
```

Two of those were found by a second implementation that read §3's
*"magnitudes carry no leading zeros"* as a statement about encoders —
which it is — and inferred nothing about decoders. **A rule stated about
the writer is not a rule about the reader.**

The last was found by writing *this* crate. Both ancestors accept
negative zero; it is a second byte string for a value that already has
one, and every other such pair refuses. That is why this crate declares
`IS-1/2` while `strand` declares `IS-1/1`.

## Asking for tag numbers

Tags are one byte, so the space is 256 values and it does not grow.

```
0            never issued
1–31         frozen — claimed by both ancestors with different meanings
32–63        netstratum's registry, with small free runs
64–79        isthmus, transport
80–127       assay, proofs
128–159      lith
160–191      chitin
192–239      linking meshes — six ranges of eight, issued on request
240–255      held back, so exhausting the space refuses rather than wraps
```

**Ask for a range from 192–239.** What a value means inside your range is
yours. One tag, one meaning, forever; a retired tag is never reissued.

Two rules learned the expensive way:

- **A claim not on `master` is not a claim.** A range was claimed on a
  branch, `master` read *unclaimed*, and a lane wrote a 142-line frame
  against a tag already taken. The registry did not collide with them —
  *it lied to them*.
- **A table is only authoritative over what it enumerates.** A grant
  table written against one registry was a claim about two.

## Saying what you speak

A **declaration**, not a negotiation. Each side states what it holds,
neither agrees to anything, there is no round trip and no failure to
agree — a negotiation has a state where both peers wait for the other to
concede, and that state is indistinguishable from a stall.

```rust
let mut wire = Vec::new();
isthmus::frame::put_frame(
    isthmus::hello::HELLO_TAG,
    &isthmus::hello::Hello::of_isthmus().encode(),
    &mut wire,
)?;
```

Revisions are compared for **equality and never ordered**. Ordering would
let a peer decide it is ahead and act on the difference, which is
authority this substrate does not have.

**A peer that speaks less is limited, never refused.** Implement only
the four points above and you can still connect — you will forward what
you cannot read, which is exactly what a linking mesh does.

## What this does not give you

Stated plainly, because discovering these later is worse.

- **No transport security.** Tampering degrades to refusal rather than
  false acceptance, because nothing above believes a claim it has not
  re-derived — but that is not confidentiality. Use an encrypting
  carrier; the frames are identical either way.
- **No node identity.** A peer is what it can demonstrate. No issuer, no
  key registry, no party to ask.
- **No freshness.** A replayed frame is valid and re-derives correctly.
  `IS-2` §6 is open.
- **No accountability.** A peer that floods cannot be named, so it
  cannot be excluded by name. Exclude at the adapter.

## Checking yourself

```
cargo test
```

43 tests. `tests/vectors.rs` asserts every byte string `IS-1` §9
publishes for the frames this crate owns, in **both** directions — one
direction alone passes for a codec that is consistently wrong.

`tests/refusals.rs` constructs one input per row of the §4 table **and
one that must be accepted for each**. A reader that refuses everything
passes a table of refusals perfectly; a gate is only a gate once both
sides of it are built.

## License

MIT OR Apache-2.0.
