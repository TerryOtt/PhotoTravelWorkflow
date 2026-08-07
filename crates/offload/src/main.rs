//! `offload` — the end-of-day offload.
//!
//! The command surface is settled in `docs/DESIGN.md` decision 8 and transcribed here. All
//! five phases are built, and the run ends by ejecting the archive SSDs (decision 22) — so
//! this file is now the order those phases run in and the one place decision 14's verdict is
//! printed, rather than a parser waiting for an implementation.

use std::collections::BTreeMap;
use std::io::{self, Write};
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

    // **Everything on this variant reaches `--help` verbatim**, so the rationale lives in `//`
    // comments and never in `///`. Decision 30's version: this is RawGeotag's whole job as a
    // subcommand — phase 5 already correlates capture times against a track and writes
    // sidecars, and this points that at a directory instead of at what tonight landed.
    //
    // Caught by pass 1 of the boomerang, which found markdown asterisks and a decision number
    // rendered into the user's `--help`.
    /// Tag an existing tree of raws against GPX tracks. Reads no config
    Geotag {
        /// The directory of raws, searched recursively.
        root: PathBuf,

        /// GPX track file, or a directory of them. Repeat as needed.
        #[arg(required = true, num_args = 1.., value_name = "GPX")]
        tracks: Vec<PathBuf>,

        /// Refuse to interpolate across a hole longer than this many seconds.
        #[arg(long, value_name = "SECONDS", default_value_t = GapLimits::DEFAULT_GAP_SECONDS)]
        max_gap_seconds: i64,

        /// Refuse to interpolate across a hole wider than this many meters.
        #[arg(long, value_name = "METERS", default_value_t = GapLimits::DEFAULT.max_meters)]
        max_gap_meters: f64,

        /// Rewrite sidecars that already exist instead of leaving them alone.
        #[arg(long)]
        force_xmp: bool,

        // Carried over from RawGeotag deliberately: this subcommand writes into a directory of
        // somebody's existing photographs, where the nightly command writes into destinations
        // it created. A preview is worth more here than there.
        /// Correlate everything and write nothing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Args)]
struct Offload {
    /// Plan the entire run and write nothing.
    #[arg(long)]
    dry_run: bool,

    /// Abort rather than warn when the two cards disagree.
    #[arg(long)]
    fail_on_source_mismatch: bool,

    /// Proceed when only one card is present; it becomes the sole source of truth.
    #[arg(long)]
    allow_single_source: bool,

    /// Run without a named archive destination; re-run the night when it returns.
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

    /// Ask this often during eject, instead of the 2s-doubling-to-60s backoff.
    //
    // Diagnostic, and it exists for one question: attempts and elapsed time are inseparable in
    // normal operation, because the retry loop increments one by spending the other. So no
    // ordinary run can say whether a long hold NEEDS many attempts or is CAUSED by them — and
    // there is a mechanism for the second, since every attempt dismounts and closes, and
    // Windows remounts eagerly. See `eject::Cadence` and DESIGN.md decision 22.
    //
    // Visible rather than hidden on purpose: this is a one-operator tool, and a knob that
    // changes eject behavior should be findable in `--help` rather than known only to whoever
    // added it. A nightly run should never set it.
    #[arg(long, value_name = "SECONDS")]
    eject_gap_seconds: Option<u64>,
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
        Some(Command::Geotag {
            root,
            tracks,
            max_gap_seconds,
            max_gap_meters,
            force_xmp,
            dry_run,
        }) => {
            return geotag_tree(
                root,
                tracks,
                GapLimits {
                    max_gap: chrono::TimeDelta::seconds(*max_gap_seconds),
                    max_meters: *max_gap_meters,
                },
                *force_xmp,
                *dry_run,
            );
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
    // **A deliberate exception, kept after a consistency review. MUST NOT be normalized.**
    // `Corroborating`, `Geotagging` and `Eject` all put their first line directly under the
    // heading; this one keeps a blank. Terry compared them on 2026-08-06 and chose to keep the
    // gap: *"it's not consistent but it looks better to my frustratingly-inconsistent human
    // meat-sac known as a brain."*
    //
    // **And the difference is structural rather than arbitrary, which is why it survives.**
    // Pre-flight's content is three sub-sections — `Camera Cards`, `Destinations`, `Tracks` —
    // where the other phases have a line or two of result directly underneath. A heading with
    // sections under it wants air; a heading with a sentence under it does not.
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
    // Two blanks above, like every phase heading — but **no blank below.** Its content is
    // entirely sub-sections, and `Writing` brings its own leading blank; adding one here would
    // put two together.
    //
    // **Corrected 2026-08-06**: this used to say `Corroborating` and `Geotagging` "do get the
    // gap". They no longer do — a status line earns no blank above it, only a sub-heading does.
    // `Pre-Flight Checks` is now the single deliberate exception, and the note at its own print
    // site says why.
    println!();
    println!();
    // `·` as the separator and `Offloading` as the verb are `WRITING.md` rules 6 and 8. *Ingest*
    // remains the repository's word for phase 3; only the operator-facing string has to match
    // the word he uses.
    //
    // **The estimate lives here rather than on the pre-flight summary**, because this is the line
    // the eye goes to. Terry, 2026-08-05: *"That's where my eye looks for it."*
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
    report_corroboration(corroboration.as_ref(), progress.heading_was_erased());

    let geotag = geotag_phase(&plan, &targets, &outcome, args, &progress)?;
    progress.clear();
    report_geotag(
        geotag.as_ref(),
        progress.heading_was_erased(),
        targets.len(),
    );

    // Decision 22: last, because phases 4 and 5 still write to the archives. The volumes
    // must be released only once nothing remains to put on them.
    let ejecting = Instant::now();
    let deadline = started + RUN_BUDGET;

    // **All five devices start together and the SSDs report the moment they are down**, rather
    // than behind a card retry that cannot change their answer. The run that forced this had
    // three SSDs down in 15 s and a CFexpress that then retried for 22 minutes and never
    // released — and reported the pair as one line, `Released 5 devices in 22m 16s`. The answer
    // that mattered existed at fifteen seconds.
    //
    // **The headings print here rather than in the report** because `watch_attempt` starts
    // writing the moment the first device is asked, so a header arriving after its own rows
    // would read backwards. That constraint is also why `Eject` carries no badge — see
    // `DESIGN.md`'s layout rules for the container-versus-step distinction.
    if !args.no_eject && outcome.landed() {
        println!();
        println!();
        println!("Eject");
        println!();
        println!("    Progress Log");
    }

    let (released, (cards, cards_took, budget_spent)) = std::thread::scope(|scope| {
        let cards = scope.spawn(|| {
            let outcomes = release_cards(&plan, args, deadline);
            (outcomes, ejecting.elapsed(), Instant::now() >= deadline)
        });

        // The archives on this thread, so their result is in hand — and printed — without
        // waiting on a card retry that cannot change it.
        //
        // **A failed write to stdout is discarded rather than propagated**, here and for the
        // cards below. The run is over and every guarantee it makes is already on disk; a
        // closed pipe at this moment must not turn a landed, verified, ejected night into a
        // non-zero exit. `println!` would have panicked on the same condition.
        let released = eject_phase(&plan, &outcome, args, deadline);
        let _ = report_ssd_release(&mut io::stdout(), released.as_deref(), ejecting.elapsed());

        (
            released,
            cards.join().expect("the card release thread panicked"),
        )
    });

    let _ = report_card_release(&mut io::stdout(), &cards, cards_took, budget_spent);

    // One fact, two renderings — see [`everything_released`]. The gate badge and the verdict's
    // colour MUST NOT be able to disagree, and computing it here is what makes that structural
    // rather than a rule a later edit has to remember.
    let clean = everything_released(released.as_deref(), &cards);
    let _ = report_unhook_gate(&mut io::stdout(), clean, args.no_eject, released.is_none());
    verdict(
        &outcome,
        released.as_deref(),
        corroboration.as_ref(),
        args,
        clean,
    );

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

/// Decision 34's row in the card block. Screen only — nothing stores it.
const BODY_LABEL: &str = "Body";

/// How long after launch eject stops trying (decision 22).
///
/// **Ninety minutes, the top of the dinner window.** It was sixty until 2026-08-06, on the
/// reasoning that the program must exit before the operator returns — which he retired himself,
/// being the only possible source for it: *"if I do get back before it's done ejecting, I will
/// happily wait."* **The failure to protect against is a drive left in the tray**, not a run
/// still working when he walks in.
///
/// **What it buys lands where the risk is.** The retry window is whatever remains after phases
/// 3–5, so it is *smallest on the biggest days* — the nights with the most freshly written data
/// and the most likely veto. On the 415 GB record day sixty left roughly eight minutes; ninety
/// leaves nearly forty.
///
/// **Nothing waits on this**: a clean first-ask eject still exits in seconds. It is a ceiling on
/// patience, never a delay.
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
            .map(|resolved| {
                scope.spawn(move || release(resolved, deadline, cadence(args), PREPARE))
            })
            .collect();

        running
            .into_iter()
            .map(|handle| handle.join().expect("an eject thread panicked"))
            .collect()
    });

    Some(released)
}

/// Eject one resolved destination, turning both failure paths into a reportable outcome.
fn release(
    resolved: &destinations::Resolved,
    deadline: Instant,
    cadence: eject::Cadence,
    prepare: eject::Prepare,
) -> Released {
    // A destination resolved by serial always has a device; if it somehow does not, that is
    // a refusal to report rather than a reason to fail the run.
    let effort = match resolved.device.as_ref() {
        Some(device) => eject::eject(
            &resolved.volume,
            device,
            deadline,
            cadence,
            prepare,
            watch_attempt(&resolved.label),
        )
        .unwrap_or_else(|error| eject::Effort {
            outcome: eject::Outcome::Held {
                reason: format!("{error:#}"),
            },
            attempts: 1,
            waited: Duration::ZERO,
        }),
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

/// Whether every removable device came down — **the single fact the gate badge and the verdict's
/// colour both key on**.
///
/// **Computed once and handed to both, so they cannot disagree.** Deriving it twice would leave
/// the report able to print a green verdict under a yellow badge, which is the exact class of
/// contradiction Terry caught in `SAFE TO STORE` — and the kind that survives review because each
/// half is individually correct.
fn everything_released(released: Option<&[Released]>, cards: &[(String, eject::Effort)]) -> bool {
    released.is_some_and(|ssds| ssds.iter().all(|r| r.effort.outcome.is_ejected()))
        && cards.iter().all(|(_, e)| e.outcome.is_ejected())
}

/// The verdict's headline, worn as a badge in the same two colours as the rest of the report.
///
/// **Green in exactly one case and yellow in every other**, which is Terry's rule stated in his
/// own terms (2026-08-06): *"that final card status should be black-on-yellow if anything but
/// 'REMOVE AND PUT IN SAFE', and white on green if 'Yank all cards'."*
///
/// **`clean` comes from [`everything_released`] rather than from anything local**, so this badge
/// and `Safe to Unhook` are the same signal rendered twice rather than two opinions.
fn verdict_badge(clean: bool, text: &str) -> String {
    let headline = format!(" {text} ");
    if clean {
        style(headline).white().bold().on_green().to_string()
    } else {
        style(headline)
            .black()
            .on_true_color(255, 255, 0)
            .to_string()
    }
}

/// Decision 14's verdict: the last line, and its phrases appear nowhere else in the report.
///
/// **The words and the colour answer different questions, and `SAFE TO STORE` in yellow is a
/// real, intended combination.** Decision 14 says *only phase 3 may change the verdict*, and
/// decision 22 keeps the cards out of it entirely — so the **words** are decided by the archive
/// SSDs alone. The **badge** follows [`everything_released`], which includes the cards, because
/// it is a *come and look* signal rather than a verdict.
///
/// So a night where all four archives released and a camera card would not prints a yellow
/// ` SAFE TO STORE `. That reads oddly for a second and is right: the archives *are* safe to
/// store, and something above still wants a glance. **An earlier version of this comment claimed
/// the phrase was unreachable while yellow. It was wrong** — `actions.is_empty()` and `clean`
/// are not the same condition, and a stuck card separates them.
fn verdict(
    outcome: &pipeline::Outcome,
    released: Option<&[Released]>,
    corroborated: Option<&phase4::Report>,
    args: &Offload,
    clean: bool,
) {
    println!();

    if !outcome.landed() {
        println!(
            "►  {}  See the unverified counts above.",
            verdict_badge(false, "NOT SAFE")
        );
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

    // **`SAFE TO STORE` is a physical instruction and MUST NOT be printed while a drive is
    // mounted.** Terry, 2026-08-06, reading a `--no-eject` run: *"'SAFE TO STORE' seems wildly
    // counterintuitive there. In my mind that reads as 'safe to pull cables and put SSD in the
    // safe'."* He is right, and that is exactly how it was meant — but the line was printing on
    // a run that had deliberately left every drive connected, one line under a badge whose whole
    // job is to stop him pulling them. **The loudest line in the report was countermanding the
    // signal directly above it, in the direction that damages hardware.**
    //
    // So the phrase is now reserved. Landing safely and being safe to disconnect are two
    // different facts, and the verdict has to say which one it is asserting.
    let Some(released) = released else {
        let why = if args.no_eject {
            "Nothing was ejected"
        } else {
            "The eject stage never ran"
        };
        println!(
            "►  {}  {why}; {claim}.",
            verdict_badge(clean, "STILL MOUNTED")
        );
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

    // **Three states, and the middle one is the reason this is not two.** A device that
    // *dismounted* but would not power down is flushed and detached — pulling it out is the whole
    // of what remains, so it is not mounted and saying so would be wrong. A device still **held**
    // is mounted, and that is the case where `SAFE TO STORE` would be a lie.
    //
    // **Only the first branch can be green**, and it is the only one that reaches `clean` — the
    // other two are yellow by construction rather than by a flag someone could get wrong. A card
    // that would not release turns `clean` false without changing a word here, which keeps
    // decision 22 intact: the cards touch the *come and look* signal, never the wording and never
    // `exit_code`, which is not even given them.
    if actions.is_empty() {
        println!("►  {}  {claim}.", verdict_badge(clean, "SAFE TO STORE"));
    } else if held.is_empty() {
        println!(
            "►  {}  {}. {claim}.",
            verdict_badge(false, "UNPLUG FIRST"),
            actions.join(" AND ")
        );
    } else {
        println!(
            "►  {}  {}. {claim}.",
            verdict_badge(false, "STILL MOUNTED"),
            actions.join(" AND ")
        );
    }
}

/// The retry cadence this run was asked for — the default backoff unless `--eject-gap-seconds`
/// overrode it.
fn cadence(args: &Offload) -> eject::Cadence {
    match args.eject_gap_seconds {
        Some(seconds) => eject::Cadence::Every(Duration::from_secs(seconds)),
        None => eject::Cadence::Backoff,
    }
}

/// How a run prepares a volume before asking PnP to remove it. **Not configurable, deliberately.**
///
/// Lock and dismount once — so decision 2's flush guarantee holds — then ask bare on every retry.
///
/// **There was a `--eject-prepare` flag for one evening and it was removed on 2026-08-06.** Terry:
/// *"a config item that is never used should not exist — that's a dangerously unused code path
/// waiting to bite us."* Two of its three values were things nobody should ever select:
/// `every-attempt` re-dismounts before each ask and is **known** to hang unwinnably — 23 refusals
/// over 19 minutes, twice out of two runs — and `never` drops the flush decision 2 depends on.
///
/// **A flag whose only correct value is the default is not configuration, it is a live path that
/// runs exclusively when someone is already having a bad night.** The other two arms of
/// [`eject::Prepare`] remain in the library with their tests, and `examples/eject-one.rs` drives
/// them directly — so the experiment is still runnable without the shipped tool offering it.
const PREPARE: eject::Prepare = eject::Prepare::FirstAttemptOnly;

/// One timestamped line per eject attempt, printed as it happens.
///
/// **The reason prints only when it CHANGES**, which is the one judgment call here. Sixteen
/// identical vetoes a minute apart is a wall of text that hides the interesting case, and a veto
/// that changes shape mid-fight is the whole open question about what holds a card. That is how
/// `PNP_VETO_TYPE(5)` was ever seen at all.
fn watch_attempt(label: &str) -> impl FnMut(eject::Attempt<'_>) + '_ {
    let mut said: Option<String> = None;

    move |attempt| {
        let (word, reason) = match attempt.outcome {
            eject::Outcome::Ejected => ("RELEASED", None),
            eject::Outcome::Dismounted { reason, .. } => ("dismounted", Some(reason)),
            eject::Outcome::Held { reason } => ("held", Some(reason)),
        };

        let next = match attempt.retry_in {
            Some(pause) => format!("retry in {}", duration_aligned(pause)),
            None => format!("after {}", duration_aligned(attempt.elapsed)),
        };

        let mut line = format!(
            "        {}  {label:<10} #{:<3} {word:<10} {next}",
            Utc::now().format("%H:%M:%SZ"),
            attempt.number
        );

        // **Four spaces deeper than the attempt line it belongs to.** Terry, 2026-08-06:
        // *"indent makes my brain see the indented lines as deeper/more detail."* It sat level
        // with the device column, which read as a second row rather than as detail about the
        // one above. Same relationship geotag's gap explanation has with its own row.
        //
        // 23 = the 8-space row indent, plus a 9-character timestamp and its two trailing
        // spaces, plus the four that make it detail rather than a sibling.
        if let Some(reason) = reason
            && said.as_deref() != Some(reason.as_str())
        {
            line.push_str("\n                       ");
            line.push_str(reason);
            said = Some(reason.clone());
        }

        // **One `println!`, because up to five of these run concurrently.** The attempt line and
        // its reason were two separate calls until 2026-08-06. `println!` locks stdout so a
        // single line cannot tear, but nothing held the lock *across* the pair — so another
        // device's attempt could land between a line and the reason belonging to it, printing
        // a veto under the wrong device.
        //
        // **It had not bitten, and that was luck rather than safety.** Two devices produced
        // reason lines in the same second on the 415 GB day and happened to interleave
        // correctly; the window only widened when the cards became concurrent as well.
        println!("{line}");
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
) -> Vec<(String, eject::Effort)> {
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
    let both: Vec<_> = std::iter::once((PRIMARY_LABEL, &plan.cards.source))
        .chain(
            plan.cards
                .other
                .as_ref()
                .map(|(card, _)| (SECONDARY_LABEL, card)),
        )
        .collect();

    // **Concurrently, for exactly the reason the archives are** — and this was a defect until
    // 2026-08-06. The two cards share one deadline, so run in sequence a card that retries to
    // the end of the budget leaves the other a single attempt at the very end of the run.
    // Measured that evening: `Primary` reached **fourteen attempts across nine minutes while
    // `Secondary` had not been asked once**. It had gone unnoticed because cards had always
    // resolved in seconds, so a sequential loop and a concurrent one looked identical.
    //
    // **Joined in spawn order**, so the report still lists Primary before Secondary however
    // they finish. Order in the report is about roles, not about who won.
    std::thread::scope(|scope| {
        let running: Vec<_> = both
            .into_iter()
            .map(|(role, card)| {
                scope.spawn(move || (role.to_string(), release_card(role, card, args, deadline)))
            })
            .collect();

        running
            .into_iter()
            .map(|handle| handle.join().expect("a card release thread panicked"))
            .collect()
    })
}

/// One card's release, turning every failure path into a reportable [`eject::Effort`].
///
/// **The whole `Effort` is kept, not just its outcome.** Discarding `attempts` and `waited`
/// is what made *how reliably does a held card recover* unanswerable on 2026-08-06: the one
/// multi-minute release this project had seen — 11 m 17 s — carried no attempt count, so nobody
/// could say whether that was sixteen asks or one lucky late one. Every run now contributes
/// that data point for free, which is the only way a sample larger than one will ever exist.
fn release_card(
    role: &str,
    card: &cards::Card,
    args: &Offload,
    deadline: Instant,
) -> eject::Effort {
    match storage::device_of(&card.volume) {
        Ok(device) => eject::eject(
            &card.volume,
            &device,
            deadline,
            cadence(args),
            PREPARE,
            watch_attempt(role),
        )
        .unwrap_or_else(|error| eject::Effort {
            outcome: eject::Outcome::Held {
                reason: format!("{error:#}"),
            },
            attempts: 1,
            waited: Duration::ZERO,
        }),
        // Zero attempts, because none was possible — see `Effort::attempts`.
        Err(error) => eject::Effort {
            outcome: eject::Outcome::Held {
                reason: format!("the card reports no device to release: {error:#}"),
            },
            attempts: 0,
            waited: Duration::ZERO,
        },
    }
}

/// The archive SSDs' half of the eject stage, printed the moment all of them are resolved.
///
/// **Reported separately from the cards, and ahead of them**, because the stakes differ by
/// Terry's own measure — *"SSDs are like two orders of magnitude more important"*. An SSD that
/// will not power down reaches the exit code (decision 18); a stuck card is tidiness. The run
/// that forced the split had three SSDs down in 15 s and a card retrying for 22 minutes, and
/// reported the pair as one line: `Released 5 devices in 22m 16s` — withholding the answer that
/// mattered, and miscounting, since four devices *had* been released.
///
/// **Writes to `out` rather than `println!` so the failure branches can be asserted** — a device
/// has to actually refuse to produce them, so a suite that only sees the clean path cannot prove
/// they render at all.
fn report_ssd_release(
    out: &mut impl Write,
    released: Option<&[Released]>,
    elapsed: Duration,
) -> io::Result<()> {
    // `None` means the stage never ran — either `--no-eject`, or phase 3 did not land so the
    // gate never opened. There is then no `Progress Log`, no `Travel SSDs` and no `Cards`, so
    // this prints the container that `main` skipped and leaves the rest to the gate, which
    // states the reason itself and so keeps the badge directly above its own cause.
    let Some(ssds) = released else {
        writeln!(out)?;
        writeln!(out)?;
        writeln!(out, "Eject")?;
        return Ok(());
    };

    if !ssds.is_empty() {
        let down = ssds
            .iter()
            .filter(|r| r.effort.outcome.is_ejected())
            .count();
        writeln!(out)?;
        writeln!(
            out,
            "    {:<pad$}{}",
            "Travel SSDs",
            step_badge(down == ssds.len()),
            pad = badge_pad(4)
        )?;
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
                writeln!(
                    out,
                    "        {:<10} ejected; ready to disconnect{effort}",
                    r.label
                )?;
            }
            // Worth its own wording: the bytes are flushed and detached either way, and an
            // operator who reads "failed" for this would worry about the wrong thing.
            eject::Outcome::Dismounted { reason, .. } => writeln!(
                out,
                "        {:<10} dismounted, not powered down — safe to unplug{effort}\n            {reason}",
                r.label
            )?,
            eject::Outcome::Held { reason } => writeln!(
                out,
                "        {:<10} still mounted — eject it from the tray{effort}\n            {reason}",
                r.label
            )?,
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
    // **Indented as content of the `Travel SSDs` sub-heading, and no longer repeating its
    // name.** Terry, 2026-08-06: the summary *"falls within the Travel SSDs subsection"*, so it
    // sits at the same depth as the rows it closes rather than level with the heading — group
    // at 4, everything belonging to it at 8. Position carries the scope, which is why saying
    // "Travel SSDs" again would be the same stutter the `Corroborating` heading had.
    writeln!(out)?;
    if down == ssds.len() {
        writeln!(
            out,
            "        All SSDs put to bed in {}. Safe to store.",
            duration(elapsed)
        )?;
    } else {
        // **This branch has to be able to fire, and a suite that only sees the clean path
        // cannot prove it does** — `a_stuck_ssd_is_named_and_not_counted_as_put_to_bed` is
        // that proof. The line it replaced printed a device count that included devices it
        // had just described as not powered down.
        let stuck = labels(ssds, |o| !o.is_ejected());
        writeln!(
            out,
            "        {} of {} SSDs put to bed in {}. {} still needs you; see above.",
            count(down),
            count(ssds.len()),
            duration(elapsed),
            stuck.join(", ")
        )?;
    }

    Ok(())
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
fn report_card_release(
    out: &mut impl Write,
    cards: &[(String, eject::Effort)],
    elapsed: Duration,
    budget_spent: bool,
) -> io::Result<()> {
    if cards.is_empty() {
        return Ok(());
    }

    // **The card badge can come out red, and that is not a downgrade.** Decision 22 keeps cards
    // away from the verdict and the exit code, and this does not touch either — it says *this
    // one needs a hand*, which is true, while the line beneath still says nothing was ever
    // written to it. A badge that could only ever be green would be the check-that-cannot-fail
    // `REVIEWING.md` warns about.
    let released = cards.iter().filter(|(_, e)| e.outcome.is_ejected()).count();
    writeln!(out)?;
    writeln!(
        out,
        "    {:<pad$}{}",
        "Cards",
        step_badge(released == cards.len()),
        pad = badge_pad(4)
    )?;
    for (label, effort) in cards {
        // **The same effort suffix the SSD rows carry, and it is here to build a sample.**
        // A card that took sixteen asks over eleven minutes and one that got lucky on its
        // second look identical without it — which is exactly the question this project could
        // not answer about its only long release. Printed on the card rows for evidence
        // rather than for the operator, who has nothing to do with the number either way.
        let asked = if effort.attempts > 1 {
            format!(
                " after {} attempts over {}",
                effort.attempts,
                duration(effort.waited)
            )
        } else {
            String::new()
        };

        match &effort.outcome {
            // A card comes *out*; an SSD gets *unplugged*. Same event, different next action.
            eject::Outcome::Ejected => {
                writeln!(
                    out,
                    "        {label:<10} ejected; remove card from reader{asked}"
                )?;
            }
            // Neither remaining branch is phrased as a failure. The tool never wrote to a card,
            // so it was safe to pull before any of this ran; what was lost is tidiness, and an
            // operator reading "failed" here would worry about data that was never at risk.
            eject::Outcome::Dismounted { reason, .. } => writeln!(
                out,
                "        {label:<10} dismounted, still listed — safe to pull anyway{asked}\n            {reason}",
            )?,
            eject::Outcome::Held { reason } => writeln!(
                out,
                "        {label:<10} still mounted — safe to pull anyway, nothing was written to it{asked}\n            {reason}",
            )?,
        }
    }

    // The second closing fact, and the one nobody is waiting on. Its elapsed time shares an
    // origin with the SSD line above, so the larger of the two is the answer to "how long to
    // put all five to bed" — which Terry asked to keep, just not at the cost of the number
    // that matters.
    let down = cards.iter().filter(|(_, e)| e.outcome.is_ejected()).count();
    // Indented under `Cards`, same as the SSD half — see the note there.
    writeln!(out)?;
    if down == cards.len() {
        writeln!(
            out,
            "        All cards put to bed in {}.",
            duration(elapsed)
        )?;
    } else {
        let stuck: Vec<&str> = cards
            .iter()
            .filter(|(_, e)| !e.outcome.is_ejected())
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

        writeln!(
            out,
            "        {} of {} cards put to bed in {}. {} never released ({}). \
             Safe to pull anyway: nothing was written to them.",
            count(down),
            count(cards.len()),
            duration(elapsed),
            stuck.join(", "),
            gave_up
        )?;
    }

    // **The cost of doing this properly, said out loud rather than discovered.** Releasing a
    // card means ejecting its device, and for a USB card reader that device *is* the reader:
    // it powers down with the card and does not wake when the next card goes in. The
    // Thunderbolt reader is untouched, because the NVMe disk's parent is a PCIe port rather
    // than the reader itself. Naming the consequence here is what turns a mystery at the next
    // offload into an expected chore — and pre-flight refuses anyway, so a forgotten replug
    // costs ten seconds rather than a night.
    if cards.iter().any(|(_, e)| e.outcome.is_ejected()) {
        writeln!(out)?;
        writeln!(
            out,
            "  !  A USB card reader powers down with its card and needs a replug before the\n     \
             next offload. The Thunderbolt reader does not. If you forget, pre-flight\n     \
             refuses with ONLY ONE CARD FOUND rather than running short."
        )?;
    }

    Ok(())
}

/// The `Eject` section's roll-up — **a go/no-go on the physical act of unplugging things.**
///
/// Green in exactly one case: every SSD *and* every card came down. `DESIGN.md`, *the badge
/// column is a go/no-go on unplugging things*, has the reasoning and the standing orders.
///
/// **Two things here look wrong and are not.** Cards can turn it yellow without contradicting
/// decision 22, because that decision keeps them out of the *verdict* and the *exit code* and
/// this touches neither. And `--no-eject` is a guaranteed yellow on purpose — that run leaves
/// every drive mounted, so green would be the one output capable of causing harm.
///
/// **It states its own reason when the stage never ran**, so the badge is never separated from
/// its cause; an earlier version left them two lines apart and they read as unrelated facts.
fn report_unhook_gate(
    out: &mut impl Write,
    clean: bool,
    no_eject: bool,
    never_ran: bool,
) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "    {:<pad$}{}",
        "Safe to Unhook",
        step_badge(clean),
        pad = badge_pad(4)
    )?;

    if never_ran {
        // **Says why, and MUST NOT say what the verdict says.** Decision 14 reserves the
        // verdict's phrases to the verdict, and this line briefly read *"— every drive is still
        // mounted"* two lines above a verdict of `STILL MOUNTED`. The badge already carries the
        // state; this only has to carry the cause.
        let why = if no_eject {
            "Withheld by --no-eject"
        } else {
            "Not reached — the run did not land"
        };
        writeln!(out, "        {why}")?;
    }

    Ok(())
}

/// The report's duration format, for **prose**: `5m 0s`, `15m 12s`.
///
/// **One shape, always** — this used to drop the minutes below a minute and render `38s`, so
/// the same quantity appeared in two formats depending on how long something took. Terry asked
/// for that consistency on 2026-08-06.
///
/// **Never zero-padded**, because `00s` reads as a clock and this is a measurement.
fn duration(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!("{}m {}s", seconds / 60, seconds % 60)
}

/// The same value **padded to a fixed width for a column**: ` 5m  0s`, `15m 12s`.
///
/// Terry, 2026-08-06: *"reserve two chars for both min and sec, no zero padding, right align
/// values."* Only the live eject attempts stack durations directly under one another, so this
/// is the one place the padding does any work.
///
/// **Deliberately not used in prose**, and that is the same call he made about the LANDED
/// banner an hour later — *"leave landed alone, it looks better as is."* A padded value mid
/// sentence reads as a double space rather than as alignment, because there is nothing beneath
/// it to align with. The format is shared; the padding is applied where it is load-bearing.
fn duration_aligned(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!("{:>2}m {:>2}s", seconds / 60, seconds % 60)
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

    report_passes(outcome);

    // **Two blank lines, because LANDED is the top-level heading of the whole run.** Every
    // phase gets two and this outranks all of them; it had one until 2026-08-06.
    println!();
    println!();
    // **No phase number.** "Phase 3" is this repository's word, not the operator's, and the
    // reader is six months out of practice at 11pm in a hotel. A label that needs `DESIGN.md`
    // to decode says nothing at the moment it is read.
    //
    // **The sentence is literally true, which is why it is safe to print.** LANDED is the point
    // where every file exists on all four destinations and has been read back off the media and
    // compared (decisions 2, 14) — so *the data is safe* is a guarantee, not encouragement.
    //
    // **Bound to a variable so the closing rule can match its width**, which moves with the
    // elapsed time and so cannot be a literal. **Indented as a subsection of `Offloading`** —
    // Terry, 2026-08-06: LANDED and `Corroborating` are what offloading *produced*, where
    // geotagging is *"value add and not part of offloading"* and stays at column 0.
    let banner = format!(
        "    ═══ LANDED in {minutes}m {seconds:02}s · you can breathe, Terry, your data is safe ═══"
    );
    println!("{banner}");
    println!();
    println!(
        "        {} files · {} GiB · read once from the source card",
        count(outcome.files),
        offload::human::gib_up(outcome.bytes)
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
            "        {} GiB moved · {:.1} GiB/s · {:.1} Gbps",
            offload::human::gib_up(moved),
            moved as f64 / GIB / seconds,
            moved as f64 * 8.0 / 1e9 / seconds
        );
    }
    println!();

    // **These four lines are where the verdict is earned**, one destination at a time — Terry
    // calls them the biggest warm fuzzy of the run — so they are worth being legible at a glance
    // rather than merely present.
    //
    // **The badge is real**: it reports a comparison made moments ago against every file on the
    // destination, and a failure here is the difference between LANDED and NOT SAFE.
    //
    // **Yellow, not red — this was the last exception standing**, and it was argued to be the
    // case that earned red *because* it is serious. That argument is the one the standing order
    // refuses: severity is not what the colour encodes, the action is. See `DESIGN.md`, *the
    // opposite of green is never red*.
    for destination in &outcome.destinations {
        let verdict = if destination.failed.is_empty() {
            style(" OK ".to_string()).white().bold().on_green()
        } else {
            style(format!(" {} UNVERIFIED ", count(destination.failed.len())))
                .black()
                .on_true_color(255, 255, 0)
        };
        println!(
            "        {:<8} {} written · {} skipped · {} verified   {verdict}",
            destination.label,
            count(destination.written),
            count(destination.skipped),
            count(destination.verified),
        );
    }

    if !outcome.unfiled.is_empty() {
        println!();
        println!(
            "        !  {} frame(s) had no readable capture time and are in _unfiled",
            count(outcome.unfiled.len())
        );
    }

    // **Closed with a rule of its own width**, so the block reads as a bounded thing rather
    // than as a heading followed by text that trails off into the next phase. Terry asked for
    // it on 2026-08-06; `chars()` rather than `len()` because `═` is three bytes and a
    // byte-length rule would come out three times too long.
    // Trimmed before counting, then re-indented, so the rule is the width of the *banner* and
    // not of the banner plus its indent.
    //
    // **Two blank lines below, matching the two above the banner.** The block is framed
    // symmetrically or it reads as top-heavy — Terry's note, and the second one here means
    // `Corroborating` supplies only one of the two it needs.
    println!();
    println!("    {}", "═".repeat(banner.trim_start().chars().count()));
    println!();

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
        offload::human::gib_up(cards.bytes),
        rig.distinct_disks,
    );

    // **`primary`/`secondary`/`sole`, the same words the eject block and the manifest use.**
    // These said `source` and `other` until 2026-08-05 — a second vocabulary for the same two
    // cards in one screen, which `WRITING.md` rule 8 exists to stop. `sole` rather than
    // `primary` when there is no second card is the informative case: it tells a tired operator
    // that corroboration will be waived *before* the run starts (decision 7).
    //
    // **Cards and destinations share one set of column widths**, measured across both blocks
    // before either prints, so the rates stack into a column the eye can compare. These are the
    // numbers that say "this card is dying" (decision 32), and a ragged edge is part of why a
    // faulty 73 MB/s card read as unremarkable beside a healthy 222. Measured rather than
    // constant because the destination subpath is configurable, and counted in `chars` so a
    // non-ASCII path does not over-pad. Separators per rule 6.
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

    // **Decision 34's line sits in the card block, not in the final summary.** The decision
    // placed it "beside decision 23's timezone line"; no such line exists, and pre-flight is
    // where the decision's own payoff argument puts it — a body that records no UTC offset
    // sends every frame to `_unfiled`, and knowing that in ten seconds rather than after
    // phase 3 is the whole point. It shares the card labels' column and carries **no badge**:
    // INFO never touches the verdict, and a mismatch is not a reason to leave drives plugged in.
    let body_row: Option<String> = cards.body.as_ref().map(preflight::BodyReport::to_string);

    let dest_rows: Vec<(&str, String, String, &str)> = rig
        .survey
        .found
        .iter()
        .map(|resolved| {
            (
                resolved.label.as_str(),
                resolved.root.display().to_string(),
                offload::human::gib_down(resolved.volume.free_bytes),
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
        .chain(body_row.iter().map(|_| BODY_LABEL))
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
    // Last in the block, because it describes what wrote the cards rather than a card.
    if let Some(line) = &body_row {
        println!("        {BODY_LABEL:<label_column$}{line}");
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
    let payload = offload::human::gib_up(cards.bytes);

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

/// Wall-clock estimate for **both** LANDED and program exit — the operator is deciding whether
/// to go to dinner, and dinner ends when the program does.
///
/// # The three terms, and why only one of them is a guess
///
/// **Phase 3, to LANDED** — the slowest destination absorbs N of writes and N of read-back
/// (decision 2's arithmetic) at 450–800 MB/s. Constants, and the spread is wide on purpose:
/// destination throughput is not measured at pre-flight.
///
/// **Phase 4, corroboration** — computed from the second card's rate, which pre-flight *has just
/// measured*, so this term is grounded rather than assumed. [`CORROBORATION_EFFICIENCY`] is the
/// read-then-hash serialization tax: 170 MB/s achieved against a 205 MB/s card, 2026-08-04.
///
/// **Phase 5 and eject** — a minute, near enough.
///
/// **Corroboration is ADDED, not overlapped**, because that is what the code does — `phase4`
/// begins after `pipeline::run` returns. Decision 2 argues the two-pass shape *lets* phase 4
/// start during the verify pass; that benefit is described and not built. **Estimating as though
/// it were would understate every run by a quarter of an hour.**
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
        "{} to LANDED, {} to done",
        span(landed_fast, landed_slow),
        span(
            landed_fast + corroboration + TAIL_MINUTES,
            landed_slow + corroboration + TAIL_MINUTES,
        ),
    )
}

/// A minute range, collapsed when both ends round to the same number.
///
/// **`1-1 min` is not a range, it is a number wearing a hyphen**, and on a small day both ends
/// of both spans landed there — `est. 1-1 min to LANDED, 2-2 min to done`. A reader who has to
/// notice that the two halves are identical before concluding *one minute* is being charged for
/// precision the estimate does not have.
///
/// The wide case is untouched, because there the range is the honest part: the two ends come
/// from the fastest and slowest destination, and a 415 GB night genuinely spans them.
fn span(fast: f64, slow: f64) -> String {
    let (fast, slow) = (fast.ceil() as u64, slow.ceil() as u64);

    if fast == slow {
        format!("{fast} min")
    } else {
        format!("{fast}-{slow} min")
    }
}

/// `offload verify <DEST>` — decision 20.
///
/// Reads nothing but the disk itself, so it works on a machine that has never seen this
/// tool's configuration. That is the promise, and it is why this takes a path rather
/// than a config label.
/// `offload geotag <ROOT> <GPX...>` — decision 30's subcommand, and RawGeotag's whole job.
///
/// **It re-reads the raws, and that is the one real difference from phase 5.** Decision 10 has
/// phase 3 hand capture times forward because every file was already in RAM to be hashed;
/// nothing has been read here, so this opens each frame to seek its EXIF. That is ~0.3 s for
/// 3,883 files (decision 17), not a copy of the day.
///
/// **`--utc-offset` deliberately does not come across** (decision 23). RawGeotag needed it for a
/// body that recorded no timezone; reintroducing it would reintroduce the gate it implies, and
/// a frame with no offset is counted and reported here rather than guessed at. Guessing is the
/// one thing the project mantra forbids — **a geotag off by more than 5 m is worse than none.**
fn geotag_tree(
    root: &Path,
    tracks: &[PathBuf],
    limits: GapLimits,
    force: bool,
    dry_run: bool,
) -> Result<ExitCode> {
    let tracks = expand_tracks(tracks)?;
    let files = raw_files(root).with_context(|| format!("walking {}", root.display()))?;

    if files.is_empty() {
        // **Not silence, and not success.** An empty walk and a tagged run must not read the
        // same way — `verify`'s `NOTHING TO VERIFY` is the same rule (decision 20).
        println!();
        println!("  NOTHING TO TAG — no .CR3 files under {}", root.display());
        println!("  Either this is not a directory of raws, or they are somewhere else.");
        return Ok(ExitCode::from(2));
    }

    println!();
    println!("Geotagging {}", root.display());
    println!();
    println!(
        "    {} frames · {} track(s)",
        count(files.len()),
        count(tracks.len())
    );

    let mut parser = MediaParser::new();
    let mut landed = Vec::with_capacity(files.len());
    let mut no_offset = 0usize;
    let mut no_capture_time = 0usize;
    let mut unreadable = 0usize;

    for (file, format) in &files {
        let relative = file.strip_prefix(root).unwrap_or(file).to_path_buf();

        match capture_time(&mut parser, file, *format, None) {
            Ok(Capture::Resolved { at, .. }) => landed.push(phase5::Landed {
                relative,
                captured: at,
            }),
            Ok(Capture::NeedsOffset) => no_offset += 1,
            Ok(Capture::NoCaptureTime) => no_capture_time += 1,
            // Counted rather than fatal: one corrupt frame must not stop the other 7,000
            // being tagged, and decision 18's fatal-out is about the *offload*, not this.
            Err(_) => unreadable += 1,
        }
    }

    // The destination is the tree itself — one root, tagged in place. Phase 5 writes a sidecar
    // beside each raw, so `root` is both where the frames are and where the packets go.
    //
    // **A dry run is the same pass with no destinations**, which is the honest reading of the
    // slice rather than a trick: phase 5 correlates every frame and counts every outcome, and
    // the write loop it feeds simply has nothing to iterate. Every number below is real except
    // `written`, which is zero because nothing was written.
    let destinations: &[Destination] = if dry_run {
        &[]
    } else {
        &[Destination {
            label: root.display().to_string(),
            root: root.to_path_buf(),
        }]
    };

    let progress = offload::progress::Progress::detect();
    let report = phase5::run(&landed, destinations, &tracks, limits, force, &progress)?;

    if dry_run {
        // **`report_geotag` is deliberately not called here.** Its closing line multiplies
        // frames by destinations into a sidecar count, and on a dry run that reads as work
        // done — the DRY RUN line above it would be arguing with the line below it.
        println!();
        println!(
            "    {} tagged · {} outside track · {} in a gap",
            count(report.tagged),
            count(report.outside_track),
            count(report.in_gap)
        );
        println!();
        println!(
            "    DRY RUN — nothing written. {} of {} frames would be tagged.",
            count(report.tagged),
            count(files.len())
        );
        // **Not "would write N sidecars".** Whether each one is written depends on
        // `--force-xmp` *and* on what is already on disk, and a preview that guessed at that
        // would be the confident-and-wrong answer this project keeps refusing.
        println!("    Re-run without --dry-run to write them.");
    } else {
        report_geotag(Some(&report), false, destinations.len());
    }

    for (count_of, what) in [
        (
            no_offset,
            "no timezone offset — decision 23 does not guess one",
        ),
        (no_capture_time, "no capture time at all"),
        (unreadable, "could not be read"),
    ] {
        if count_of > 0 {
            println!("    {} skipped: {what}", count(count_of));
        }
    }

    println!();
    Ok(ExitCode::SUCCESS)
}

/// Every raw the engine can read under `root`, paired with the format that reads it.
///
/// **Not `pipeline::cr3_files`, and that difference is the point.** Phase 3 is CR3-only by
/// constraint (decision 24) because that is what the camera shoots; this subcommand replaces
/// RawGeotag, which read **NEF** as well — and the two take different paths through `raw.rs`,
/// since `read_strategy` sends CR3 streaming and NEF whole-file. Walking only CR3 here would
/// have quietly dropped a format the engine already handles.
///
/// The extension table is `RawFormat`'s own, asked through `matches_extension`, so adding a
/// format to the engine is enough — nothing here holds a second opinion about what a raw is.
fn raw_files(root: &Path) -> Result<Vec<(PathBuf, RawFormat)>> {
    let mut found: Vec<(PathBuf, RawFormat)> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let path = entry.into_path();
            let extension = path.extension()?.to_str()?;
            let format = RawFormat::ALL
                .iter()
                .find(|format| format.matches_extension(extension))?;
            Some((path, *format))
        })
        .collect();

    // Sorted **by path**, so a run is deterministic and two runs of the same tree report in the
    // same order. Deliberately not by deriving `Ord` on `RawFormat`: the engine is validated
    // code (decision 17) and does not need a trait added to it for a caller's convenience.
    found.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(found)
}

/// Each argument is a `.gpx` file or a directory of them, flattened and sorted.
///
/// **Not recursive**, matching RawGeotag: a directory of tracks is a directory of tracks, and
/// walking into subdirectories would quietly pull in a different day's logging.
fn expand_tracks(given: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut tracks = Vec::new();

    for path in given {
        if path.is_dir() {
            let entries =
                std::fs::read_dir(path).with_context(|| format!("reading {}", path.display()))?;
            tracks.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .and_then(|extension| extension.to_str())
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("gpx"))
                    }),
            );
        } else {
            tracks.push(path.clone());
        }
    }

    if tracks.is_empty() {
        anyhow::bail!("no .gpx tracks found in what you named");
    }

    tracks.sort();
    Ok(tracks)
}

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

/// A static record of the two passes, printed where the live bars used to be.
///
/// **The bars are erased before this and cannot simply be left.** `MultiProgress` repaints
/// wherever the cursor is, so a plain `println!` beside live bars *collides* — on 2026-08-05
/// that drew the LANDED banner inside eight progress rows, twice. So `progress.clear()` hands
/// the terminal back first, and what the operator watched fill up disappears with it.
///
/// **Two lines rather than eight.** A row per destination per pass would be four identical
/// `50/50`s twice over, and the LANDED table three lines below already says more. What this adds
/// is the *shape* of the work — two passes happened, both completed — not the detail.
///
/// **The write pass counts written plus skipped**, because a convergence run writes nothing and
/// still has to hash every target to prove the skip (see [`pipeline::DestinationOutcome`]).
/// Counting `written` alone would report a converged run as having done no work at all.
fn report_passes(outcome: &pipeline::Outcome) {
    if outcome.destinations.is_empty() {
        return;
    }

    let all = outcome.destinations.len();
    let files = outcome.files;

    // Named rather than inlined: "how many finished" and "how many there are" appearing as two
    // bare numbers in a format string is exactly where an off-by-one hides.
    let wrote = |d: &pipeline::DestinationOutcome| d.written + d.skipped == files;
    let written_through = outcome.destinations.iter().filter(|d| wrote(d)).count();
    // **`failed` as well as the count**, or a destination that read back a mismatch would still
    // be counted as having verified every file and the badge would come out green on a run the
    // verdict calls NOT SAFE.
    let verified_through = outcome
        .destinations
        .iter()
        .filter(|d| d.verified == files && d.failed.is_empty())
        .count();

    // **Says "all N" only when it is all of them.** A pass that finished on three of four
    // destinations is the interesting case, and a line that rounds it up to "all" would hide
    // the one thing worth seeing.
    // Capitalized: it opens a line, and every line a human reads starts with a capital.
    let titled = |done: usize| {
        if done == all {
            format!("All {} destinations", count(all))
        } else {
            format!("{} of {} destinations", count(done), count(all))
        }
    };

    // **Heading at 4, status at 8 — the shape every other section uses**, rather than a label
    // and its value sharing a line. Terry, 2026-08-06, reading it on a real terminal: the
    // one-line form matched neither `Camera Cards` above it nor `Travel SSDs` below, and a
    // layout outlier reads as an outlier fact.
    //
    // **One blank line above `Verifying`, not two.** These are the two *passes of* offloading
    // rather than phases in their own right, and `progress.rs` sets that convention — two for a
    // phase, one for a pass — which the live bars already follow.
    println!();
    println!(
        "    {:<pad$}{}",
        "Writing",
        step_badge(written_through == all),
        pad = badge_pad(4)
    );
    println!(
        "        {} · {}/{}",
        titled(written_through),
        count(files),
        count(files)
    );
    println!();
    println!(
        "    {:<pad$}{}",
        "Verifying",
        step_badge(verified_through == all),
        pad = badge_pad(4)
    );
    println!(
        "        {} · {}/{}",
        titled(verified_through),
        count(files),
        count(files)
    );
}

/// The badge beside a step's heading — **whether the numbers below need reading at all.**
///
/// Never red; see [`docs/DESIGN.md`](../../../docs/DESIGN.md), *the opposite of green is never
/// red* and *the badge column is a go/no-go on unplugging things*, which carry the reasoning and
/// the standing orders.
///
/// **Three things here look wrong and are not, so leave them alone without reading that first:**
///
/// - **The yellow badge is not bold.** `black().bold()` emits `ESC[1;30m` and this console
///   promotes bold black to *intense* black, which is grey. Adding `bold` to make it louder is
///   what silences it. Green keeps `bold` because bright white is genuinely stronger on green.
/// - **The background is a true colour, not `on_yellow()`.** Palette yellow is `#C19C00` here —
///   a dark mustard that black barely shows on.
/// - **Both badges are five cells.** `!!!` is two wider than `✓`, hence the tick's extra space
///   each side.
fn step_badge(clean: bool) -> String {
    if clean {
        style("  \u{2713}  ").white().bold().on_green().to_string()
    } else {
        style(" !!! ")
            .black()
            .on_true_color(255, 255, 0)
            .to_string()
    }
}

/// The **absolute** column every badge starts at, whatever its heading's indent.
///
/// **Pinned rather than padded from the heading**, because the badges are read as a column and
/// a ragged one does not scan. `Geotagging` sits at indent 0 and the rest at 4, so padding each
/// label to a fixed width put its tick four columns left of the others — which Terry spotted
/// immediately and then talked himself out of, on the grounds that they were at different
/// levels. **The hierarchy is carried by the heading's own indent; the badge is a separate
/// signal and belongs in a straight line.**
const BADGE_COLUMN: usize = 19;

/// The label width for a heading at `indent`, so its badge lands on [`BADGE_COLUMN`].
fn badge_pad(indent: usize) -> usize {
    BADGE_COLUMN.saturating_sub(indent)
}

/// A phase heading carrying its badge — and **the badge reaches a captured log too**.
///
/// **The problem this solves.** At a terminal `progress.clear()` erases the live heading, so
/// the record reprints it and the badge goes there. In a log nothing is cleared, the heading is
/// still on screen, and reprinting it would stutter — which would have left `Corroborating` and
/// `Geotagging` with no badge at all in **exactly the mode Terry gets when running the offload
/// through Claude**, which `CONOPS.md`'s shooting-day contract says is most nights with
/// internet.
///
/// So it returns a suffix instead: empty when the heading carried the badge, and the badge
/// itself when the caller must put it on the status line. **One signal, two placements, never
/// absent.**
#[must_use]
fn badged_heading(name: &str, indent: usize, erased: bool, clean: bool) -> String {
    let badge = step_badge(clean);
    println!();

    if !erased {
        return format!("   {badge}");
    }

    if indent == offload::progress::PHASE {
        println!();
    }
    println!(
        "{:indent$}{:<pad$}{badge}",
        "",
        name,
        pad = badge_pad(indent)
    );
    String::new()
}

/// A phase heading with the blank lines it is owed, or just the gap when the live heading is
/// still on screen and only the record follows.
///
/// **Two blank lines above a phase, one above a pass**, and the blanks belong to the heading
/// rather than to the record — in a captured log the live heading survives, so reprinting it
/// would stutter while two blank lines would open a gap above the rows it labels.
///
/// **This comment had drifted onto [`step_badge`]** until 2026-08-06, the same way `progress.rs`
/// lost `clear`'s. See that one for the mechanism.
fn phase_heading(name: &str, indent: usize, erased: bool) {
    println!();
    if erased {
        // Two blanks for a phase, one for a subsection — `progress.rs`'s own convention, and
        // the reason `Corroborating` reads as belonging to `Offloading` rather than starting
        // something new.
        if indent == offload::progress::PHASE {
            println!();
        }
        println!("{:indent$}{name}", "");
    }
}

/// Decision 4: **say "this card looks like it is failing" in those words.** A photographer
/// reading a bare count should not have to know what normal looks like.
fn report_corroboration(report: Option<&phase4::Report>, heading_was_erased: bool) {
    let Some(report) = report else {
        // Always headed: the waived path never drew bars, so nothing put the word on screen.
        phase_heading("Corroborating", offload::progress::PASS, true);
        println!("        Waived — only one card was present (--allow-single-source)");
        return;
    };

    // **Clean means nothing disagreed.** A transient read error is deliberately not counted
    // against it — the re-read agreed, so the data is fine and the badge is about the data.
    // `suspect_card` gets its own loud paragraph below and does not need to dim this.
    let badge = badged_heading(
        "Corroborating",
        offload::progress::PASS,
        heading_was_erased,
        report.mismatched.is_empty(),
    );
    print!("        {} matched", count(report.matched));
    if report.transient > 0 {
        // Not a data problem — the re-read agreed. It is a *reader* problem, and worth
        // saying so before it becomes one.
        print!(
            " · {} transient read error(s), re-read agreed",
            count(report.transient)
        );
    }
    println!(" · {} mismatched{badge}", count(report.mismatched.len()));

    for (name, source, other) in &report.mismatched {
        println!(
            "            {} — deleted everywhere, quarantined\n                 Source {}\n                  Other {}",
            name.display(),
            &source[..16.min(source.len())],
            &other[..16.min(other.len())]
        );
    }

    if !report.mismatched.is_empty() {
        println!("            Quarantine  {}", report.quarantine.display());
    }

    if report.suspect_card() {
        println!();
        println!(
            "        !  That is far more disagreement than the one or two a healthy pair of \
             cards\n           produces. THIS LOOKS LIKE A FAILING CARD — replace it before the \
             next shoot."
        );
    }
}

fn report_geotag(report: Option<&phase5::Report>, heading_was_erased: bool, destinations: usize) {
    let Some(report) = report else {
        // Always headed: the skipped path never drew bars, so nothing put the word on screen.
        phase_heading("Geotagging", offload::progress::PHASE, true);
        println!("    Not run — no tracks (--no-gpx)");
        return;
    };

    // **Green on `outside_track == 0`, not on "every frame tagged".** Frames in a gap are the
    // gap rule working — decision 16 refuses to invent a coordinate — so a run with a few is
    // correct and must not go red. Frames *outside* the track mean the logger was not running
    // during the shoot, which `DESIGN.md` records as this workflow's standing operator risk,
    // and is the one thing here worth walking over to look at.
    let badge = badged_heading(
        "Geotagging",
        offload::progress::PHASE,
        heading_was_erased,
        report.outside_track == 0,
    );
    print!(
        "    {} tagged · {} outside track",
        count(report.tagged),
        count(report.outside_track)
    );
    if report.in_gap > 0 {
        print!(" · {} in a gap too wide to bridge", count(report.in_gap));
    }
    println!("{badge}");

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

    // **The arithmetic is printed rather than left to the reader.** Terry, 2026-08-06: *"Do the
    // math for 11pm Terry."* A bare `196 left alone` beside `49 tagged` invites him to work out
    // where 196 came from on a night when he shot 50 frames — and the answer is one sidecar per
    // tagged frame on each destination, which the tool knows and he should not have to derive.
    //
    // **The product explains the total, so it goes first.** On a fresh run those 196 are
    // *written* and none are left alone; on a convergence run it is the reverse. Attaching the
    // multiplication to either half would make it wrong half the time.
    println!(
        "    {} frames × {} destinations = {} sidecars · {} written · {} left alone \
         (already tagged)",
        count(report.tagged),
        count(destinations),
        count(report.tagged * destinations),
        count(report.written),
        count(report.skipped)
    );
}

/// The eject report's failure branches, which a real run will not produce on demand.
///
/// **A device has to actually refuse to exercise these**, and on this rig that has happened
/// once in a way nobody could schedule. So the clean path is the only one a run has ever
/// printed, and until now the failure text was unexecuted code that looked fine in review —
/// exactly the shape `docs/REVIEWING.md` calls a diagnostic that cannot fail.
#[cfg(test)]
mod tests {
    use super::*;

    /// **Every format the engine declares, and each paired with the one that reads it.**
    ///
    /// This nearly shipped CR3-only, which would have silently dropped NEF — a format `raw.rs`
    /// already handles, through a *different* `read_strategy`. The pairing is what the test is
    /// really pinning: handing a NEF to `RawFormat::Cr3` sends it down the streaming path it
    /// does not survive, and the failure would look like a corrupt file rather than a bug.
    #[test]
    fn every_declared_raw_format_is_found_and_paired_with_its_reader() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let root = scratch.path();
        std::fs::create_dir(root.join("2018-10-20")).expect("a date folder");

        std::fs::write(root.join("b.CR3"), "").expect("a raw");
        std::fs::write(root.join("a.cr3"), "").expect("a raw, lowercase");
        std::fs::write(root.join("2018-10-20").join("d.NEF"), "").expect("a raw, nested");
        std::fs::write(root.join("notes.txt"), "").expect("not a raw");
        std::fs::write(root.join("c.xmp"), "").expect("a sidecar, not a raw");

        let found = raw_files(root).expect("a walk");

        let pairs: Vec<(String, RawFormat)> = found
            .iter()
            .map(|(path, format)| {
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    *format,
                )
            })
            .collect();

        // **Path order, not filename order** — `2018-10-20\d.NEF` sorts before `a.cr3` because
        // digits precede letters. That is what keeps a tree walk grouped by directory, and
        // asserting filenames here is what made this test fail the first time it ran.
        assert_eq!(
            pairs,
            [
                ("d.NEF".to_owned(), RawFormat::Nef),
                ("a.cr3".to_owned(), RawFormat::Cr3),
                ("b.CR3".to_owned(), RawFormat::Cr3),
            ],
            "sorted by path, case-insensitive extensions, recursive, sidecars ignored"
        );

        // **The engine's table is the only authority.** If a format is added there and this
        // count does not move, this walk has grown its own idea of what a raw is.
        assert_eq!(
            RawFormat::ALL.len(),
            2,
            "a format was added to the engine — check that raw_files still finds it"
        );
    }

    /// A track argument is a file **or** a directory of them, and a directory is **not** walked
    /// recursively — descending would quietly pull in a different day's logging, which is the
    /// one way this could tag a frame against the wrong track and still look like it worked.
    #[test]
    fn tracks_expand_from_files_and_flat_directories_only() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let dir = scratch.path();

        std::fs::write(dir.join("b.gpx"), "").expect("a track");
        std::fs::write(dir.join("a.gpx"), "").expect("a track");
        std::fs::write(dir.join("notes.txt"), "").expect("a non-track");
        std::fs::create_dir(dir.join("deeper")).expect("a subdirectory");
        std::fs::write(dir.join("deeper").join("other-day.gpx"), "").expect("a track below");

        let found = expand_tracks(&[dir.to_path_buf()]).expect("tracks");

        let names: Vec<String> = found
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["a.gpx", "b.gpx"],
            "sorted, .gpx only, not recursive"
        );

        // A named file is taken as given, whatever its extension — the operator pointed at it.
        let named = expand_tracks(&[dir.join("notes.txt")]).expect("an explicitly named file");
        assert_eq!(named.len(), 1);

        // **Nothing found is an error, never an empty run.** Phase 5 against zero tracks would
        // report every frame as outside the track, which reads as a dead logger rather than as
        // a mistyped path.
        let empty = tempfile::tempdir().expect("an empty directory");
        assert!(expand_tracks(&[empty.path().to_path_buf()]).is_err());
    }

    /// **A mismatch line MUST name both sides**, and that is what a tidy-up would drop.
    ///
    /// Shortening this to *"does not match the config"* would leave Terry, at 11pm, to go and
    /// find out what the config actually says — at the moment he is deciding whether tonight
    /// is normal. It is also what lets Claude offer the exact edit rather than asking him to
    /// read the value out.
    #[test]
    fn the_body_line_names_both_sides_of_a_mismatch() {
        let line = (preflight::BodyReport::Unexpected {
            observed: geotag::raw::BodyIdentity {
                make: Some("Canon".to_owned()),
                model: Some("Canon EOS R5".to_owned()),
                serial: Some("212024001418".to_owned()),
            },
            configured: config::Body {
                model: "Canon EOS R5".to_owned(),
                serial: "082021001047".to_owned(),
            },
        })
        .to_string();

        assert!(line.contains("212024001418"), "the observed serial: {line}");
        assert!(
            line.contains("082021001047"),
            "the configured serial: {line}"
        );

        // INFO, not WARNING. A `!` prefix is the report's exit-2 level, and decision 34
        // rejected exit 2 for a signal that repeats on every night of a trip.
        assert!(!line.starts_with('!'), "must not read as a warning: {line}");
    }

    fn released(label: &str, outcome: eject::Outcome, attempts: u32) -> Released {
        Released {
            label: label.to_owned(),
            effort: eject::Effort {
                outcome,
                attempts,
                waited: Duration::from_secs(11),
            },
        }
    }

    fn held(reason: &str) -> eject::Outcome {
        eject::Outcome::Held {
            reason: reason.to_owned(),
        }
    }

    fn render_ssds(items: &[Released], elapsed: Duration) -> String {
        let mut out = Vec::new();
        report_ssd_release(&mut out, Some(items), elapsed).expect("writing to a Vec");
        String::from_utf8(out).expect("the report is UTF-8")
    }

    /// 677 s is 11 m 17 s — the one long card release this project has actually observed.
    fn card(label: &str, outcome: eject::Outcome, attempts: u32) -> (String, eject::Effort) {
        (
            label.to_owned(),
            eject::Effort {
                outcome,
                attempts,
                waited: Duration::from_secs(677),
            },
        )
    }

    fn render_cards(
        items: &[(String, eject::Effort)],
        elapsed: Duration,
        budget_spent: bool,
    ) -> String {
        let mut out = Vec::new();
        report_card_release(&mut out, items, elapsed, budget_spent).expect("writing to a Vec");
        String::from_utf8(out).expect("the report is UTF-8")
    }

    #[test]
    fn every_ssd_down_reads_as_all_put_to_bed() {
        let text = render_ssds(
            &[
                released("SanDisk", eject::Outcome::Ejected, 2),
                released("OWC", eject::Outcome::Ejected, 1),
            ],
            Duration::from_secs(13),
        );

        // Indented under the `Travel SSDs` heading and not repeating it — position carries
        // the scope.
        assert!(
            text.contains("\n        All SSDs put to bed in 0m 13s. Safe to store."),
            "{text}"
        );
        assert!(!text.contains("Travel SSDs —"), "{text}");
        // Effort prints only when Windows made the run work for it. A device that powered
        // down on the first ask should read as cleanly as it behaved.
        assert!(
            text.contains("ejected; ready to disconnect after 2 attempts over 0m 11s"),
            "{text}"
        );
        assert!(
            !text.contains("OWC        ejected; ready to disconnect after"),
            "{text}"
        );
    }

    /// **The bug the split was made to kill.** One conflated closing line reported
    /// `Released 5 devices in 22m 16s` on a night when four were released and a CFexpress
    /// had just been described as never releasing. A count that includes devices the same
    /// report calls stuck is the 2026-08-05 card-dismount lie in a new place.
    #[test]
    fn a_stuck_ssd_is_named_and_not_counted_as_put_to_bed() {
        let text = render_ssds(
            &[
                released("SanDisk", eject::Outcome::Ejected, 1),
                released("OWC", held("CONFIGRET(23), PNP_VETO_TYPE(6)"), 40),
                released(
                    "WD",
                    eject::Outcome::Dismounted {
                        veto: eject::Veto::Device,
                        reason: "the enclosure declined to power down".to_owned(),
                    },
                    3,
                ),
            ],
            Duration::from_secs(90 * 60),
        );

        assert!(
            text.contains("\n        1 of 3 SSDs put to bed in 90m 0s."),
            "{text}"
        );
        assert!(text.contains("OWC, WD still needs you"), "{text}");
        assert!(!text.contains("All SSDs"), "{text}");

        // **Two failures, two instructions, and collapsing them is what made a successful run
        // read as a chore.** A held volume is still mounted and only the tray will shift it; a
        // dismounted one is flushed and detached, so pulling it is the whole of what remains.
        assert!(
            text.contains("still mounted — eject it from the tray"),
            "{text}"
        );
        assert!(
            text.contains("dismounted, not powered down — safe to unplug"),
            "{text}"
        );
    }

    /// The 90-minute give-up, which Terry asked to be *declared* rather than inferred from a
    /// large number: a reader seeing `90m 00s` should not have to work out whether that was
    /// persistence or a hang.
    #[test]
    fn cards_that_never_release_declare_the_budget() {
        let text = render_cards(
            &[
                card("Primary", held("held by STORAGE\\Volume{...}"), 90),
                card("Secondary", eject::Outcome::Ejected, 1),
            ],
            Duration::from_secs(90 * 60),
            true,
        );

        assert!(
            text.contains("\n        1 of 2 cards put to bed in 90m 0s."),
            "{text}"
        );
        assert!(
            text.contains("Primary never released (retried to the 90-minute budget and gave up)"),
            "{text}"
        );

        // **Never phrased as a failure, at any volume.** The tool never wrote to a card, so it
        // was safe to pull before this ran and is safe to pull now (decision 22).
        assert!(
            text.contains("Safe to pull anyway: nothing was written to them."),
            "{text}"
        );
        assert!(!text.to_lowercase().contains("fail"), "{text}");
    }

    /// The same failure with time left on the clock MUST NOT claim the budget was spent —
    /// that is the distinction `budget_spent` exists to carry.
    #[test]
    fn cards_that_gave_up_early_do_not_claim_the_budget() {
        let text = render_cards(
            &[card("Primary", held("something else went wrong"), 1)],
            Duration::from_secs(42),
            false,
        );

        assert!(text.contains("never released (gave up)"), "{text}");
        assert!(!text.contains("budget"), "{text}");
    }

    /// **The attempt count on a card row exists to build a sample, not to inform the operator.**
    /// Discarding it is what made *how reliably does a held card recover* unanswerable about the
    /// only long release this project has seen — 11 m 17 s, with no record of whether that was
    /// sixteen asks or one lucky late one. A card that recovers late and one that recovers on
    /// its second look MUST NOT render identically.
    #[test]
    fn a_card_that_took_many_attempts_says_how_many() {
        let late = render_cards(
            &[card("Primary", eject::Outcome::Ejected, 16)],
            Duration::from_secs(677),
            false,
        );
        assert!(
            late.contains("ejected; remove card from reader after 16 attempts over 11m 17s"),
            "{late}"
        );

        // First-ask success stays clean, exactly as the SSD rows do.
        let prompt = render_cards(
            &[card("Primary", eject::Outcome::Ejected, 1)],
            Duration::from_secs(1),
            false,
        );
        assert!(
            prompt.contains("ejected; remove card from reader\n"),
            "{prompt}"
        );
        assert!(!prompt.contains("attempts"), "{prompt}");
    }

    /// A range whose ends agree is a number, and a small day put both spans there.
    #[test]
    fn an_estimate_range_collapses_when_both_ends_agree() {
        assert_eq!(span(0.2, 0.9), "1 min");
        assert_eq!(span(1.0, 1.0), "1 min");

        // The wide case is the honest one and must survive: the ends are the fastest and the
        // slowest destination, and a big night genuinely spans them.
        assert_eq!(span(17.2, 30.1), "18-31 min");
    }

    /// A card that never released MUST NOT trigger the replug warning: the reader only powers
    /// down with a card it actually ejected, so warning here would send the operator to fix
    /// something that did not happen.
    #[test]
    fn the_replug_warning_follows_an_actual_eject() {
        let stuck = render_cards(
            &[card("Primary", held("still mounted"), 90)],
            Duration::from_secs(42),
            false,
        );
        assert!(!stuck.contains("needs a replug"), "{stuck}");

        let clean = render_cards(
            &[card("Primary", eject::Outcome::Ejected, 1)],
            Duration::from_secs(9),
            false,
        );
        assert!(clean.contains("needs a replug"), "{clean}");
    }

    const CLEAN: &str = "\u{2713}";
    const ATTENTION: &str = "!!!";

    /// Goes through [`everything_released`] rather than asserting on a hand-built flag, so these
    /// exercise the rule the report actually uses.
    fn render_gate(released: Option<&[Released]>, cards: &[(String, eject::Effort)]) -> String {
        let mut out = Vec::new();
        let clean = everything_released(released, cards);
        report_unhook_gate(&mut out, clean, released.is_none(), released.is_none())
            .expect("writing to a Vec");
        String::from_utf8(out).expect("the report is UTF-8")
    }

    /// One green render, so the glyph and the badge are exercised end to end. **Which
    /// combinations are green is `only_a_wholly_released_rig_is_clean`'s job** — three more
    /// render tests differing only in their fixture proved the same `everything_released` call
    /// three more times.
    #[test]
    fn a_wholly_released_rig_renders_the_clean_badge() {
        let text = render_gate(
            Some(&[released("SanDisk", eject::Outcome::Ejected, 1)]),
            &[card("Primary", eject::Outcome::Ejected, 1)],
        );
        assert!(text.contains(CLEAN), "{text}");
        assert!(!text.contains(ATTENTION), "{text}");
    }

    /// **`--no-eject` MUST be yellow**, and this is the test that says so out loud rather than
    /// leaving it to fall out of the `None`. The flag leaves every drive mounted, so a green
    /// column here is the single most dangerous thing this report could print — see
    /// `DESIGN.md`, *the badge column is a go/no-go on unplugging things*.
    #[test]
    fn a_withheld_eject_is_yellow_rather_than_absent() {
        let text = render_gate(None, &[]);
        assert!(text.contains(ATTENTION), "{text}");
        assert!(!text.contains(CLEAN), "{text}");

        // **The badge MUST arrive with its reason**, on the line beneath it. Split across a
        // blank line they read as two unrelated facts, which is how the first version rendered.
        assert!(text.contains("Withheld by --no-eject"), "{text}");
    }

    /// **`SAFE TO STORE` is a physical instruction and MUST NOT appear while a drive is mounted.**
    /// It was printing on `--no-eject` runs one line under a yellow badge telling Terry not to
    /// touch anything — the loudest line in the report countermanding the signal above it, in the
    /// direction that damages hardware. This is the regression test for that.
    #[test]
    fn a_mounted_drive_is_never_called_safe_to_store() {
        let gate = render_gate(None, &[]);

        assert!(gate.contains(ATTENTION), "{gate}");
        assert!(gate.contains("Withheld by --no-eject"), "{gate}");

        // Neither of the verdict's two phrases may appear here — not the one that would be a
        // lie, and not the one that is true, because decision 14 keeps both for the last line.
        let shouted = gate.to_ascii_uppercase();
        assert!(!shouted.contains("SAFE TO STORE"), "{gate}");
        assert!(!shouted.contains("STILL MOUNTED"), "{gate}");
    }

    /// **The gate badge and the verdict colour are the same fact, so they cannot disagree.**
    /// This walks every shape a night can end in and asserts `everything_released` is green only
    /// for the one that earns it. It is the invariant `SAFE TO STORE` violated by being derived
    /// separately — each half individually correct, the pair contradictory.
    #[test]
    fn only_a_wholly_released_rig_is_clean() {
        let up = || released("SanDisk", eject::Outcome::Ejected, 1);
        let stuck = || released("OWC", held("still mounted"), 40);
        let card_up = || card("Primary", eject::Outcome::Ejected, 1);
        let card_stuck = || card("Primary", held("still mounted"), 90);

        assert!(everything_released(Some(&[up()]), &[card_up()]));

        // Nothing ejected at all — `--no-eject`, and the case that started this.
        assert!(!everything_released(None, &[]));
        // A card alone is enough to withhold green, by Terry's rule and without touching
        // `exit_code`, which is never given the cards.
        assert!(!everything_released(Some(&[up()]), &[card_stuck()]));
        assert!(!everything_released(Some(&[stuck()]), &[card_up()]));
        assert!(!everything_released(Some(&[up(), stuck()]), &[card_up()]));

        // A dismounted device did not power down, so it is not released either.
        let limp = released(
            "WD",
            eject::Outcome::Dismounted {
                veto: offload::eject::Veto::Device,
                reason: "would not power down".into(),
            },
            3,
        );
        assert!(!everything_released(Some(&[limp]), &[card_up()]));
    }
}
