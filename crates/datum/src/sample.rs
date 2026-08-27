//! A deterministic sampler.
//!
//! Several measurements here are rates over a population, and a rate
//! whose population changes between runs is not a measurement. `std`
//! has no seeded generator and `datum` should not take a dependency to
//! get one, so this is `splitmix64` — small enough to read, and fixed
//! by its seed on every machine.
//!
//! It is not for cryptography and nothing here pretends otherwise.

/// A fixed sequence, determined entirely by its seed.
pub struct Sampler {
    state: u64,
}

impl Sampler {
    /// The seed used by every measurement in this crate, so two runs
    /// of the same test see the same population.
    pub const SEED: u64 = 20_260_804;

    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The crate's standard population.
    pub fn standard() -> Self {
        Self::new(Self::SEED)
    }

    /// splitmix64. Every bit of the output depends on the whole state,
    /// so a low seed does not make a low first draw.
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A whole number in `low..=high`.
    ///
    /// Rejection-free and very slightly biased for ranges that do not
    /// divide the word — stated rather than hidden, because it does not
    /// matter for a collision rate and would matter for anything else.
    pub fn in_range(&mut self, low: i64, high: i64) -> i64 {
        if high <= low {
            return low;
        }
        let span = high.abs_diff(low).saturating_add(1);
        low.saturating_add((self.next() % span) as i64)
    }

    /// True with probability `numer / denom`.
    pub fn chance(&mut self, numer: u64, denom: u64) -> bool {
        denom != 0 && self.next() % denom < numer
    }

    /// A charge in `low..=high`, never zero — a zero-charge edge is a
    /// different experiment and would quietly inflate every collision
    /// rate by making holonomies vanish.
    pub fn charge(&mut self, low: i64, high: i64) -> i64 {
        for _ in 0..8 {
            let drawn = self.in_range(low, high);
            if drawn != 0 {
                return drawn;
            }
        }
        if high > 0 {
            high
        } else {
            low
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sampler;

    #[test]
    fn the_same_seed_gives_the_same_sequence() {
        let mut a = Sampler::new(7);
        let mut b = Sampler::new(7);
        for _ in 0..64 {
            assert_eq!(a.in_range(-100, 100), b.in_range(-100, 100));
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Sampler::new(1);
        let mut b = Sampler::new(2);
        let differs = (0..32).any(|_| a.in_range(0, 1_000) != b.in_range(0, 1_000));
        assert!(differs, "two seeds produced the same first 32 draws");
    }

    #[test]
    fn a_range_is_respected_and_a_charge_is_never_zero() {
        let mut s = Sampler::standard();
        for _ in 0..2_000 {
            let drawn = s.in_range(-5, 5);
            assert!((-5..=5).contains(&drawn), "{drawn} escaped its range");
            assert_ne!(s.charge(-5, 5), 0, "a charge came out zero");
        }
    }

    #[test]
    fn a_degenerate_range_returns_its_only_value() {
        let mut s = Sampler::standard();
        assert_eq!(s.in_range(3, 3), 3);
        assert_eq!(s.in_range(9, 2), 9, "an inverted range yields its low");
    }
}
