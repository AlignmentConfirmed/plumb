# POW++ — the physics, the proofs, and the economics, as implemented

Every equation in this document is enforced by a named test. Where a
claim is analogy rather than implementation, it is marked as such. The
arithmetic throughout is $\mathbb{Q}$ with unbounded exact rationals
(`Ratio<BigInt>`); the engine contains no floats, and the isolation
test reads the source to enforce this.

---

## 1 · What the engine proves

The engine verifies three predicate families. Everything credited on
the ledger is an instance of one of them.

### 1.1 Domain 1 — closure of a boundary (PoWC)

A claim presents an oriented boundary: facets $(a_i, \sigma_i, \phi_i)$
where $a_i$ is an axis, $\sigma_i \in \{\text{Low}, \text{High}\}$ an
orientation, and $\phi_i \in \mathbb{Q}$ an exact flux. The engine
computes the per-axis divergence, which is a vector, not a scalar:

$$
D_a \;=\; \sum_{\substack{i:\, a_i = a \\ \sigma_i = \text{High}}} \phi_i
\;-\; \sum_{\substack{i:\, a_i = a \\ \sigma_i = \text{Low}}} \phi_i
$$

The claim closes iff $D_a = 0$ for every axis $a$, with every axis
carrying both faces (an incomplete axis is `Incomplete`, not zero).
This is the discrete divergence theorem applied in reverse: for a
region $V$ with boundary $\partial V$,

$$
\oint_{\partial V} F \cdot dA \;=\; \int_V (\nabla \cdot F)\, dV,
$$

so certifying $D_a = 0$ on every axis certifies a source-free
configuration: flux in equals flux out, per dimension, with
incommensurable axes never summed. The engine computes no total. The
code's comment records that a cross-axis total was written once and
removed, because adding fluxes across axes is the operation this crate
exists to refuse.

Gauge invariance is implemented. `Boundary::regauged` shifts every
facet on one axis by a constant $b$. The divergence on that axis
changes by

$$
\Delta D_a = b \times (\#\text{high faces} - \#\text{low faces}),
$$

so a re-gauge is unobservable iff the axis is balanced. This is the
discrete statement that closure is a gauge-invariant property of
balanced boundaries, and that a physical potential's zero point is not
an observable. *(Enforced: `assay::flux`, `convergence_laws`.)*

### 1.2 Domain 2 — admissible construction (PoUW, compiled)

A shape is a charged simple graph: $n$ orbs, edges
$\{(i,j,q_{ij})\}$ with $i < j$, $q_{ij} \in \mathbb{Q}\setminus\{0\}$,
no duplicates, no self-loops, at least one edge. The verified
predicate is well-formed constructibility. *(Enforced:
`assay::shape`; re-expressed in domain 3 by `complex::from_shape`
with verdict equivalence over the constructible corpus.)*

### 1.3 Domain 3 — cycles in a declared universe (the universal checker)

The general form, of which domains 1–2 are special cases. A claim
declares a finite chain complex as data: cell counts
$(n_0, \dots, n_K)$ and sparse boundary operators
$\partial_k : C_k \to C_{k-1}$ with entries in $\mathbb{Q}$, where
$C_k \cong \mathbb{Q}^{n_k}$ is the free module on the $k$-cells.
The engine verifies two things:

(i) The declaration is a complex:

$$
\partial_{k} \circ \partial_{k+1} \;=\; 0 \qquad \text{for all } k,
$$

checked by exact sparse multiplication. This is the fundamental
identity of homological algebra, that the boundary of a boundary is
zero, and the discrete analogue of $d^2 = 0$ in exterior calculus. A
declaration violating it refuses as `NotAComplex`: it is not a valid
universe, and no claim over it is meaningful.

(ii) The witness is a cycle: a sparse chain
$c = \sum_j c_j \cdot e_j \in C_k$ with

$$
\partial_k\, c \;=\; 0, \qquad\text{i.e.}\qquad c \in Z_k = \ker \partial_k .
$$

At every $(k{-}1)$-cell, the signed weighted flux of the witness
cancels exactly. For $k = 1$ this is Kirchhoff's current law: a
1-cycle is a conserved flow, with current in equal to current out at
every vertex. The simnet's clients submit Kirchhoff-conserving loop
currents on $n$-rings, each with a different $n$, each verified by
exact cancellation. An open chain refuses as `OpenBoundary{cell}`; the
refusal names the leaking cell.

*(Enforced: `assay::complex`, `tests/complex_laws.rs (mod declared)`,
`tests/market.rs (mod domains)`. The engine's fixed axioms are exactly (i) and
(ii) plus exactness and fuel; every geometry — hexagon, five-simplex,
or any other — is data.)*

---

## 2 · Identity: work is defined by its structure

$$
\mathrm{work\_id}(w) \;=\; \mathrm{canonical\_bytes}(w)\big|_{\text{transport}=0}
$$

Canonical form is enforced (sorted entries, nonzero coefficients, no
duplicates; non-canonical bytes refuse), so one structure maps to one
byte string, and identity is content-addressing: the same closure
under any nonce, route, or timestamp is the same work. This replaces
difficulty ladders: work is a mathematical object, and a given object
occurs once. *(Enforced: `work_id` tests in every domain; transport is
not part of identity but is present on the wire.)*

---

## 3 · The economics, as theorems

Credit lives in the extent monoid
$\mathcal{E} = \bigoplus_a \mathbb{N}$ (one component per axis),
ordered componentwise; there is no scalar collapse, and `volume()`
(the product across axes) was removed from the code.

The credit function:

$$
\kappa(w) \;=\;
\begin{cases}
(1,\dots,1) \in \mathbb{N}^{\#\text{axes}} & \text{domain 1, closed} \\
(1,\dots,1) \in \mathbb{N}^{\#\text{orbs}} & \text{domain 2, admitted} \\
(n_0, \dots, n_K) & \text{domain 3, cycle verified} \\
\varnothing \;(\text{no credit}) & \text{otherwise}
\end{cases}
$$

Domain 3's credit is the breadth of the universe the closure was
verified in: one component per dimension, the cell counts.

T1 (Soundness — no credit without closure). If `credit_claim(b)`
succeeds then the body parsed, verified by re-derivation, and
$\kappa \neq \varnothing$. Claims that do not close on every axis earn
nothing. *Proof: the single code path `credit_claim_inner` — parse →
`verifies()` → nonempty axes — with refusal tests in every domain
suite.*

T2 (At-most-once — no double-spend of work). For any book $B$ and
bodies $b_1, b_2$ with equal structure, $\mathrm{credit}(b_2)$ refuses
as `Replay` after $\mathrm{credit}(b_1)$, for any transports. *Proof:
T2 reduces to work_id equality (§2) plus the `seen` set; tested across
transports and across real TCP.*

T3 (Federated conservation). Books under `merge_acts_from` form an
idempotent, commutative, associative union keyed by work_id: a
grow-only set of credit acts. On any connected gossip topology, all
courts converge to the same book, and total credit is a function of
the set of distinct verified works, independent of how many times or
along which routes acts traveled. *Proof: merge refuses seen work_ids
(replay tests across sockets); convergence observed as byte-identical
XDCT snapshots on the simnet ring.* This is a conservation law in the
economic sense: gossip creates no value.

T4 (No inflation). Total credit on axis $a$ equals
$\sum_{w \in W} \kappa_a(w)$ over the set $W$ of distinct verified
works. By T1 every summand is backed by a verified closure; by T2/T3
each $w$ appears once. The money supply is the ledger of verified
mathematics.

T5 (Settlement solvency). `enact_if_funded` settles a claim on priced
space iff the credit stack covers the price componentwise:

$$
\text{settle} \iff \kappa_{\text{total}} \succeq p \quad
(\kappa_a \geq p_a \;\; \forall a).
$$

No axis subsidizes another: correct-but-unfunded settles nothing, and
funded-but-unclosed settles nothing (T1). *(Enforced:
`settle::join_with_work`, `covers`, board tests.)*

T6 (Priced computation). Every verification is fuel-metered — one unit
per multiply-accumulate or comparison — and total: `verify` returns
the exact cost $\mu(w)$ on success, and over-budget evaluation refuses
as `FuelExhausted{budget}`, naming the budget, with the budget
readable off a board price axis. Verification cost is therefore a
first-class economic quantity:
$\mu(w) = O(|\partial| + |c| \cdot \deg)$, linear in the sparse data.
*(Enforced: metered verify, `fuel_budget`, the over-budget test.)*

T7 (Attributed work — the signature layer). With enforcement on,
credit additionally requires an Ed25519 attestation over the BLAKE3
hash of the exact envelope bytes, by a key the chain binds to a holder
within an epoch window. Forged, stale, unbound, and rotated-away keys
each refuse by name; the check reads no payload, so carriers can
police admission without gaining an arbiter's power. *(Enforced:
`datum::admission`, over real TCP.)*

---

## 4 · The asymmetry that makes it work

A proof-of-work economy requires verification to be cheap and
production to be the real cost. Here the asymmetry is structural, not
tuned:

- Verification is a deterministic pass over sparse data:
  $O(|\text{entries}|)$ exact operations, metered (T6).
- Production is finding the object: exhibiting
  $c \in \ker \partial_k$ subject to the constraints the priced space
  and the declared universe impose. Unconstrained kernel elements are
  linear algebra; the constrained versions — prescribed support,
  minimal support, bounded coefficients, a prescribed relative
  boundary — include problems equivalent to exact integer feasibility
  and shortest-homologous-chain, which is NP-hard in general
  dimension. The market chooses instances by pricing space; the engine
  pays only for found-and-closed.

The asymmetry also prices efficiency: verification fuel is a
deterministic function of witness structure, so a more efficient
witness is a measured consensus fact, and the optimization market
(`IMPLEMENTATION.md` §6b) pays it — yield rebates on unspent budget at
discovery, standing refinement bounties on settled work, with
improvements appended as equivalences rather than rewrites.

Two consequences distinguish this from hash-grinding PoW:

1. Zero-variance reward. $\kappa(w)$ is a deterministic function of
   structure: no lottery, no expected-value mining, no variance to
   pool away.
2. The artifact outlives the payment. A hash preimage certifies only
   spent electricity; a verified cycle is the knowledge object — a
   conserved flow, a closure certificate, a homology witness —
   recorded, content-addressed, and reusable by anyone reading the
   chain.

---

## 5 · What "useful" means here

The engine proves closure, not usefulness. This is a deliberate
division of labor:

- The verifiable part — conservation, boundary annihilation, exact
  arithmetic — is universal: any discipline whose certificates can be
  phrased as a chain with prescribed boundary in a declared complex
  uses the same fixed evaluator. Kirchhoff flows and source-free flux
  configurations are in the suite today; the declared-domain mechanism
  (`Act::Declare`, IS-6/5) registers richer universes — routing
  certificates, matching witnesses, constraint systems over
  $\mathbb{Q}$ — without changing the binary.
- The value part is priced, not decreed. A registered universe is a
  vocabulary, not a truth (registration-is-not-trust is a test); the
  board's funded space is the network's only oracle of which closures
  are worth paying for. Usefulness is a market fact recorded on the
  ledger, matching the whitepaper's stated guarantee:
  verified-or-convergent, never useful-by-assertion.

The demo domains (rings, shapes, flux boxes) are small by design: they
are the calibration weights of the instrument, not its cargo.

---

## 6 · One claim, end to end

The simnet's 7-cycle, as the whole stack processes it:

$$
\partial\,e_i = v_{(i+1) \bmod 7} - v_i, \qquad
c = \sum_{i=0}^{6} e_i
\;\Rightarrow\;
\partial c = \sum_i \left(v_{i+1 \bmod 7} - v_i\right) = 0 .
$$

1. Physics — the telescoping sum cancels exactly in $\mathbb{Q}$: a
   conserved loop current (§1.3). Cost metered: ~28 operations (T6).
2. Identity — canonical bytes, transport zeroed: $\mathrm{work\_id}$
   (§2).
3. Attribution — Ed25519 over BLAKE3 of the envelope; the chain binds
   the key to `client-1` (T7).
4. Transit — the carrier forwards the envelope unread; the signature
   survives because it binds bytes, not routes.
5. Credit — $\kappa = (7, 7)$: seven vertices, seven edges of verified
   universe (T1). The copy refuses (T2).
6. Federation — every court in the ring converges on the same act,
   once (T3, T4).
7. Settlement — the credit stack covers priced space componentwise or
   it does not (T5).

The economy is a market in which the money supply is the set of
distinct, exactly-verified conservation certificates, attributed by
chain-bound keys, priced per axis, and not counterfeitable,
replayable, or inflatable by transport.
