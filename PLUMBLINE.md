# PLUMBLINE — the wire language

**Plumbline** is the name of the wire language specified by the IS
documents and implemented in this workspace. A peer declares which
**Plumbline revisions** it speaks (IS-5); revisions compare for
equality and are never ordered.

The name refers to the speech, not the record: records, envelopes,
declarations, and acts are Plumbline. The chain is the result of
Plumbline records after they pass through the court.

## The parts of speech

| Plumbline | linguistic role | pinned by |
|---|---|---|
| tag | word | the registry (IS-3): a grant table |
| granted range | dialect | a deed on the chain; retired ranges never reissue |
| record `tag ‖ len ‖ value` | utterance | IS-1 + conformance vectors |
| conformance vector | meaning | the behavior a conforming reader must perform |
| declaration (hello) | introduction | IS-5: identity, revisions spoken, holdings |
| claim envelope | assertion | tags 80–82, opaque in transit |
| attestation | signature line | `sig`, scheme byte, beside the envelope |
| act | performative | IS-6: an operation that changes chain state |
| skip-unknown | forwarding | unknown words are carried unchanged |

A Plumbline sentence is judged **settled or unsettled**, not true or
false. Settlement is defined across multiple dimensions.

## Multi-dimensional structure

The syntax is linear: bytes arrive in sequence. Each layer of meaning
above the byte is a space with more than one axis, and each axis is
enforced by a named test. The statements do not totally order, the
predicates are not scalar, and the vocabulary is not fixed at design
time.

### 1 · Predicates are vectors

A claim's credit is an `Extent`: one exact-rational component per
axis. Settlement requires closure on every axis of the priced space;
being correct on one axis and unfunded on another does not settle.
There is no aggregate score. `volume()` (the product across axes) is
not present in the code.
*Enforced:* `datum::extent`, `reward` multi-axis cover,
`tests/court_laws.rs (mod board)`.

### 2 · Nouns are regions

A deed is a **box**: one inclusive range per axis, padded to the
edge's dimensionality. A holding in Plumbline is an n-dimensional
region, and estates nest: a **moon** is an estate within an estate,
held at one greater depth (IS-6/3). Depth is a dimension of ownership.
*Enforced:* `isthmus::deed::Deed { region, within }`, containment
laws.

### 3 · New dimensions can open at runtime

`Act::Open` is a speech act that opens a new axis on a live edge.
Dimensionality is not a design-time constant: the board prices
11-axis space in tests, the master equation exercises 5-D estates, and
an axis opened later reads every earlier deed as pinned to its zero
slice.
*Enforced:* `Act::Open`, `Estate::Galaxy { axes, region }`,
`tests/estates.rs (mod master_equation)` (laws S · E · C · W · M).

### 4 · Time is a partial order

Acts on one chain fold horizontally; across chains there is no shared
clock. Each axis carries its own partial order, and two acts with no
path between them are **concurrent**. Plumbline is coordinator-free
because its time is plural.
*Enforced:* per-axis partial order in the deed laws;
`Hello::against` answering `Some(None)` for concurrent.

### 5 · Cross-chain reference: the vertical axis

`Act::Anchor` cites another chain by name, height, and digest,
providing a dimension across ledgers: what a chain knows of other
chains is a **frontier** (how far into each it has seen), and
multi-chain knowledge is a **sphere of frontiers**, not a merged log.
Plumbline can reference speech it does not carry.
*Enforced:* `isthmus::sphere`, `tests/estates.rs (mod vertical)`.

### 6 · Meaning varies by speaker-pair

Revisions compare for equality, never for order, so there is no single
current meaning, only meanings-in-common between pairs of speakers
(`shared_revisions`). Dialects coexist on one wire because
skip-unknown carries what it cannot read. The semantic space is
dimensioned by the participants, and a session is held to the
intersection.
*Enforced:* `isthmus::hello`, `sdk::attach::agree`, skip tests.

## One-sentence definition

> **Plumbline is a language whose utterances are linear bytes and
> whose meanings live in spaces: vector predicates, region nouns,
> axes that open at runtime, plural time, cross-chain reference, and
> pairwise semantics — each dimension held by a test that can fail.**

## Naming

The name is new; the constructs it names predate it. No tag, vector,
or revision string changes because the language now has a name. A
reader that has not encountered the name "Plumbline" interoperates
byte-for-byte with one that has, per the skip-unknown rule for unknown
words.
