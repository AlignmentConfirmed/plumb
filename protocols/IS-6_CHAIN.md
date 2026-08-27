# IS-6 — THE CHAIN

**Status:** **ENFORCED** — `isthmus::deed` codec, `datum::ledger`
founding chain, well-formed induction, act vectors.

**Dimensionality:** horizontal acts live on one chain; **estate space**
is **n-axial** via `Open` (not capped at 2-D; 11-D board path exists).
**Vertical** `Anchor` is observation only — grants no ground and opens
no axes. Causal multi-chain structure is a **frontier sphere**, not a
single total order. Product law:
the lab's `decide/linkage-estates.md`.

The authority's record, as stored bytes.

`IS-3` §5 publishes a **grant table** — who holds which tags. That
table is a *rendering*. This document specifies the thing it is
rendered from: an append-only sequence of acts, stored as `.tlv`, from
which every answer about who holds what is a fold.

**Until this document, the format was specified nowhere.** An outside
implementation could read `IS-3`'s table and could not read or produce
the record that table comes from — so it could verify nothing and
append nothing. `IS-6` closes that.

## 1. What a chain is

```
.tlv -> kernel -> local mesh -> SUBSTRATE -> local mesh | kernel
```

A chain is a **sequence of acts and nothing else**. Nothing is edited
in place, nothing is erased, and `Retire` is an entry rather than a
deletion. Every reading — who holds tag 64, how much space is open,
what the axes are — is derived by folding the acts in order.

Two consequences that decide the rest of the document:

1. **A reader that mis-folds one act reports a different history as
   this one.** That is why §5's refusal rule is what it is.
2. **A chain is not a snapshot.** Comparing stored bytes against a
   rebuild is correct exactly until the chain first grows. Coverage —
   *every claim recorded somewhere in the history* — is the
   evolution-proof invariant.

## 2. Framing

Each act is one record under `IS-1` §1, unchanged:

```
record = tag u8 ‖ LE32(length) ‖ value
```

A chain is those records concatenated, in order. Nothing separates
them and nothing frames the whole; a chain's length is however many
bytes it is.

## 3. Primitives

| primitive | layout |
|---|---|
| `tag` | `LE64` — a coordinate on one axis |
| `text` | `LE16(len) ‖ utf8`. Not utf8 is not text: **refuse**, never lossy-convert |
| `blob` | `LE32(len) ‖ bytes`. Opaque, and the width is the writer's choice |
| `region` | `LE16(axes) ‖ (tag low ‖ tag high)…` — per-axis inclusive ranges, in axis order |

`text` is `LE16` and `blob` is `LE32` deliberately. A name is this
protocol's own vocabulary and a short ceiling is honest; a blob carries
a digest whose width is **somebody else's** decision, and a narrow
length there would be this document putting a ceiling on a stranger's
choice.

## 4. The acts

| tag | act | value |
|---|---|---|
| 1 | `Encumber` | `tag low ‖ tag high ‖ text by ‖ text witnessed` |
| 2 | `Issue` | `text holder ‖ tag low ‖ tag high` |
| 3 | `Retire` | `text holder` |
| 4 | `Open` | `text axis ‖ tag max` |
| 5 | `EncumberBox` | `region ‖ text by ‖ text witnessed` |
| 6 | `IssueBox` | `text holder ‖ region` |
| 7 | `Cede` | `text from ‖ text to ‖ region` |
| 8 | `Anchor` | `text chain ‖ LE64 height ‖ blob digest ‖ text witnessed` |
| 9 | `Sublet` | `text from ‖ text to ‖ region` |

### 4.1 Observations carry provenance

`Encumber`, `EncumberBox` and `Anchor` are **observations of somebody
else's facts**, and each carries `witnessed` — where the fact was read.
An observation without provenance is indistinguishable from this chain
having decided something.

What legitimately enters a chain is somebody else's claim, with a
pointer to where it was read. **This chain's own documents are never an
input.** Parsing our own prose back in would make an append-only
history depend on a file anyone can edit afterwards.

### 4.2 Tags 4–7 are additive, and tag 8 is the vertical

Tags 1–3 act on **one line**: an interval, issuance marching along it,
exhaustion at the end. Tags 4–7 open the space — axes, boxes,
conveyance — so a deed becomes a box and growth stops meaning *further
along the line*.

Tags 1–7 are all **horizontal**: they move this chain's own fold, and
this chain's acts are totally ordered among themselves because one
party appends them.

### 4.3 `Cede` transfers; `Sublet` nests

They look alike on the wire — `from`, `to`, a region — and they are
opposites.

| | `Cede` (7) | `Sublet` (9) |
|---|---|---|
| the region must be | a **slab** of the owner's estate | **inside** the owner's estate |
| afterwards the owner | **shrinks** by exactly that slab | **keeps every point of it** |
| the recipient's depth | the seller's | the granter's, **plus one** |
| live deeds now | stay disjoint | **overlap, on purpose** |

A sublet is the moon: an estate within an estate, with the planet still
there. That last row is a deliberate break of the disjointness the
theorems were stated with, and §8 restates them rather than dropping
them.

The recipient must hold nothing in both cases, so neither act can give
one party two estates.

**Tag 8 is the only act that names another chain.** Acts in different
chains are unordered — that is what having no shared instant means —
*except* through anchors. An anchor at height `h` naming chain `c` at
height `k` orders every act of `c` below `k` before every act of this
chain at or above `h`. See §6.

### 4.x `Bind` (tag 10) — the presenting key

`text(holder) ‖ u8(scheme) ‖ blob(key) ‖ LE64(from_epoch) ‖
LE64(until_epoch)`

Identity, not ground. The key bytes are opaque here for the same
reason an anchor's digest is: which scheme signs is an edge's
decision, named by the scheme byte (`0x01` = Ed25519/BLAKE3, the
signature leaf) and never interpreted by the chain. The last bind for
a holder wins; the superseded key stays in the acts, because rotation
is history, not revision.

### 4.y `Declare` (tag 11) — the published domain definition

`text(holder) ‖ LE64(tag) ‖ blob(definition)`

Speech, not ground. The definition bytes are opaque to the chain —
a court's evaluator interprets them (the declared-complex codec
today). The resolver's rule is read-time: a definition counts only
while its declarer holds the tag's live deed, so a vocabulary lapses
with its grant. A later declare for the same tag supersedes the
earlier; supersession is an append.

## 5. An unknown act REFUSES. It does not skip.

On a mesh, an unknown tag is stepped over whole and forwarded (`IS-1`
§10): a frame that is not yours is still somebody's.

**In a chain the opposite rule holds.** Every act moves the fold, so a
reader that skipped one would fold a *different history* and report it
as this one — the worst available outcome, delivered quietly. A chain
carrying an act a reader does not implement is a chain that reader
cannot read, and it must say so.

This is what makes §4.2's additions safe: a chain carrying tags 4–8
refuses on a reader that has only 1–3, rather than misfolding.

## 6. Height is a count

A height is **how many acts**, so height 0 is the empty chain and there
is no off-by-one to disagree about. `Anchor { chain, height: k }` means
*I observed that chain's first `k` acts, and they digest to this.*

The digest is over the **canonical re-encoding** of the cited prefix —
the acts, re-framed per §2 — and not over whatever bytes arrived. Two
peers that framed the same acts differently would otherwise disagree
about a history they agree on. The acts are the chain; the bytes are
how it travelled.

**Which function produced the digest is not specified here**, for the
same reason `IS-5` §3.1 does not specify it: picking one would be this
document choosing a security property for every integrator. A verifier
takes the function in.

### 6.1 A chain's name is not in its bytes

Nothing in this format states what a chain calls itself. The name is
context the acts are read *in*, on the same footing as the layout, so a
party reading a chain off disk learns the history and not who kept it.

A name reaches a peer over `IS-5` §3.1. A chain with no name can anchor
others and cannot be anchored: downstream works, upstream does not.

### 6.2 An anchor grants nothing

Anchoring does not adopt the observed chain's deeds, submit to its
authority, or merge its history. It covers no ground on this edge — so
a vertical can never collide with a horizontal, and appending one is
always safe.

## 7. Test vectors

Every act tag, exactly as the codec produces it.

```
C1  Encumber 1-31 by "ancestral", witnessed "both registries"
012c000000 0100000000000000 1f00000000000000
           0900 616e6365737472616c
           0f00 626f74682072656769737472696573

C2  Issue "isthmus" 64-79
0219000000 0700 697374686d7573 4000000000000000 4f00000000000000

C3  Retire "isthmus"
0309000000 0700 697374686d7573

C4  Open axis "revision", max 7
0412000000 0800 7265766973696f6e 0700000000000000

C5  EncumberBox [(1,3),(0,1)] by "north", witnessed "read"
052f000000 0200
           0100000000000000 0300000000000000
           0000000000000000 0100000000000000
           0500 6e6f727468
           0400 72656164

C6  IssueBox "newcomer" [(8,15),(0,1)]
062c000000 0800 6e6577636f6d6572 0200
           0800000000000000 0f00000000000000
           0000000000000000 0100000000000000

C7  Cede "owner" -> "buyer", slab [(12,15)]
0720000000 0500 6f776e6572 0500 6275796572
           0100 0c00000000000000 0f00000000000000

C8  Anchor: chain "south" at height 15, 8-byte digest, witnessed
    "session"
0824000000 0500 736f757468
           0f00000000000000
           08000000 0102030405060708
           0700 73657373696f6e

C9  Sublet: "owner" grants "moon" the region [(40,47)]
091f000000 0500 6f776e6572 0400 6d6f6f6e
           0100 2800000000000000 2f00000000000000
```

Generated by `datum/tests/chain.rs::the_published_chain_vectors_are_
what_the_codec_produces` and asserted against this hex, exactly as
`IS-1` §9's twelve are.

**Two of these were hand-derived wrong on the first attempt** — C1's
and C6's length fields, `0x1e` for `0x2c` and `0x28` for `0x2c`. That
is the fourth and fifth time hand-typed bytes have been wrong across
this work, and it is the whole argument for the generator. The
mistakes are recorded rather than quietly fixed, because a document
whose vectors were once wrong and now are not should say which.

C8's digest is a **literal**, eight bytes counting up, for the reason
§6 gives: this document frames a digest and names no function.

## 8. Well-formedness

A chain built by an issuer refuses its invalid states by construction.
A chain that arrived from history did not, so it is **checked** — a
decidable predicate over the acts, in order:

1. **One live deed per holder.** A second `Issue` to a live holder is a
   flaw. A holder needing more space retires and reattaches wider.
2. **No issue onto taken ground** — encumbered, deeded, or retired —
   with exactly one lawful overlap: an issue may land on an encumbrance
   **by the same name**. That is a party's own observed claim becoming
   its deed proper, and nothing else.
3. **No future axes.** An act never carries more coordinates than axes
   open at its position. A history referencing a direction that did not
   exist yet is not a history.
4. **Cession is from a live estate, of a slab, to a holder that holds
   nothing, and never to itself.**
5. **`H2′` for sublets.** A sub-estate is strictly inside its granter's
   estate on every axis, the granter holds a live estate, the recipient
   holds nothing, and **moons of one estate are disjoint from each
   other**. See §8.2.
6. **One axis name, one extent.** A repeated `Open` with the same
   extent is a replay and folds to nothing (§8.1). The same name with a
   *different* extent is not a replay, and the fold keeps the first —
   so the second declaration would vanish. That is a flaw.

Encumbrances may overlap each other: two observations of a collision
are two true observations.

### 8.2 `H2′` — what containment costs, and what it buys

The cocycle verification the substrate rests on assumed two things:

```
H1   each holder holds at most one live deed
H2   live deeds do not overlap
```

`Sublet` breaks `H2` on purpose — a moon and its planet cover the same
ground. So `H2` is **restated**, not dropped:

```
H2'  live deeds AT THE SAME DEPTH are pairwise disjoint,
     and every deed is strictly inside its parent
```

Depth is the length of the containment chain: `0` is ground held on the
edge, `1` a moon, `2` a moon of a moon.

**`H2′` reduces to `H2` on a chain with no sublets**, so nothing already
proven is surrendered. What it buys is the property `H2` was only ever a
way of getting: the deeds covering any point have **distinct depths**,
so the containment chain over that point is totally ordered and has a
unique maximum. "Who holds this point" therefore still has exactly one
answer — the deepest — and the admitted relation is still a function,
which is what the cocycle theorem actually needed.

Two consequences worth stating because an implementation can get each
one wrong while passing everything else:

- **A cession from a sub-estate inherits the seller's containment.**
  Otherwise a moon could sell ground to a stranger and the stranger
  would hold it at depth 0 — an escape hatch out of every estate,
  reachable in two acts.
- **A reader must resolve to the deepest holder.** Answering with the
  first or the shallowest is conformant with every other section and
  reports the planet where the moon is standing.

### 8.1 Replay

`IS-2` §6.1 rules that a frame with an effect must be idempotent under
replay, *either naturally or by carrying an identity the receiver dedups
on*. **The acts are effects** — each moves the fold — so the rule
applies to every one of them. `IS-2` §6.5 records the audit; the result
is that each act is in exactly one of two lawful states:

| | acts |
|---|---|
| **idempotent naturally** | `Encumber`, `EncumberBox`, `Retire`, `Anchor` |
| **refused on the second application**, so the effect never lands | `Issue`, `IssueBox`, `Cede`, `Sublet` |

`Open` was in neither: it doubled the axis count and was accepted. It
is now idempotent by the second of `IS-2` §6.1's remedies — the act
already carries `axis`, and a fold **ignores an `Open` whose axis is
already open**. No sequence number, no window, no seen-set was added;
the existing rule had simply never been applied here.

An implementation that opened a second axis for a repeated `Open` would
be conformant with every other section of this document and wrong.

**Anchors are exempt from all of it.** A vertical constrains nothing
about this edge's ground, and it must not — or a chain would become
ill-formed because of what a stranger appended to theirs. Whether an
anchor's digest is *true* is a different question, answerable only by
someone holding both chains.

## 9. What this does not settle

- **Storage beyond one file.** The authority keeps its chain as one
  `.tlv` and git history is the append-only guarantee. Nothing here
  says what a chain too large for that does.
- **Who may append.** The format says what an act looks like, not who
  is entitled to write one. On a substrate with no lock, that is not a
  question the encoding can answer.
- **Retraction.** There is none, by construction. A dispute is settled
  by further acts — see the lab's `decide/arbitration.md` — and never by removing
  one.
- **Compaction.** A fold is over the whole history, every time. Whether
  a chain may publish a checkpoint that a reader can start from, and
  what would make such a checkpoint trustworthy, is undecided.
- **Three-way anchoring.** §6's ordering is stated pairwise. Whether
  three chains anchoring around a cycle say anything a reader can use
  is not settled here.

## 10. Revisions

| revision | change |
|---|---|
| `IS-6/1` | the chain: framing, primitives, acts 1–8, the refusal rule, well-formedness, and the eight vectors |
| `IS-6/2` | §8.1 replay — a repeated `Open` folds to nothing, and one axis name with two extents is a flaw. **A behaviour change**, not a clarification: an `IS-6/1` reader opens a second axis where this one opens none, so the two disagree about the shape of the space |
| `IS-6/5` | tag 11 `Declare` — a domain definition on the record: `holder ‖ LE64(tag) ‖ blob(definition)`. Registration requires holding the grant (resolver's read-time rule); the definition lapses with the deed; supersession is an append. UC4 of the universal checker: a court learns a discipline from the chain alone, no rebuild |
| `IS-6/4` | tag 10 `Bind` — a holder's presenting key on the record: `holder ‖ scheme(u8) ‖ blob(key) ‖ LE64(from_epoch) ‖ LE64(until_epoch)`. Binds key × the holder's grants × an epoch window as a chain fact (S3, `IMPLEMENTATION.md`). The last bind for a holder supersedes earlier ones — rotation is an append. A bind covers no ground (like `Anchor`) so it collides with nothing horizontal. Additive on the wire: an older reader refuses tag 10 per §5. A holder with no bind is **legacy/unbound** — visible, and refusable by courts that demand keys |
| `IS-6/3` | tag 9 `Sublet` — estates within estates. §4.3 separates it from `Cede`, §8.2 restates `H2` as `H2′`, and C9 is its vector. Additive on the wire (an older reader refuses tag 9 per §5), and a **restatement** of the theorems rather than a weakening: `H2′` reduces to `H2` where nothing is sublet |
