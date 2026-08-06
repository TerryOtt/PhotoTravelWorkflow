//! `offload` — the end-of-day offload.
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
use console::style;
use geotag::format::RawFormat;
use geotag::raw::{Capture, MediaParser, capture_time};
use geotag::track::GapLimits;
// Thousands separators (`docs/WRITING.md` rule 6). It lives in the library rather than here
// so `progress.rs` can hold the bars to the same rule the report follows — they printed a
// bare `3883` beside the report's `3,883` until 2026-08-05.
use offload::human::count;
use offload::pipeline::Destination;
use offload::runlog::RunLog;
use offload::{
    cards, config, destinations, eject, manifest, marker, naming, phase4, phase5, pipeline, power,
    preflight, storage, verify,
};

#[derive(Debug, Parser)]
#[command(name = "offload", version, about = "One command, four verified copies")]
struct Cli {
    /// No subcommand is the nightly offload — the bare `offload` of the ritual.
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
                "offload: sync is not implemented yet ({}) — see docs/DESIGN.md \
                 decision 20.",
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

    // **Created before pre-flight, not after, because pre-flight is the first thing that
    // makes the operator wait.** Walking both cards takes two to four seconds with nothing on
    // screen — short enough to be harmless and long enough to wonder, which is precisely the
    // gap this module exists to close. Bars at a terminal, plain lines when captured to a
    // file, and shared by every phase so their output stacks rather than fighting for the
    // cursor (`CONOPS.md`).
    let progress = offload::progress::Progress::detect();

    // **Plain prints, not `Progress` bars, and the distinction is load-bearing.** Nothing
    // animates during pre-flight, and a live `MultiProgress` bar owns the cursor — so a
    // heading held open across `report`'s ordinary `println!`s would have them fighting over
    // the same lines. Bars start at phase 3, where something actually moves.
    // Two blanks above a phase heading, matching `Progress::section`. These two headings are
    // plain prints rather than managed bars — nothing animates during pre-flight, and the
    // `Offloading` line is printed before any bar exists — so they carry their own spacing.
    println!();
    println!();
    // **"Pre-Flight Checks", the operator's term** — he is an aviation geek and adopted it,
    // and it fits the vocabulary the tool already had: the run ends at `LANDED`. Both words
    // come from the same place, which is why neither reads as jargon here.
    println!("Pre-Flight Checks");
    // Every other heading gets a blank line under it from `Progress::section`; this one is a
    // plain print and was the only one whose first line sat jammed against it.
    println!();
    // Three periods, not U+2026. Terry, 2026-08-05: the single-glyph ellipsis "bothers my old
    // school DOS ANSI UI eyes." His screen, his call — and an ASCII ellipsis cannot render as
    // a box on a console that is having a bad code-page day, which is a small real benefit
    // beyond taste.
    println!("    Enumerating files on camera cards...");

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

    // Flush left: this is a phase heading, the parent of the `Writing` and `Verifying`
    // sections below it, and it used to sit indented as though it were an item in a list.
    //
    // Two blanks above, like every phase heading — but **no blank below, and it is the one
    // phase heading without one.** Its content is entirely sub-sections, and `Writing` brings
    // its own leading blank; adding one here would put two together. `Pre-Flight Checks`,
    // `Corroborating` and `Geotagging` all have rows directly under them and do get the gap.
    println!();
    println!();
    // **`·` and not `...` before the estimate.** The ellipsis was doing a separator's job in the
    // middle of a line, while the summary line above uses a middle dot for exactly that — two
    // punctuation marks for one role. The trailing `...` on `Enumerating files on camera cards...`
    // stays, because there it is not a separator: it marks work in progress with nothing after
    // it, which is the only thing an ellipsis should mean here.
    //
    // **`Offloading`, not `Ingesting`.** The binary is `offload`, `CONOPS.md` calls the act
    // *the offload*, and the screen said a third word for the same thing — which is the exact
    // drift `WRITING.md` rule 8 exists to stop. *Ingest* stays as the repository's name for
    // phase 3 in `DESIGN.md` and `pipeline.rs`; it is the operator-facing string that has to
    // match the word he uses, the same split as `primary` on disk and `Primary` on screen.
    //
    // **The estimate lives here rather than on the pre-flight summary**, because this is the
    // line the eye goes to: the summary says what tonight *is*, and this says what is starting
    // and how long it will take. Terry, 2026-08-05: *"That's where my eye looks for it."*
    println!(
        "Offloading {} files to {} destinations · est. {}",
        count(plan.cards.files.len()),
        targets.len(),
        estimate(
            plan.cards.bytes,
            plan.cards
                .other
                .as_ref()
                .map(|(_, speed)| speed.bytes_per_second()),
        ),
    );

    let started = Instant::now();
    let card_root = card_root(&plan.cards.source);

    // Bound before the struct borrows it: the serial is formatted from the volume, and the
    // role names what was observed rather than a card type the tool cannot know
    // (decision 12).
    let source_serial = plan.cards.source.volume.serial_text();
    let source = pipeline::Source {
        role: source_role(&plan),
        volume_serial: &source_serial,
    };

    let outcome = pipeline::run(
        &plan.cards.files,
        &targets,
        &run_id,
        source,
        &card_root,
        &log,
        &progress,
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

    // **Every phase hands the terminal back before its report prints**, here and again after
    // corroboration and geotagging. `MultiProgress` owns a block and repaints it wherever the
    // cursor is, so a plain `println!` while bars are live does not land below them — it
    // collides. On 2026-08-05 that put the LANDED banner inside eight progress rows and drew
    // the rows twice, once on each side of it.
    progress.clear();
    landed(&outcome, elapsed);

    // Phases 4 and 5 both run after LANDED and may take as long as they like — decision 14
    // lets only phase 3 change the verdict, so neither a mismatch nor a geotag miss is a
    // downgrade at the top; both are counts in the body.
    let corroboration =
        corroboration_phase(&plan, &targets, &outcome, &runs_root, args, &progress)?;
    record_corroboration(&targets, &outcome, corroboration.as_ref())?;
    progress.clear();
    report_corroboration(corroboration.as_ref());

    let geotag = geotag_phase(&plan, &targets, &outcome, args, &progress)?;
    progress.clear();
    report_geotag(geotag.as_ref());

    // Decision 22: last, because phases 4 and 5 still write to the archives. The volumes
    // must be released only once nothing remains to put on them.
    let ejecting = Instant::now();
    let deadline = started + RUN_BUDGET;

    // **All five start together, and the SSDs report the moment they are down.** Terry,
    // 2026-08-06: *"I am INTERESTED in total time to eject all five, but SSD are like two
    // orders of magnitude more important. Let's start shutting them all down at the same time
    // but print how long until SSD were fully put to bed as soon as all three are down."*
    //
    // **The run that produced that instruction is the argument for it.** The three SSDs were
    // down in 15 s; a CFexpress then retried for 22 minutes and never released; and the one
    // conflated closing line reported the pair as `Released 5 devices in 22m 16s`. The answer
    // that actually mattered existed at fifteen seconds and was withheld for twenty-two
    // minutes — and the count was wrong as well, since four devices had been released.
    //
    // This is decision 14's rule about LANDED applied to the stage Terry calls his number one
    // risk: announce a milestone when it happens rather than only in the final summary.
    //
    // **They share the deadline and not the stakes.** An SSD that will not power down reaches
    // the exit code (decision 18); a card can never touch it, and `exit_code` is not even
    // given the card results so that it cannot start.
    let (released, (cards, cards_took, budget_spent)) = std::thread::scope(|scope| {
        let cards = scope.spawn(|| {
            let outcomes = release_cards(&plan, args, deadline);
            (outcomes, ejecting.elapsed(), Instant::now() >= deadline)
        });

        // The archives on this thread, so their result is in hand — and printed — without
        // waiting on a card retry that cannot change it.
        let released = eject_phase(&plan, &outcome, args, deadline);
        report_ssd_release(released.as_deref(), args, ejecting.elapsed());

        (
            released,
            cards.join().expect("the card release thread panicked"),
        )
    });

    report_card_release(&cards, cards_took, budget_spent);
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
fn source_role(plan: &preflight::Preflight) -> &'static str {
    if plan.cards.agreed { PRIMARY } else { SOLE }
}

/// One destination's eject result, kept with its label for the verdict.
struct Released {
    label: String,
    effort: eject::Effort,
}

/// One gibibyte. The divisor Windows uses everywhere the operator can check a number —
/// Explorer, PowerShell's `1GB`, a drive's properties dialog — so it is the divisor this
/// report uses for every *size*. Rates against a link stay decimal; see [`offload::human`].
const GIB: f64 = (1u64 << 30) as f64;

/// The card phase 3 read from, **as recorded on disk**.
///
/// The manifest and the run log carry this verbatim as `source_card`, which `verify` reads
/// years later — decision 28 makes changing the spelling a schema change. Lowercase, and it
/// stays lowercase however the screen chooses to render it.
const PRIMARY: &str = "primary";

/// The only card there was, under `--allow-single-source` (decision 7). Also on disk.
///
/// Distinct from [`PRIMARY`] because the record must not imply a second card existed:
/// `waived` corroboration and a sole source are the same night, and a reader years later
/// should be able to see that from either field.
const SOLE: &str = "sole";

/// The same roles, capitalized, **for the screen only**.
///
/// **The split is not cosmetic.** [`PRIMARY`] and [`SOLE`] are written into the manifest and
/// the run log; these three are written to a terminal. Capitalizing the screen ones lines the
/// cards up with the destination labels printed beside them; capitalizing the stored ones
/// would be a schema change under decision 28, silently invalidating every archive written
/// before it.
///
/// **There is deliberately no stored counterpart to [`SECONDARY_LABEL`]**, and its absence is
/// the tell: only the *source* card's role is ever recorded, and the second card is by
/// definition not the source. A `secondary` on disk would be a field nothing reads.
const PRIMARY_LABEL: &str = "Primary";

/// The card phase 4 corroborated against. Screen only — see [`PRIMARY_LABEL`].
const SECONDARY_LABEL: &str = "Secondary";

/// See [`PRIMARY_LABEL`].
const SOLE_LABEL: &str = "Sole";

/// How long after launch eject stops trying (decision 22).
///
/// **Ninety minutes, which is the top of the dinner window rather than the bottom.** It was
/// sixty until 2026-08-06, chosen as the conservative end of `CONOPS.md`'s 60–90 — on the
/// reasoning that the program must exit before the operator returns.
///
/// **The operator retired that reasoning himself**, and he is the only possible source for
/// it: *"this app is run when I'm away for dinner. Let's push the eject timeframe to 90 mins.
/// If I do get back before it's done ejecting, I will happily wait."* So returning to a run
/// still arguing with Windows is not the failure the sixty was protecting against — **the
/// failure is a drive left in the tray**, and waiting a few minutes is cheaper than a chore.
///
/// **What it buys lands exactly where the risk is.** The retry window is what is left after
/// phases 3–5, so it is *smallest on the biggest days* — the nights with the most freshly
/// written data, the most scanner activity and the most likely veto. On the 415 GB record day
/// sixty minutes left roughly eight; ninety leaves nearly forty. Decision 22 has the table.
///
/// **Nothing waits on this.** A run that ejects cleanly on the first ask still exits in
/// seconds; the budget is a ceiling on patience, never a delay.
const RUN_BUDGET: Duration = Duration::from_secs(90 * 60);

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

/// Release both camera cards, so the ritual ends with all five removable devices settled.
///
/// **Nothing here may change the verdict or the exit code.** The tool never wrote to a card,
/// so it was safe to pull before this ran and is safe to pull if it fails — this is tidiness,
/// and letting it downgrade anything would claim it bought a guarantee it did not.
///
/// **Cards take the same path as destinations, which is a correction.** They used to get lock
/// and dismount only, on the reasoning that powering a reader down would be the wrong device.
/// A dismount releases nothing (decision 22), so the cards stayed in the tray — and the
/// measured cost of the full sequence is smaller than that reasoning assumed: the Thunderbolt
/// reader survives it untouched, and the USB one comes back on a replug.
fn release_cards(
    plan: &preflight::Preflight,
    args: &Offload,
    deadline: Instant,
) -> Vec<(String, eject::Outcome)> {
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
    std::iter::once((PRIMARY_LABEL, &plan.cards.source))
        .chain(
            plan.cards
                .other
                .as_ref()
                .map(|(card, _)| (SECONDARY_LABEL, card)),
        )
        .map(|(role, card)| {
            let outcome = match storage::device_of(&card.volume) {
                Ok(device) => eject::eject(&card.volume, &device, deadline)
                    .map(|effort| effort.outcome)
                    .unwrap_or_else(|error| eject::Outcome::Held {
                        reason: format!("{error:#}"),
                    }),
                Err(error) => eject::Outcome::Held {
                    reason: format!("the card reports no device to release: {error:#}"),
                },
            };
            (role.to_string(), outcome)
        })
        .collect()
}

/// The archive SSDs' half of the eject stage, printed the moment all of them are resolved.
///
/// **Split from the cards on 2026-08-06, at Terry's direction, and the run that prompted it is
/// the argument.** The three SSDs were down in 15 s while a CFexpress retried for 22 minutes
/// and never released — and one shared closing line reported the pair as
/// `Released 5 devices in 22m 16s`. Two things were wrong with that. The answer that matters
/// existed at fifteen seconds and was withheld for twenty-two minutes; and the count was a lie,
/// because four devices had been released, which is the same shape decision 22 fixed once
/// already when the report claimed cards were released and a dismount had released nothing.
///
/// His framing: *"SSD are like two orders of magnitude more important."* An SSD that will not
/// power down reaches the exit code (decision 18) and leaves a chore. A card that will not
/// release is tidiness, and the tool never wrote to it.
///
/// **The heading lives here** because this half always prints first, and the card half needs
/// it already on screen.
///
/// **They keep sub-headings.** `Travel SSDs` and `Cards` at one level in: the two groups differ
/// in what you do with them and in what a failure means, and grouping is what stops five rows
/// reading as an undifferentiated pile. Same shape as `Pre-Flight Checks` — phase at column 0,
/// groups at 4, rows at 8.
fn report_ssd_release(released: Option<&[Released]>, args: &Offload, elapsed: Duration) {
    println!();
    println!();
    println!("Eject");
    println!();

    // `None` means the stage never ran: either `--no-eject`, or phase 3 did not land so the
    // gate never opened. The verdict says which; this only avoids claiming an empty list of
    // devices was released.
    let Some(ssds) = released else {
        if args.no_eject {
            println!("    withheld by --no-eject");
        }
        return;
    };

    if !ssds.is_empty() {
        println!("    Travel SSDs");
    }
    for r in ssds {
        // What it cost, but only when it cost anything — a device that powered down on the
        // first ask should read as cleanly as it behaved. When Windows did make the run work
        // for it, that is worth printing: decision 22 can only be tuned from real numbers, and
        // these are the only ones a run produces.
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
            // **"ready to disconnect", not "powered down".** The operator's phrasing, and it
            // answers the question actually being asked at this moment: may I pull the cable.
            eject::Outcome::Ejected => {
                println!(
                    "        {:<10} ejected; ready to disconnect{effort}",
                    r.label
                );
            }
            // Worth its own wording: the bytes are flushed and detached either way, and an
            // operator who reads "failed" for this would worry about the wrong thing.
            eject::Outcome::Dismounted { reason } => println!(
                "        {:<10} dismounted, not powered down — safe to unplug{effort}\n            {reason}",
                r.label
            ),
            eject::Outcome::Held { reason } => println!(
                "        {:<10} still mounted — eject it from the tray{effort}\n            {reason}",
                r.label
            ),
        }
    }

    // Flush left: the closing fact of this half of the stage rather than a row in the list
    // above it. **It prints here, before the cards are joined**, which is the whole change —
    // this is the number Terry is waiting on and it must not queue behind a card retry that
    // cannot affect it.
    let down = ssds
        .iter()
        .filter(|r| r.effort.outcome.is_ejected())
        .count();
    println!();
    println!();
    if down == ssds.len() {
        println!(
            "Travel SSDs — all {} put to bed in {}. Safe to store.",
            count(down),
            duration(elapsed)
        );
    } else {
        // **This branch has to be able to fire, and a suite that only sees the clean path
        // cannot prove it does** — mutation-check it by forcing a Held outcome. The line it
        // replaced printed a device count that included devices it had just described as not
        // powered down.
        let stuck = labels(ssds, |o| !o.is_ejected());
        println!(
            "Travel SSDs — {} of {} put to bed in {}. {} still needs you; see above.",
            count(down),
            count(ssds.len()),
            duration(elapsed),
            stuck.join(", ")
        );
    }
}

/// The cards' half, printed when they resolve — which may be long after the SSDs.
///
/// **Nothing here may change the verdict or the exit code**, and `exit_code` is not even given
/// these results so that it cannot start. The tool never wrote to a card, so it was safe to
/// pull before this ran and is safe to pull if it fails (decision 22).
///
/// `budget_spent` distinguishes *the retry ran out of time* from *it gave up for another
/// reason*, because Terry asked for the 90-minute case to be declared rather than left to be
/// inferred from a large number.
fn report_card_release(cards: &[(String, eject::Outcome)], elapsed: Duration, budget_spent: bool) {
    if cards.is_empty() {
        return;
    }

    println!();
    println!("    Cards");
    for (label, outcome) in cards {
        match outcome {
            // A card comes *out*; an SSD gets *unplugged*. Same event, different next action.
            eject::Outcome::Ejected => {
                println!("        {label:<10} ejected; remove card from reader");
            }
            // Neither remaining branch is phrased as a failure. The tool never wrote to a card,
            // so it was safe to pull before any of this ran; what was lost is tidiness, and an
            // operator reading "failed" here would worry about data that was never at risk.
            eject::Outcome::Dismounted { reason } => println!(
                "        {label:<10} dismounted, still listed — safe to pull anyway\n            {reason}",
            ),
            eject::Outcome::Held { reason } => println!(
                "        {label:<10} still mounted — safe to pull anyway, nothing was written to it\n            {reason}",
            ),
        }
    }

    // The second closing fact, and the one nobody is waiting on. Its elapsed time shares an
    // origin with the SSD line above, so the larger of the two is the answer to "how long to
    // put all five to bed" — which Terry asked to keep, just not at the cost of the number
    // that matters.
    let down = cards.iter().filter(|(_, o)| o.is_ejected()).count();
    println!();
    println!();
    if down == cards.len() {
        println!(
            "Cards — all {} put to bed in {}.",
            count(down),
            duration(elapsed)
        );
    } else {
        let stuck: Vec<&str> = cards
            .iter()
            .filter(|(_, o)| !o.is_ejected())
            .map(|(label, _)| label.as_str())
            .collect();

        // **Declared rather than inferred from a large number.** Terry asked for the budget
        // case to say so: a reader who sees `90m 00s` should not have to work out whether that
        // was persistence or a hang.
        let gave_up = if budget_spent {
            format!(
                "retried to the {}-minute budget and gave up",
                RUN_BUDGET.as_secs() / 60
            )
        } else {
            "gave up".to_string()
        };

        println!(
            "Cards — {} of {} put to bed in {}. {} never released ({}). \
             Safe to pull anyway: nothing was written to them.",
            count(down),
            count(cards.len()),
            duration(elapsed),
            stuck.join(", "),
            gave_up
        );
    }

    // **The cost of doing this properly, said out loud rather than discovered.** Releasing a
    // card means ejecting its device, and for a USB card reader that device *is* the reader:
    // it powers down with the card and does not wake when the next card goes in. The
    // Thunderbolt reader is untouched, because the NVMe disk's parent is a PCIe port rather
    // than the reader itself. Naming the consequence here is what turns a mystery at the next
    // offload into an expected chore — and pre-flight refuses anyway, so a forgotten replug
    // costs ten seconds rather than a night.
    if cards.iter().any(|(_, outcome)| outcome.is_ejected()) {
        println!();
        println!(
            "  !  A USB card reader powers down with its card and needs a replug before the\n     \
             next offload. The Thunderbolt reader does not. If you forget, pre-flight\n     \
             refuses with ONLY ONE CARD FOUND rather than running short."
        );
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
    progress: &offload::progress::Progress,
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
        progress,
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
fn landed(outcome: &pipeline::Outcome, elapsed: Duration) {
    let minutes = elapsed.as_secs() / 60;
    let seconds = elapsed.as_secs() % 60;

    println!();
    // **No phase number.** "phase 3" is this repository's word, not the operator's, and the
    // reader of this line is six months out of practice at 11pm in a hotel — CONOPS measures
    // two trips a year. A label that needs `DESIGN.md` to decode is a label that says nothing
    // at the moment it is read.
    //
    // **The sentence is literally true, which is why it is safe to print.** LANDED is the
    // point where every file exists on all four destinations and has been read back off the
    // media and compared (decisions 2, 14) — so *the data is safe* is the guarantee, not
    // encouragement. Everything after this line is corroboration, geotags and tidying, and
    // decision 14 exists to keep those from ever being confused with this.
    println!(
        "═══ LANDED in {minutes}m {seconds:02}s · you can breathe, Terry, your data is safe ═══"
    );
    println!();
    println!(
        "  {} files · {} GiB · read once from the source card",
        count(outcome.files),
        offload::human::gib(outcome.bytes)
    );

    // What the hardware actually did, which no single per-device rate shows. The source read
    // plus every destination's write-or-skip-check and verify read — on a fresh run that is
    // 9N, and it stays 9N under convergence because a skip still reads the target to compare.
    //
    // **This is a health signal, not a boast.** Half of it is unbuffered reads and the other
    // half is write-through, so the number cannot be inflated by a cache; a run that comes in
    // well under the last one says something is wrong on the bus before anyone goes looking.
    let moved = outcome.bytes
        + outcome
            .destinations
            .iter()
            .map(|d| d.bytes_moved)
            .sum::<u64>();
    let seconds = elapsed.as_secs_f64();
    if seconds > 0.0 {
        println!(
            // **GiB and GiB/s here, not decimal**, so this line divides: the sizes above are
            // GiB, and a reader who checks `moved ÷ elapsed` must get the rate printed beside
            // it. `Gbps` stays decimal because it is a *bits* unit whose only purpose is
            // comparison against a link — 10 Gbps is 10^10 bits by definition, and a figure
            // in GiB/s cannot be held up against the number printed on the cable.
            "  {} GiB moved · {:.2} GiB/s · {:.1} Gbps",
            offload::human::gib(moved),
            moved as f64 / GIB / seconds,
            moved as f64 * 8.0 / 1e9 / seconds
        );
    }
    println!();

    // **These four lines are the emotional payload of the whole run**, and the operator said
    // so plainly on 2026-08-05: *"That's the biggest warm fuzzy of the whole run. Genuine
    // blood pressure drop at those four lines."* Decision 14 makes the verdict the last line
    // and the answer; this is where the answer is *earned*, one destination at a time, and it
    // is worth being legible at a glance rather than merely present.
    //
    // **Unlike the capacity tick in pre-flight, this badge can come out red.** That one is a
    // receipt for a check that already refused the run; this one reports a comparison made
    // moments ago against 3,883 files per destination, and a failure here is the difference
    // between LANDED and NOT SAFE. So the colour carries meaning rather than reassurance,
    // which is what `REVIEWING.md` asks of anything that looks like a verdict.
    for destination in &outcome.destinations {
        let verdict = if destination.failed.is_empty() {
            style(" OK ".to_string()).white().bold().on_green()
        } else {
            style(format!(" {} UNVERIFIED ", count(destination.failed.len())))
                .white()
                .bold()
                .on_red()
        };
        println!(
            "  {:<8} {} written · {} skipped · {} verified   {verdict}",
            destination.label,
            count(destination.written),
            count(destination.skipped),
            count(destination.verified),
        );
    }

    if !outcome.unfiled.is_empty() {
        println!();
        println!(
            "  !  {} frame(s) had no readable capture time and are in _unfiled",
            count(outcome.unfiled.len())
        );
    }

    // **No run-log path here.** It was the only line in this block that was not about the
    // data being safe, and `CONOPS.md` says this block is what earns walking away — a file
    // path serves nobody at that moment. Nothing is lost: the location is deterministic —
    // `_runs` under the laptop copy, one directory per run id, sorted by timestamp — so the
    // newest run is always the last entry.

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

    // **Indented by logical grouping**, at the operator's request 2026-08-05: the pre-flight
    // heading sits at 4, its summary at 8, and each group of things — cards, destinations,
    // tracks — gets a heading at 12 with its rows at 16. A flat block of eleven lines makes
    // the reader work out which lines belong together; the indent says it.
    println!(
        "    {} files {} · {} GiB · {} destinations verified distinct",
        count(cards.files.len()),
        if cards.agreed {
            "on both cards".to_string()
        } else {
            format!("· single source ({})", cards.source.label())
        },
        offload::human::gib(cards.bytes),
        rig.distinct_disks,
    );

    // **`primary`/`secondary`/`sole`, the same words the eject block and the manifest use.**
    // These lines said `source` and `other` until 2026-08-05 — a second vocabulary for the
    // same two cards, in the same screen of output, which `WRITING.md` rule 8 exists to stop.
    // `sole` rather than `primary` when there is no second card is the informative case: it
    // tells a tired operator that corroboration will be waived before the run starts, not
    // after (decision 7).
    //
    // Rates are right-aligned to a fixed width so the two stack into a column the eye can
    // compare at a glance. These are the numbers that say "this card is dying" (decision 32),
    // and a ragged left edge is part of why a faulty 73 MB/s card read as unremarkable beside
    // a healthy 222. Separators per rule 6; the width holds a four-digit rate with its comma.
    // **Cards and destinations share one set of column widths**, measured across both blocks
    // before either prints. They are two lists of the same shape — a name, where it is, and a
    // number — so letting each size its own columns made `E:\` and `C:\Travel\Images` start in
    // different places and the two numbers land in different places, which is exactly the
    // ragged edge that made a faulty 73 MB/s card read as unremarkable beside a healthy 222
    // (decision 32).
    //
    // Measured rather than constant: the destination subpath is configurable, so any hardcoded
    // width is a bet on one config. Counted in `chars`, not bytes, so a non-ASCII path does not
    // over-pad. Numbers carry separators per `WRITING.md` rule 6.
    let source_role = if cards.other.is_some() {
        PRIMARY_LABEL
    } else {
        SOLE_LABEL
    };

    let mut card_rows = vec![(
        source_role,
        cards.source.label(),
        count((cards.source_speed.bytes_per_second() / 1e6).round() as usize),
    )];
    if let Some((other, speed)) = &cards.other {
        card_rows.push((
            SECONDARY_LABEL,
            other.label(),
            count((speed.bytes_per_second() / 1e6).round() as usize),
        ));
    }

    let dest_rows: Vec<(&str, String, String, &str)> = rig
        .survey
        .found
        .iter()
        .map(|resolved| {
            (
                resolved.label.as_str(),
                resolved.root.display().to_string(),
                offload::human::gib_whole(resolved.volume.free_bytes),
                match &resolved.matched {
                    destinations::Match::SerialAtNewVolume { .. } => {
                        "   ! REFORMATTED — update the config's volume_guid"
                    }
                    destinations::Match::VolumeOnly => "   ! no serial reported; found by GUID",
                    _ => "",
                },
            )
        })
        .collect();

    let width = |values: &[&str]| values.iter().map(|v| v.chars().count()).max().unwrap_or(0);
    let labels: Vec<&str> = card_rows
        .iter()
        .map(|(label, ..)| *label)
        .chain(dest_rows.iter().map(|(label, ..)| *label))
        .chain(rig.survey.missing.iter().map(|m| m.label.as_str()))
        .collect();
    let places: Vec<&str> = card_rows
        .iter()
        .map(|(_, place, _)| place.as_str())
        .chain(dest_rows.iter().map(|(_, place, ..)| place.as_str()))
        .collect();
    let numbers: Vec<&str> = card_rows
        .iter()
        .map(|(.., n)| n.as_str())
        .chain(dest_rows.iter().map(|(_, _, n, _)| n.as_str()))
        .collect();

    let label_column = width(&labels) + 2;
    // Four, not two. Column 2 holds anything from `D:\` to `C:\Travel\Images`, so the gap
    // the eye actually sees runs from fifteen spaces down to two — and two is tight enough to
    // read as crowded on the destination rows, which are the ones being compared. Widening
    // costs the card block, which is two rows deep and where nobody is tracking a gap.
    let place_column = width(&places) + 4;
    let number_column = width(&numbers);

    println!();
    println!("    Camera Cards");
    for (label, place, rate) in &card_rows {
        println!("        {label:<label_column$}{place:<place_column$}{rate:>number_column$} MB/s");
    }

    println!();
    println!("    Destinations");
    // **The free-space number is an answer, not a fact.** It settles *does tonight fit*, and
    // printed bare it made the reader do the subtraction. Terry: "tired 11pm Terry will
    // appreciate the reminder why we printed that; it's not a flex that we can read Windows
    // filesystem data." So the comparison and its verdict print beside it.
    //
    // **The tick cannot come out any other way, and that is said here rather than hidden.**
    // `preflight` refuses the run outright when a destination holds less than the payload plus
    // 5 %, so everything reaching this line has already passed. This is a *receipt* for a check
    // that happened, not the check itself — the distinction `REVIEWING.md` draws when it warns
    // about a diagnostic that cannot fail. The failing case is loud and lives in
    // `preflight.rs`: `NOT ENOUGH ROOM ON <label>`, quoting both numbers in the same units.
    let payload = offload::human::gib(cards.bytes);

    // **White on green, via `console`** — which decision 29 declared for exactly this
    // ("verdict styling, and whether this is a terminal at all") and which nothing had
    // imported until now, making it one of the declared-but-unused crates `TRIP-HYGIENE.md`
    // tracks by hand.
    //
    // `style` renders through `colors_enabled()`, so a redirected run — every run Claude
    // drives — gets the plain characters and no escape sequences in the log. That is the same
    // off-tty trap that nearly shipped the progress bars invisible, except here the library
    // handles it instead of silently rendering nothing.
    // Bold white on standard green, not bright green: bright-on-white is the *low* contrast
    // pairing, and bold lifts the glyph rather than washing out the badge behind it.
    let fits = style(" \u{2713} ").white().bold().on_green();

    for (label, place, free, note) in &dest_rows {
        println!(
            "        {label:<label_column$}{place:<place_column$}\
             {free:>number_column$} GiB free, > {payload} GiB  {fits}{note}"
        );
    }

    for missing in &rig.survey.missing {
        println!(
            "        {:<label_column$}EXCLUDED — {}",
            missing.label, missing.reason
        );
    }

    println!();
    println!("    Tracks");
    println!(
        "        {} in {}",
        count(rig.tracks.len()),
        plan_gpx_dir(rig)
    );

    // Deliberately outside the groups and back at the left: it is a warning about the machine
    // rather than an item in any of them, and a warning that indents itself into a list is a
    // warning that reads as a list item.
    if !awake.engaged() {
        println!();
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

/// Wall-clock estimate for **both** the moment the data is safe and the moment the program
/// exits.
///
/// **It used to answer only the first, and that was the wrong question to answer alone.**
/// Terry, 2026-08-05: *"I'd want that number to be estimate to program termination."* LANDED
/// is the product, so it stays — but the operator is deciding whether to go to dinner, and
/// dinner ends when the program does.
///
/// # The three terms, and why only one of them is a guess
///
/// **Phase 3, to LANDED** — the slowest destination absorbs N of writes and N of read-back
/// (decision 2's arithmetic), at 450–800 MB/s. These stay constants because destination
/// throughput is not measured at pre-flight; the spread is wide on purpose.
///
/// **Phase 4, corroboration** — computed from the second card's rate, which pre-flight *has
/// just measured*, so this term is grounded rather than assumed. It reads all of N off the
/// slowest device in the rig. [`CORROBORATION_EFFICIENCY`] is the read-then-hash serialization
/// tax, measured 2026-08-04: 170 MB/s achieved against a 205 MB/s card.
///
/// **Phase 5 and eject** — a minute, near enough. Geotagging ran ~20 s and eject 6 s on the
/// last three runs.
///
/// **Corroboration is added, not overlapped**, because that is what the code does: `phase4`
/// begins after `pipeline::run` returns. Decision 2 argues for the two-pass shape partly on
/// the grounds that it *lets* phase 4 start during the verify pass — a benefit that is
/// described and not built. Estimating as though it were would understate every run by a
/// quarter of an hour.
///
/// Checked against the 2026-08-05 run: predicted 26–33 min, actual 27 m 06 s.
fn estimate(bytes: u64, corroboration_bytes_per_second: Option<f64>) -> String {
    const SLOW_SSD_BYTES_PER_SECOND: f64 = 450e6;
    const FAST_SSD_BYTES_PER_SECOND: f64 = 800e6;

    /// Corroboration reads *and* hashes on one thread, so it never reaches the card's raw
    /// rate. Measured 2026-08-04: 170 MB/s from a card that reads 205.
    const CORROBORATION_EFFICIENCY: f64 = 0.83;

    /// Geotagging plus eject, which are small and roughly fixed.
    const TAIL_MINUTES: f64 = 1.0;

    let bytes = bytes as f64;
    let landed_fast = bytes * 2.0 / FAST_SSD_BYTES_PER_SECOND / 60.0;
    let landed_slow = bytes * 2.0 / SLOW_SSD_BYTES_PER_SECOND / 60.0;

    // No second card means corroboration is waived (decision 7), so it costs nothing rather
    // than being estimated at zero — the same night, said two different ways.
    let corroboration = corroboration_bytes_per_second
        .map_or(0.0, |rate| bytes / (rate * CORROBORATION_EFFICIENCY) / 60.0);

    format!(
        "{}-{} min to LANDED, {}-{} min to done",
        landed_fast.ceil() as u64,
        landed_slow.ceil() as u64,
        (landed_fast + corroboration + TAIL_MINUTES).ceil() as u64,
        (landed_slow + corroboration + TAIL_MINUTES).ceil() as u64,
    )
}

/// `offload verify <DEST>` — decision 20.
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

    // Decision 14's rule, applied to `verify`: the verdict is the last line and each of these
    // phrases appears nowhere else in the output. **Matched exhaustively rather than chained
    // through `if`** — a fifth outcome added later is a compile error here instead of silently
    // falling through into whichever branch happens to be last, which is how the previous
    // version reported an empty disk as `CLEAN`.
    let verdict = report.verdict();
    println!(
        "►  {}",
        match verdict {
            verify::Verdict::Clean =>
                "CLEAN — every recorded file is present and matches".to_string(),

            // **This is the outcome that used to be spelled `CLEAN`**, and the wording has to
            // do two things the old line did not: say plainly that nothing was proven, and
            // name both causes, because the operator cannot tell them apart from here. A disk
            // wiped since its last run and a path that was never an archive root produce
            // exactly the same empty walk.
            verify::Verdict::NothingToVerify => format!(
                "NOTHING TO VERIFY — no manifest found under {}. Either this is not an \
                 archive root, or this disk has been cleared. Nothing was checked, and \
                 nothing here says the photographs are fine.",
                root.display()
            ),

            verify::Verdict::Incomplete =>
                "CANNOT FULLY VERIFY — a manifest could not be read; the photographs it \
                 covers were not checked, and nothing here says they are damaged"
                    .to_string(),

            verify::Verdict::Damaged => format!(
                "NOT CLEAN — {} damaged, {} missing",
                count(report.damaged()),
                count(report.missing())
            ),
        }
    );

    Ok(match verdict {
        verify::Verdict::Clean => ExitCode::SUCCESS,
        // Decision 18's code 2 — completed, and something wants your attention. An empty disk
        // is not a *failure* of the command, which ran exactly as designed; it is emphatically
        // not a pass either, and a script keying on the exit status must not read it as one.
        verify::Verdict::NothingToVerify
        | verify::Verdict::Incomplete
        | verify::Verdict::Damaged => ExitCode::from(2),
    })
}

/// Phase 5, or `None` when the night was declared trackless (decision 26).
fn geotag_phase(
    plan: &preflight::Preflight,
    targets: &[Destination],
    outcome: &pipeline::Outcome,
    args: &Offload,
    progress: &offload::progress::Progress,
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
        progress,
    )
    .map(Some)
}

/// Decision 4: **say "this card looks like it is failing" in those words.** A photographer
/// reading a bare count should not have to know what normal looks like.
fn report_corroboration(report: Option<&phase4::Report>) {
    println!();
    let Some(report) = report else {
        println!("Corroborating");
        println!("    waived — only one card was present (--allow-single-source)");
        return;
    };

    // **The heading is reprinted here rather than surviving from the live display.**
    // `progress.clear()` hands the terminal back before this runs, which takes the bars
    // *and* their heading with it. The live block answers "how far along is it"; this
    // one is the record - the same relationship the phase 3 bars have with the LANDED
    // table.
    println!("Corroborating");
    print!("    {} matched", count(report.matched));
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
            "        {} — deleted everywhere, quarantined\n             source {}\n              other {}",
            name.display(),
            &source[..16.min(source.len())],
            &other[..16.min(other.len())]
        );
    }

    if !report.mismatched.is_empty() {
        println!("        quarantine  {}", report.quarantine.display());
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
        println!("Geotagging");
        println!("    not run — no tracks (--no-gpx)");
        return;
    };

    println!();
    println!("Geotagging");
    print!(
        "    {} tagged · {} outside track",
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
        println!("        {note}");
    }
    if let Some(note) = report.gap_note() {
        println!("        {note}");
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
        "    {} sidecars written · {} left alone (already tagged)",
        count(report.written),
        count(report.skipped)
    );
}
