//! Shared helpers. Tests may panic; the library may not.

/// The layout the ancestors already speak: `tag u8 ‖ LE32(length)`.
///
/// Every test that frames a record names a layout, because a record has
/// no shape without one. That is the point of the change that introduced
/// this: the header is a structure, so somebody has to say which.
#[allow(dead_code)]
pub fn founding() -> isthmus::layout::Layout {
    isthmus::layout::Layout::founding()
}

/// Parse lowercase hex with no separators.
#[allow(dead_code)] // not every test file needs both helpers
pub fn hex(text: &str) -> Vec<u8> {
    let chars: Vec<char> = text.chars().collect();
    let pairs = chars.chunks_exact(2);
    assert!(
        pairs.remainder().is_empty(),
        "hex string has an odd length"
    );
    pairs
        .map(|pair| {
            let s: String = pair.iter().collect();
            u8::from_str_radix(&s, 16).expect("not hex")
        })
        .collect()
}

/// Render as lowercase hex with no separators, matching `IS-1` §9.
#[allow(dead_code)]
pub fn show(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
