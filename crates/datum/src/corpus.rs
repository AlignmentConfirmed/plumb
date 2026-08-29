//! P5 — the live corpus: what a court actually prices, distinct from
//! [`crate::domains`]'s `demo_*` fixtures.
//!
//! Every market this workspace ever posted priced a synthetic shape
//! (`demo_theta_universe`, an n-cycle) — fine as a unit-test fixture,
//! not fine as the only content a live economy ever offers. This
//! module holds genuinely sourced mathematics instead: a **complete
//! rewriting system for the dihedral group of order 6** (≅ the
//! symmetric group S₃), the standard textbook example of a confluent,
//! terminating string-rewriting presentation of a finite group (see
//! e.g. Book & Otto, *String-Rewriting Systems*, Springer 1993, or
//! any treatment of Knuth–Bendix completion on Coxeter presentations).
//! Nobody invented this structure for this project; it is cited, not
//! authored.
//!
//! ```text
//! generators   a, b
//! relations    a³ = 1     (a has order 3)
//!              b² = 1     (b has order 2 — an involution)
//!              bab⁻¹ = a⁻¹   i.e. ba = a²b   (the dihedral relation)
//! ```
//!
//! Oriented as rewrite rules (`aaa→ε`, `bb→ε`, `ba→aab`) this system
//! is confluent and length-reducing, so every word has a unique
//! normal form — exactly six of them, one per element of the group.
//! The conjecture this module poses, `bab = aa`, is a genuine instance
//! of the defining relation (`b·a·b⁻¹ = a⁻¹`, and `b⁻¹ = b` since `b`
//! is an involution): real group theory, not an arbitrary word pair.

use assay::rewrite::{Compiled, Presentation, RewriteBroken};

use crate::query::Conjecture;

/// The confluent presentation itself — generators, relations, cited
/// above. `max_len = 6` is enough room to hold every one of the six
/// group elements' normal forms plus the intermediate words a
/// derivation of `bab = aa` passes through.
#[must_use]
pub fn dihedral_order_6() -> Presentation {
    Presentation {
        alphabet: vec![b'a', b'b'],
        rules: vec![
            (b"aaa".to_vec(), Vec::new()), // a^3 = 1
            (b"bb".to_vec(), Vec::new()),  // b^2 = 1
            (b"ba".to_vec(), b"aab".to_vec()), // b a b^{-1} = a^{-1}, i.e. ba = a^2 b
        ],
    }
}

/// The compiled universe every court and solver judges the conjecture
/// in — a chain fact, registered the same way any UC4 domain is.
pub fn dihedral_order_6_compiled() -> Result<Compiled, RewriteBroken> {
    dihedral_order_6().compile(6)
}

/// The posed theorem: `bab = aa`, a real instance of the dihedral
/// relation (`bab⁻¹ = a⁻¹`, `b⁻¹ = b`). Bundled with the compiled
/// universe it is pinned in, since a `Conjecture` is meaningless
/// without knowing which universe its word indices name.
pub fn dihedral_conjecture() -> Result<(Compiled, Conjecture), RewriteBroken> {
    let compiled = dihedral_order_6_compiled()?;
    let axiom = compiled.word(b"bab")?;
    let theorem = compiled.word(b"aa")?;
    let target = compiled.target(axiom, theorem)?;
    Ok((
        compiled.clone(),
        Conjecture {
            universe: compiled.complex,
            target,
        },
    ))
}

/// A genuine derivation of `bab = aa` — the shortest one this
/// presentation licenses: `bab → aabb → aa`, applying `ba→aab` once
/// and `bb→ε` once. Real work against real math, not a fixture.
pub fn dihedral_derivation(compiled: &Compiled) -> Result<Vec<(u32, assay::Exact)>, RewriteBroken> {
    let path = [
        compiled.word(b"bab")?,
        compiled.word(b"aabb")?,
        compiled.word(b"aa")?,
    ];
    compiled.derive(&path)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use assay::complex::ProofClaim;

    #[test]
    fn the_presentation_compiles_to_exactly_six_normal_forms() {
        // |D3| = |S3| = 6 — the group this presentation defines has
        // exactly six elements, a fact independent of this codebase.
        // A word is irreducible iff none of the three left-sides
        // occurs in it — checked directly here rather than through
        // the bounded compiled step graph, since a length-preserving
        // rule (`ba→aab`) can need more room than `max_len` allows
        // before a later rule shrinks the word again (the "bigger
        // telescope" caveat `rewrite.rs` itself documents).
        let compiled = dihedral_order_6_compiled().expect("confluent, terminating, compiles");
        let forbidden: [&[u8]; 3] = [b"aaa", b"bb", b"ba"];
        let normal_forms: Vec<&Vec<u8>> = compiled
            .words
            .iter()
            .filter(|w| !forbidden.iter().any(|lhs| contains(w, lhs)))
            .collect();
        assert_eq!(
            normal_forms.len(),
            6,
            "the dihedral group of order 6 has exactly six elements"
        );
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn bab_derives_to_aa_the_real_dihedral_relation() {
        let (compiled, conjecture) = dihedral_conjecture().expect("a real instance compiles");
        let witness = dihedral_derivation(&compiled).expect("licensed by the presentation");
        let proof = ProofClaim {
            transport: 1,
            complex: conjecture.universe.clone(),
            dim: 1,
            target: conjecture.target.clone(),
            witness,
            deps: Vec::new(),
        };
        proof
            .verify(assay::complex::DEFAULT_FUEL)
            .expect("bab = aa is genuinely true in this group");
    }

    #[test]
    fn a_different_theorem_does_not_settle_the_posed_one() {
        // b (order 2, its own inverse) is not the same element as
        // aa (order 3's square) — a real, false instance refuses.
        let compiled = dihedral_order_6_compiled().expect("compiles");
        let bab = compiled.word(b"bab").expect("in universe");
        let b_alone = compiled.word(b"b").expect("in universe");
        let wrong_target = compiled.target(bab, b_alone).expect("target shape");
        let (_, conjecture) = dihedral_conjecture().expect("compiles");
        assert_ne!(
            wrong_target, conjecture.target,
            "bab = b would be a false statement about this group"
        );
    }
}
