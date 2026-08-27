# PROSPECTIVE — x402 payment rails on the settlement layer

**Status: PROSPECTIVE. Architecture only — nothing below is built.**
Depends on the signature layer ([`signatures.md`](signatures.md),
designed, not built) and on transport (open). The topological variant
additionally depends on
[`topological-cryptography.md`](topological-cryptography.md) clearing
its cryptanalytic bar.

## 0 · The division of labor

[x402](https://www.x402.org/) is the HTTP-native payment protocol: an
agent hits an API, receives `402 Payment Required`, signs a stablecoin
authorization (EIP-3009 `TransferWithAuthorization` for USDC), and the
server releases the resource. It solves **liquidity over web rails**.
It does not solve trust: in a standard x402 flow the server releases
data it never verified, and the client pays for answers that may be
hallucinated.

Landing x402 on plumb splits the problem along the seam this
repository already enforces:

- **x402 handles the fiat-pegged money.** No volatile layer-1 token to
  invent; agents pay in USDC over standard HTTP.
- **plumb handles the truth.** The money moves only if the math shows
  the work closed on every axis.
- **The signature layer handles identity.** Baseline Ed25519/BLAKE3;
  topological invariants as the successor scheme behind the same seam.

## 1 · The flow

### 1.1 Request and challenge

An agent requests a verifiable computation over plain HTTP. Instead of
answering, the server posts the problem to the **board** as priced
space (a bounty on n axes) and replies `402` with escrow instructions
in the payment-required body:

```json
{
  "price": "5000000",
  "token": "USDC",
  "escrow_facilitator": "0xFaci1itat0r…",
  "settlement_condition": "plumb claim settles for query_id",
  "query_id": "<work board reference>"
}
```

The agent signs the x402 authorization; the facilitator holds the
$5.00 in escrow. **The `settlement_condition` names a plumb settlement
event, not a server's promise** — that substitution is the entire
point.

### 1.2 Claim production and transit

Independent solvers race. A solver with a solution and its
deterministic execution trace signs the claim to bind identity for
payout:

- **Baseline (scheme 0x01):** Ed25519 over the BLAKE3 envelope hash,
  grant-bound per `signatures.md`.
- **Topological (scheme ≥0x02, PROSPECTIVE):** the solver applies
  their private deformation (braid conjugacy, internal gauge shift) to
  the content-addressed `work_id`; identity is the invariant, not a
  public key.

The signed claim travels as an opaque envelope over `isthmus`.
Carriers may check the signature/flux consistency **at the envelope
only** — spam control without payload access, which keeps carriers
carriers and not arbiters.

### 1.3 Epistemic settlement

At the court, verification is two deterministic checks:

1. **The epistemic check** — `assay` re-derives: the execution trace
   is run against the problem's constraints, and the claim either
   closes on every priced axis or earns nothing. Replay is refused
   structurally: `work_id` is content-addressed, so a copied answer is
   the same answer.
2. **The identity check** — the signature (curve or invariant)
   verifies against the live grant on the chain, binding the payout to
   the party the ledger authorized.

### 1.4 Release

On settlement, the ledger emits a **settlement receipt** — a signed,
chain-anchored statement that claim `work_id` settled against
`query_id` in epoch `E`. The facilitator verifies the receipt and
executes the EIP-3009 transfer on the payment chain (Base, Solana).
The solver's wallet is credited; the agent received an answer that was
verified before it was paid for.

## 2 · What plumb must add (and what it must not)

| piece | where | status |
|---|---|---|
| Board entry ↔ `query_id` correspondence | court (`board`) | board exists; the HTTP naming is new |
| Settlement receipt as a signed act | court + signature layer | needs S1–S7; receipt is a `RewardAct` projection |
| Envelope-level signature check for carriers | substrate edge | S5 |
| 402 challenge encoder / facilitator client | **outside the workspace** | deliberately: an HTTP server and an EVM/Solana client belong to a gateway crate, not to the substrate or the court |

The last row is a law, not a preference: `isthmus` imports nothing and
`datum` is imported by nothing. An x402 gateway is a **kernel-class
edge** — it attaches through the SDK, holds a grant, and translates
between HTTP/EIP-3009 and plumb envelopes. The court never learns
HTTP; the substrate never learns USDC.

## 3 · Trust boundaries, stated plainly

- **The facilitator is trusted with custody.** x402's escrow
  facilitator holds the authorization and executes the transfer. Plumb
  narrows what it must be trusted *about* — it can verify a settlement
  receipt instead of taking the server's word — but it remains a
  custodial third party. A facilitator that ignores receipts can still
  release or withhold funds wrongly; the receipt makes that
  **provable**, not impossible.
- **The payment chain is external.** EIP-3009 execution finality is
  Base/Solana's property, not plumb's. A reorg on the payment chain
  does not un-settle the claim; the two ledgers are anchored (the
  receipt cites the court chain), never merged.
- **Verified-or-convergent, not true** still rules: an x402 buyer of
  open-domain work is paying for convergence among independent
  solvers, and the challenge body should say which guarantee —
  re-derivation or convergence — the price buys.
- **The topological variant inherits its own bar.** Until
  `topological-cryptography.md` leaves PROSPECTIVE, x402 flows sign
  with scheme 0x01. Quantum-resistance claims wait for cryptanalysis,
  not marketing.

## 4 · Task list

| ID | task | done when |
|---|---|---|
| **X1** | `query_id` naming: board entry addressable from outside | board test naming a priced space by stable id |
| **X2** | Settlement receipt act: signed, epoch-stamped, exportable | receipt round-trip + verify-without-court test |
| **X3** | Gateway crate skeleton (kernel-class, SDK-attached): 402 challenge encode, receipt → EIP-3009 call | gateway attaches via `sdk::attach`, holds a grant |
| **X4** | Facilitator verification vector: receipt bytes + expected verdict | conformance vector a facilitator in any language can check |
| **X5** | Challenge body declares the guarantee (re-derivation vs convergence) | refusal test: unpriced guarantee refused |

X1–X2 are court work behind the signature layer (S1–S7 first). X3–X5
are gateway work and can proceed against stubs once X2's receipt
format is vectored.
