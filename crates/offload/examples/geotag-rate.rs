//! What phase 5 actually costs — the measurement that says **do not parallelize it**.
//!
//! ```text
//! cargo run --release --example geotag-rate -- C:\Travel\GPX\2024-10-02.gpx
//! ```
//!
//! **This exists to stop a good idea being re-proposed.** On 2026-08-07 `--jobs` was found to
//! be parsed and never read, phase 5 was a sequential loop, and a `rayon` pool was built to fix
//! it. It worked: **~1.5–1.8× on this workload, knee at four threads.** It was then **reverted**,
//! and the numbers below are why.
//!
//! **Phase 5 is ~20 s of an 89-minute run.** Terry, on seeing the speedup: *"if we are only
//! seeing a ~1.8x improvement on a 400 GB day where geotagging is like 20 seconds, my feeling is
//! to revert it. That's too much complexity for a tiny wall clock win."* He is applying this
//! project's own rule — `CLAUDE.md`'s *both metrics are thresholds, not gradients*: **do not
//! trade anything for wall clock**, not clarity, not a safety check, not an afternoon of
//! engineering. Nine seconds on eighty-nine minutes is **0.17 %**.
//!
//! **The 12× that argued for building it was a different workload.** RawGeotag once tagged
//! *years* of files across the NAS in one pass — many directories, SMB latency, threads hiding
//! round trips. **That is not a use case this project has**, and once that was clear the
//! justification was gone; the mistake was continuing to build after the premise fell.
//!
//! **What it costs to keep the sequential version**, measured here so the trade stays visible:
//! run this, and if phase 5 ever grows past a minute or two, the pool is a known ~1.7× sitting
//! in git history at `fd730da`.
//!
//! **It never opens a raw file**, matching phase 5 — decision 10 has capture times already.
//! Nothing here touches a camera card, and nothing is written outside a temporary directory.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use geotag::track::{GapLimits, Track};
use offload::phase5::{self, Landed};
use offload::pipeline::Destination;
use offload::progress::Progress;

/// The largest day on record, so the number is the worst real case rather than a typical one.
const FRAMES: usize = 7_395;

/// Four copies, as the rig actually writes (decision 11).
const DESTINATIONS: usize = 4;

/// One day, one folder — decision 31's layout. **The reason a pool stops helping at four
/// threads:** NTFS serializes *metadata* operations within a single directory, and every
/// sidecar of a day lands in one.
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
    let landed = spread(first, last, FRAMES);

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

    // Phase 3 creates the date folders before phase 5 ever runs, so the harness has to as well
    // — deliberately outside the timed section, since a real phase 5 never pays for it.
    for destination in &destinations {
        if let Err(error) = std::fs::create_dir_all(destination.root.join(DATE_FOLDER)) {
            eprintln!("making {}: {error}", destination.root.display());
            return ExitCode::FAILURE;
        }
    }

    let started = Instant::now();
    let report = match phase5::run(
        &landed,
        &destinations,
        &tracks,
        GapLimits::DEFAULT,
        &Progress::Silent,
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("phase 5: {error:#}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed();

    // The control: a run that tagged nothing measured the *miss* path, which writes no sidecars
    // and is not the work. A timing without this check could not fail.
    if report.tagged == 0 {
        eprintln!("\n  ! nothing tagged — this measured the miss path, not the work");
        return ExitCode::FAILURE;
    }

    println!();
    println!("  Track    {gpx}");
    println!("  Span     {first} .. {last}");
    println!();
    println!("  Frames   {FRAMES} x {DESTINATIONS} destinations");
    println!(
        "  Result   {} tagged, {} sidecars written, {} outside the track",
        report.tagged, report.written, report.outside_track
    );
    println!(
        "  Wall     {:.2}s  ({:.0} frames/s)",
        elapsed.as_secs_f64(),
        FRAMES as f64 / elapsed.as_secs_f64()
    );
    println!();
    println!("  Sequential, deliberately. A thread pool measured ~1.7x here and was reverted:");
    println!("  phase 5 is ~20 s of an 89-minute run, so the win is under a fifth of a percent.");

    ExitCode::SUCCESS
}

/// Capture times spread evenly across the track, so every frame resolves.
///
/// **Evenly rather than randomly**, so the arms of `Track::lookup` are hit in the same
/// proportion on every run and two timings compare to each other.
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
