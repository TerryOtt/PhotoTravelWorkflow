//! `photoday` — the end-of-day offload.
//!
//! The command surface is settled in `docs/DESIGN.md` decision 8 and transcribed here. All
//! five phases are built, and the run ends by ejecting the archive SSDs (decision 22) — so
//! this file is now the order those phases run in and the one place decision 14's verdict is
//! printed, rather than a parser waiting for an implementation.

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
use photoday::{
    cards, config, destinations, eject, manifest, marker, naming, phase4, phase5, pipeline, power,
    preflight, verify,
};

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

/// The nightly command, end to end: pre-flight, phase 3, corroboration, geotag, eject.
///
/// **LANDED is announced the moment it happens, in the middle of this function**, because it
/// *is* the product (decision 14) and everything after it is explicitly gravy. The verdict,
/// though, is printed once at the very end — phases 4 and 5 still write to the archives, and
/// eject cannot be attempted until they are done.
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

    // Phases 4 and 5 both run after LANDED and may take as long as they like — decision 14
    // lets only phase 3 change the verdict, so neither a mismatch nor a geotag miss is a
    // downgrade at the top; both are counts in the body.
    let corroboration = corroboration_phase(&plan, &targets, &outcome, &runs_root, args)?;
    record_corroboration(&targets, &outcome, corroboration.as_ref())?;
    report_corroboration(corroboration.as_ref());

    let geotag = geotag_phase(&plan, &targets, &outcome, args)?;
    report_geotag(geotag.as_ref());

    // Decision 22: last, because phases 4 and 5 still write to the archives. The volumes
    // must be released only once nothing remains to put on them.
    let ejecting = Instant::now();
    let released = eject_phase(&plan, &outcome, args, started + RUN_BUDGET);
    report_eject(released.as_deref(), args, ejecting.elapsed());

    // After the archives, because phase 4 read the SDXC and phase 3 the CFexpress — and
    // before the verdict, which does not consider the result (decision 22).
    report_cards(&dismount_cards(&plan, args));
    verdict(&outcome, released.as_deref(), corroboration.as_ref(), args);

    Ok(exit_code(
        &outcome,
        released.as_deref(),
        corroboration.as_ref(),
        args,
    ))
}

/// Decision 18's three codes, from what the run actually produced.
///
/// **Every condition decision 18 names gets a line here**, because the previous version
/// tested only `unfiled` and returned 0 for everything else — so the 2026-08-04 run exited
/// 0 while two archive SSDs sat un-powered-down and the verdict said to deal with them by
/// hand. An exit code is what a script and a tired operator both key on, and it was
/// claiming nothing wanted attention while the report above it said otherwise.
///
/// The one condition decision 18 lists that is not testable here is a **stray** — a
/// non-CR3 file on a card (decision 24). The walk does not yet carry them out of
/// pre-flight, so there is nothing to consult; when it does, it belongs in this function.
fn exit_code(
    outcome: &pipeline::Outcome,
    released: Option<&[Released]>,
    corroborated: Option<&phase4::Report>,
    args: &Offload,
) -> ExitCode {
    // Phase 3 is the only thing that can make a run *fail* rather than merely want
    // attention, and it is the same test the verdict uses.
    if !outcome.landed() {
        return ExitCode::from(2);
    }

    let wants_attention =
        // A file whose EXIF could not be read, parked in `_unfiled` (decision 21).
        !outcome.unfiled.is_empty()
        // A confirmed two-card mismatch: deleted everywhere, tombstoned, quarantined
        // (decisions 3, 4). Transient re-read agreements are not mismatches and are
        // deliberately not counted here.
        || corroborated.is_some_and(|report| !report.mismatched.is_empty())
        // An eject that did not power the device down — whether it stayed mounted or
        // dismounted without powering down, both leave the operator something to do
        // (decision 22).
        || released.is_some_and(|released| released.iter().any(|r| !r.effort.outcome.is_ejected()))
        // Corroboration could not finish, so the eject gate held (decisions 7, 22). The
        // SSDs are still mounted and the verdict says to insert the SDXC and re-run.
        || (released.is_none() && !args.no_eject)
        // The three declared degradations, each of which narrows what the run certifies.
        || args.allow_single_source
        || !args.without.is_empty()
        || args.no_gpx;

    if wants_attention {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
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

/// One destination's eject result, kept with its label for the verdict.
struct Released {
    label: String,
    effort: eject::Effort,
}

/// The card phase 3 read from, in operator-facing output.
///
/// The manifest calls the same thing `source_card` and keeps that name, because a field on
/// disk is read by `verify` years later and renaming it would be a schema change under
/// decision 28. The report is read by a tired human at a desk; these are for them.
const PRIMARY: &str = "primary";

/// The card phase 4 corroborated against.
const SECONDARY: &str = "secondary";

/// How long after launch eject stops trying (decision 22).
///
/// **The operator is at dinner, and dinner is 60–90 minutes** (`CONOPS.md`). A run reaches
/// LANDED in about a quarter of that and finishes in about half, so the rest of the hour is
/// time nobody is waiting through — which makes it free to spend asking Windows again.
/// **As long as the program exits inside the hour, taking longer costs nothing at all**, and
/// a drive that powers itself down at minute 40 is worth far more than one that gave up at
/// minute 36 and left a chore.
const RUN_BUDGET: Duration = Duration::from_secs(60 * 60);

/// Decision 22: eject when nothing remains for the current cards.
///
/// **The gate is "complete", not "all matched".** A mismatch resolved by
/// deletion-and-tombstone is settled, and so is a `waived` verdict on a declared
/// single-source night — only work the current cards could still answer for holds it. Phase
/// 4 aborting takes the `?` path long before here, so reaching this function at all means
/// corroboration finished.
///
/// Returns `None` when nothing was attempted, which the verdict distinguishes from an eject
/// that was tried and refused.
fn eject_phase(
    plan: &preflight::Preflight,
    outcome: &pipeline::Outcome,
    args: &Offload,
    deadline: Instant,
) -> Option<Vec<Released>> {
    if args.no_eject || !outcome.landed() {
        return None;
    }

    let targets: Vec<_> = plan
        .rig
        .survey
        .found
        .iter()
        .filter(|resolved| resolved.ejectable())
        .collect();

    // Concurrently, and not for speed — eject is the last thing the run does and nobody is
    // waiting on it. It is because the devices share one deadline: done in sequence, a drive
    // that retried to the end of the budget would leave the others a single attempt each,
    // and whatever holds one freshly written volume is usually holding all of them. Run
    // together, every device gets the whole window.
    let released = std::thread::scope(|scope| {
        let running: Vec<_> = targets
            .iter()
            .map(|resolved| scope.spawn(move || release(resolved, deadline)))
            .collect();

        running
            .into_iter()
            .map(|handle| handle.join().expect("an eject thread panicked"))
            .collect()
    });

    Some(released)
}

/// Eject one resolved destination, turning both failure paths into a reportable outcome.
fn release(resolved: &destinations::Resolved, deadline: Instant) -> Released {
    // A destination resolved by serial always has a device; if it somehow does not, that is
    // a refusal to report rather than a reason to fail the run.
    let effort = match resolved.device.as_ref() {
        Some(device) => {
            eject::eject(&resolved.volume, device, deadline).unwrap_or_else(|error| eject::Effort {
                outcome: eject::Outcome::Held {
                    reason: format!("{error:#}"),
                },
                attempts: 1,
                waited: Duration::ZERO,
            })
        }
        None => eject::Effort {
            outcome: eject::Outcome::Held {
                reason: "the volume reports no physical device to power down".into(),
            },
            attempts: 0,
            waited: Duration::ZERO,
        },
    };

    Released {
        label: resolved.label.clone(),
        effort,
    }
}

/// Decision 14's verdict: the last line, and its phrases appear nowhere else in the report.
fn verdict(
    outcome: &pipeline::Outcome,
    released: Option<&[Released]>,
    corroborated: Option<&phase4::Report>,
    args: &Offload,
) {
    println!();

    if !outcome.landed() {
        println!("►  NOT SAFE — see the unverified counts above");
        return;
    }

    // What an ejected disk is a claim *about* differs between a two-card night and a
    // declared single-source one, and decision 22 says the verdict must not let the same
    // eject imply more than it proved.
    let claim = if corroborated.is_some() {
        "every file from both cards is accounted for"
    } else {
        "every file from the one card present is accounted for — corroboration was waived"
    };

    let Some(released) = released else {
        if args.no_eject {
            println!("►  SAFE TO STORE — eject withheld by --no-eject; {claim}");
        } else {
            println!("►  SAFE TO STORE — nothing to eject; {claim}");
        }
        return;
    };

    // Two failures, two different instructions — and collapsing them is what made a
    // successful run read as a chore. A volume something still holds is mounted, and the
    // tray icon is the only way to shift it. A volume that dismounted but would not power
    // down is flushed and detached: pulling it out is the whole of what remains, and
    // sending the operator to the tray for it asks them to repeat work already done.
    let held: Vec<&str> = labels(released, |o| matches!(o, eject::Outcome::Held { .. }));
    let unplug: Vec<&str> = labels(released, |o| matches!(o, eject::Outcome::Dismounted { .. }));

    let mut actions = Vec::new();
    if !held.is_empty() {
        actions.push(format!("EJECT {} BY HAND", held.join(", ")));
    }
    if !unplug.is_empty() {
        actions.push(format!("UNPLUG {}", unplug.join(", ")));
    }

    if actions.is_empty() {
        println!("►  EJECTED — SAFE TO STORE. {claim}.");
    } else {
        println!("►  SAFE TO STORE — {}. {claim}.", actions.join(" AND "));
    }
}

/// The labels of the released devices whose outcome matches `wanted`.
fn labels(released: &[Released], wanted: impl Fn(&eject::Outcome) -> bool) -> Vec<&str> {
    released
        .iter()
        .filter(|r| wanted(&r.effort.outcome))
        .map(|r| r.label.as_str())
        .collect()
}

fn report_eject(released: Option<&[Released]>, args: &Offload, elapsed: Duration) {
    println!();
    let Some(released) = released else {
        if args.no_eject {
            println!("  Eject    withheld by --no-eject");
        }
        return;
    };

    // **Eject is a timed stage, and the clock is the point** (decision 22). A retry that runs
    // for twenty minutes is the tool working; unlabeled, twenty silent minutes read as a
    // hang. The operator asked for this specifically, and the difference between the two
    // readings is entirely whether the duration is on the screen.
    println!("  Eject    ({})", duration(elapsed));

    for r in released {
        // What it cost, but only when it cost anything — a device that powered down on the
        // first ask should read as cleanly as it behaved. When Windows did make the run work
        // for it, that is worth printing: decision 22 can only be tuned from real numbers,
        // and these are the only ones a run produces.
        let effort = if r.effort.attempts > 1 {
            format!(
                " after {} attempts over {}",
                r.effort.attempts,
                duration(r.effort.waited)
            )
        } else {
            String::new()
        };

        match &r.effort.outcome {
            eject::Outcome::Ejected => {
                println!("           {:<8} powered down{effort}", r.label);
            }
            // Worth its own wording: the bytes are flushed and detached either way, and an
            // operator who reads "failed" for this would worry about the wrong thing.
            eject::Outcome::Dismounted { reason } => println!(
                "           {:<8} dismounted, not powered down — safe to unplug{effort}\n           {reason}",
                r.label
            ),
            eject::Outcome::Held { reason } => println!(
                "           {:<8} still mounted — eject it from the tray{effort}\n           {reason}",
                r.label
            ),
        }
    }
}

/// Dismount both camera cards, so the ritual ends with all five removable devices settled.
///
/// **Nothing here may change the verdict or the exit code.** The tool never wrote to a card,
/// so it was safe to pull before this ran and is safe to pull if it fails — this is tidiness,
/// and letting it downgrade anything would claim it bought a guarantee it did not.
fn dismount_cards(
    plan: &preflight::Preflight,
    args: &Offload,
) -> Vec<(String, eject::CardOutcome)> {
    if args.no_eject {
        return Vec::new();
    }

    // **Primary and secondary, not drive letters and not card types.** A letter is the one
    // identifier decision 6 exists to keep out of decisions, and the tool genuinely cannot
    // say "CFexpress": decision 7 identifies cards by measurement because serial,
    // removability and bus type all fail, and a CFexpress in a bridge reader enumerates as
    // USB. What is actually known is which card fed phase 3 and which corroborated it, so
    // that is what the report says. `source_card` in the manifest is the same concept under
    // its durable name (decision 12).
    std::iter::once((PRIMARY, &plan.cards.source))
        .chain(plan.cards.other.as_ref().map(|(card, _)| (SECONDARY, card)))
        .map(|(role, card)| {
            let outcome = eject::dismount_card(&card.volume).unwrap_or_else(|error| {
                eject::CardOutcome::Held {
                    reason: format!("{error:#}"),
                }
            });
            (role.to_string(), outcome)
        })
        .collect()
}

fn report_cards(cards: &[(String, eject::CardOutcome)]) {
    if cards.is_empty() {
        return;
    }

    // Heading once, then indented rows — the same shape as the eject block above it. The
    // first version repeated "Cards" on every line and read as two unrelated events.
    println!();
    println!("  Cards");
    for (label, outcome) in cards {
        match outcome {
            eject::CardOutcome::Dismounted => {
                println!("           {label:<9} dismounted — safe to pull");
            }
            // Deliberately not phrased as a failure: the card was always safe to pull, and an
            // operator reading "held" here would worry about data that was never at risk.
            eject::CardOutcome::Held { reason } => println!(
                "           {label:<9} still mounted — safe to pull regardless, nothing was written to it\n           {reason}",
            ),
        }
    }
}

/// `4m 12s`, or `38s` under a minute — the report's duration format.
fn duration(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

/// A card's root, which is what decision 4 pairs the two cards on.
///
/// The mount point rather than the `DCIM` directory, because `Ingested::card_relative` is
/// relative to the volume — falling back to `DCIM` only when a card somehow reports no
/// mount point at all.
fn card_root(card: &cards::Card) -> PathBuf {
    card.volume
        .mount_points
        .first()
        .cloned()
        .unwrap_or_else(|| card.dcim.clone())
}

/// Phase 4, or `None` when there was no second card to consult (decision 7).
///
/// **Runs before phase 5 because it is numbered before it, and because it can abort.**
/// With `--fail-on-source-mismatch` a disagreement stops the run, and stopping before
/// writing sidecars for frames whose provenance is in doubt is the better order.
fn corroboration_phase(
    plan: &preflight::Preflight,
    targets: &[pipeline::Destination],
    outcome: &pipeline::Outcome,
    runs_root: &Path,
    args: &Offload,
) -> Result<Option<phase4::Report>> {
    let Some((other, _)) = plan.cards.other.as_ref() else {
        return Ok(None);
    };

    phase4::run(
        &outcome.ingested,
        &card_root(&plan.cards.source),
        &card_root(other),
        targets,
        &runs_root.join("quarantine"),
        args.fail_on_source_mismatch,
    )
    .map(Some)
}

/// Resolve every manifest entry this run left pending (decision 12).
///
/// **Phase 3 wrote each entry with `corroborated: None`, meaning the question was open.**
/// This is what closes it, and it must run whether or not phase 4 did: a single-source run
/// still has to say *waived* rather than leaving the record permanently ambiguous.
///
/// Entries are grouped by day folder because that is where a manifest lives, and applied to
/// every destination because each carries its own copy.
fn record_corroboration(
    targets: &[pipeline::Destination],
    outcome: &pipeline::Outcome,
    report: Option<&phase4::Report>,
) -> Result<()> {
    let deleted: BTreeMap<&Path, &(PathBuf, String, String)> = report
        .map(|report| {
            report
                .mismatched
                .iter()
                .map(|entry| (entry.0.as_path(), entry))
                .collect()
        })
        .unwrap_or_default();

    // Absent phase 4 the verdict is *waived by declaration*, which is a claim about the run
    // rather than about the file — decision 7.
    let default_verdict = if report.is_some() {
        manifest::Corroborated::Matched
    } else {
        manifest::Corroborated::Waived
    };
    let deleted_utc = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut by_folder: BTreeMap<&Path, Vec<manifest::Outcome>> = BTreeMap::new();
    for frame in &outcome.ingested {
        let relative = frame.destination_relative.as_path();
        let (Some(folder), Some(name)) = (relative.parent(), relative.file_name()) else {
            continue;
        };

        let disputed = deleted.get(relative);
        by_folder
            .entry(folder)
            .or_default()
            .push(manifest::Outcome {
                name: name.to_string_lossy().into_owned(),
                corroborated: match disputed {
                    Some(_) => manifest::Corroborated::Mismatched,
                    None => default_verdict,
                },
                deletion: disputed.map(|(_, source, other)| manifest::Deletion {
                    source_sha256: source.clone(),
                    other_sha256: other.clone(),
                    reason: "the two cards disagreed, confirmed on a re-read of both".into(),
                    deleted_utc: deleted_utc.clone(),
                }),
            });
    }

    for destination in targets {
        for (folder, outcomes) in &by_folder {
            manifest::corroborate(&destination.root.join(folder), outcomes).with_context(|| {
                format!(
                    "recording corroboration in {}'s manifest for {}",
                    destination.label,
                    folder.display()
                )
            })?;
        }
    }

    Ok(())
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

    // No verdict here, deliberately. Decision 14 puts the verdict on the *last* line and
    // says its phrases appear nowhere else, so announcing one at LANDED — before
    // corroboration, geotagging and eject have had their say — would give the operator two
    // places to look and a chance to read the wrong one.
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

/// Decision 4: **say "this card looks like it is failing" in those words.** A photographer
/// reading a bare count should not have to know what normal looks like.
fn report_corroboration(report: Option<&phase4::Report>) {
    println!();
    let Some(report) = report else {
        println!("  Corrob   waived — only one card was present (--allow-single-source)");
        return;
    };

    print!("  Corrob   {} matched", count(report.matched));
    if report.transient > 0 {
        // Not a data problem — the re-read agreed. It is a *reader* problem, and worth
        // saying so before it becomes one.
        print!(
            " · {} transient read error(s), re-read agreed",
            count(report.transient)
        );
    }
    println!(" · {} mismatched", count(report.mismatched.len()));

    for (name, source, other) in &report.mismatched {
        println!(
            "           {} — deleted everywhere, quarantined\n             source {}\n              other {}",
            name.display(),
            &source[..16.min(source.len())],
            &other[..16.min(other.len())]
        );
    }

    if !report.mismatched.is_empty() {
        println!("           quarantine  {}", report.quarantine.display());
    }

    if report.suspect_card() {
        println!();
        println!(
            "  !  That is far more disagreement than the one or two a healthy pair of cards\n     \
             produces. THIS LOOKS LIKE A FAILING CARD — replace it before the next shoot."
        );
    }
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
