# IS-3 — REGISTRY

**Status:** the grant/deed model is implemented in `isthmus::deed` and
the founding chain; this document's table renders the chain (see
`IS-6`). Collision and coverage are checked by the `datum` registry and
chain tests.

## 1. Ownership

`isthmus` owns the numbering space. Every kernel is a guest, and no
grantee inherits another's assignments.

Rule: **one tag, one meaning, forever; a retired tag is never reissued.**

## 2. Grants

A grantee holds a contiguous range and claims tags inside it without
another grantee's merge landing on top.

```
isthmus     transport and work tags
datum       court tags
<kernel>    kernel frames (reserved ranges)
<mesh>      one range per mesh that links in
```

`isthmus` forwards every granted range by length and interprets none. A
grant is a claim on numbers, not on meaning: what a value means is the
grantee's, and the mesh never reads it.

## 3. Claiming

A grantee claims a tag by recording it in its own range's table before
any code uses it, and verifies after any merge that the claim survived.
A tag named only in prose is not protected by a merge check.

## 4. The width

One byte (`IS-1` §1): 256 values, shared. `cargo test --test registry`
measures the claimed and free counts. A tag claimed only in prose, with
no registry-table entry, is counted but must be tabled before it is
granted a range.

## 5. The grant table

Grants are contiguous, so a grantee can claim inside its own range
without another's merge landing on top.

| range | grantee | size | state |
|---|---|---|---|
| 0 | — | 1 | never issued: a zero-filled buffer decodes to tag 0, so a frame from nowhere names nothing |
| 1–31 | ancestral | 31 | frozen (§5.1) |
| 32–54 · 60 · 62–63 | reserved | 26 | claimed in an external registry; not grantable |
| 55–59 · 61 | — | 6 | free, too small to grant |
| 64–79 | `isthmus` | 16 | transport: session, advert, grant, refusal, reattachment |
| 80–86 | `isthmus` / `datum` | 7 | work and court: claim (80), receipt (81), shape-claim (82), attestation (83), witness (84), query (85), busy/register (86) |
| 87–127 | — | 41 | unassigned |
| 128–159 | reserved | 32 | external kernel frames |
| 160–191 | reserved | 32 | external kernel frames |
| 192–239 | linking meshes | 48 | six ranges of 8, issued on request |
| 240–255 | reserved | 16 | not issued (§5.3) |

`assay` holds no wire tags: it is the isolated verification engine, and
cannot read or write a framing tag, so it is granted none.

### 5.1 Why 1–31 is frozen

Fifteen values in this band are claimed by more than one registry with
different meanings (1–7, 10–13, 16–19). One tag, one meaning, forever —
so no number in the band can be granted without breaking the rule for
another holder. Freezing costs 31 values; grantees relocate into their
granted ranges.

### 5.2 Grants are numbers, not meanings

What a value means inside a range is the grantee's. A grantee may retire
a tag; it may not reissue one.

### 5.3 Why 240–255 is held back

So that exhausting the space refuses, rather than a grantee taking the
last value and the next one wrapping. Do not issue from it without
settling the width question first.

## 6. A tag inside a grant is derived, not declared

A tag within a grant is derived from the record kind's name and the
deed, not assigned centrally:

```
tag = deed.low() + (spread(kind) mod deed.width())

spread(kind):                       FNV-1a, 64-bit
    h = 0xcbf29ce484222325
    for each byte b of kind (UTF-8):
        h = (h XOR b) * 0x100000001b3   (mod 2^64)
```

`kind` is the record kind's name, chosen by its author. Nothing central
assigns it.

The hash is not a security function: it spreads names over a small
range and neither hides nor authenticates anything. That is sufficient
because the offset is not a secret, and a party choosing colliding names
collides only inside its own deed.

Guarantees:

1. **Deterministic.** A function of the kind and the deed alone. Two
   parties that never meet compute the same tag with nothing exchanged.
2. **Collision-free across holders.** Deeds are disjoint (`IS-6` §8.2),
   and a derivation never leaves its deed, so two holders' record kinds
   cannot land on one tag.
3. **Stable under growth.** The tag does not depend on what else the
   vocabulary contains, so declaring a new record kind never moves an
   existing one.

A record kind is its offset: two edges that deed a holder different
ranges agree on what a given record kind is and disagree only about its
number.

Two kinds in one vocabulary can derive one tag. This is reported, not
silently resolved: `Deed::collisions` names the pair and the author
renames one. Held by `isthmus/tests/deed_suite.rs (mod derivation_laws)`.

## 7. Frames that name a kernel type

Two granted frames carry a dependency on a kernel type (`IS-1` §7): the
relation frame carries `Support` (the 1-skeleton, not the full
relation), and the manifold frame's `cells` section is optional. A
grantee holding either must not freeze its layout before those are
resolved.

The rule: **a frame that names a kernel type carries a dependency on
that type.** `cargo test` pins structure per named type.

