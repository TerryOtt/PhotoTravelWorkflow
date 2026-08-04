//! `photoday` — the end-of-day offload.
//!
//! The command surface is settled in `docs/DESIGN.md` decision 8 and transcribed here;
//! the five phases behind it are not built yet. Parsing is real rather than stubbed so
//! that `--help` answers honestly, and so the surface `CONOPS.md` and `UPDATING.md`
//! already tell the operator to type cannot drift from the design while the phases land.

use std::collections::BTreeMap;
use std::num::NonZero;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use geotag::format::RawFormat;
use geotag::raw::{Capture, MediaParser, capture_time};
use geotag::track::GapLimits;
use photoday::{config, destinations, naming, power, preflight};

#[derive(Debug, Parser)]
#[command(
    name = "photoday",
    version,
    about = "One command, four verified copies"
)]
struct Cli {
    /// No subcommand is the nightly offload — the bare `photoday` of the ritual.
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    offload: Offload,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Re-verify a destination against the manifests it carries. Reads no config.
    Verify {
        /// The archive root — a path, never a config label (decision 20).
        dest: PathBuf,
    },
    /// Backfill a destination that missed an offload, from the laptop working copy.
    Sync {
        /// The archive root to bring current.
        dest: PathBuf,
    },
}

#[derive(Debug, Args)]
struct Offload {
    /// Plan the entire run and write nothing.
    #[arg(long)]
    dry_run: bool,

    /// CPU pool for hashing, EXIF and XMP — not the I/O fan-out, which is structural.
    #[arg(long, value_name = "N", default_value_t = default_jobs())]
    jobs: usize,

    /// Abort rather than warn when the two cards disagree.
    #[arg(long)]
    fail_on_source_mismatch: bool,

    /// Proceed when only one card is present; it becomes the sole source of truth.
    #[arg(long)]
    allow_single_source: bool,

    /// Run without a named archive destination; sync the disk when it returns.
    #[arg(long, value_name = "LABEL")]
    without: Vec<String>,

    /// Override when tracks aren't in the usual place.
    #[arg(long, value_name = "PATH", conflicts_with = "no_gpx")]
    gpx: Option<PathBuf>,

    /// Proceed with no tracks at all; raws land as normal, no sidecars are written.
    #[arg(long)]
    no_gpx: bool,

    /// Refuse to interpolate across a longer hole.
    //
    // Both defaults come from the engine rather than being written again here, so the
    // limit the CLI advertises and the limit `Track::lookup` enforces cannot drift
    // apart. Decision 16 renamed these from RawGeotag's `--max-gap`/`--max-distance`;
    // the values behind them are the same ones, and deliberately harsh.
    #[arg(long, value_name = "S", default_value_t = GapLimits::DEFAULT_GAP_SECONDS)]
    max_gap_seconds: i64,

    /// Refuse to interpolate across a wider hole.
    #[arg(long, value_name = "M", default_value_t = GapLimits::DEFAULT.max_meters)]
    max_gap_meters: f64,

    /// Overwrite existing XMP on every destination, or on just the one named.
    //
    // A doc comment here would be printed by `--help`, so this note is an ordinary one.
    // `Option<Option<_>>` is clap's encoding for a flag whose value is optional, which is
    // what decision 8's `--force-xmp[=<DEST>]` needs: absent, bare and named are three
    // distinct instructions, and a bare `--force-xmp` must not read as "no destination".
    // `require_equals` keeps `--force-xmp SSD-A` from parsing the label as a positional.
    #[arg(long, value_name = "DEST", num_args = 0..=1, require_equals = true)]
    force_xmp: Option<Option<String>>,

    /// Leave the archive SSDs mounted when the run ends.
    #[arg(long)]
    no_eject: bool,
}

/// Logical CPUs, per decision 15. Falls back to 1 on the platforms that cannot answer;
/// Windows always can, so the fallback is unreachable here rather than a real default.
fn default_jobs() -> usize {
    std::thread::available_parallelism().map_or(1, NonZero::get)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match dispatch(&cli) {
        Ok(code) => code,
        Err(error) => {
            // Decision 18: print why, exit non-zero, stop. `{:#}` so anyhow's context
            // chain reads as one line of cause rather than being swallowed.
            eprintln!("\n{error:#}\n");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: &Cli) -> Result<ExitCode> {
    if let Some(command) = &cli.command {
        // `verify` and `sync` are decision 20's, and neither is built yet.
        eprintln!("photoday: {command:?} is not implemented yet — see docs/DESIGN.md.");
        return Ok(ExitCode::FAILURE);
    }

    if !cli.offload.dry_run {
        eprintln!(
            "photoday: the offload itself is not wired up yet. `photoday --dry-run` \
             runs pre-flight against the real rig and plans the whole night."
        );
        return Ok(ExitCode::FAILURE);
    }

    dry_run(&cli.offload)
}

/// Plan the entire run and write nothing (decision 8).
fn dry_run(offload: &Offload) -> Result<ExitCode> {
    let config = config::load()?;

    // Held for the length of the plan, so a long walk over a slow card cannot be
    // interrupted by the machine suspending (decision 9).
    let awake = power::StayAwake::request();

    let plan = preflight::run(
        &config,
        offload.allow_single_source,
        &offload.without,
        offload.no_gpx,
    )?;

    report(&plan, &awake);
    let unnameable = plan_names(&plan)?;

    Ok(if unnameable == 0 {
        ExitCode::SUCCESS
    } else {
        // Decision 18's code 2: completed, but something wants your attention.
        ExitCode::from(2)
    })
}

/// The summary that actually lets you leave (decision 9).
fn report(plan: &preflight::Preflight, awake: &power::StayAwake) {
    let cards = &plan.cards;
    let rig = &plan.rig;

    println!();
    println!(
        "{} files {} · {:.1} GB · {} destinations verified distinct · est. {}",
        count(cards.files.len()),
        if cards.agreed {
            "on both cards".to_string()
        } else {
            format!("· single source ({})", cards.source.label())
        },
        cards.bytes as f64 / 1e9,
        rig.distinct_disks,
        estimate(cards.bytes),
    );
    println!();

    println!(
        "  source   {}  {:.0} MB/s",
        cards.source.label(),
        cards.source_speed.bytes_per_second() / 1e6
    );
    if let Some((other, speed)) = &cards.other {
        println!(
            "  other    {}  {:.0} MB/s",
            other.label(),
            speed.bytes_per_second() / 1e6
        );
    }
    println!();

    for resolved in &rig.survey.found {
        println!(
            "  {:<8} {:<28} disk {:<3} {:.0} GB free{}",
            resolved.label,
            resolved.root.display().to_string(),
            resolved
                .device
                .as_ref()
                .map_or("?".to_string(), |device| device.disk_number.to_string()),
            resolved.volume.free_bytes as f64 / 1e9,
            match &resolved.matched {
                destinations::Match::SerialAtNewVolume { .. } =>
                    "   ! REFORMATTED — update the config's volume_guid",
                destinations::Match::VolumeOnly => "   ! no serial reported; found by GUID",
                _ => "",
            }
        );
    }

    for missing in &rig.survey.missing {
        println!("  {:<8} EXCLUDED — {}", missing.label, missing.reason);
    }

    println!();
    println!(
        "  tracks   {} in {}",
        count(rig.tracks.len()),
        plan_gpx_dir(rig)
    );

    if !awake.engaged() {
        println!("  !  the machine would not agree to stay awake for this run");
    }
}

fn plan_gpx_dir(rig: &preflight::Rig) -> String {
    rig.tracks
        .first()
        .and_then(|track| track.parent())
        .map_or_else(|| "(none)".to_string(), |dir| dir.display().to_string())
}

/// Name every output file exactly, which is what makes this a rehearsal rather than a
/// summary. Returns how many frames could not be named and would land in `_unfiled`.
fn plan_names(plan: &preflight::Preflight) -> Result<usize> {
    let mut parser = MediaParser::new();
    let mut unnameable = 0usize;
    let mut by_day: BTreeMap<String, usize> = BTreeMap::new();

    println!();

    for file in &plan.cards.files {
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        // The path-based reader, which seeks the container rather than reading 45 MB —
        // this is why a 3,883-frame day plans in seconds rather than minutes.
        let relative = match capture_time(&mut parser, file, RawFormat::Cr3, None) {
            Ok(Capture::Resolved { at, .. }) => naming::destination_path(at, name),
            _ => {
                unnameable += 1;
                naming::unfiled_path("<run-id>", name)
            }
        };

        *by_day
            .entry(
                relative
                    .parent()
                    .map_or_else(|| "?".into(), |parent| parent.display().to_string()),
            )
            .or_default() += 1;

        println!("  {}", relative.display());
    }

    println!();
    for (day, frames) in &by_day {
        println!("  {day}   {} frames", count(*frames));
    }

    if unnameable > 0 {
        println!();
        println!(
            "  !  {} frame(s) have no readable capture time and would land in _unfiled",
            count(unnameable)
        );
    }

    Ok(unnameable)
}

/// Wall-clock estimate, bound by the slowest destination absorbing N of writes and N of
/// read-back verification (decision 2's arithmetic).
fn estimate(bytes: u64) -> String {
    const SLOW_SSD_BYTES_PER_SECOND: f64 = 450e6;
    const FAST_SSD_BYTES_PER_SECOND: f64 = 800e6;

    let slow = (bytes as f64 * 2.0 / SLOW_SSD_BYTES_PER_SECOND / 60.0).ceil() as u64;
    let fast = (bytes as f64 * 2.0 / FAST_SSD_BYTES_PER_SECOND / 60.0).ceil() as u64;

    format!("{fast}-{slow} min")
}

/// Thousands separators, per docs/WRITING.md rule 6.
fn count(n: usize) -> String {
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
