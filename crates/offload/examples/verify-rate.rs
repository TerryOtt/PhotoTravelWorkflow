//! What does the hash choice actually cost phase 3?
//!
//! ```text
//! cargo run --release --example verify-rate -- G:\Travel\Images\2022\2022-09-27
//! ```
//!
//! `examples/hash-rate.rs` measures hashes against memory, which answers a question
//! phase 3 never asks. **This measures the path phase 3 actually walks**: an unbuffered
//! read off a real device with the hash computed over each chunk before the next read is
//! issued.
//!
//! That serialization is the point. `winio::unbuffered_sha256` reads a chunk, hashes it,
//! then reads the next — nothing is in flight during the hash — so the effective rate is
//! `1/(1/read + 1/hash)` rather than `min(read, hash)`. On a device that reads faster
//! than the hash runs, the hash binds, and the design's "hashing disappears across cores"
//! reasoning (decision 15) does not apply *within* one destination's verify pass.
//!
//! Three algorithms, one device, same bytes:
//!
//! - **SHA-256** — what the tool uses, chosen on longevity (decision 17)
//! - **BLAKE3** — measured 2.2x faster in memory, rejected on longevity
//! - **XXH3** — not a candidate at all; a non-cryptographic checksum, here purely to
//!   bound the question. If a hash this fast cannot move the number, no hash can.
//!
//! The "read only" row is the ceiling: what this device gives with no hashing at all.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::Instant;

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const SECTOR: usize = 4096;

/// The tool's own request size, so this measures what phase 3 does rather than an
/// idealized version of it.
const CHUNK: usize = 16 * 1024 * 1024;

/// Enough to be steady on every device here without taking all night.
const SAMPLE: u64 = 4 * 1024 * 1024 * 1024;

fn main() {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: verify-rate <directory of CR3 files>");
        std::process::exit(2);
    };

    let files = cr3_files(&dir);
    if files.is_empty() {
        eprintln!("no CR3 files in {}", dir.display());
        std::process::exit(1);
    }

    println!(
        "{} GB per measurement, {} MiB unbuffered requests, from {}\n",
        SAMPLE / (1024 * 1024 * 1024),
        CHUNK / (1024 * 1024),
        dir.display()
    );

    // Wake the device so the first row does not pay for it — the lesson from the card
    // speed test, which billed a reader's wake-up to the transfer and chose wrongly.
    read_and_hash(&files, 256 * 1024 * 1024, Algorithm::None);

    println!("  {:<24} {:>9}  {:>8}", "algorithm", "MB/s", "vs SHA-256");

    let mut sha_rate = 0.0;
    for algorithm in [
        Algorithm::None,
        Algorithm::Sha256,
        Algorithm::Blake3,
        Algorithm::Xxh3,
    ] {
        let start = Instant::now();
        let read = read_and_hash(&files, SAMPLE, algorithm);
        let rate = read as f64 / start.elapsed().as_secs_f64() / 1e6;

        if algorithm == Algorithm::Sha256 {
            sha_rate = rate;
        }

        let relative = if sha_rate > 0.0 && algorithm != Algorithm::Sha256 {
            format!("{:.2}x", rate / sha_rate)
        } else if algorithm == Algorithm::Sha256 {
            "—".to_string()
        } else {
            "(ceiling)".to_string()
        };

        println!("  {:<24} {rate:>9.0}  {relative:>8}", algorithm.name());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Algorithm {
    /// Read only — the device's ceiling with nothing computed.
    None,
    Sha256,
    Blake3,
    Xxh3,
}

impl Algorithm {
    fn name(self) -> &'static str {
        match self {
            Self::None => "read only, no hash",
            Self::Sha256 => "SHA-256 (sha2, SHA-NI)",
            Self::Blake3 => "BLAKE3",
            Self::Xxh3 => "XXH3 (not a candidate)",
        }
    }
}

/// Read `limit` bytes unbuffered, hashing each chunk before the next read is issued —
/// exactly what the verify pass does.
fn read_and_hash(files: &[PathBuf], limit: u64, algorithm: Algorithm) -> u64 {
    use sha2::Digest as _;

    let layout = std::alloc::Layout::from_size_align(CHUNK, SECTOR).expect("a valid layout");
    // SAFETY: non-zero size; freed below exactly once.
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    // SAFETY: `raw` is valid for `CHUNK` initialized bytes.
    let buffer = unsafe { std::slice::from_raw_parts_mut(raw, CHUNK) };

    let mut sha = sha2::Sha256::new();
    let mut blake = blake3::Hasher::new();
    let mut xxh = xxhash_rust::xxh3::Xxh3::new();

    let mut total = 0u64;
    'outer: for path in files {
        let Ok(mut file) = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_NO_BUFFERING)
            .open(path)
        else {
            continue;
        };

        loop {
            let read = match file.read(buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };

            match algorithm {
                Algorithm::None => {}
                Algorithm::Sha256 => sha.update(&buffer[..read]),
                Algorithm::Blake3 => {
                    blake.update(&buffer[..read]);
                }
                Algorithm::Xxh3 => xxh.update(&buffer[..read]),
            }

            total += read as u64;
            if total >= limit {
                break 'outer;
            }
        }
    }

    // Consume the digests so nothing here can be optimized away as dead.
    std::hint::black_box((sha.finalize(), blake.finalize(), xxh.digest()));

    // SAFETY: allocated above with this exact layout.
    unsafe { std::alloc::dealloc(raw, layout) };
    total
}

fn cr3_files(dir: &PathBuf) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("reading the directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("cr3"))
        })
        .collect();
    files.sort();
    files
}
