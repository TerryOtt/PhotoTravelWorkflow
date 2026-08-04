//! `photoday` — the end-of-day offload.
//!
//! The command surface is settled in `docs/DESIGN.md` decision 8 and transcribed here;
//! the five phases behind it are not built yet. Parsing is real rather than stubbed so
//! that `--help` answers honestly, and so the surface `CONOPS.md` and `UPDATING.md`
//! already tell the operator to type cannot drift from the design while the phases land.

use std::collections::BTreeMap;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use geotag::format::RawFormat;
use geotag::raw::{Capture, MediaParser, capture_time};
use geotag::track::GapLimits;
use photoday::pipeline::Destination;
use photoday::runlog::RunLog;
use photoday::{config, destinations, marker, naming, phase5, pipeline, power, preflight, verify};

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
    match &cli.command {
        Some(Command::Verify { dest }) => return verify_destination(dest),
        Some(Command::Sync { dest }) => {
            eprintln!(
                "photoday: sync is not implemented yet ({}) — see docs/DESIGN.md                  decision 20.",
                dest.display()
            );
            return Ok(ExitCode::FAILURE);
        }
        None => {}
    }

    if cli.offload.dry_run {
        dry_run(&cli.offload)
    } else {
        offload(&cli.offload)
    }
}

/// The nightly command: pre-flight, then phase 3.
///
/// Phases 4 and 5 are not built, so this stops at LANDED and says so. That is an honest
/// place to stop rather than an arbitrary one — LANDED *is* the product (decision 14),
/// and everything after it is explicitly gravy.
fn offload(args: &Offload) -> Result<ExitCode> {
    let config = config::load()?;
    let awake = power::StayAwake::request();

    let plan = preflight::run(
        &config,
        args.allow_single_source,
        &args.without,
        args.no_gpx,
    )?;
    report(&plan, &awake);

    let run_id = Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();

    // `_runs` lives on the laptop's copy alone (decision 14): `verify` reads nothing but
    // a destination's marker and manifests, so this is not part of what makes an archive
    // self-describing, and the laptop is the one destination `--without` can never name.
    let runs_root = laptop_root(&plan)?.join("_runs").join(&run_id);
    let log = RunLog::open(&runs_root.join("run.jsonl"))?;

    let targets: Vec<Destination> = plan
        .rig
        .survey
        .found
        .iter()
        .map(|resolved| Destination {
            label: resolved.label.clone(),
            root: resolved.root.clone(),
        })
        .collect();

    println!();
    println!(
        "  ingesting {} files to {} destinations…",
        count(plan.cards.files.len()),
        targets.len()
    );

    let started = Instant::now();
    // The card's mount point, so phase 3 can record each frame's card-relative path —
    // which is what decision 4 pairs the two cards on.
    let card_root = plan
        .cards
        .source
        .volume
        .mount_points
        .first()
        .cloned()
        .unwrap_or_else(|| plan.cards.source.dcim.clone());

    let outcome = pipeline::run(
        &plan.cards.files,
        &targets,
        &run_id,
        source_card(&plan),
        &card_root,
        &log,
    )?;
    let elapsed = started.elapsed();

    // Each destination says what it is, so an archive pulled from the safe years from
    // now proves itself on a machine that has never seen this config (decision 6).
    for resolved in &plan.rig.survey.found {
        marker::write(
            &resolved.root,
            &resolved.label,
            resolved.device.as_ref().and_then(|d| d.serial.as_deref()),
            &Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        )?;
    }

    landed(&outcome, elapsed, &runs_root);

    // Phase 5 runs after LANDED and may take as long as it likes — decision 14 lets only
    // phase 3 change the verdict, so a geotag miss is a count in the body and never a
    // downgrade at the top.
    let geotag = geotag_phase(&plan, &targets, &outcome, args)?;
    report_geotag(geotag.as_ref());

    Ok(if outcome.landed() && outcome.unfiled.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    })
}

/// Which card fed the run, for the per-file record (decision 12).
///
/// **In a gated two-card run the faster card is the CFexpress**, which is decision 7's
/// premise measured rather than assumed — the two are unambiguous, near 65 ms against
/// UHS-II's 240 ms. A declared single-source run cannot know which type survived, so it
/// records the card rather than guessing at it.
fn source_card(plan: &preflight::Preflight) -> &'static str {
    if plan.cards.agreed {
        "cfexpress"
    } else {
        "single"
    }
}

/// The laptop copy's root — the destination found by path rather than by hardware.
fn laptop_root(plan: &preflight::Preflight) -> Result<PathBuf> {
    plan.rig
        .survey
        .found
        .iter()
        .find(|resolved| !resolved.ejectable())
        .map(|resolved| resolved.root.clone())
        .context(
            "no destination on this machine's own disk, so there is nowhere to put the \
             run log — `_runs` lives on the laptop copy (decision 14)",
        )
}

/// Decision 14's verdict, announced when it happens rather than only at the end.
fn landed(outcome: &pipeline::Outcome, elapsed: Duration, runs_root: &Path) {
    let minutes = elapsed.as_secs() / 60;
    let seconds = elapsed.as_secs() % 60;

    println!();
    println!("═══ LANDED · phase 3 took {minutes}m {seconds:02}s ═══",);
    println!();
    println!(
        "  {} files · {:.1} GB · read once from the source card",
        count(outcome.files),
        outcome.bytes as f64 / 1e9
    );
    println!();

    for destination in &outcome.destinations {
        println!(
            "  {:<8} {} written · {} skipped · {} verified   {}",
            destination.label,
            count(destination.written),
            count(destination.skipped),
            count(destination.verified),
            if destination.failed.is_empty() {
                "OK".to_string()
            } else {
                format!("{} UNVERIFIED", count(destination.failed.len()))
            }
        );
    }

    if !outcome.unfiled.is_empty() {
        println!();
        println!(
            "  !  {} frame(s) had no readable capture time and are in _unfiled",
            count(outcome.unfiled.len())
        );
    }

    println!();
    println!("  run log  {}", runs_root.join("run.jsonl").display());
    println!();
    println!(
        "►  {}",
        if outcome.landed() {
            "SAFE TO STORE — corroboration and geotagging are not built yet, so nothing \
             has been ejected"
        } else {
            "NOT SAFE — see the unverified counts above"
        }
    );
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

/// `photoday verify <DEST>` — decision 20.
///
/// Reads nothing but the disk itself, so it works on a machine that has never seen this
/// tool's configuration. That is the promise, and it is why this takes a path rather
/// than a config label.
fn verify_destination(root: &Path) -> Result<ExitCode> {
    let report = verify::destination(root)?;

    println!();
    match (&report.label, &report.created_utc, &report.last_run_utc) {
        (Some(label), Some(created), Some(last)) => println!(
            "  {label}  ·  {}  ·  archiving since {created}  ·  last run {last}",
            root.display()
        ),
        _ => println!(
            "  {}  ·  no readable destination marker — verifying by its manifests alone",
            root.display()
        ),
    }
    println!();

    for folder in &report.folders {
        let name = folder
            .folder
            .strip_prefix(root)
            .unwrap_or(&folder.folder)
            .display()
            .to_string();

        println!(
            "  {:<28} {:>6} verified{}{}{}",
            name,
            count(folder.checked),
            if folder.tombstoned > 0 {
                format!(" · {} tombstoned", count(folder.tombstoned))
            } else {
                String::new()
            },
            if folder.damaged.is_empty() {
                String::new()
            } else {
                format!(" · {} DAMAGED", count(folder.damaged.len()))
            },
            if folder.missing.is_empty() {
                String::new()
            } else {
                format!(" · {} MISSING", count(folder.missing.len()))
            },
        );
    }

    // Kept apart from damage, always. A manifest this build cannot read says nothing
    // whatever about the photographs beside it (decisions 12, 28).
    for (path, why) in &report.unreadable_manifests {
        println!();
        println!("  !  {}", path.display());
        println!("     {why}");
    }

    for folder in &report.folders {
        for name in &folder.damaged {
            println!("  !  DAMAGED   {name}");
        }
        for name in &folder.missing {
            println!("  !  MISSING   {name}");
        }
        for name in &folder.unrecorded {
            println!("  ?  not in the manifest: {name}");
        }
    }

    println!();
    println!(
        "  {} files verified across {} folders",
        count(report.checked()),
        count(report.folders.len())
    );

    println!();
    println!(
        "►  {}",
        if report.clean() {
            "CLEAN — every recorded file is present and matches".to_string()
        } else if !report.unreadable_manifests.is_empty() && report.damaged() == 0 {
            "CANNOT FULLY VERIFY — a manifest could not be read; the photographs it              covers were not checked, and nothing here says they are damaged"
                .to_string()
        } else {
            format!(
                "NOT CLEAN — {} damaged, {} missing",
                count(report.damaged()),
                count(report.missing())
            )
        }
    );

    Ok(if report.clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    })
}

/// Phase 5, or `None` when the night was declared trackless (decision 26).
fn geotag_phase(
    plan: &preflight::Preflight,
    targets: &[Destination],
    outcome: &pipeline::Outcome,
    args: &Offload,
) -> Result<Option<phase5::Report>> {
    if args.no_gpx || plan.rig.tracks.is_empty() {
        return Ok(None);
    }

    let limits = GapLimits {
        max_gap: chrono::TimeDelta::seconds(args.max_gap_seconds),
        max_meters: args.max_gap_meters,
    };

    phase5::run(
        &outcome.landed,
        targets,
        &plan.rig.tracks,
        limits,
        args.force_xmp.is_some(),
    )
    .map(Some)
}

fn report_geotag(report: Option<&phase5::Report>) {
    let Some(report) = report else {
        println!();
        println!("  Geotag   not run — no tracks (--no-gpx)");
        return;
    };

    println!();
    print!(
        "  Geotag   {} tagged · {} outside track",
        count(report.tagged),
        count(report.outside_track)
    );
    if report.in_gap > 0 {
        print!(" · {} in a gap too wide to bridge", count(report.in_gap));
    }
    println!();

    // The pattern, not the count. A bare "1,383 outside track" could be a dead logger, a
    // late start or a day of dropouts, and the response differs for each — so when every
    // miss sits on one side, name the boundary.
    if let Some(note) = report.boundary_note() {
        println!("           {note}");
    }
    if let Some(note) = report.gap_note() {
        println!("           {note}");
    }

    // Decision 23: a uniform miss pattern is a clock, not a logging gap, and saying so
    // in words is the difference between finding out tonight and finding out on
    // Lightroom's map three weeks later.
    if let Some(offset) = report.systematic_offset() {
        let minutes = offset.num_minutes();
        println!(
            "  !  misses look systematic (~{}{}:{:02}) — check the camera clock",
            if minutes < 0 { "-" } else { "+" },
            (minutes / 60).abs(),
            (minutes % 60).abs()
        );
    }

    println!(
        "           {} sidecars written · {} left alone (already tagged)",
        count(report.written),
        count(report.skipped)
    );
}
