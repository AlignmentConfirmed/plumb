# PROSPECTIVE — topological cryptography (research track)

**Status: PROSPECTIVE. Architecture only — do not pretend it is
built, and do not deploy it.** The ratified baseline is Ed25519 +
BLAKE3 ([`signatures.md`](signatures.md)); this document specifies the
successor scheme that would enter through the scheme-agility seam
(scheme tag ≥ `0x02`) **if and only if** it survives independent
cryptanalysis. §5 states plainly why that bar is not yet met.

## 0 · Why plumb is the natural host

Replacing PKI with topological cryptography shifts identity,
authorization, and signatures from number-theoretic hardness (discrete
log, curve pairings) to algebraic topology: non-commutative group
actions and homological invariants. Identity stops being a certificate
and becomes **membership in an equivalence class under deformation**;
a signature stops being a curve point and becomes **a proof of valid
topological transport (boundary closure) across a combinatorial
manifold**.

This is not foreign vocabulary here. `assay` already verifies claims
as *boundary flux that closes on every axis* — exact chains whose
divergence cancels. A signature scheme whose verification predicate is
"the boundary closes" speaks the court's native language: the same
`∂∂ = 0` discipline, applied to identity instead of work.

## 1 · The primitives: topological hard problems

| | Traditional PKI | Topological |
|---|---|---|
| Private key | scalar `x ∈ Z_q` | braid / path / cochain `c ∈ C_k` |
| Public identity | point `P = x·G` | conjugate / boundary / homology class |
| Signature | `(r, s)` via Schnorr/ECDSA | non-trivial cycle / holonomy / isotopy witness |
| Verification | curve arithmetic, pairings | knot invariant / Smith Normal Form / gauge flux |

### 1a · Braid groups (Artin groups, non-commutative)

The braid group `B_n` on `n` strands, generators `σ_1 … σ_(n−1)`:

```
σ_i σ_j = σ_j σ_i                    |i − j| ≥ 2
σ_i σ_(i+1) σ_i = σ_(i+1) σ_i σ_(i+1)
```

**Hard problem — Conjugacy Search (CSP):** given `x ∈ B_n` and its
conjugate `y = w x w⁻¹`, recovering the conjugator `w` is hard when
`w` is drawn from a sufficiently large subgroup.

**Key agreement (Ko–Lee / Anshel–Anshel–Goldfeld):** Alice holds
private `a` in a left subgroup, Bob private `b` in a right subgroup
whose elements commute with the left's. Against public `x`, Alice
publishes `y_A = a x a⁻¹`, Bob publishes `y_B = b x b⁻¹`, and both
reach the shared secret `K = a b x b⁻¹ a⁻¹`.

### 1b · Homological obstruction (chain complexes)

A cell complex `K` with chain groups `C_k(K; Z)` and boundary
operators `∂_k : C_k → C_(k−1)` satisfying `∂∂ = 0`.

**Hard problem — shortest homologous chain:** finding an integral
chain `c` with `∂c = z` under strict norm/support constraints is
lattice-SVP/CVP-equivalent, NP-hard in higher dimensions.

**Identity as a homology class:** an identity is a non-trivial class
`[z] ∈ H_k(K; Z)`. The holder proves ownership by presenting a
cochain witness that evaluates to the published invariant without
revealing the minimal chain.

### 1c · Discrete gauge holonomy

A discrete flat connection on a 2-complex. The holonomy around closed
loops (discrete Wilson loops) is gauge-invariant. A valid agent applies
local gauge transformations — internal state shifts arbitrarily —
while every boundary holonomy (the publicly observable commitment)
stays fixed. Identity is *what survives your own transformations*.

## 2 · The pipeline, in plumb's terms

```
[ private state ]            [ transport ]                [ public claim ]
braid / interior cochain ──▶ conjugate / gauge-shift  ──▶ opaque envelope
                                                               │
                                                               ▼
[ settlement ledger ] ◀── verify invariant / SNF / flux ◀── [ substrate ]
credit claim              (boundary closure check)          carriers: flux
                                                            conservation only
```

**Identity generation** — no keypair. Against a global base complex
`K` (or standard braid alphabet), the agent generates a private word /
gauge configuration; the published identity is an invariant vector
(Smith Normal Form torsion invariants, Alexander polynomial
coefficients, or a commutator set `{w x_i w⁻¹}`).

**Signing = transport.** Hash the payload `M` deterministically to a
cycle `z_M` (or braid `x_M`); the signature is the private action on
it — `Σ_M = w x_M w⁻¹`, or the cochain sum `Ω_M = c_priv + δ(M)` — a
deformed representative that preserves the global invariant while
hiding the deformation.

**Verification = invariant equality.** No modular exponentiation:
check `I(Σ_M) = I(x_M)` for a polynomial-time invariant `I` (Garside
normal form, super summit representative, discrete Hodge
decomposition), or check boundary closure `∂Ω_M = z_expected` over `Z`.

## 3 · Module mapping

| module | Ed25519 baseline | topological |
|---|---|---|
| envelopes (`isthmus`) | carries 64-byte signature record | carries canonical braid word / connection frame |
| carriers | verify signature bytes vs pubkey | check flux conservation at vertices — **payload never decoded**, preserving the arbiter refusal |
| grant deeds (IS-3) | bind `holder_key` to tag range + epoch | bind a characteristic class (Euler class, homology torsion subgroup) to tag range + epoch |
| verification (`assay` / court) | curve arithmetic | SNF reduction over `Z`; polynomial invariant evaluation — the same exact-rational machinery the court already trusts |

The scheme enters through the seam `signatures.md` reserves: a scheme
tag ≥ `0x02`, admitted by chain act, refused by peers that do not
speak it. No wire break.

## 4 · Requirements any implementation must meet

1. **Canonical forms are mandatory.** Every representation reduces to
   a unique canonical form before verification — Shortlex/Garside
   normal form on braids, canonical SNF bases on chains. Without this,
   the scheme is malleable by construction: infinitely many words name
   one signature.
2. **Size honesty.** An Ed25519 pubkey is 32 bytes; a braid word or
   invariant vector spans hundreds of bytes to kilobytes of TLV. The
   wire carries it (LE32 lengths), but board pricing and session
   bounds must account for it.
3. **Determinism end to end.** Hash-to-cycle and hash-to-braid must be
   deterministic and canonical, or two honest verifiers disagree about
   `z_M` and the refusal is wrongly charged to the presenter.

## 5 · Cryptanalytic reality — why this is PROSPECTIVE

Stated with the same bluntness the rest of this repository uses about
its own gaps:

- **Braid-group cryptography has a broken history.** Naive Ko–Lee and
  AAG fell to Dehornoy reduction, Garside/super-summit conjugacy
  analysis, length-based attacks, and the Lawrence–Krammer linear
  representation. Any braid instantiation here must assume the
  attacker has those tools: braid index `n ≥ 80`, word lengths past
  minimal geodesic bounds, or — preferably — non-linear homological
  representations (twisted homology, discrete gauge constructions)
  rather than free braid conjugacy.
- **The homological and gauge constructions are younger than the
  attacks on braids** — which cuts both ways: fewer known breaks,
  and far less cryptanalytic attention. Absence of attacks is not
  evidence of strength.
- **The bar for leaving PROSPECTIVE:** a concrete parameter set,
  a canonical-form specification with test vectors, and published
  independent cryptanalysis — the same "genuinely independent reader"
  gate this project applies to its own wire documents, applied where
  the stakes are highest. Until then, scheme `0x01` (Ed25519/BLAKE3)
  signs everything that ships.

## 6 · Summary

- **Keys** become private structural deformations — braids, internal
  gauge shifts, cochains.
- **Signatures** become boundary-preserving topological transports.
- **Verification** becomes deterministic evaluation of homological
  invariants: `∂∂ = 0`, conjugacy normal forms, flat-connection
  holonomies.
- **In plumb**, this is not an aesthetic fit but a structural one: the
  court already settles claims by boundary closure. This track asks
  whether identity can be settled the same way — and holds deployment
  until cryptanalysis, not enthusiasm, answers.
