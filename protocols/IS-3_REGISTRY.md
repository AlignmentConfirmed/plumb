# IS-3 — REGISTRY

**Status:** grant/deed model **ENFORCED** via `isthmus::deed` + founding
chain; this document’s table is a **rendering** of the chain (see
`IS-6`). Collision/coverage **MEASURED** in `datum` registry/chain tests.
§4 corrected historical blocking claims.

## 1. Ownership

`isthmus` owns the numbering space. Both kernels are guests. Neither
inherits the other's assignments.

Rule carried over from both ancestors unchanged: **one tag, one
meaning, forever; a retired tag is never reissued.**

## 2. Grants

A grantee holds a contiguous range and claims inside it without another
grantee's merge landing on top. This is the mechanism `strand` already
uses to hold 19-31 for `pouw`, applied one level out.

```
isthmus     transport tags
assay       proof frames
lith        kernel frames
chitin      kernel frames
<mesh>      one range per mesh that links in
```

`isthmus` forwards every granted range by length and interprets none.
A grant is a claim on numbers, not a claim on meaning: what a value
means is the grantee's, and the mesh never reads it.

## 3. Claiming

A grantee claims a tag by editing its own range's table before any code
uses it, and verifies after any merge that the claim survived. Recorded
because netstratum lost a landing not to a collision but to a merge
silently dropping the hunk that recorded one.

## 4. The width

One byte in both projects (`IS-1` §1), so 256 values, shared.
`cargo test --test registry`:

```
netstratum claimed  52      claimed by both, different meanings  15
strand claimed      17      claimed in tables                    54
strand held         12      claimed in prose only                 2
                            claimed, all told                    56
                            free                                200
```

A constraint to plan against, not a blocker. Re-run before granting the
last quarter.

**Tags 83 and 84 are claimed in prose and appear in no registry table.**
A tag named only in a sentence is a tag no merge check protects — which
is the failure §3 records netstratum already suffering. They are counted
here and must be tabled before they are granted a range.
`measure/tag-draw.md`.

## 5. The grant table

Grants are contiguous so a grantee can claim inside its own range
without another's merge landing on top.

**Every range below is free in both registries.** The first draft of
this table was not, and §5.4 records what that cost.

| range | grantee | size | state |
|---|---|---|---|
| 0 | — | 1 | **never issued.** A zero tag is what a zero-filled buffer decodes to, and a frame that arrives from nowhere must not name anything |
| 1–31 | *ancestral* | 31 | frozen, see §5.1 |
| 32–54 · 60 · 62–63 | *encumbered* | 26 | claimed in netstratum's registry, and `51` in strand's. **Not grantable.** See §5.4 |
| 55–59 · 61 | *fragments* | 6 | free in both, too small to grant. Held |
| 64–79 | `isthmus` | 16 | transport: session, reference advert, facet grant, refusal, reattachment |
| 80–127 | ~~`assay`~~ **struck** | 0 | **retired, never reissued.** See §5.5 |
| 128–159 | `lith` | 32 | kernel frames — relation, closures, manifold, witness, receipt |
| 160–191 | `chitin` | 32 | kernel frames — chronicle records, axis vectors, spine |
| 192–239 | linking meshes | 48 | six ranges of 8, issued on request |
| 240–255 | reserved | 16 | **not issued.** See §5.3 |

### 5.1 Why 1–31 is frozen rather than reissued

Fifteen values are claimed by both projects with different meanings:
1–7, 10–13, 16–19. One tag, one meaning, forever — so no number in that
band can go to either project without breaking the rule for the other.
Freezing costs 31 values; both projects relocate into their granted
ranges. It is the only migration this table requires.

### 5.2 Grants are numbers, not meanings

What a value means inside a range is the grantee's. A grantee may retire
a tag; it may not reissue one.

## 5.7 One range, two registries, two names — reconciled

`strand`'s registry reserves `64–79` with **`held for datum-lane`**.
§5 grants `64–79` to **`isthmus`**. Same range, two names, and until
this section nothing connected them.

The names differing is not the defect. **Nobody having written down
whether they are the same party is** — and that is precisely how the
`32–47` reissue happened (§5.4): two tables written independently, and
no one compared them.

They are one grant. The lane produces the crate:

| foreign registry says | `IS-3` §5 grants to |
|---|---|
| `datum-lane` (strand) | `isthmus` |

`datum::registry::RECONCILED` carries that row and
`tests/registry.rs::every_range_held_for_a_named_party_is_reconciled_with_is3`
enforces it: every range a registry reserves for a *named* party must
be reconciled to an `IS-3` grantee, and the grantee must be the one §5
grants that range to. **A new name fails the test until somebody states
which grantee it is.** Guessing is the one repair not on offer.

### 5.7.1 What the gate found on its first run

A second reservation nobody had noticed: **`strand` reserves `19–31` for
`pouw`**, inside the band §5.1 freezes.

§5.1 freezes `1–31` because fifteen values in it are claimed by both
projects with different meanings, and rules that *"both projects
relocate into their granted ranges."* A standing reservation inside the
frozen band says one has not. It cannot be reconciled with a grantee,
because §5 grants nobody anything there.

So it is **counted, not resolved**. The test pins the set at exactly
`{strand: pouw}`, so a second such reservation fails rather than
accumulating. Two documents disagree; the disagreement is on the record
with a number attached, and neither document is being quietly ignored.

## 5.6 RULED — a tag inside a grant is DERIVED, not declared

`netstratum`'s mesh writes `MESH_HEAD_TAG: u8 = 64`. §5 grants 64–79 to
`isthmus`. Two substrate-layer protocols, written without knowledge of
each other, both reached for the same byte — measured by
`datum/tests/both_meshes.rs`.

**Neither was wrong.** Each was choosing a global number from a private
constant, and a constant cannot be right here: it encodes an assumption
about who else exists, and who else exists is exactly what a substrate
cannot know. A kernel that has not attached yet cannot have been
consulted. It is `grants_available() -> 6` again, wearing a number
instead of a count.

### The derivation

```
tag = deed.low() + (spread(kind) mod deed.width())

spread(kind):                       FNV-1a, 64-bit
    h = 0xcbf29ce484222325
    for each byte b of kind (UTF-8):
        h = (h XOR b) * 0x100000001b3   (mod 2^64)
```

`kind` is the record kind's **name**, chosen by its author. Nothing
central assigns it and nothing has to be told about it.

**The hash is not a security function and is not claimed as one.** It
spreads names over a small range; it hides nothing and authenticates
nothing. That is sufficient because the offset is not a secret and
because a party choosing names that collide only collides inside its
own deed. Requiring a cryptographic hash would cost every integrator a
dependency to buy a property nothing here uses.

### What it guarantees

1. **Deterministic.** A function of the kind and the deed alone. Two
   parties that never meet compute the same answer, with nothing
   exchanged.
2. **Collision-free across holders, structurally.** Deeds are disjoint
   (`IS-6` §8.2, `H2′`) and a derivation never leaves its deed, so
   `netstratum`'s `head` and `isthmus`'s `head` cannot land on one tag —
   whatever either calls it.
3. **Stable under growth.** It does not depend on what else the
   vocabulary contains, so declaring a tenth record kind never moves
   the other nine. An assignment that probed for a free slot would pack
   tighter and move tags as the vocabulary grew: a wire break dressed
   as an optimisation.

### The offset is the identity; the tag is the frame

`deed.rs` already rules that the offset within a box is gauge-invariant
and the box's origin is the frame. Applied to vocabulary: **a record
kind IS its offset.** Two edges that deed the same holder different
ranges still agree exactly on what a `head` record is, and they
disagree about its number — which is the correct way round.

### The cost, and it is paid visibly

Two kinds in one vocabulary can derive one tag. This is **reported, not
resolved**: `Deed::collisions` names the pair and the author renames
one. Silently probing past it would trade a visible refusal for a
property nobody could rely on.

Held by `isthmus/tests/deed_suite.rs (mod derivation_laws)` — determinism, containment,
cross-holder disjointness, stability under growth, order-invariant
collision reporting, and the refusal to derive anything from a deed
with no region (which would land on tag 0, the void).

### §5.5 RULED — `assay`'s grant is struck, and the table needed nothing else

`assay` was specified as a **pure convergence engine with absolute
dependency isolation**: it imports neither the mesh nor a kernel, parses
no frame, and serialises no payload. It is therefore incapable of
reading or writing a wire tag, and reserving 48 of them for it was an
architectural contradiction — a grant to a crate that is blind to the
thing being granted.

Struck. The 48 values become **retired, not open**: §5.2's rule is that
a tag may be retired and never reissued, and that applies to this court
too. So the strike spends the whole block, not only the 16 values
`strand` disputed. That is the price of the rule that stops space being
laundered through retire-and-regrant, and it is paid in full here.

The dispute that forced it: `strand 02b8432` grants **80–95 to
capacity-lane** for deeds, estates and vaults — things that genuinely
cross a wire. That grant stands.

**And the rest of the table needed no change.** Measured after the
strike, every remaining granted range is free in both registries:

```
isthmus         64-79    contested: 0
lith            128-159  contested: 0
chitin          160-191  contested: 0
linking meshes  192-239  contested: 0
reserved        240-255  contested: 0
```

A relocation was ruled and is **not performed**, for two reasons, both
arithmetic rather than preference:

1. **190 contiguous values cannot begin at 128.** The tag is one byte
   (`IS-1` §1), so `128 + 190 − 1 = 317` and the largest tag is 255.
   The block `128–255` holds **128** values. After the strike the
   grants total exactly 128 — `isthmus` 16, `lith` 32, `chitin` 32,
   linking meshes 48 — so they fit only by giving up the 16 reserved
   values as well, which §5.3 holds back on purpose.
2. **The destination overlaps the source.** `lith` and `chitin` already
   sit at 128–191. Relocating into `128+` means retiring them first,
   and retired ground is never reissued — so the move would spend the
   very range it was moving into. 64 of the 128 available values would
   be burned to reach them.

The intent behind the ruling — *`isthmus` takes a clean, unfragmented
substrate* — is satisfied by the strike alone. `64–79` is uncontested,
and `strand`'s own registry reserves it: `| 64–79 | held for
datum-lane |`. The substrate is already clean; it did not need to move
to become so.

### 5.4 The first draft granted numbers that were already taken

`32–47` went to `isthmus` and `48–95` to `assay`. netstratum's registry
claims `32`–`50`, `52`–`54`, `60` and `62`–`63`. **Both ranges collided
on the day they were written**, and this document did not notice because
it counted the *space* and never checked its own table against it.

strand found it from the other side, issuing `32–47` to datum-lane as a
standing grant and then discovering every number in it was taken. Its
header records the finding: *"the grant was issued against this table
alone, and this table is not the whole tag space."*

Measured now rather than reasoned about —
`tests/registry.rs::the_grant_table_uses_only_free_numbers` fails on any
granted range that either registry claims. `1–31` is exempt because it
is frozen rather than granted.

The lesson is the one §3 already carries about prose claims, arriving
from the other direction: **a table is only authoritative over what it
enumerates.** A grant written against one registry is a claim about two.

### 5.3 Why 240–255 is held back

So exhausting the space is a day something refuses, rather than a day a
grantee takes the last value and the next one wraps. Do not issue from
it without settling the width question first.

## 6. Frames that name a kernel type

Two granted frames carry a dependency on a kernel type, and both were
understated when drafted because `lith` gained relations of arbitrary
arity and wired them into `Manifold`.

- The **relation** frame carries `Support`, which is the 1-skeleton and
  not the relation. A cells frame is owed.
- The **manifold** frame omits `cells`, which decodes to a valid
  manifold and the wrong one.

Both are specified in `IS-1` §7. A grantee holding either must not
freeze its layout before that is resolved.

The rule: **a frame that names a kernel type carries a dependency on
that type**, and pinning the kernel's commit does not catch it.
`cargo test` pins structure per named type for that reason.

## 7. What is not decided here

Which specific numbers a grantee assigns inside its own range. That is
the grantee's, by §5.2.
