//! Are the destinations tunnel-limited when *writing*, or drive-limited?
//!
//! ```text
//! cargo run --release --example write-contention -- F:\Travel\Images I:\Travel\Images J:\Travel\Images
//! ```
//!
//! `examples/contention.rs` answered this for reads and the answer was unambiguous: the two
//! USB SSDs manage 724 and 703 MB/s alone and 360 each in company, so **reading, they are
//! limited by the shared USB tunnel rather than by themselves**. Writing has never been
//! measured the same way. Every write figure on this rig — the ~292 MB/s per destination in
//! `DESIGN.md`'s wall-clock table — was taken with three or four streams already running,
//! which describes contention and cannot say what one drive can do on its own.
//!
//! **That gap has a decision attached to it.** A wider USB tunnel is only worth having if
//! the drives can use one. If each SSD writes at ~300 MB/s alone, the tunnel is nowhere near
//! the constraint on the write pass and no dock or laptop changes anything; if they write at
//! 600+ alone and collapse in company, the tunnel binds and better hardware would pay.
//!
//! So: write to each destination alone, then to all of them at once, and report what each
//! one lost. **Alone-versus-together is the whole experiment**, exactly as it is for reads.
//!
//! **On sample size, and why it is deliberately modest.** A consumer SSD absorbs the first
//! several gigabytes into an SLC cache far faster than it can sustain, so a short probe
//! reports the cache and flatters the drive. This one writes enough to be steady and reports
//! per-window rates so a cache cliff is visible as a cliff — but it does not try to exhaust
//! the cache, because **a real run already does that**: phase 3 writes 187 GB to every
//! destination, and its per-destination rate is the honest sustained number. This probe
//! exists for the comparison the run cannot make, since a run always writes everywhere at
//! once and can never show a drive on its own.
//!
//! Writes only to destinations. Never to a camera card — see the guard below.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use photoday::cards;
use photoday::winio::write_through;

/// A realistic frame: the day's average is 48.5 MB.
const FRAME_BYTES: usize = 48 * 1024 * 1024;

/// Per device, per phase. Large enough to outlast a burst, small enough that the whole
/// sweep is a few minutes and does not put needless wear on the archive drives.
const SAMPLE: u64 = 12 * 1024 * 1024 * 1024;

/// How often to report during the solo phase. Short enough to show an SLC cliff.
const WINDOW: Duration = Duration::from_secs(10);

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
        "{} GB per device per phase, {} MB frames, write-through as phase 3 writes\n",
        SAMPLE / (1024 * 1024 * 1024),
        FRAME_BYTES / (1024 * 1024)
    );

    let mut alone = Vec::new();
    for root in &roots {
        println!("  alone  {}", root.display());
        let rate = write_sample(root, &frame, true);
        println!("      {rate:>27.0} MB/s\n");
        alone.push(rate);
    }

    println!("  together — every destination at once, as a real run does");
    let together: Vec<f64> = std::thread::scope(|scope| {
        let handles: Vec<_> = roots
            .iter()
            .map(|root| scope.spawn(|| write_sample(root, &frame, false)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    println!(
        "\n  {:<28} {:>10} {:>10} {:>10}",
        "", "alone", "together", "kept"
    );
    for ((root, a), t) in roots.iter().zip(&alone).zip(&together) {
        let label = root.to_string_lossy();
        let label = label.chars().take(28).collect::<String>();
        println!("  {label:<28} {a:>10.0} {t:>10.0} {:>9.0}%", t / a * 100.0);
    }
    println!(
        "\n  sum alone    {:>8.0} MB/s\n  sum together {:>8.0} MB/s",
        alone.iter().sum::<f64>(),
        together.iter().sum::<f64>()
    );
    println!(
        "\n  A device that keeps its solo rate in company is not sharing a constraint.\n  \
         One that halves is — and then a wider tunnel is worth having."
    );

    for root in &roots {
        cleanup(&probe_dir(root));
    }
}

/// Writes `SAMPLE` bytes and returns MB/s. Reports per-window rates when `verbose`, which
/// is how an SLC cache running out shows up as a cliff rather than hiding inside a mean.
fn write_sample(root: &Path, frame: &[u8], verbose: bool) -> f64 {
    let probe = probe_dir(root);
    std::fs::create_dir_all(&probe).expect("creating the probe directory");

    let overall = Instant::now();
    let mut window_start = Instant::now();
    let mut window_bytes = 0u64;
    let mut written = 0u64;
    let mut n = 0usize;

    while written < SAMPLE {
        write_through(&probe.join(format!("probe_{n:04}.CR3")), frame).expect("write-through");
        written += frame.len() as u64;
        window_bytes += frame.len() as u64;
        n += 1;

        if verbose && window_start.elapsed() >= WINDOW {
            println!(
                "      {:>5}s {:>21.0} MB/s",
                overall.elapsed().as_secs(),
                window_bytes as f64 / window_start.elapsed().as_secs_f64() / 1e6
            );
            window_start = Instant::now();
            window_bytes = 0;
        }
    }

    let rate = written as f64 / overall.elapsed().as_secs_f64() / 1e6;
    cleanup(&probe);
    rate
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
