//! Are the destinations tunnel-limited when *writing*, or drive-limited?
//!
//! ```text
//! cargo run --release --example write-contention -- F:\Travel\Images I:\Travel\Images J:\Travel\Images
//! ```
//!
//! `examples/contention.rs` answered this for reads and the answer was unambiguous. Writing
//! has never been measured the same way: every write figure on this rig — the ~292 MB/s per
//! destination in `DESIGN.md`'s wall-clock table, and the ~400 the real run achieved — was
//! taken with several streams already running, which describes contention and cannot say
//! what one drive does on its own.
//!
//! **That gap has a purchase decision behind it.** A wider path is only worth having if the
//! drives can use one. If each SSD writes ~300 MB/s alone, the path is nowhere near the
//! constraint on the write pass; if they write 600+ alone and collapse together, it binds.
//!
//! # This probe was wrong once, and the fix is the interesting part
//!
//! The first version measured every device solo in sequence, then all together, deleting
//! its sample after each phase. On 2026-08-04 it reported **every device keeping more than
//! 100 % of its solo rate in company** — 110 %, 105 %, 107 % — which is impossible: a device
//! cannot write faster because competitors were added. Its untouched control drifted 24 %
//! between two runs.
//!
//! **The cause was ordering, not arithmetic.** A device measured solo was sampled
//! immediately after prior activity, but by the time the together phase ran it had enjoyed a
//! minute of idle while the other devices took their turns — time to process TRIM and catch
//! up on garbage collection. The together phase systematically got the rested drive.
//!
//! Three things follow, and all three are load-bearing:
//!
//! - **Alternate the conditions** rather than batching them, so neither is systematically
//!   favored by what came before it.
//! - **Repeat, and report the spread.** A single number cannot say whether it is
//!   reproducible, and a 24 % swing must announce itself rather than being averaged into a
//!   confident-looking mean.
//! - **Settle between samples.** A drive that has just absorbed gigabytes is still working;
//!   measuring it then measures the backlog.
//!
//! **On sample size.** A consumer SSD absorbs the first several gigabytes into an SLC cache
//! far faster than it can sustain, so a short probe flatters the drive. This one does not
//! try to exhaust the cache, because **a real run already does**: phase 3 writes ~200 GB to
//! every destination, and its per-destination rate is the honest sustained figure. This
//! probe exists for the comparison a run cannot make, since a run always writes everywhere
//! at once and can never show a drive alone.
//!
//! Writes only to destinations. Never to a camera card — see [`refuse_camera_card`].

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use photoday::cards;
use photoday::winio::write_through;

/// A realistic frame: the day's average is 48.5 MB.
const FRAME_BYTES: usize = 48 * 1024 * 1024;

/// Per device, per sample. Small enough that alternating conditions several times stays
/// within a few minutes and does not put needless wear on the archive drives.
const SAMPLE: u64 = 4 * 1024 * 1024 * 1024;

/// How many times each condition is measured. Two would show disagreement; three says which
/// of the two was the outlier.
const ROUNDS: usize = 3;

/// Idle time before each sample, so a drive is measured rather than its garbage-collection
/// backlog. The single most important line in this file — without it the conditions are not
/// comparable, which is exactly how the first version produced an impossible result.
const SETTLE: Duration = Duration::from_secs(20);

fn main() {
    let roots: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        eprintln!("usage: write-contention <destination dir> [<destination dir> ...]");
        std::process::exit(2);
    }
    for root in &roots {
        refuse_camera_card(root);
    }

    let frame: Vec<u8> = (0..FRAME_BYTES)
        .map(|i| (i.wrapping_mul(31) % 251) as u8)
        .collect();

    println!(
        "{} GB per device per sample · {ROUNDS} rounds · {}s settle · write-through, as phase 3 writes",
        SAMPLE / (1024 * 1024 * 1024),
        SETTLE.as_secs()
    );
    println!("conditions alternate, so neither is systematically handed a rested drive\n");

    let mut alone: Vec<Vec<f64>> = vec![Vec::new(); roots.len()];
    let mut together: Vec<Vec<f64>> = vec![Vec::new(); roots.len()];

    for round in 1..=ROUNDS {
        // Alternating which condition leads, so any residual order effect cancels across
        // rounds instead of accumulating in one column.
        if round % 2 == 1 {
            measure_alone(&roots, &frame, round, &mut alone);
            measure_together(&roots, &frame, round, &mut together);
        } else {
            measure_together(&roots, &frame, round, &mut together);
            measure_alone(&roots, &frame, round, &mut alone);
        }
    }

    println!(
        "\n  {:<26} {:>18} {:>18} {:>7}",
        "", "alone", "together", "kept"
    );
    for (i, root) in roots.iter().enumerate() {
        let label: String = root.to_string_lossy().chars().take(26).collect();
        let (a, a_spread) = median_and_spread(&alone[i]);
        let (t, t_spread) = median_and_spread(&together[i]);
        println!(
            "  {label:<26} {a:>8.0} ±{a_spread:<8.0} {t:>8.0} ±{t_spread:<8.0} {:>6.0}%",
            t / a * 100.0
        );
    }

    println!(
        "\n  Medians of {ROUNDS} rounds, ± half the observed range. **If a ± overlaps the gap\n  \
         between the two columns, the difference is not measured — it is noise.**\n  \
         A device keeping its solo rate in company is not sharing a constraint; one that\n  \
         halves is, and then a wider path is worth having.\n\n  \
         Any 'kept' above 100% means the conditions still were not comparable. Do not\n  \
         explain it away — it is the signature this probe was rewritten to remove."
    );

    for root in &roots {
        cleanup(&probe_dir(root));
    }
}

fn measure_alone(roots: &[PathBuf], frame: &[u8], round: usize, into: &mut [Vec<f64>]) {
    for (i, root) in roots.iter().enumerate() {
        settle();
        let rate = write_sample(root, frame);
        println!(
            "  round {round}  alone     {:<26} {rate:>7.0} MB/s",
            short(root)
        );
        into[i].push(rate);
    }
}

fn measure_together(roots: &[PathBuf], frame: &[u8], round: usize, into: &mut [Vec<f64>]) {
    settle();
    let rates: Vec<f64> = std::thread::scope(|scope| {
        let handles: Vec<_> = roots
            .iter()
            .map(|root| scope.spawn(|| write_sample(root, frame)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for (i, (root, rate)) in roots.iter().zip(&rates).enumerate() {
        println!(
            "  round {round}  together  {:<26} {rate:>7.0} MB/s",
            short(root)
        );
        into[i].push(*rate);
    }
}

/// Let the drives finish what the last sample started.
fn settle() {
    std::thread::sleep(SETTLE);
}

/// Writes `SAMPLE` bytes and returns MB/s, cleaning up after itself.
fn write_sample(root: &Path, frame: &[u8]) -> f64 {
    let probe = probe_dir(root);
    std::fs::create_dir_all(&probe).expect("creating the probe directory");

    let start = Instant::now();
    let mut written = 0u64;
    let mut n = 0usize;
    while written < SAMPLE {
        write_through(&probe.join(format!("probe_{n:04}.CR3")), frame).expect("write-through");
        written += frame.len() as u64;
        n += 1;
    }
    let rate = written as f64 / start.elapsed().as_secs_f64() / 1e6;

    cleanup(&probe);
    rate
}

/// Median, and half the range as a spread — which is what says whether a difference between
/// two columns is a finding or a coincidence.
fn median_and_spread(samples: &[f64]) -> (f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let spread = (sorted[sorted.len() - 1] - sorted[0]) / 2.0;
    (median, spread)
}

fn short(root: &Path) -> String {
    root.to_string_lossy().chars().take(26).collect()
}

fn probe_dir(root: &Path) -> PathBuf {
    root.join("_write-probe")
}

/// **Never a camera card.** `CONOPS.md` makes in-camera formatting the only way a card is
/// written to by anything, and that binds diagnostics as well as the tool.
///
/// The check lives in [`cards::is_on_camera_card`] rather than here because a copy in each
/// probe is how one of them drifts — which is exactly what happened. See that function for
/// the two incidents this prevents.
fn refuse_camera_card(root: &Path) {
    if cards::is_on_camera_card(root) {
        eprintln!(
            "refusing to write to {} — it is on a camera card, and nothing here writes to those (see docs/CONOPS.md). Point this at a destination instead.",
            root.display()
        );
        std::process::exit(2);
    }
}

fn cleanup(probe: &Path) {
    if let Err(error) = std::fs::remove_dir_all(probe)
        && probe.exists()
    {
        eprintln!("could not clean up {}: {error}", probe.display());
    }
}
