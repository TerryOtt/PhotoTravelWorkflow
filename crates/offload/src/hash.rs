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
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finish()
}

/// A streaming hasher, so the verify pass can hash a chunk at a time.
///
/// **Normally this is SHA-256 and nothing else.** Under the `hash-experiments` feature
/// it dispatches on `OFFLOAD_HASH`, which exists to measure what decision 17's choice
/// costs — see `examples/verify-rate.rs`. The default build has no such branch and no
/// such dependencies.
#[allow(
    variant_size_differences,
    reason = "only the `hash-experiments` build has more than one variant, and boxing \
              `Sha256` to even them up would add an allocation per file on phase 3's hot \
              path — a cost to the primary metric, paid to tidy a measuring rig"
)]
pub enum Hasher {
    Sha256(Sha256),
    #[cfg(feature = "hash-experiments")]
    Blake3(Box<blake3::Hasher>),
    #[cfg(feature = "hash-experiments")]
    Xxh3(Box<xxhash_rust::xxh3::Xxh3>),
}

/// Written by hand rather than derived, for two reasons that point the same way.
/// `xxhash_rust::xxh3::Xxh3` implements no `Debug`, so a derive does not compile under
/// `hash-experiments` at all — and a partially consumed hasher's internal state is noise in
/// a panic message anyway. The algorithm is the only part a reader can act on.
impl std::fmt::Debug for Hasher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Sha256(_) => "Hasher::Sha256",
            #[cfg(feature = "hash-experiments")]
            Self::Blake3(_) => "Hasher::Blake3",
            #[cfg(feature = "hash-experiments")]
            Self::Xxh3(_) => "Hasher::Xxh3",
        })
    }
}

impl Hasher {
    #[cfg(not(feature = "hash-experiments"))]
    pub fn new() -> Self {
        Self::Sha256(Sha256::new())
    }

    #[cfg(feature = "hash-experiments")]
    pub fn new() -> Self {
        match std::env::var("OFFLOAD_HASH").unwrap_or_default().as_str() {
            "blake3" => Self::Blake3(Box::new(blake3::Hasher::new())),
            "xxh3" => Self::Xxh3(Box::new(xxhash_rust::xxh3::Xxh3::new())),
            _ => Self::Sha256(Sha256::new()),
        }
    }

    /// What this build is actually hashing with, for the report to say so.
    #[cfg(not(feature = "hash-experiments"))]
    pub fn name() -> &'static str {
        "sha256"
    }

    #[cfg(feature = "hash-experiments")]
    pub fn name() -> &'static str {
        match std::env::var("OFFLOAD_HASH").unwrap_or_default().as_str() {
            "blake3" => "blake3",
            "xxh3" => "xxh3",
            _ => "sha256",
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha256(hasher) => hasher.update(bytes),
            #[cfg(feature = "hash-experiments")]
            Self::Blake3(hasher) => {
                hasher.update(bytes);
            }
            #[cfg(feature = "hash-experiments")]
            Self::Xxh3(hasher) => hasher.update(bytes),
        }
    }

    pub fn finish(self) -> Digest32 {
        match self {
            Self::Sha256(hasher) => hasher.finalize().into(),
            #[cfg(feature = "hash-experiments")]
            Self::Blake3(hasher) => *hasher.finalize().as_bytes(),
            #[cfg(feature = "hash-experiments")]
            Self::Xxh3(hasher) => {
                // 128 bits widened into the 256-bit slot, so the manifest shape does not
                // change while the experiment runs. Not a candidate for real use.
                let mut digest = [0u8; 32];
                digest[..16].copy_from_slice(&hasher.digest128().to_be_bytes());
                digest
            }
        }
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
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

    /// The hand-written `Debug` names the algorithm and nothing else.
    ///
    /// Written by hand because `xxhash_rust::xxh3::Xxh3` implements no `Debug`, so a derive
    /// does not compile under `hash-experiments` at all. This asserts the deliberate half:
    /// a partially consumed hasher's internal state is noise in a panic message, and the
    /// algorithm is the only part a reader can act on. The default build has one variant,
    /// and `OFFLOAD_HASH` is unset under test, so both builds answer the same here.
    #[test]
    fn the_debug_impl_names_the_algorithm_rather_than_the_state() {
        assert_eq!(format!("{:?}", Hasher::new()), "Hasher::Sha256");
    }

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
