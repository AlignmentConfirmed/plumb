# INVENTORY — every protocol in the environment

**Reconciled 2026-08-07** against live trees. Canonical status map:
[`../FOUNDATION.md`](../FOUNDATION.md). This file is an inventory, not
the foundation — when they disagree, FOUNDATION + code win.

Four columns decide whether a thing is a protocol or a habit:
**where it is defined**, **whether anything decodes it**, **whether it is
built or drafted**, and **whether an outsider could implement it from
the document alone**.

The last column is the one that matters for publishing, and almost
everything fails it.

## Substrate — shared, or meant to be

| protocol | where | built? | decoded by? | implementable from the doc? |
|---|---|---|---|---|
| `IS-1` WIRE | `datum/protocols` | **built** in `isthmus` | `isthmus::frame` | **yes** — §9 vectors |
| `IS-2` SESSION | `datum/protocols` | §7 **built**; §6 **OPEN** | `isthmus::session`, `datum::session` | partly |
| `IS-3` REGISTRY | `datum/protocols` | deeds **built**; doc table is rendering | `isthmus::deed`, `datum` registry tests | partly |
| `IS-4` WITNESS | `datum/protocols` | **drafted** | — | frame stated |
| `IS-5` HANDSHAKE | `datum/protocols` | frame **built** | `isthmus::hello` | yes — §7 vectors |
| `IS-6` CHAIN | `datum/protocols` | **built, live** | `isthmus::deed`, `datum::ledger` | yes — §7 act vectors |

`IS-6` was written last and existed longest. The authority's record has
been stored as `.tlv` since the founding, and its format was specified
**nowhere** — an outsider could read `IS-3` §5's grant table and could
not read or produce the record that table is a rendering of, so it
could verify nothing and append nothing. The gap widened when the
vertical (`Act::Anchor`, tag 8) was added to an undocumented codec, and
that is what forced the document.

`IS-1` is largely a *description of what both projects already do*, which
is why a relation crossed without changing either. That makes it cheap
to finish and easy to mistake for finished.

## The two meshes

| protocol | where | built? | carries |
|---|---|---|---|
| strand wire + tag registry | `strand/src/wire.rs` | **yes**, live | relations, manifolds, closures, witnesses, refusals, apertures |
| netstratum chronicle exchange | `netstratum/engine/mesh` | **yes**, live | `head`, `receive`, `authority`, `yield_authority`, `append_as` |

**Two mesh protocols, and only one has been measured carrying the other's
kernel.** They are not variants — netstratum's exchanges chronicle
segments and settles authority; strand's exchanges structure and
authorizes by position. Both are substrate-layer and neither knows the
other.

## Record

| protocol | where | built? | implementable from the doc? |
|---|---|---|---|
| `NS-1` DOP | `netstratum/protocols` | yes | **YES** — §3 publishes byte-exact vectors |
| `NS-3` VRP | `netstratum/protocols` | yes | §2.4 declares the receipt frame; used by `datum` to read 1713 stored ratios |
| `NS-5` MESH | `netstratum/protocols` | yes | partly |
| `arbor` | `xylarium/arbor` | yes | no document |

`NS-1` §3 was the only thing in the environment that passed the last
column outright, and `IS-1` §9 now copies it:

> *"The `event_bytes` given are the EXACT bytes hashed, so any
> independent implementation of §2.1 can verify every digest below from
> this document alone."*

That sentence is the standard. Everything published should meet it.

## Proof — POW++

| | where | built? | decoded by? |
|---|---|---|---|
| strand `pouw` | `strand/src/pouw` | yes (tollway) | domain path; tags 16–20 claimed — wire readers incomplete |
| netstratum work | `engine/solver`, `engine/solver-verify` | yes (tollway) | `verify_work(bytes)` |
| `assay` | `crates/assay` | **yes** — multi-axial closure leaf | in-process `assess`; portable `Claim` body (boundary domain) |
| highway work frames | `isthmus::work` | **yes** — tags 80/81 opaque | carrier classifies; **never verifies** |
| court credit | `datum::reward` | **yes** — per-axis; work_id-primary | re-derives via assay |

Both tollway engines verify useful work over ARC in **disjoint** formats.
Measured: neither reads the other's and neither misreads it (safe
failure). Portable Shape-domain PoUW on the highway remains a gap —
see FOUNDATION Wave C.

The strand barrier for *its* native witness is above the format —
`strand::pouw` private `Frame`/`Keying` — so third parties cannot
construct that type. The highway path does not require that type.

## Kernels — the exits

| | where | commensurable? |
|---|---|---|
| `lith` | `xylarium/lith` | **yes**, measured at FAR across the wire |
| netstratum's kernel | `netstratum/engine/{kernel,cfe,rfa-core}` | yes, by `netstratum − GROUND + vanishing == lith` |

Not a source of gaps. Recorded so the next reader does not go looking
here.

## Games

| | where | state |
|---|---|---|
| netstratum | `engine/conductor`, `clients/cli` | live |
| `orrery` | `xylarium/orrery` | a README and a `.git`. No code |

One participant has something to play.

## What this inventory says

Three things are **built and live**: the strand wire, the netstratum
chronicle exchange, and both proof systems. They do not know each other.

`IS-2` s7 is resolved and its rule implemented; s6 freshness remains the
one hole in the session.

`assay` **exists** (`crates/assay`) as the multi-axial physics leaf.
What does **not** yet exist: portable full Shape-domain PoUW as one
constructible highway type (Wave C). The handshake is `IS-5` —
frame implemented; full product session incomplete. `IS-4` is drafted —
observer / witness / watcher roles.

Three things are now **published to a standard an outsider could use**:
`NS-1` §3, `IS-1` §9 which copied it, and `IS-5` §7. Thirteen byte-exact
vectors in total, each produced by calling the codec and asserted
against the hex the document carries, so the two cannot drift apart
silently.
