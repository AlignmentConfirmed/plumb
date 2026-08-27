//! Exact rational bytes shared by boundary and shape claims.
//!
//! ```text
//! sign u8 ‖ LE32(len) ‖ numer BE ‖ LE32(len) ‖ denom BE
//! ```

use num_bigint::{BigInt, BigUint, Sign};
use num_rational::Ratio;
use num_traits::{One, Zero};

use crate::Exact;

/// Why an exact field was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactBroken {
    /// Buffer ended mid-field.
    Truncated,
    /// Sign byte not 0 or 1.
    SignByte(u8),
    /// Leading zero in a magnitude.
    LeadingZero,
    /// Denominator is zero.
    ZeroDenominator,
    /// Negative zero.
    NegativeZero,
    /// Non-reduced fraction.
    NotReduced,
}

/// Write one exact rational.
pub fn put_exact(value: &Exact, out: &mut Vec<u8>) {
    let negative = value.numer().sign() == Sign::Minus;
    out.push(u8::from(negative));
    for part in [value.numer().magnitude(), value.denom().magnitude()] {
        let bytes = if part.is_zero() {
            Vec::new()
        } else {
            part.to_bytes_be()
        };
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&bytes);
    }
}

/// Read one exact rational.
pub fn take_exact(bytes: &[u8], at: &mut usize) -> Result<Exact, ExactBroken> {
    let sign = take_u8(bytes, at)?;
    if sign > 1 {
        return Err(ExactBroken::SignByte(sign));
    }
    let numer = take_magnitude(bytes, at)?;
    let denom = take_magnitude(bytes, at)?;
    if denom.is_zero() {
        return Err(ExactBroken::ZeroDenominator);
    }
    if numer.is_zero() && sign == 1 {
        return Err(ExactBroken::NegativeZero);
    }
    if numer.is_zero() {
        if !denom.is_one() {
            return Err(ExactBroken::NotReduced);
        }
        return Ok(Ratio::from_integer(BigInt::from(0)));
    }
    let n = BigInt::from_biguint(
        if sign == 1 {
            Sign::Minus
        } else {
            Sign::Plus
        },
        numer,
    );
    let d = BigInt::from_biguint(Sign::Plus, denom);
    let reduced = Ratio::new(n.clone(), d.clone());
    if reduced.numer() != &n || reduced.denom() != &d {
        return Err(ExactBroken::NotReduced);
    }
    Ok(reduced)
}

/// Read a `u8`.
pub fn take_u8(bytes: &[u8], at: &mut usize) -> Result<u8, ExactBroken> {
    let b = bytes.get(*at).copied().ok_or(ExactBroken::Truncated)?;
    *at = at.saturating_add(1);
    Ok(b)
}

/// Read a little-endian `u32`.
pub fn take_u32(bytes: &[u8], at: &mut usize) -> Result<u32, ExactBroken> {
    let slice = take_bytes(bytes, at, 4)?;
    let mut arr = [0u8; 4];
    arr.copy_from_slice(slice);
    Ok(u32::from_le_bytes(arr))
}

/// Read a little-endian `u64`.
pub fn take_u64(bytes: &[u8], at: &mut usize) -> Result<u64, ExactBroken> {
    let slice = take_bytes(bytes, at, 8)?;
    let mut arr = [0u8; 8];
    arr.copy_from_slice(slice);
    Ok(u64::from_le_bytes(arr))
}

/// Read `n` bytes.
pub fn take_bytes<'a>(bytes: &'a [u8], at: &mut usize, n: usize) -> Result<&'a [u8], ExactBroken> {
    let end = at.checked_add(n).ok_or(ExactBroken::Truncated)?;
    let slice = bytes.get(*at..end).ok_or(ExactBroken::Truncated)?;
    *at = end;
    Ok(slice)
}

fn take_magnitude(bytes: &[u8], at: &mut usize) -> Result<BigUint, ExactBroken> {
    let len = take_u32(bytes, at)? as usize;
    let slice = take_bytes(bytes, at, len)?;
    if slice.first() == Some(&0) {
        return Err(ExactBroken::LeadingZero);
    }
    Ok(BigUint::from_bytes_be(slice))
}
