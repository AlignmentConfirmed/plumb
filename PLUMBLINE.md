# PLUMBLINE — the language this system speaks

**Plumbline** is the name of the wire language the IS documents
specify and this workspace implements. Plumb is the instrument;
a plumbline is what the instrument produces — the line everything
else is checked against. A peer declares which **Plumbline revisions**
it speaks (IS-5); revisions compare for equality and are never
ordered.

The name attaches to the **speech**, not the record: records,
envelopes, declarations, and acts are Plumbline. The chain is not
Plumbline — it is what Plumbline becomes after it survives the court.

## The parts of speech

| Plumbline | linguistic role | pinned by |
|---|---|---|
| tag | word | the registry (IS-3): a grant table, not a dictionary |
| granted range | dialect | a deed on the chain; retired ranges never reissue |
| record `tag ‖ len ‖ value` | utterance | IS-1 + conformance vectors |
| conformance vector | meaning | what a conforming reader must *do* — semantics with teeth |
| declaration (hello) | introduction | IS-5: who I am, what I speak, what I hold |
| claim envelope | assertion | tags 80–82, opaque in transit |
| attestation | signature line | `sig`, scheme byte, beside the envelope |
| act | performative | IS-6: speech that changes the world it describes |
| skip-unknown | politeness | words you don't know are carried, not corrected |

A sentence in Plumbline is not judged true or false. It is judged
**settled or unsettled** — and settlement is where the language stops
being one-dimensional.

## How Plumbline is multi-dimensional

A conventional protocol is one-dimensional three times over: its
statements totally order (a linear log), its predicates are scalar
(valid/invalid, one number), and its vocabulary is fixed at design
time. Plumbline refuses all three totalities. The syntax is linear —
bytes arrive in a row, as bytes must — but every layer of *meaning*
above the byte is a space with more than one axis, and each axis is
enforced by a named test, not a metaphor.

### 1 · Predicates are vectors, not scalars

A claim's credit is an `Extent` — one exact-rational component per
axis — and settlement demands closure on **every** axis of the priced
space: correct on one axis and unfunded on another settles nothing.
There is no total "score"; `volume()` (the product across axes) was
deliberately struck from the code, because collapsing axes into one
number is exactly the flattening this language exists to refuse.
*Enforced:* `datum::extent`, `reward` multi-axis cover, `tests/board.rs`.

### 2 · Nouns are regions, not points

A deed — the language's proper noun — is a **box**: one inclusive
range per axis, padded to the edge's dimensionality. Holding ground in
Plumbline is holding an n-dimensional region, and estates nest:
a **moon** is an estate within an estate, held at one more depth
(IS-6/3). Depth is itself a dimension of ownership.
*Enforced:* `isthmus::deed::Deed { region, within }`, containment laws.

### 3 · The language can mint new dimensions mid-sentence

`Act::Open` is a speech act that **opens a new axis** on a live edge.
Dimensionality is not a design-time constant: the board has priced
11-axis space in tests, the master equation exercises 5-D estates, and
an axis opened later reads every earlier deed as pinned to its zero
slice — old sentences stay true in the larger space.
*Enforced:* `Act::Open`, `Estate::Galaxy { axes, region }`,
`tests/master_equation.rs` (laws S · E · C · W · M).

### 4 · Time is a partial order, not a line

Acts on one chain fold horizontally; across chains there is no shared
clock and no pretence of one. Each axis carries its own partial
order, and two acts with no path between them are **concurrent** —
a first-class answer, not a failure to know the "real" order. A
language whose statements totally ordered would need a coordinator to
do the ordering; Plumbline is coordinator-free because its time is
already plural.
*Enforced:* per-axis partial order in the deed laws;
`Hello::against` answering `Some(None)` for concurrent.

### 5 · Statements about other conversations: the vertical axis

`Act::Anchor` cites another chain — name, height, digest — which
gives the language a dimension *across* ledgers: what a chain knows
of other chains is a **frontier** (how far into each it has seen),
and multi-chain knowledge is a **sphere of frontiers**, not a merged
log. Plumbline can talk about speech it does not carry.
*Enforced:* `isthmus::sphere`, `tests/vertical.rs`.

### 6 · Meaning varies by speaker-pair, and no one is wrong

Revisions compare for equality, never for order — so the language has
no single "current" meaning, only meanings-in-common between pairs of
speakers (`shared_revisions`). Dialects coexist on one wire because
skip-unknown carries what it cannot read. The semantic space is
dimensioned by *who is talking*, and a session is held to the
intersection.
*Enforced:* `isthmus::hello`, `sdk::attach::agree`, skip tests.

## The one-sentence definition

> **Plumbline is a language whose utterances are linear bytes and
> whose meanings live in spaces: vector predicates, region nouns,
> axes that open at runtime, plural time, cross-chain reference, and
> pairwise semantics — with every dimension held by a test that can
> fail.**

## What is named, what is new

The name is new (decided 2026-08-27); everything it names predates
it. Nothing on the wire changes: no tag, no vector, no revision
string moves because the language now has a name. A reader who has
never heard the word "Plumbline" interoperates byte-for-byte with one
who has — which is, fittingly, the language's own first rule about
unknown words.
