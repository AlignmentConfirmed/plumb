# DECIDED — the universal checker: a fixed engine, domains as data

**Status: DECIDED 2026-08-27, priority ruled AHEAD of the beta
network.** The beta ships on the checker, not before it — testers
onboarding onto the compiled-domain engine would onboard onto the
legacy path.

## The thesis

The Rust binary is not the intelligence. It is the invariant
proof-checker — fixed axioms, like ZF set theory or the reduction
rules of the λ-calculus. If the binary had to be recompiled to
introduce a new concept or a higher-order geometry, the system would
fail as an epistemic substrate. The engine stays strictly fixed while
the mathematical universe atop it expands dynamically over the wire.

```text
┌──────────────────────────────────────────────────────────┐
│                 EVOLVING NETWORK LAYER                   │
│  new geometries      extended dialects      axiom packs  │
│  (declared complexes)  (IS-3 tag grants)   (rule vectors)│
└─────────────────────────────┬────────────────────────────┘
                              │ evaluated over the wire
                              ▼
┌──────────────────────────────────────────────────────────┐
│              IMMUTABLE PLUMB ENGINE (RUST)               │
│  1. boundary closure over declared complexes (∂∂ = 0)    │
│  2. exact-rational / integer reduction, no floats        │
│  3. bounded, deterministic evaluation (fuel, size)       │
└──────────────────────────────────────────────────────────┘
```

## What is already true (do not rebuild it)

- **The arithmetic discipline.** Everything is `Ratio<BigInt>` or
  integers; refuse-not-repair; no floats anywhere, enforced by tests
  that read the source. The engine already rejects continuous space
  and its transcendentals — you never square a circle here.
- **The carrier layer.** Skip-unknown + tag grants already let new
  vocabularies travel without recompiling carriers. The substrate
  needs nothing from this decision.
- **Closure-not-semantics.** assay's verdict is already "does the
  divergence cancel on every axis" — it does not know what a flux
  means. The *shape* of the engine is right; its *scope* is wrong.

## The gap this closes

Today the court can only verify what assay compiles: domain 1
(boundary flux) and domain 2 (Shape). A third domain is a Rust edit
and a redeploy of every verifier — exactly the failure the thesis
names. Judgment rules on shapes are compiled (`Shape::admit`), and the
tag-51 defect — a hexagon crossing as a five-simplex — is what happens
when geometry is assumed by the reader instead of carried as data.

After this: a geometry is fully specified by incidence matrices,
boundary operators, and exact metric weights **in the claim payload
or a chain-registered definition**. The engine checks that the
submitted chain/cochain closes against the declared structure. New
disciplines register over the wire; no code deploys.

## Guardrails (the dangers, priced in)

1. **An axiom pack is code by another name.** Every declared
   evaluation is fuel- and size-bounded, deterministic, and total —
   refusal, never divergence. Bounds are priced on the board like any
   other axis.
2. **Declared ≠ trusted.** A domain definition on the chain is a
   vocabulary, not a truth: claims under it still settle by
   re-derivation/convergence. Registering nonsense buys the right to
   have nonsense refused expensively.
3. **The legacy path stays until equivalence is measured.** Compiled
   domains 1 and 2 remain live until the declared re-expression of
   Shape produces the same verdicts on the whole existing corpus —
   a test, not a migration memo.

## Task ladder (UC — universal checker)

| ID | task | done when |
|---|---|---|
| **UC1** | Declared-complex codec: incidence matrices, boundary operators, exact weights as data (no geometry structs) | round-trip vectors; a hexagon and a five-simplex are distinct bytes |
| **UC2** | The fixed evaluator in assay: submitted chain closes against a declared complex (∂∂ = 0, conservation per axis), exact arithmetic, fuel-bounded | closure accepts, non-closure refuses, fuel exhaustion refuses — all named |
| **UC3** | Domain 3 claim body: declared-complex reference + witness chain; content-addressed `work_id` | replay refuses across transports |
| **UC4** | Domain registration on the chain: definition published + bound to a tag grant; courts resolve tag → definition from chain state | a node learns a new discipline from the chain alone, no rebuild |
| **UC5** | Shape re-expressed as a declared complex; verdict-equivalence against compiled domain 2 | equivalence suite green; closes the tag-51 defect class |
| **UC6** | Bounds priced: fuel/size as board axes on declared-domain space | over-budget evaluation refuses with the price named |

## Consequence for the roadmap

Phase D (beta network) is **blocked on UC1–UC4**: genesis, grants, and
the onboarding kit describe the declared-domain engine, not the
legacy one. UC5–UC6 may land during beta. ROADMAP.md carries the
reordering.
