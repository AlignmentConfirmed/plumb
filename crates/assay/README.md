# assay — the convergence engine

Given a manifold's oriented boundary, does the flux through it cancel?
If it cancels **on every axis**, the manifold closes, and this crate
mints `Upsilon` — the only proof of closure there is.

```text
isthmus  ->  nothing          superhighway (never imports assay)
kernels  ->  ASSAY            exits query this
assay    ->  nothing          multi-axial physics leaf
nodes    ->  assay + isthmus  produce / verify; wire re-derives
datum    ->  isthmus + assay + edges   court + lab
```

Foundation map: the lab's FOUNDATION.md. POW++: PoUW construct +
PoWC multi-axial settle. Portable claims: boundary (`domain=1`, n-axis
flux) and Shape (`domain=2`, rank-free orbs/edges — not a 2-D grid).
Master equation **(C)(W):** the lab's decide/linkage-estates.md.

## The five rules, and where each is enforced

| rule | enforced by |
|---|---|
| **1. Leaf node** — no `isthmus`, no `lith`, no `chitin`, no `path =` | `tests/isolation.rs`, reading `Cargo.toml` **and** every source file |
| **2. Exact rational arithmetic** — no floating point, no tolerance | `tests/isolation.rs`, reading the source for `f32`/`f64`/`EPSILON` |
| **3. Unfolded** — one component per axis, never collapsed | the return type: `Boundary::divergence` is an `Extent`, not a scalar |
| **4. Structural states, not gates** — no `bool` threshold | `Convergence` is an enum and open arms carry residue |
| **5. The witness** — zero-sized, unforgeable, no payload | `Upsilon(())`'s private field; the compiler refuses to build one outside this crate |

Rules 1 and 2 are checked by reading the source rather than by a lint,
because a lint can be `#[allow]`ed at the one site that needed it.

## The verdict is never a boolean

```rust
pub enum Convergence {
    Closed(Upsilon),                    // every axis cancels exactly
    Open { residue: Extent },           // ≥1 axis open — residue per axis
    Incomplete { axis, missing },       // one face only on an axis
    Unmeasured,                         // empty boundary — not closure
}
```

**There is no `Cancelling` / `Divergent` split.** A `+1` on one axis
against a `−1` on another is simply `Open` with residue `[+1, −1]`.
Summing axes to name a state was the fold this crate exists to refuse —
an engine that summed before comparing to zero would mint a witness for
an open manifold and be wrong in a way nothing downstream could see.

The substrate learned the same lesson one layer up: *zero holonomy does
not mean no distortion; it means the distortions cancel.*

`Unmeasured` is the other refusal worth naming. An empty boundary has
zero divergence on every axis, and minting a proof of closure from it
would be a gate that cannot fail.

## Why `Upsilon` cannot be forged

```rust
pub struct Upsilon(());
//                 ^^ private
```

A tuple struct with a private field cannot be constructed outside the
module that declares it, so no other crate can write `Upsilon(())` —
the compiler refuses.

To be exact about what that proves: the value is minted at run time
from a real measurement, and what the compiler enforces is the
**monopoly on minting**. Holding an `Upsilon` proves *this crate*
concluded closure. Anything stronger would be a claim wider than the
mechanism.

Being zero-sized is what makes it useless to forge by other means:
nothing to tamper with, no token to replay, nothing to serialise. It
cannot cross a wire, deliberately — a proof that could travel would be
a proof a peer could be handed rather than reach.

## Gauge invariance is conditional, and the condition is stated

Re-gauging an axis shifts every facet on it. The divergence changes by

```text
by × (high faces − low faces)
```

which is zero **exactly when the axis is balanced** — as a box is, with
one face at each end. On an axis whose faces have been subdivided
unevenly, the gauge is observable.

The first version of the law claimed invariance unconditionally and
failed against two high faces and one low. The claim was wider than the
mechanism, so the mechanism is what is documented. `Boundary::is_balanced`
is the predicate a caller asks when it needs the invariance.

## Running it

```
cargo test          # 14 laws
cargo clippy --all-targets
cargo doc --no-deps
```
