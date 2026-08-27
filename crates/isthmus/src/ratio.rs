//! The exact rational, and the strictness the shared rule adopts.
//!
//! ```text
//! sign u8 ‖ LE32(len) ‖ numerator BE ‖ LE32(len) ‖ denominator BE
//! ```
//!
//! `IS-1` §3. Sign is 0 for non-negative and 1 for negative. Magnitudes
//! are big-endian and carry no leading zeros. **Zero's magnitude is the
//! empty string**, not a zero byte — that is the rule most first
//! implementations get wrong, and `IS-1` §9 vector V3 exists to catch it.
//!
//! ## Exact, and only exact
//!
//! There is no floating point anywhere on this wire and no rounding mode
//! to agree on. Two peers that disagree about a value disagree about
//! bytes, which is checkable; two peers that agree to within an epsilon
//! have agreed to nothing a third can verify.
//!
//! ## Refuse, never repair
//!
//! A decoder that silently reduces `2/4` accepts two byte strings for
//! one value. Everything above this layer takes an address over bytes,
//! so a second spelling of a value is a second address for it.
//!
//! One ancestor did repair. Adopting the strict rule tightens that
//! reader's acceptance and changes nothing either project emits — see
//! `datum/measure/ratio-strictness.md`.

use num_bigint::{BigInt, BigUint, Sign};
use num_rational::Ratio;
use num_traits::{One, Zero};

use crate::frame::{Malformed, Reader};

/// An exact rational.
///
/// This is `num_rational::Ratio<BigInt>` and not a wrapper around it,
/// deliberately: it is the same type both kernels already hold, so a
/// value crosses without a conversion. A conversion is a place a
/// canonical form is lost quietly.
pub type Exact = Ratio<BigInt>;

/// Big-endian magnitude with no leading zeros; empty for zero.
fn magnitude(value: &BigUint) -> Vec<u8> {
    if value.is_zero() {
        Vec::new()
    } else {
        value.to_bytes_be()
    }
}

/// Write an exact rational into a value buffer.
///
/// Encodes the canonical form and nothing else. `Ratio` normalises on
/// construction — denominator positive, terms reduced — so every byte
/// string this produces is one [`decode`] accepts.
pub fn put_ratio(value: &Exact, out: &mut Vec<u8>) {
    let negative = value.numer().sign() == Sign::Minus;
    out.push(u8::from(negative));

    for part in [value.numer().magnitude(), value.denom().magnitude()] {
        let bytes = magnitude(part);
        // A magnitude longer than u32::MAX bytes is not reachable from
        // any decoded value: `sized` refuses a declared length past the
        // record, so nothing that large ever arrived to be re-encoded.
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&bytes);
    }
}

/// Encode one exact rational as a standalone value.
pub fn encode(value: &Exact) -> Vec<u8> {
    let mut out = Vec::new();
    put_ratio(value, &mut out);
    out
}

/// Read one magnitude, refusing a leading zero byte.
fn take_magnitude(reader: &mut Reader<'_>) -> Result<BigUint, Malformed> {
    let bytes = reader.sized()?;
    if bytes.first() == Some(&0) {
        return Err(Malformed::LeadingZero);
    }
    Ok(BigUint::from_bytes_be(bytes))
}

/// Read an exact rational from the front of a reader.
///
/// Every refusal `IS-1` §4 permits is produced here, and the checks run
/// most-specific first so the refusal *names* what is wrong rather than
/// reporting the first rule that happens to catch it.
///
/// # Order of refusals
///
/// ```text
/// sign not in {0,1}         SignByte
/// leading zero byte         LeadingZero
/// denominator zero          ZeroDenominator
/// numerator zero, sign 1    NegativeZero        <- owed as IS-1/2
/// numerator zero, denom ≠ 1 NonCanonicalZero
/// gcd(numer, denom) ≠ 1     NotReduced
/// ```
///
/// The two zero rows come before the gcd check on purpose. `gcd(0, 5)`
/// is 5, so `0/5` would otherwise refuse as `NotReduced` and
/// `NonCanonicalZero` would be a variant no input can reach — a gate
/// that cannot fail, one level up in the type system.
///
/// The *verdict* is `refuse` either way, so this does not move
/// conformance. It moves which name the refusal carries.
pub fn take_ratio(reader: &mut Reader<'_>) -> Result<Exact, Malformed> {
    let sign = reader.u8()?;
    let negative = match sign {
        0 => false,
        1 => true,
        other => return Err(Malformed::SignByte(other)),
    };

    let numer = take_magnitude(reader)?;
    let denom = take_magnitude(reader)?;

    if denom.is_zero() {
        return Err(Malformed::ZeroDenominator);
    }

    if numer.is_zero() {
        if negative {
            return Err(Malformed::NegativeZero);
        }
        if !denom.is_one() {
            return Err(Malformed::NonCanonicalZero);
        }
        return Ok(Exact::zero());
    }

    let signed = BigInt::from_biguint(
        if negative { Sign::Minus } else { Sign::Plus },
        numer,
    );
    let positive = BigInt::from_biguint(Sign::Plus, denom);

    // `Ratio::new` reduces. If reducing moved either term, the bytes
    // were not canonical. The denominator is non-zero above, so this
    // cannot be the panicking case.
    let reduced = Ratio::new(signed.clone(), positive.clone());
    if reduced.numer() != &signed || reduced.denom() != &positive {
        return Err(Malformed::NotReduced);
    }

    Ok(reduced)
}

/// Decode a value that is exactly one exact rational and nothing else.
///
/// Refuses trailing bytes. A caller reading a ratio *inside* a larger
/// layout wants [`take_ratio`] instead — trailing bytes there are the
/// next field, not a defect.
pub fn decode(bytes: &[u8]) -> Result<Exact, Malformed> {
    let mut reader = Reader::new(bytes);
    let value = take_ratio(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
