//! The value of the multi-axial settlement section at one cell (Phase 6,
//! #62): a credit in the grade's homology group
//!
//! ```text
//!   H_k(C) = ℤ^{b_k}  ⊕  (⊕_i ℤ/m_iℤ)
//! ```
//!
//! Free axes count in ℤ; torsion axes count in ℤ/m_iℤ, where the invariant
//! factors `m_i` come from the SNF we already extract
//! ([`crate::geometry::grade_shapes`], cached at `Act::Declare`). This is
//! the mathematics behind the section: the book is a section of the graded
//! homology bundle, and a cell's credit lives in the homology of its grade.
//!
//! **Accumulation is commutative and associative on every axis** — free
//! addition in ℤ and modular addition in ℤ/m_iℤ both are — so the settled
//! credit at a cell is **independent of the order** contributions arrive.
//! That order-independence is the convergence (A4) guarantee: disjoint
//! commits can run in parallel and every node still folds to the identical
//! section, with no global lock. Authority is the closure of each
//! contribution; convergence is this abelian, order-free accumulation.

use crate::geometry::GradeShape;
use std::collections::BTreeMap;

/// A credit valued in one grade's homology group `ℤ^free ⊕ (⊕ ℤ/m_iℤ)`.
/// Typed by the [`GradeShape`] of its grade, which supplies the free rank
/// and the torsion moduli `m_i`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxialCredit {
    /// Free axes — ℤ-valued, unbounded (saturating at the `i128` rails).
    free: Vec<i128>,
    /// Torsion axes — axis `i` is a residue in `[0, m_i)`.
    torsion: Vec<u64>,
    /// The torsion moduli `m_i` of this cell's grade, so a merge reduces
    /// without re-resolving the grade's shape. The section is **self-
    /// describing**: a relay (#72) holds only the section, never the claim
    /// that made it, and cannot re-derive `m_i` from a tag alone (a claim
    /// may embed its own universe rather than declare one on the chain).
    moduli: Vec<u64>,
}

impl AxialCredit {
    /// The zero credit for `shape`: 0 on every free and torsion axis — the
    /// additive identity of the grade's homology group.
    #[must_use]
    pub fn zero(shape: &GradeShape) -> Self {
        Self {
            free: vec![0; shape.free_rank],
            torsion: vec![0; shape.torsion.len()],
            moduli: shape.torsion.clone(),
        }
    }

    /// A raw contribution: `free` amounts in ℤ and `torsion` amounts (not
    /// yet reduced — [`AxialCredit::accumulate`] reduces them mod `m_i`).
    #[must_use]
    pub fn of(free: Vec<i128>, torsion: Vec<u64>) -> Self {
        Self {
            free,
            torsion,
            moduli: Vec::new(),
        }
    }

    /// The grade shape this cell carries — its free rank and torsion moduli.
    /// A merge reconstructs the shape from the cell itself, so no external
    /// resolver is needed.
    fn shape(&self) -> GradeShape {
        GradeShape {
            free_rank: self.free.len(),
            torsion: self.moduli.clone(),
        }
    }

    /// Accumulate `delta` into this credit under `shape`: free axes add in
    /// ℤ (saturating), torsion axis `i` adds in `ℤ/m_iℤ` (wraps modulo the
    /// invariant factor). Axes past either rank are ignored, so a
    /// malformed contribution can never panic — it simply cannot credit an
    /// axis the grade does not have.
    pub fn accumulate(&mut self, delta: &AxialCredit, shape: &GradeShape) {
        for (axis, &d) in delta.free.iter().enumerate() {
            if let Some(slot) = self.free.get_mut(axis) {
                *slot = slot.saturating_add(d);
            }
        }
        for (axis, &d) in delta.torsion.iter().enumerate() {
            let modulus = shape.torsion.get(axis).copied().unwrap_or(1).max(1);
            if let Some(slot) = self.torsion.get_mut(axis) {
                let sum = u128::from(*slot) + u128::from(d);
                // sum % modulus < modulus ≤ u64::MAX, so this never truncates.
                *slot = u64::try_from(sum % u128::from(modulus)).unwrap_or(0);
            }
        }
    }

    /// The free axes' ℤ values.
    #[must_use]
    pub fn free(&self) -> &[i128] {
        &self.free
    }

    /// The torsion axes' residues (axis `i` in `[0, m_i)`).
    #[must_use]
    pub fn torsion(&self) -> &[u64] {
        &self.torsion
    }

    /// True iff this is the additive identity — zero on every axis. A grade
    /// at zero carries no homology (present-but-zero equals never-deposited),
    /// which the section anchor relies on to stay canonical.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.free.iter().all(|v| *v == 0) && self.torsion.iter().all(|v| *v == 0)
    }
}

/// A grade coordinate: `(registered-domain tag, homological dimension)`.
/// Torsion is a property of a *grade's* homology (it appears only in the SNF
/// basis, never on an individual cell), so credit accumulates per grade —
/// the type-correct home of the free⊕torsion structure [`AxialCredit`]
/// carries. The `Section` stores these nested, so the domain is a first-class
/// base point rather than half of a flat key.
pub type GradeId = (u64, u32);

/// A domain's **stalk**: its graded-homology fibre — the grades (by
/// dimension) over one registered-domain base point, each with credit
/// accumulated in that grade's `H_k`. The first-class unit the sheaf routing
/// / sharding / anchor (#65, #72) operate on.
pub type Stalk = BTreeMap<u32, AxialCredit>;

/// The **multi-axial settlement section** (Phase 6, #62): the convergent
/// credit, a **sheaf of stalks** — `domain tag → (dimension → AxialCredit)`,
/// sparse at every level. A domain is a base point; its stalk is the graded
/// homology fibre over it; each grade's cell is an [`AxialCredit`] in
/// `H_k = ℤ^free ⊕ (⊕ ℤ/m_iℤ)`. Sparse maps at the identity levels (domain,
/// grade), dense `Vec` bases inside `AxialCredit` (the axes are a basis) —
/// the right representation per level, never a dense `Vec<Vec<Vec>>` tower
/// that would materialize the empty product space.
///
/// This is the book's convergence object (§6h): claims deposit in any order
/// and it **converges**, because [`AxialCredit::accumulate`] is commutative
/// and associative (confluent). Convergence over the torsion axes is
/// *guaranteed by finiteness* (`ℤ/m_iℤ`); the free axes grow (the market).
///
/// The section is the **value** layer of the guard/section split. The
/// monotonic exactly-once **guard** — which claims are already deposited —
/// is a *separate* layer that makes deposits idempotent (`⊕` is not), and
/// lives with the book's `seen` set. Guard for exactly-once, group for
/// any-order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    domains: BTreeMap<u64, Stalk>,
}

impl Section {
    /// An empty section.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Deposit `delta` into grade `(tag, dim)` (typed by `shape`).
    /// Order-independent: the settled section does not depend on the order
    /// deposits arrive — the convergence guarantee that lets disjoint
    /// commits run concurrently. The caller has passed the exactly-once guard.
    pub fn deposit(&mut self, (tag, dim): GradeId, shape: &GradeShape, delta: &AxialCredit) {
        self.domains
            .entry(tag)
            .or_default()
            .entry(dim)
            .or_insert_with(|| AxialCredit::zero(shape))
            .accumulate(delta, shape);
    }

    /// Merge another section into this one — the abelian union of two
    /// settled sections. Each cell carries its own torsion moduli, so the
    /// sum reduces correctly with no external resolver.
    ///
    /// This is the **group** half of the guard/section split (§6h): merging
    /// is commutative and associative, so nodes that settled disjoint work
    /// converge to the same section — the path-independent limit across
    /// nodes (#67), the operation a federation relay (#72) applies to a
    /// peer's [`Section::encode`] bytes. Exactly-once — idempotency under
    /// re-merge — is the **guard**'s job (the book's `seen` set), not the
    /// group's; a relay exchanges each peer's contribution once.
    pub fn merge(&mut self, other: &Section) {
        use std::collections::btree_map::Entry;
        for ((tag, dim), remote) in other.cells() {
            match self.domains.entry(tag).or_default().entry(dim) {
                Entry::Vacant(v) => {
                    // Absent here: adopt the peer's cell, already reduced and
                    // self-describing (it carries its own moduli).
                    v.insert(remote.clone());
                }
                Entry::Occupied(mut o) => {
                    // Present on both: the abelian sum, reduced per this
                    // cell's moduli.
                    let shape = o.get().shape();
                    let delta =
                        AxialCredit::of(remote.free().to_vec(), remote.torsion().to_vec());
                    o.get_mut().accumulate(&delta, &shape);
                }
            }
        }
    }

    /// The accumulated credit at grade `(tag, dim)`, if any.
    #[must_use]
    pub fn at(&self, (tag, dim): GradeId) -> Option<&AxialCredit> {
        self.domains.get(&tag)?.get(&dim)
    }

    /// A domain's whole stalk (its graded fibre), if the section carries any
    /// — the unit the sheaf routing / anchor operate on.
    #[must_use]
    pub fn stalk(&self, tag: u64) -> Option<&Stalk> {
        self.domains.get(&tag)
    }

    /// The number of domains (base points) the section spans.
    #[must_use]
    pub fn domains(&self) -> usize {
        self.domains.len()
    }

    /// The total number of grade cells across all domains.
    #[must_use]
    pub fn spanned(&self) -> usize {
        self.domains.values().map(BTreeMap::len).sum()
    }

    /// The domain stalks in canonical (tag) order.
    pub fn stalks(&self) -> impl Iterator<Item = (u64, &Stalk)> {
        self.domains.iter().map(|(&tag, stalk)| (tag, stalk))
    }

    /// Every `((tag, dim), credit)` in canonical order (tag, then dim) — the
    /// deterministic basis an order-independent anchor commits to (#65).
    pub fn cells(&self) -> impl Iterator<Item = (GradeId, &AxialCredit)> {
        self.domains
            .iter()
            .flat_map(|(&tag, stalk)| stalk.iter().map(move |(&dim, credit)| ((tag, dim), credit)))
    }

    /// An **order-independent commitment** to the settled section (#65): a
    /// BLAKE3 digest of the non-zero cells in canonical `(tag, dim)` order.
    ///
    /// This is the anchor re-based on the section (§6h). The old anchor
    /// hashed the *ordered act log* — order-**dependent**, so two nodes that
    /// merged the same contributions in different orders reached the same
    /// section yet **different** digests: phantom divergence. Here the digest
    /// is a function of the section *value*: because the section is
    /// order-independent and zero cells (the homology identity —
    /// present-but-zero equals never-deposited) are skipped, any two nodes
    /// that reached the same net section, in any order and via any reduction
    /// path, commit to the **same** anchor.
    #[must_use]
    pub fn anchor(&self) -> [u8; 32] {
        let mut buf = Vec::new();
        for ((tag, dim), credit) in self.cells() {
            if credit.is_zero() {
                continue; // present-but-zero == absent — keep the anchor canonical
            }
            buf.extend_from_slice(&tag.to_le_bytes());
            buf.extend_from_slice(&dim.to_le_bytes());
            let free = credit.free();
            buf.extend_from_slice(&u64::try_from(free.len()).unwrap_or(u64::MAX).to_le_bytes());
            for v in free {
                buf.extend_from_slice(&v.to_le_bytes());
            }
            let torsion = credit.torsion();
            buf.extend_from_slice(&u64::try_from(torsion.len()).unwrap_or(u64::MAX).to_le_bytes());
            for v in torsion {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        sig::envelope_hash(&buf)
    }

    /// Encode the section to durable bytes: every cell in canonical
    /// `(tag, dim)` order, so `decode(encode(s)) == s` exactly. This is the
    /// durable stalk store (#65) — a court can persist its converged
    /// settlement state and resume it — and the form a relay (#72) carries.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let cells: Vec<_> = self.cells().collect();
        let mut buf = Vec::new();
        buf.extend_from_slice(&(cells.len() as u64).to_le_bytes());
        for ((tag, dim), credit) in cells {
            buf.extend_from_slice(&tag.to_le_bytes());
            buf.extend_from_slice(&dim.to_le_bytes());
            let free = credit.free();
            buf.extend_from_slice(&(free.len() as u64).to_le_bytes());
            for v in free {
                buf.extend_from_slice(&v.to_le_bytes());
            }
            let torsion = credit.torsion();
            buf.extend_from_slice(&(torsion.len() as u64).to_le_bytes());
            for v in torsion {
                buf.extend_from_slice(&v.to_le_bytes());
            }
            buf.extend_from_slice(&(credit.moduli.len() as u64).to_le_bytes());
            for v in &credit.moduli {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf
    }

    /// Reconstruct a section from [`Section::encode`] bytes. Refuses a
    /// truncated, over-long, or trailing-bytes buffer rather than loading a
    /// partial section: a court resuming from a corrupt store must refuse,
    /// not silently forget — the same rule the book snapshot follows.
    pub fn decode(bytes: &[u8]) -> Result<Self, SectionBroken> {
        let mut p = 0usize;
        // Each cell is at least 36 bytes (tag 8, dim 4, three length
        // prefixes 8 each), so a count that cannot fit refuses before any
        // allocation.
        let count = rd_len(bytes, &mut p, 36)?;
        let mut domains: BTreeMap<u64, Stalk> = BTreeMap::new();
        for _ in 0..count {
            let tag = rd_u64(bytes, &mut p)?;
            let dim = rd_u32(bytes, &mut p)?;
            let free_len = rd_len(bytes, &mut p, 16)?;
            let mut free = Vec::with_capacity(free_len);
            for _ in 0..free_len {
                free.push(rd_i128(bytes, &mut p)?);
            }
            let torsion_len = rd_len(bytes, &mut p, 8)?;
            let mut torsion = Vec::with_capacity(torsion_len);
            for _ in 0..torsion_len {
                torsion.push(rd_u64(bytes, &mut p)?);
            }
            let moduli_len = rd_len(bytes, &mut p, 8)?;
            let mut moduli = Vec::with_capacity(moduli_len);
            for _ in 0..moduli_len {
                moduli.push(rd_u64(bytes, &mut p)?);
            }
            domains.entry(tag).or_default().insert(
                dim,
                AxialCredit {
                    free,
                    torsion,
                    moduli,
                },
            );
        }
        if p != bytes.len() {
            return Err(SectionBroken::TrailingBytes);
        }
        Ok(Self { domains })
    }
}

/// Why [`Section::decode`] refused a durable buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionBroken {
    /// The buffer ended in the middle of a field.
    Truncated,
    /// A length prefix declared more elements than the buffer can hold.
    Overlong,
    /// Bytes remained after a complete section decoded.
    TrailingBytes,
}

fn rd_u64(b: &[u8], p: &mut usize) -> Result<u64, SectionBroken> {
    let end = p.checked_add(8).ok_or(SectionBroken::Truncated)?;
    let slice = b.get(*p..end).ok_or(SectionBroken::Truncated)?;
    *p = end;
    Ok(u64::from_le_bytes(slice.try_into().unwrap_or([0; 8])))
}

fn rd_u32(b: &[u8], p: &mut usize) -> Result<u32, SectionBroken> {
    let end = p.checked_add(4).ok_or(SectionBroken::Truncated)?;
    let slice = b.get(*p..end).ok_or(SectionBroken::Truncated)?;
    *p = end;
    Ok(u32::from_le_bytes(slice.try_into().unwrap_or([0; 4])))
}

fn rd_i128(b: &[u8], p: &mut usize) -> Result<i128, SectionBroken> {
    let end = p.checked_add(16).ok_or(SectionBroken::Truncated)?;
    let slice = b.get(*p..end).ok_or(SectionBroken::Truncated)?;
    *p = end;
    Ok(i128::from_le_bytes(slice.try_into().unwrap_or([0; 16])))
}

/// Read a length prefix and refuse it if it cannot fit in the remaining
/// bytes (`stride` bytes per element), so a hostile buffer cannot force a
/// huge allocation before the element reads fail.
fn rd_len(b: &[u8], p: &mut usize, stride: usize) -> Result<usize, SectionBroken> {
    let n = usize::try_from(rd_u64(b, p)?).map_err(|_| SectionBroken::Overlong)?;
    let remaining = b.len().saturating_sub(*p);
    if n.saturating_mul(stride) > remaining {
        return Err(SectionBroken::Overlong);
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(free_rank: usize, torsion: &[u64]) -> GradeShape {
        GradeShape {
            free_rank,
            torsion: torsion.to_vec(),
        }
    }

    #[test]
    fn a_torsion_axis_counts_in_z_mod_m() {
        // Grade with one ℤ/4ℤ axis: m credits return to zero.
        let s = shape(0, &[4]);
        let mut c = AxialCredit::zero(&s);
        c.accumulate(&AxialCredit::of(vec![], vec![3]), &s);
        assert_eq!(c.torsion(), &[3]);
        c.accumulate(&AxialCredit::of(vec![], vec![3]), &s);
        assert_eq!(c.torsion(), &[2], "3 + 3 = 6 ≡ 2 (mod 4)");
        c.accumulate(&AxialCredit::of(vec![], vec![2]), &s);
        assert_eq!(c.torsion(), &[0], "2 + 2 = 4 ≡ 0 (mod 4): back to identity");
    }

    #[test]
    fn a_free_axis_counts_in_z_unbounded() {
        let s = shape(1, &[]);
        let mut c = AxialCredit::zero(&s);
        for _ in 0..3 {
            c.accumulate(&AxialCredit::of(vec![1_000_000_000], vec![]), &s);
        }
        assert_eq!(c.free(), &[3_000_000_000], "free axis does not wrap");
    }

    #[test]
    fn accumulation_is_order_independent_the_convergence_guarantee() {
        // The heart of A4 under parallel commit: a ⊕ b == b ⊕ a on both
        // free (ℤ) and torsion (ℤ/mℤ) axes, so the settled section does not
        // depend on which disjoint commit landed first.
        let s = shape(2, &[6, 5]);
        let a = AxialCredit::of(vec![7, -3], vec![4, 3]);
        let b = AxialCredit::of(vec![10, 8], vec![5, 4]);

        let mut ab = AxialCredit::zero(&s);
        ab.accumulate(&a, &s);
        ab.accumulate(&b, &s);

        let mut ba = AxialCredit::zero(&s);
        ba.accumulate(&b, &s);
        ba.accumulate(&a, &s);

        assert_eq!(ab, ba, "commit order cannot change the settled section");
        // And the values are correct: free adds in ℤ, torsion mod m.
        assert_eq!(ab.free(), &[17, 5]);
        assert_eq!(ab.torsion(), &[3, 2], "(4+5)%6=3, (3+4)%5=2");
    }

    #[test]
    fn accumulation_is_associative() {
        let s = shape(1, &[7]);
        let (a, b, c) = (
            AxialCredit::of(vec![2], vec![3]),
            AxialCredit::of(vec![5], vec![6]),
            AxialCredit::of(vec![1], vec![4]),
        );
        let mut left = AxialCredit::zero(&s); // (a ⊕ b) ⊕ c
        left.accumulate(&a, &s);
        left.accumulate(&b, &s);
        left.accumulate(&c, &s);
        let mut right = AxialCredit::zero(&s); // a ⊕ (b ⊕ c)
        let mut bc = AxialCredit::zero(&s);
        bc.accumulate(&b, &s);
        bc.accumulate(&c, &s);
        right.accumulate(&a, &s);
        // bc is already reduced; feeding it back accumulates identically.
        right.accumulate(&AxialCredit::of(bc.free().to_vec(), bc.torsion().to_vec()), &s);
        assert_eq!(left, right);
    }

    #[test]
    fn free_and_torsion_axes_are_orthogonal() {
        // A purely free contribution never perturbs a torsion axis, and
        // vice versa — the direct-sum decomposition, no cross-contamination.
        let s = shape(1, &[4]);
        let mut c = AxialCredit::zero(&s);
        c.accumulate(&AxialCredit::of(vec![9], vec![0]), &s);
        assert_eq!(c.free(), &[9]);
        assert_eq!(c.torsion(), &[0], "free credit left the torsion axis at 0");
        c.accumulate(&AxialCredit::of(vec![0], vec![3]), &s);
        assert_eq!(c.free(), &[9], "torsion credit left the free axis unchanged");
        assert_eq!(c.torsion(), &[3]);
    }

    #[test]
    fn the_section_converges_regardless_of_deposit_order() {
        // the #62 guarantee at the section level: the same deposits into the
        // same grades, in any order, fold to the identical section — so
        // disjoint commits parallelize and every node reaches one limit.
        let g0: GradeId = (100, 0); // domain 100, dim 0 — H_0 with a ℤ/6
        let g1: GradeId = (100, 1); // domain 100, dim 1 — free
        let g2: GradeId = (200, 0); // a different domain, orthogonal grade
        let s0 = shape(1, &[6]);
        let s1 = shape(2, &[]);
        let s2 = shape(0, &[4]);
        let deposits = [
            (g0, &s0, AxialCredit::of(vec![3], vec![4])),
            (g1, &s1, AxialCredit::of(vec![7, -2], vec![])),
            (g0, &s0, AxialCredit::of(vec![1], vec![5])),
            (g2, &s2, AxialCredit::of(vec![], vec![3])),
            (g1, &s1, AxialCredit::of(vec![10, 8], vec![])),
        ];

        let mut forward = Section::new();
        for (grade, shp, delta) in &deposits {
            forward.deposit(*grade, shp, delta);
        }
        let mut reverse = Section::new();
        for (grade, shp, delta) in deposits.iter().rev() {
            reverse.deposit(*grade, shp, delta);
        }
        assert_eq!(forward, reverse, "commit order cannot change the section");

        // And the values are the abelian sums, torsion reduced per grade.
        assert_eq!(forward.spanned(), 3);
        assert_eq!(forward.at(g0).map(AxialCredit::torsion), Some(&[3u64][..]), "(4+5)%6=3");
        assert_eq!(forward.at(g1).map(AxialCredit::free), Some(&[17i128, 6][..]));
        assert_eq!(forward.at(g2).map(AxialCredit::torsion), Some(&[3u64][..]));
    }

    #[test]
    fn a_grades_torsion_converges_to_identity_over_its_order() {
        // Convergence is guaranteed by finiteness: m deposits of a ℤ/mℤ
        // cycle return the grade to the additive identity.
        let g: GradeId = (1, 0);
        let s = shape(0, &[5]);
        let mut section = Section::new();
        for _ in 0..5 {
            section.deposit(g, &s, &AxialCredit::of(vec![], vec![1]));
        }
        assert_eq!(section.at(g).map(AxialCredit::torsion), Some(&[0u64][..]), "5 ≡ 0 (mod 5)");
    }

    #[test]
    fn disjoint_grades_do_not_interfere() {
        let s = shape(1, &[]);
        let mut section = Section::new();
        section.deposit((1, 0), &s, &AxialCredit::of(vec![5], vec![]));
        section.deposit((2, 0), &s, &AxialCredit::of(vec![9], vec![]));
        assert_eq!(section.at((1, 0)).map(AxialCredit::free), Some(&[5i128][..]));
        assert_eq!(section.at((2, 0)).map(AxialCredit::free), Some(&[9i128][..]));
        assert_eq!(section.at((3, 0)), None, "an untouched grade carries nothing");
    }

    #[test]
    fn a_domain_stalk_is_a_first_class_unit() {
        // The sheaf structure: a domain's grades form a stalk you can pull
        // out whole (for routing/anchoring), and domains are disjoint base
        // points. cells() flattens in a deterministic (tag, dim) order.
        let mut section = Section::new();
        section.deposit((100, 0), &shape(1, &[6]), &AxialCredit::of(vec![5], vec![4]));
        section.deposit((100, 1), &shape(2, &[]), &AxialCredit::of(vec![1, 2], vec![]));
        section.deposit((200, 0), &shape(1, &[6]), &AxialCredit::of(vec![7], vec![2]));

        assert_eq!(section.domains(), 2, "two base points");
        assert_eq!(section.spanned(), 3, "three grade cells total");
        assert_eq!(
            section.stalk(100).map(BTreeMap::len),
            Some(2),
            "H_0 and H_1 over domain 100"
        );
        assert!(section.stalk(300).is_none(), "an untouched domain has no stalk");

        let coords: Vec<GradeId> = section.cells().map(|(g, _)| g).collect();
        assert_eq!(
            coords,
            vec![(100, 0), (100, 1), (200, 0)],
            "canonical order: tag, then dim — the anchor basis"
        );
    }

    #[test]
    fn the_anchor_is_independent_of_deposit_order() {
        // #65: the anchor commits to the section value, so the same net
        // section reached in any order commits to the same digest — the fix
        // for the order-dependent act-log hash.
        let s0 = shape(1, &[6]);
        let s1 = shape(2, &[]);
        let s2 = shape(0, &[4]);
        let deposits = [
            ((100u64, 0u32), &s0, AxialCredit::of(vec![3], vec![4])),
            ((100, 1), &s1, AxialCredit::of(vec![7, -2], vec![])),
            ((200, 0), &s2, AxialCredit::of(vec![], vec![3])),
            ((100, 0), &s0, AxialCredit::of(vec![1], vec![5])),
        ];
        let mut forward = Section::new();
        for (grade, shp, delta) in &deposits {
            forward.deposit(*grade, shp, delta);
        }
        let mut reverse = Section::new();
        for (grade, shp, delta) in deposits.iter().rev() {
            reverse.deposit(*grade, shp, delta);
        }
        assert_eq!(
            forward.anchor(),
            reverse.anchor(),
            "same net section → same anchor, any order"
        );
    }

    #[test]
    fn the_anchor_skips_zero_cells() {
        // A grade whose torsion wrapped back to zero (5 deposits of ℤ/5) is
        // present with zero value, but must anchor the same as a section that
        // never touched it — present-but-zero == absent (homology identity).
        let s = shape(0, &[5]);
        let mut wrapped = Section::new();
        for _ in 0..5 {
            wrapped.deposit((1, 0), &s, &AxialCredit::of(vec![], vec![1]));
        }
        assert!(wrapped.at((1, 0)).is_some(), "the zero grade is present in the store");
        assert_eq!(
            wrapped.anchor(),
            Section::new().anchor(),
            "present-zero anchors as absent"
        );
    }

    #[test]
    fn different_sections_commit_to_different_anchors() {
        let s = shape(1, &[]);
        let mut a = Section::new();
        a.deposit((1, 0), &s, &AxialCredit::of(vec![5], vec![]));
        let mut b = Section::new();
        b.deposit((1, 0), &s, &AxialCredit::of(vec![6], vec![]));
        assert_ne!(a.anchor(), b.anchor(), "distinct settled value → distinct anchor");
    }

    #[test]
    fn the_codec_round_trips_a_section_and_its_anchor() {
        // #65: the durable stalk store. A court can persist its converged
        // section and resume it byte-for-byte, so the committed anchor is
        // the same after a restart as before.
        let mut s = Section::new();
        s.deposit((100, 0), &shape(1, &[6]), &AxialCredit::of(vec![5], vec![4]));
        s.deposit((100, 1), &shape(2, &[]), &AxialCredit::of(vec![1, -2], vec![]));
        s.deposit((200, 0), &shape(0, &[5]), &AxialCredit::of(vec![], vec![3]));

        let decoded = Section::decode(&s.encode()).expect("round-trip");
        assert_eq!(decoded, s, "decode(encode(s)) == s");
        assert_eq!(
            decoded.anchor(),
            s.anchor(),
            "the committed anchor survives the codec"
        );
    }

    #[test]
    fn the_empty_section_round_trips() {
        let s = Section::new();
        assert_eq!(Section::decode(&s.encode()), Ok(Section::new()));
    }

    #[test]
    fn decode_refuses_trailing_bytes() {
        let mut bytes = Section::new().encode();
        bytes.push(0xff);
        assert_eq!(Section::decode(&bytes), Err(SectionBroken::TrailingBytes));
    }

    #[test]
    fn decode_refuses_a_truncated_buffer() {
        let mut s = Section::new();
        s.deposit((1, 0), &shape(1, &[]), &AxialCredit::of(vec![5], vec![]));
        let bytes = s.encode();
        assert_eq!(
            Section::decode(&bytes[..bytes.len() - 3]),
            Err(SectionBroken::Truncated),
            "a court resumes from a whole store or refuses"
        );
    }

    #[test]
    fn the_section_limit_is_path_independent_across_nodes() {
        // #67: however the settlement work is partitioned across nodes, the
        // combined convergent section — and its committed anchor — is the
        // same as one node that saw all of it. Disjoint contributions ⊕ to
        // one limit, so merging is order- and partition-independent.
        let shape_of = |g: GradeId| -> GradeShape {
            match g {
                (100, 0) => shape(1, &[6]),
                (100, 1) => shape(2, &[]),
                (200, 0) => shape(0, &[5]),
                _ => shape(0, &[]),
            }
        };
        let all = [
            ((100u64, 0u32), AxialCredit::of(vec![3], vec![4])),
            ((100, 1), AxialCredit::of(vec![7, -2], vec![])),
            ((100, 0), AxialCredit::of(vec![1], vec![5])),
            ((200, 0), AxialCredit::of(vec![], vec![3])),
            ((200, 0), AxialCredit::of(vec![], vec![4])),
        ];

        // One node settles everything.
        let mut single = Section::new();
        for (g, d) in &all {
            single.deposit(*g, &shape_of(*g), d);
        }

        // Two nodes settle a disjoint partition (domain 100 vs 200), each in
        // its own order — then exchange and merge, as a federation would.
        let deposit = |section: &mut Section, indices: &[usize]| {
            for &i in indices {
                let (g, d) = &all[i];
                section.deposit(*g, &shape_of(*g), d);
            }
        };
        let mut node_a = Section::new();
        deposit(&mut node_a, &[2, 0, 1]); // domain 100, shuffled
        let mut node_b = Section::new();
        deposit(&mut node_b, &[4, 3]); // domain 200, shuffled

        let mut a_plus_b = node_a.clone();
        a_plus_b.merge(&node_b);
        let mut b_plus_a = node_b.clone();
        b_plus_a.merge(&node_a);

        assert_eq!(
            a_plus_b.anchor(),
            single.anchor(),
            "partition across nodes reaches the same committed anchor"
        );
        assert_eq!(
            b_plus_a.anchor(),
            single.anchor(),
            "and the merge is symmetric — either order reaches the one limit"
        );
        assert_eq!(a_plus_b, b_plus_a, "A⊕B == B⊕A as sections, not just anchors");
    }

    #[test]
    fn merge_is_the_group_sum_on_an_overlapping_grade() {
        // Two nodes crediting the SAME grade with DIFFERENT work sum there:
        // the grade's credit is the abelian sum, torsion reduced per grade.
        let shape_of = |_: GradeId| shape(1, &[6]);
        let mut a = Section::new();
        a.deposit((7, 0), &shape_of((7, 0)), &AxialCredit::of(vec![10], vec![4]));
        let mut b = Section::new();
        b.deposit((7, 0), &shape_of((7, 0)), &AxialCredit::of(vec![5], vec![5]));
        a.merge(&b);
        assert_eq!(a.at((7, 0)).map(AxialCredit::free), Some(&[15i128][..]), "10 + 5");
        assert_eq!(a.at((7, 0)).map(AxialCredit::torsion), Some(&[3u64][..]), "(4+5)%6=3");
    }

    #[test]
    fn decode_refuses_an_overlong_count_without_allocating() {
        // A count of u64::MAX cannot fit — refuse before allocating, never
        // loop toward it.
        let bytes = u64::MAX.to_le_bytes();
        assert_eq!(Section::decode(&bytes), Err(SectionBroken::Overlong));
    }
}
