//! X2 — the settlement receipt: what external rails settle against.
//!
//! A receipt is a court's signed, epoch-stamped statement that a
//! specific work settled against a specific query. Its whole design
//! constraint: **verifiable without a court** — a facilitator holding
//! only the receipt bytes and the public chain can check it, because
//! the signature verifies over the receipt's exact bytes and the
//! signer's key resolves to a bound holder through the chain
//! (`Act::Bind`), the same way a claim's attestation does.
//!
//! The receipt makes facilitator misbehavior **provable, not
//! impossible** — the custody boundary stated in the x402 ruling
//! stands; this is the narrow thing the facilitator must be trusted
//! about, made narrow.

use isthmus::deed::Ledger;

use crate::admission;
use crate::reward::Credit;

/// Why a receipt did not verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptRefused {
    /// The bytes are not a receipt.
    Malformed,
    /// The signature does not bind the signer to these bytes.
    Forged,
    /// No holder on the chain is bound to the signing key.
    Unbound,
    /// The signer is bound, but not to the court the receipt names —
    /// a court cannot sign in another court's name.
    NotThatCourt,
    /// The receipt's epoch falls outside the signer's bind window.
    Stale,
    /// A scheme this reader does not speak.
    UnknownScheme(u8),
}

/// The statement: work settled against a query, in an epoch, at a
/// court.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// The issuing court, as the chain names it.
    pub court: String,
    /// The epoch the settlement landed in.
    pub epoch: u64,
    /// The question this settles (X1).
    pub query_id: [u8; 32],
    /// The structure that settled it — content address of the work.
    pub work_id: Vec<u8>,
    /// The credit the settlement earned, per axis.
    pub axes: Vec<u128>,
}

impl Receipt {
    /// Canonical bytes — what the court signs.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let court = self.court.as_bytes();
        let len = u16::try_from(court.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(court.get(..usize::from(len)).unwrap_or(court));
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&self.query_id);
        let wl = u32::try_from(self.work_id.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&wl.to_le_bytes());
        out.extend_from_slice(&self.work_id);
        let n = u32::try_from(self.axes.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for axis in &self.axes {
            out.extend_from_slice(&axis.to_le_bytes());
        }
        out
    }

    /// Read canonical bytes back.
    pub fn decode(bytes: &[u8]) -> Result<Self, ReceiptRefused> {
        let mut at = 0usize;
        let take = |at: &mut usize, n: usize| -> Result<&[u8], ReceiptRefused> {
            let end = at.saturating_add(n);
            let piece = bytes.get(*at..end).ok_or(ReceiptRefused::Malformed)?;
            *at = end;
            Ok(piece)
        };
        let mut l2 = [0u8; 2];
        l2.copy_from_slice(take(&mut at, 2)?);
        let court = String::from_utf8(take(&mut at, usize::from(u16::from_le_bytes(l2)))?.to_vec())
            .map_err(|_| ReceiptRefused::Malformed)?;
        let mut l8 = [0u8; 8];
        l8.copy_from_slice(take(&mut at, 8)?);
        let epoch = u64::from_le_bytes(l8);
        let mut query_id = [0u8; 32];
        query_id.copy_from_slice(take(&mut at, 32)?);
        let mut l4 = [0u8; 4];
        l4.copy_from_slice(take(&mut at, 4)?);
        let work_id = take(&mut at, u32::from_le_bytes(l4) as usize)?.to_vec();
        let mut n4 = [0u8; 4];
        n4.copy_from_slice(take(&mut at, 4)?);
        let n = u32::from_le_bytes(n4) as usize;
        let mut axes = Vec::with_capacity(n.min(1 << 12));
        for _ in 0..n {
            let mut a = [0u8; 16];
            a.copy_from_slice(take(&mut at, 16)?);
            axes.push(u128::from_le_bytes(a));
        }
        if at != bytes.len() {
            return Err(ReceiptRefused::Malformed);
        }
        Ok(Self {
            court,
            epoch,
            query_id,
            work_id,
            axes,
        })
    }
}

/// A receipt with the court's signature over its exact bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedReceipt {
    /// The statement.
    pub receipt: Receipt,
    /// Ed25519/BLAKE3 over [`Receipt::encode`].
    pub attestation: sig::Attestation,
}

/// Issue a receipt for a settled credit, signed by the court's key.
#[must_use]
pub fn issue(
    court: &str,
    epoch: u64,
    query_id: [u8; 32],
    credit: &Credit,
    key: &sig::Keypair,
) -> SignedReceipt {
    let receipt = Receipt {
        court: court.to_owned(),
        epoch,
        query_id,
        work_id: credit.work_id.as_bytes().to_vec(),
        axes: credit.axes.components().to_vec(),
    };
    let attestation = key.attest(&receipt.encode());
    SignedReceipt {
        receipt,
        attestation,
    }
}

/// Verify a receipt against **chain state alone** — no court, no
/// book. The facilitator's whole check:
///
/// 1. the signature binds the signer to these exact bytes;
/// 2. the chain binds the signer's key to a holder;
/// 3. that holder IS the court the receipt names;
/// 4. the receipt's epoch is inside the bind window.
pub fn verify(signed: &SignedReceipt, chain: &Ledger) -> Result<(), ReceiptRefused> {
    let bytes = signed.receipt.encode();
    match signed.attestation.verify(&bytes) {
        Ok(()) => {}
        Err(sig::SigRefused::UnknownScheme(s)) => return Err(ReceiptRefused::UnknownScheme(s)),
        Err(_) => return Err(ReceiptRefused::Forged),
    }
    let holder =
        admission::holder_of_key(chain, signed.attestation.scheme, &signed.attestation.signer)
            .ok_or(ReceiptRefused::Unbound)?;
    if holder != signed.receipt.court {
        return Err(ReceiptRefused::NotThatCourt);
    }
    let binding = chain
        .binding_of(&holder)
        .ok_or(ReceiptRefused::Unbound)?;
    if signed.receipt.epoch < binding.from_epoch || signed.receipt.epoch > binding.until_epoch {
        return Err(ReceiptRefused::Stale);
    }
    Ok(())
}
