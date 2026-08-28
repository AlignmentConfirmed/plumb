//! X1 — the query: a demand-posed problem, addressable from outside.
//!
//! A query is what a PAYER fixes: the space demanded, the vocabulary
//! the answer must arrive under, the guarantee class the price buys,
//! and the opaque problem statement. Its identity is the BLAKE3 of
//! that canonical encoding — and deliberately **not** the funding
//! position, which grows as offers merge: a question keeps its name
//! while the pot fills.
//!
//! X5's law starts here: the guarantee is **declared, never
//! defaulted**. Re-derivation and convergence are different purchases
//! — one buys proof, the other buys agreement among independent
//! parties, and agreement is not correctness. An unknown guarantee
//! byte refuses; a market that let the guarantee be implied would be
//! selling two products under one price tag.

use isthmus::layout::Tag;

/// What the price buys. Declared by the poser, carried in the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guarantee {
    /// The answer verifies by re-derivation: proof.
    Rederivation,
    /// The answer settles by convergence among independent solvers:
    /// agreement, which is not correctness — and says so.
    Convergence,
}

impl Guarantee {
    fn byte(self) -> u8 {
        match self {
            Guarantee::Rederivation => 1,
            Guarantee::Convergence => 2,
        }
    }
}

/// Why a query refused to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryBroken {
    /// Bytes ended before the structure did.
    Truncated,
    /// Bytes continued after the structure ended.
    Trailing,
    /// A guarantee byte this market does not sell. Refused, never
    /// defaulted (X5).
    UnpricedGuarantee(u8),
    /// The poser's name was not text.
    BadText,
}

/// A demand-posed problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Who asks, as they name themselves.
    pub poser: String,
    /// The estate shape the answer must fund, one extent per axis.
    pub shape: Vec<u128>,
    /// The tag the answer arrives under (a registered domain, or a
    /// work tag).
    pub domain_tag: Tag,
    /// What the price buys — declared.
    pub guarantee: Guarantee,
    /// The problem statement, opaque here: a target boundary, a
    /// conjecture space, a corpus task digest — the vocabulary's own.
    pub statement: Vec<u8>,
}

impl Query {
    /// Canonical bytes — everything the POSER fixes, nothing that
    /// funding changes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.guarantee.byte());
        out.extend_from_slice(&self.domain_tag.to_le_bytes());
        let n = u32::try_from(self.shape.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for extent in &self.shape {
            out.extend_from_slice(&extent.to_le_bytes());
        }
        let poser = self.poser.as_bytes();
        let len = u16::try_from(poser.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(poser.get(..usize::from(len)).unwrap_or(poser));
        let sl = u32::try_from(self.statement.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&sl.to_le_bytes());
        out.extend_from_slice(&self.statement);
        out
    }

    /// The name the outside world addresses this problem by: BLAKE3
    /// of the canonical bytes. Stable across funding, transport, and
    /// time — a question is what it asks.
    #[must_use]
    pub fn query_id(&self) -> [u8; 32] {
        sig::envelope_hash(&self.encode())
    }

    /// Read canonical bytes back.
    pub fn decode(bytes: &[u8]) -> Result<Self, QueryBroken> {
        let mut at = 0usize;
        let take = |at: &mut usize, n: usize| -> Result<&[u8], QueryBroken> {
            let end = at.saturating_add(n);
            let piece = bytes.get(*at..end).ok_or(QueryBroken::Truncated)?;
            *at = end;
            Ok(piece)
        };
        let guarantee = match take(&mut at, 1)?.first().copied() {
            Some(1) => Guarantee::Rederivation,
            Some(2) => Guarantee::Convergence,
            Some(other) => return Err(QueryBroken::UnpricedGuarantee(other)),
            None => return Err(QueryBroken::Truncated),
        };
        let mut tag_bytes = [0u8; 8];
        tag_bytes.copy_from_slice(take(&mut at, 8)?);
        let domain_tag = Tag::from_le_bytes(tag_bytes);
        let mut n_bytes = [0u8; 4];
        n_bytes.copy_from_slice(take(&mut at, 4)?);
        let n = u32::from_le_bytes(n_bytes) as usize;
        let mut shape = Vec::with_capacity(n.min(1 << 12));
        for _ in 0..n {
            let mut e = [0u8; 16];
            e.copy_from_slice(take(&mut at, 16)?);
            shape.push(u128::from_le_bytes(e));
        }
        let mut l_bytes = [0u8; 2];
        l_bytes.copy_from_slice(take(&mut at, 2)?);
        let poser = String::from_utf8(take(&mut at, usize::from(u16::from_le_bytes(l_bytes)))?.to_vec())
            .map_err(|_| QueryBroken::BadText)?;
        let mut s_bytes = [0u8; 4];
        s_bytes.copy_from_slice(take(&mut at, 4)?);
        let statement = take(&mut at, u32::from_le_bytes(s_bytes) as usize)?.to_vec();
        if at != bytes.len() {
            return Err(QueryBroken::Trailing);
        }
        Ok(Self {
            poser,
            shape,
            domain_tag,
            guarantee,
            statement,
        })
    }
}

/// SQ4 — a conjecture: a query whose statement pins BOTH the universe
/// and the theorem. Without this, a posed question flattens to
/// "exhibit any closure" — the poser could not say WHICH boundary
/// they are paying to see closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conjecture {
    /// The universe the derivation must live in.
    pub universe: assay::complex::DeclaredComplex,
    /// The prescribed boundary the answer must close onto:
    /// `theorem − axioms`, canonical.
    pub target: Vec<(u32, assay::Exact)>,
}

impl Conjecture {
    /// Canonical statement bytes for [`Query::statement`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let universe = self.universe.encode();
        let ul = u32::try_from(universe.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&ul.to_le_bytes());
        out.extend_from_slice(&universe);
        let n = u32::try_from(self.target.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for (cell, coeff) in &self.target {
            out.extend_from_slice(&cell.to_le_bytes());
            assay::exact_codec::put_exact(coeff, &mut out);
        }
        out
    }

    /// Read a conjecture statement back. A statement that is not a
    /// conjecture is not an error — it is a plain universe statement,
    /// and the caller falls back to the closure question.
    pub fn decode(bytes: &[u8]) -> Result<Self, QueryBroken> {
        let mut at = 0usize;
        let take = |at: &mut usize, n: usize| -> Result<&[u8], QueryBroken> {
            let end = at.saturating_add(n);
            let piece = bytes.get(*at..end).ok_or(QueryBroken::Truncated)?;
            *at = end;
            Ok(piece)
        };
        let mut l4 = [0u8; 4];
        l4.copy_from_slice(take(&mut at, 4)?);
        let ul = u32::from_le_bytes(l4) as usize;
        let universe = assay::complex::DeclaredComplex::decode(take(&mut at, ul)?)
            .map_err(|_| QueryBroken::Truncated)?;
        let mut n4 = [0u8; 4];
        n4.copy_from_slice(take(&mut at, 4)?);
        let n = u32::from_le_bytes(n4) as usize;
        let mut target = Vec::with_capacity(n.min(1 << 12));
        let mut cursor = at;
        for _ in 0..n {
            let mut c4 = [0u8; 4];
            c4.copy_from_slice(bytes.get(cursor..cursor.saturating_add(4)).ok_or(QueryBroken::Truncated)?);
            cursor = cursor.saturating_add(4);
            let coeff = assay::exact_codec::take_exact(bytes, &mut cursor)
                .map_err(|_| QueryBroken::Truncated)?;
            target.push((u32::from_le_bytes(c4), coeff));
        }
        if cursor != bytes.len() {
            return Err(QueryBroken::Trailing);
        }
        Ok(Self { universe, target })
    }
}
