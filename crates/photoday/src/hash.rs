//! The one hash function, and the one spelling of it.
//!
//! SHA-256, chosen on measurement and on longevity (decision 17): the `sha2` crate
//! selects its SHA-NI backend at runtime, and `sha256sum` ships with every operating
//! system — so a person in 2031 holding one of these disks and no copy of this tool can
//! still recompute a manifest by hand. BLAKE3 measured 2.2x faster and lost on exactly
//! that point.

use sha2::{Digest, Sha256};

/// A SHA-256 digest as bytes.
///
/// Compared and carried as bytes rather than as a hex `String`, so a comparison cannot
/// silently fail on case and no allocation happens per file. Hex is a rendering, and
/// [`hex`] is where it happens — at the manifest and report boundary, once.
pub type Digest32 = [u8; 32];

/// The canonical hash of a photo, taken from the buffer phase 3 already holds.
pub fn sha256(bytes: &[u8]) -> Digest32 {
    Sha256::digest(bytes).into()
}

/// Lowercase hex, the form the manifest and `sha256sum` both use.
pub fn hex(digest: &Digest32) -> String {
    use std::fmt::Write as _;

    digest
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned against the published SHA-256 of the empty input and of `abc`. These are
    /// the two vectors every implementation is checked against, and they prove the
    /// wiring — crate, byte order and hex spelling — rather than merely that some hash
    /// came out.
    #[test]
    fn the_published_test_vectors_match() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// Hex is lowercase and always 64 characters, including for a digest with a leading
    /// zero byte — where a naive formatter drops the pad and produces 63.
    #[test]
    fn hex_is_lowercase_and_zero_padded_to_sixty_four_characters() {
        let rendered = hex(&sha256(b"abc"));
        assert_eq!(rendered.len(), 64);
        assert_eq!(rendered, rendered.to_lowercase());

        let mut leading_zero: Digest32 = [0xff; 32];
        leading_zero[0] = 0x00;
        leading_zero[1] = 0x0a;
        assert!(hex(&leading_zero).starts_with("000a"));
        assert_eq!(hex(&leading_zero).len(), 64);
    }
}
