//! What `--jobs` is worth in phase 5 (decision 15), measured rather than assumed.
//!
//! ```text
//! cargo run --release --example geotag-rate -- C:\Travel\GPX\2024-10-02.gpx
//! ```
//!
//! **Why this exists.** `--jobs` was parsed and never read until 2026-08-07; phase 5 was a
//! sequential loop. RawGeotag's own `-j` measured ~12x on SMB and 3,883 CR3s in 5.8 s at
//! `-j 20` against 48 s at `-j 2`, which is the number that argued for building the pool —
//! but that was *RawGeotag's* workload on *its* storage, and quoting it for this tool would
//! be the borrowed-measurement failure `REVIEWING.md` refuses.
//!
//! **What it measures, stated so the number is not over-read.** Correlating a frame against
//! a real track and writing its sidecars, which is the whole of phase 5's per-frame work.
//! Sidecars go to a temporary directory on the laptop's own disk, so this is the **local**
//! case. A destination on the hub or the NAS will differ, and the NAS is where RawGeotag's
//! 12x came from.
//!
//! **It never opens a raw file**, matching phase 5 — decision 10 has capture times already.
//! Nothing here touches a camera card.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use geotag::track::{GapLimits, Track};
use offload::phase5::{self, Landed};
use offload::pipeline::Destination;
use offload::progress::Progress;

/// Enough frames to be a real day rather than a warm-up. The 415 GB run was 7,395.
const FRAMES: usize = 7_395;

/// Four copies, as the rig actually writes (decision 11).
const DESTINATIONS: usize = 4;

/// One day, one folder — decision 31's layout, and the case that matters: NTFS serializes
/// *metadata* operations within a single directory, so all four thousand sidecars landing in
/// one folder per destination is exactly where a pool is supposed to stop helping.
const DATE_FOLDER: &str = r"2024\2024-10-02";

fn main() -> ExitCode {
    let Some(gpx) = std::env::args().nth(1) else {
        eprintln!("usage: geotag-rate <a .gpx track>");
        return ExitCode::from(2);
    };
    let tracks = vec![PathBuf::from(&gpx)];

    let track = match Track::load(&tracks) {
        Ok(track) => track,
        Err(error) => {
            eprintln!("loading {gpx}: {error:#}");
            return ExitCode::FAILURE;
        }
    };
    let (first, last) = track.span();

    println!();
    println!("  Track   {gpx}");
    println!("  Span    {first} .. {last}");
    println!("  Frames  {FRAMES} x {DESTINATIONS} destinations");
    println!();
    println!(
        "  {:<8}{:>10}{:>12}{:>12}",
        "jobs", "wall", "frames/s", "vs 1"
    );
    println!("  {}", "-".repeat(42));

    let landed = spread(first, last, FRAMES);
    let mut baseline = None;

    // Ascending, so the one-thread case runs first and pays whatever cold cost there is —
    // charging it to the *parallel* rows would flatter them.
    for jobs in [1usize, 2, 4, 8, 12, 20] {
        // A fresh directory per run: an existing sidecar is skipped rather than written
        // (decision 16), so reusing one would measure the skip path from the second row on
        // and report a speedup that is really a no-op.
        let scratch = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("making a scratch directory: {error}");
                return ExitCode::FAILURE;
            }
        };

        let destinations: Vec<Destination> = (0..DESTINATIONS)
            .map(|n| Destination {
                label: format!("dest-{n}"),
                root: scratch.path().join(format!("dest-{n}")),
            })
            .collect();

        // Phase 3 creates the date folders before phase 5 ever runs, so the harness has to
        // as well — and it is deliberately outside the timed section, since a real phase 5
        // never pays for it.
        for destination in &destinations {
            if let Err(error) = std::fs::create_dir_all(destination.root.join(DATE_FOLDER)) {
                eprintln!("making {}: {error}", destination.root.display());
                return ExitCode::FAILURE;
            }
        }

        let started = Instant::now();
        let report = phase5::run(
            &landed,
            &destinations,
            &tracks,
            GapLimits::DEFAULT,
            false,
            jobs,
            &Progress::Silent,
        );
        let elapsed = started.elapsed();

        match report {
            Ok(report) => {
                let rate = FRAMES as f64 / elapsed.as_secs_f64();
                let speedup = match baseline {
                    None => {
                        baseline = Some(elapsed.as_secs_f64());
                        "—".to_owned()
                    }
                    Some(base) => format!("{:.2}x", base / elapsed.as_secs_f64()),
                };
                println!(
                    "  {jobs:<8}{:>9.2}s{rate:>12.0}{speedup:>12}",
                    elapsed.as_secs_f64()
                );

                // Printed once, and it is the control: a row that tagged nothing would be
                // measuring the *miss* path, which writes no sidecars and is not the work.
                if baseline.is_some() && jobs == 1 {
                    println!(
                        "  {:<8}{} tagged, {} written, {} outside the track",
                        "", report.tagged, report.written, report.outside_track
                    );
                    if report.tagged == 0 {
                        eprintln!(
                            "\n  ! nothing tagged — this measured the miss path, not the work"
                        );
                        return ExitCode::FAILURE;
                    }
                }
            }
            Err(error) => {
                eprintln!("phase 5 at -j {jobs}: {error:#}");
                return ExitCode::FAILURE;
            }
        }
    }

    println!();
    println!("  Local disk only. The NAS and hub cases are not measured here.");
    ExitCode::SUCCESS
}

/// Capture times spread evenly across the track, so every frame resolves.
///
/// **Evenly rather than randomly**, so the two arms of `Track::lookup` are hit in the same
/// proportion on every run and the rows compare to each other.
fn spread(first: DateTime<Utc>, last: DateTime<Utc>, count: usize) -> Vec<Landed> {
    let span = (last - first).num_seconds().max(1);

    (0..count)
        .map(|n| {
            let at = first + TimeDelta::seconds(span * n as i64 / count as i64);
            Landed {
                relative: PathBuf::from(format!(r"{DATE_FOLDER}\{n:05}.CR3")),
                captured: at,
            }
        })
        .collect()
}
