//! Decision 17's hashing measurement, made reproducible.
//!
//! The design rejects SHA3-256 and BLAKE3 on numbers measured on this laptop, and says
//! to re-run them whenever the crates or the machine change. Until this existed, nothing
//! in the repository could:
//!
//! ```text
//! cargo run --release --example hash-rate
//! ```
//!
//! **`--release` is not optional.** A debug build measures LLVM's opinion of unoptimized
//! code rather than the CPU's SHA extensions, and reports roughly a tenth of the truth —
//! which would read as a catastrophic regression and is only a wrong command.
//!
//! The two rejected algorithms are dev-dependencies, so they are measurable here and
//! absent from the shipped binary.

use std::fmt::Write as _;
use std::time::Instant;

/// The chunk phase 3 feeds the hasher from its in-memory buffer.
const CHUNK_BYTES: usize = 8 * 1024 * 1024;

/// 2 GiB in total — long enough that process start-up and the first cache fills stop
/// being visible in the rate.
const TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn main() {
    let chunk = pseudo_random_chunk();
    let rounds = (TOTAL_BYTES / CHUNK_BYTES as u64) as usize;

    println!(
        "{} MiB through each hasher in {} MiB chunks, single-threaded\n",
        TOTAL_BYTES / (1024 * 1024),
        CHUNK_BYTES / (1024 * 1024)
    );
    println!(
        "{:<8}  {:<18}  {:>12}  Digest",
        "Crate", "Algorithm", "Per core"
    );

    report("sha2", "SHA-256 (SHA-NI)", || {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for _ in 0..rounds {
            hasher.update(&chunk);
        }
        hasher.finalize().to_vec()
    });

    report("sha3", "SHA3-256", || {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        for _ in 0..rounds {
            hasher.update(&chunk);
        }
        hasher.finalize().to_vec()
    });

    report("blake3", "BLAKE3", || {
        let mut hasher = blake3::Hasher::new();
        for _ in 0..rounds {
            hasher.update(&chunk);
        }
        hasher.finalize().as_bytes().to_vec()
    });
}

/// Times one hasher over `TOTAL_BYTES` and prints its row.
///
/// The digest prefix is printed rather than dropped so the work cannot be optimized
/// away: a hash whose result nothing reads is a loop the compiler is entitled to delete,
/// and the failure mode of that is an implausibly good number rather than an error.
fn report(krate: &str, algorithm: &str, hash: impl FnOnce() -> Vec<u8>) {
    let start = Instant::now();
    let digest = hash();
    let elapsed = start.elapsed();

    // MB/s decimal, matching how the design states every other throughput figure —
    // storage rates in docs/DESIGN.md are the drives' own MB, not MiB.
    let rate = TOTAL_BYTES as f64 / elapsed.as_secs_f64() / 1_000_000.0;

    println!(
        "{krate:<8}  {algorithm:<18}  {:>7} MB/s  {}...",
        separated(rate.round() as u64),
        hex_prefix(&digest)
    );
}

/// Deterministic filler, xorshift64. Content cannot change any of these rates, but a
/// buffer of one repeated byte invites a memory subsystem to be cleverer than the real
/// workload — 45 MB of raw sensor data — ever lets it be.
fn pseudo_random_chunk() -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..CHUNK_BYTES)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// `2252` → `2,252`, per docs/WRITING.md rule 6, which covers program output.
///
/// Local to this example on purpose: RawGeotag's `count()` does this for the tool proper
/// and arrives with the engine lift (decision 17). Delete this then rather than leaving
/// two of them.
fn separated(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn hex_prefix(digest: &[u8]) -> String {
    let mut out = String::new();
    for byte in digest.iter().take(4) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
