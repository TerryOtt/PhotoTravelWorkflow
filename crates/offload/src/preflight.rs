//! Phases 1 and 2 — the ten seconds that decide whether you can leave.
//!
//! Decision 9: the worst outcome for a walk-away tool is returning from dinner to a run
//! that died two minutes in. So everything that can be asserted before a byte moves is
//! asserted here, and the order is forced by the data rather than chosen — **N** is
//! phase 1's output and phase 2's input, because a capacity assertion needs a number to
//! compare against.
//!
//! It also puts the fatal that means *equipment failure* ahead of the ones that merely
//! mean *go fetch something*.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use geotag::format::RawFormat;
use geotag::raw::{self, BodyIdentity, MediaParser};

use crate::cards::{self, Card, Speed};
use crate::config::{self, Config};
use crate::destinations::{self, Survey};
use crate::pipeline::cr3_files;
use crate::storage::{self, Volume};

/// Spare room demanded beyond **N**, so a destination that only just fits is refused at
/// the desk rather than filling up at 2am.
const CAPACITY_MARGIN: f64 = 1.05;

/// What phase 1 established: what tonight is.
#[derive(Debug)]
pub struct Cards {
    pub source: Card,
    pub source_speed: Speed,
    pub other: Option<(Card, Speed)>,
    /// Every `*.CR3` on the source card, absolute and sorted.
    pub files: Vec<PathBuf>,
    /// **N** — one copy of the day.
    pub bytes: u64,
    /// True when two cards presented one identical listing (decision 27). False only on
    /// a declared single-source run, where nothing agreed and nothing claims to have.
    pub agreed: bool,
    /// What the first frame says the camera was (decision 34). `None` when the config
    /// names no body, which is the only way to switch this off.
    pub body: Option<BodyReport>,
}

/// What decision 34's check found. **INFO in every arm** — none of these touches the
/// verdict or the exit code, because a body mismatch persists across every night of a trip
/// and a signal that repeats is one the operator learns to read past.
#[derive(Debug, PartialEq, Eq)]
pub enum BodyReport {
    /// The frame and the config agree, model and serial.
    AsConfigured { model: String, serial: String },
    /// They disagree. Carries both sides so the report can name the difference rather than
    /// asserting one, which is what lets Claude offer the config edit.
    Unexpected {
        observed: BodyIdentity,
        configured: config::Body,
    },
    /// The frame carries no `Make`, `Model` or `CameraSerialNumber` at all.
    ///
    /// **Not a mismatch**, and kept separate for that reason: *this frame says nothing*
    /// and *this is the wrong camera* would send the operator to different places.
    FrameSaysNothing,
    /// The frame could not be read. **Never fatal** — decision 34 reports and never
    /// refuses, and a night's shooting is not held up by a reporting feature failing.
    Unreadable(String),
}

impl fmt::Display for BodyReport {
    /// The report's row, **INFO in every arm**, and that is load-bearing rather than mild.
    ///
    /// **No `!` prefix and no badge in any branch.** A `!` block is the report's WARNING level
    /// and carries exit 2; a body mismatch is true on *every* run until the config is edited,
    /// so spending exit 2 on it would train the operator to read past a code that also means
    /// unfiled frames, a confirmed mismatch and a refused eject. Decision 34 rejected exactly
    /// that.
    ///
    /// **What makes INFO sufficient rather than lax is the other reader.** `../CLAUDE.md` binds
    /// Claude to act on this line every time it disagrees — ask what changed, offer the config
    /// edit — while a tired human sees a plain fact about his camera. The line must still stand
    /// alone, because hotel internet does not.
    ///
    /// `Display` rather than a private helper in `main.rs` so `examples/body-check.rs` renders
    /// the **same** string the report does. A probe with its own copy of the formatting can
    /// agree today and drift tomorrow, and it would be the probe everyone believed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AsConfigured { model, serial } => write!(f, "{model} · {serial} — as configured"),

            // Both sides, always. Naming only the observed one would make the reader go and
            // look up what was expected, at the moment he is deciding whether tonight is normal.
            Self::Unexpected {
                observed,
                configured,
            } => write!(
                f,
                "{observed} — does not match the config (expected {} · {})",
                configured.model, configured.serial
            ),

            Self::FrameSaysNothing => write!(f, "the first frame records no camera identity"),

            // The run is unaffected — this arm exists so a reporting feature that fails says so
            // instead of printing nothing, which would be indistinguishable from a match.
            Self::Unreadable(why) => write!(f, "could not be read — {why}"),
        }
    }
}

/// What phase 2 established: whether the rig can take it.
#[derive(Debug)]
pub struct Rig {
    pub survey: Survey,
    pub distinct_disks: usize,
    pub unidentified: usize,
    /// Tracks found in the configured directory.
    pub tracks: Vec<PathBuf>,
}

/// Both phases, and the estimate that actually lets you leave.
#[derive(Debug)]
pub struct Preflight {
    pub cards: Cards,
    pub rig: Rig,
}

/// Phase 1 — the camera card contents.
///
/// Walking the cards is what produces the file set and **N**, so it happens at any card
/// count; what a second card adds is the match, not the walk.
pub fn phase1(config: &Config, volumes: &[Volume], allow_single_source: bool) -> Result<Cards> {
    let survey = destinations::survey_against(config, volumes);
    let found = cards::find(volumes, &survey.volume_guids());

    if found.is_empty() {
        bail!(
            "no camera card found. A card is a volume carrying a DCIM directory that is \
             not one of your destinations; check that a reader is connected and a card \
             is seated in it"
        );
    }

    if found.len() == 1 && !allow_single_source {
        bail!(
            "ONLY ONE CARD FOUND — {} is all there is.\n\n\
             Every frame is shot to both cards. If this offload has only one, a card, a \
             reader, or the camera has failed. Check the rig.\n\n\
             Refusing to run. If one card is truly all there is tonight, re-run with \
             --allow-single-source.",
            found[0].label()
        );
    }

    let chosen = cards::choose(&found)?;

    // Decision 27's gate: with two cards, both must present one listing before phase 3
    // moves a byte. Sizes convict unequal content without reading one; equal-size
    // divergence is what phase 4's hash pass exists to find.
    let agreed = match &chosen.other {
        Some((other, _)) => {
            gate(&chosen.source, other)?;
            true
        }
        None => false,
    };

    let files = cr3_files(&chosen.source.dcim)
        .with_context(|| format!("walking {}", chosen.source.dcim.display()))?;

    let bytes = files
        .iter()
        .filter_map(|file| std::fs::metadata(file).ok())
        .map(|meta| meta.len())
        .sum();

    let body = config
        .body
        .as_ref()
        .map(|configured| check_body(files.first(), configured));

    Ok(Cards {
        source: chosen.source,
        source_speed: chosen.source_speed,
        other: chosen.other,
        files,
        bytes,
        agreed,
        body,
    })
}

/// Decision 34 — is this the camera the config names?
///
/// **One frame, from the source card only.** Decision 34 was written as *the first frame on
/// each card*; decision 27's gate has since made the second read redundant, because a
/// two-card run has already proved the pair holds one identical listing before this point,
/// and a single-source run has no second card to read. Reading both would ask a question
/// that is answered above.
///
/// **The payoff is decision 23, not contract policing.** A body that does not record
/// `OffsetTimeOriginal` sends *every* frame to `_unfiled` (decision 21) — and without this,
/// that is discovered only after the whole day has streamed through phase 3. One frame here
/// turns a 35-minute discovery into a ten-second one, while the fix is still a decision
/// about tonight rather than a fact about it.
pub fn check_body(first: Option<&PathBuf>, configured: &config::Body) -> BodyReport {
    let Some(path) = first else {
        // An empty card cannot reach here — phase 1 needs files to compute N — but the
        // signature admits it, and inventing a mismatch from no evidence is the one
        // outcome this check must never produce.
        return BodyReport::FrameSaysNothing;
    };

    let mut parser = MediaParser::new();
    match raw::body_identity(&mut parser, path, RawFormat::Cr3) {
        Ok(observed) => compare_body(observed, configured),
        Err(error) => BodyReport::Unreadable(format!("{error:#}")),
    }
}

/// Everything about the body check that does not depend on reading a file.
///
/// Split out for the same reason `raw::resolve` is: **no CR3 is committed to this
/// repository**, so a test that had to open one could not run. The comparison is the part
/// that can be wrong in a way nobody notices — a match declared on a body that is not his —
/// and it is now the part under test.
fn compare_body(observed: BodyIdentity, configured: &config::Body) -> BodyReport {
    if observed.is_empty() {
        return BodyReport::FrameSaysNothing;
    }

    // **Trimmed, never normalized further.** Camera-written EXIF strings are padded often
    // enough to be worth trimming; folding case or stripping punctuation could map two real
    // bodies onto one string, which is the failure this check exists to catch.
    let same = |observed: Option<&String>, expected: &str| {
        observed.is_some_and(|value| value.trim() == expected.trim())
    };

    if same(observed.model.as_ref(), &configured.model)
        && same(observed.serial.as_ref(), &configured.serial)
    {
        BodyReport::AsConfigured {
            model: configured.model.clone(),
            serial: configured.serial.clone(),
        }
    } else {
        BodyReport::Unexpected {
            observed,
            configured: configured.clone(),
        }
    }
}

/// Decision 27 — both cards must present the same listing, name for name and size for
/// size, before anything moves.
///
/// **Pairing is by card-relative path**, so two cards agree only if the same frame sits
/// at the same place on both. Hashing is deliberately not done here: reading every byte
/// of both cards before phase 3 is the posture decision 1 rejected, and sizes already
/// convict unequal content without a read.
fn gate(source: &Card, other: &Card) -> Result<()> {
    let ours = listing(source)?;
    let theirs = listing(other)?;

    if ours == theirs {
        return Ok(());
    }

    let only_ours: Vec<&PathBuf> = ours.keys().filter(|k| !theirs.contains_key(*k)).collect();
    let only_theirs: Vec<&PathBuf> = theirs.keys().filter(|k| !ours.contains_key(*k)).collect();
    let differing: Vec<&PathBuf> = ours
        .iter()
        .filter(|(path, size)| theirs.get(*path).is_some_and(|other| other != *size))
        .map(|(path, _)| path)
        .collect();

    bail!(
        "THE TWO CARDS DO NOT HOLD THE SAME FILES.\n\n\
         {source} has {} files, {other} has {}.\n\
         {} only on {source}, {} only on {other}, {} present on both at different sizes.\n\n\
         Every frame is shot to both cards, so a diverged pair means a slot, a card, or \
         the camera has failed. Refusing to run before anything is written.\n\n\
         If one card is the complete one, remove the other and re-run with \
         --allow-single-source.",
        ours.len(),
        theirs.len(),
        only_ours.len(),
        only_theirs.len(),
        differing.len(),
        source = source.label(),
        other = other.label(),
    )
}

/// Card-relative path to size, for every `*.CR3` on a card.
fn listing(card: &Card) -> Result<BTreeMap<PathBuf, u64>> {
    let root = card.dcim.parent().unwrap_or(&card.dcim).to_path_buf();

    let mut listing = BTreeMap::new();

    for file in cr3_files(&card.dcim)? {
        let relative = file.strip_prefix(&root).unwrap_or(&file).to_path_buf();
        let size = std::fs::metadata(&file)
            .with_context(|| format!("sizing {}", file.display()))?
            .len();
        listing.insert(relative, size);
    }

    Ok(listing)
}

/// Phase 2 — the rig.
///
/// `without` names destinations the operator has declared absent (decision 25); anything
/// else missing is fatal here, while you are still standing at the desk.
pub fn phase2(
    config: &Config,
    volumes: &[Volume],
    needed_bytes: u64,
    without: &[String],
    no_gpx: bool,
) -> Result<Rig> {
    let survey = destinations::survey_against(config, volumes);

    let undeclared: Vec<&destinations::Missing> = survey
        .missing
        .iter()
        .filter(|missing| !without.contains(&missing.label))
        .collect();

    if let Some(missing) = undeclared.first() {
        bail!(
            "DESTINATION MISSING — {} ({}).\n\n\
             Plug it in, or re-run with --without {} and re-run the night when it returns.\n\
             If the drive is dead, remove it from config.json and finish the trip on three.",
            missing.label,
            missing.reason,
            missing.label
        );
    }

    let (distinct_disks, unidentified) = survey.distinct_disks();
    if distinct_disks + unidentified < survey.found.len() {
        bail!(
            "TWO DESTINATIONS ARE THE SAME PHYSICAL DISK.\n\n\
             {} copies resolved to {distinct_disks} distinct devices, so at least two of \
             them would land on one disk — which is one failure away from being no backup \
             at all.\n\n\
             Check the config's disk serials against `storage-inventory`.",
            survey.found.len()
        );
    }

    for resolved in &survey.found {
        let needed = (needed_bytes as f64 * CAPACITY_MARGIN) as u64;
        if resolved.volume.free_bytes < needed {
            // GiB, matching the free-space column this refusal sends the operator to check
            // and matching File Explorer, which is where he will actually go look. A refusal
            // that quotes a different unit than the drive's own properties dialog is a
            // refusal he has to do arithmetic on before he can act on it.
            //
            // **The two roundings deliberately disagree**: free rounds DOWN and needed rounds
            // UP, so the printed pair always straddles the truth outward. Rounding both the
            // same way lets this line read `387 GiB free, 387 GiB needed` while refusing —
            // a refusal whose own numbers appear to contradict it is worse than no numbers.
            bail!(
                "NOT ENOUGH ROOM ON {} — {} GiB free, {} GiB needed for tonight plus \
                 margin.",
                resolved.label,
                crate::human::gib_down(resolved.volume.free_bytes),
                crate::human::gib_up(needed)
            );
        }
    }

    let tracks = gpx_tracks(&config.gpx_dir);
    if tracks.is_empty() && !no_gpx {
        bail!(
            "NO GPX TRACKS in {}.\n\n\
             Copy tonight's tracks off the logger, point --gpx somewhere else, or re-run \
             with --no-gpx to land the raws untagged.",
            config.gpx_dir.display()
        );
    }

    Ok(Rig {
        survey,
        distinct_disks,
        unidentified,
        tracks,
    })
}

/// Every `*.gpx` in the configured directory, sorted.
fn gpx_tracks(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut tracks: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("gpx"))
        })
        .collect();

    tracks.sort();
    tracks
}

/// Both phases, in the order the data forces.
pub fn run(
    config: &Config,
    allow_single_source: bool,
    without: &[String],
    no_gpx: bool,
) -> Result<Preflight> {
    let volumes = storage::volumes().context("enumerating volumes")?;

    let cards = phase1(config, &volumes, allow_single_source)?;
    let rig = phase2(config, &volumes, cards.bytes, without, no_gpx)?;

    Ok(Preflight { cards, rig })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card_at(root: &Path, frames: &[(&str, usize)]) -> Card {
        let dcim = root.join("DCIM").join("100CANON");
        std::fs::create_dir_all(&dcim).expect("a card tree");

        for (name, size) in frames {
            std::fs::write(dcim.join(name), vec![0u8; *size]).expect("a frame");
        }

        Card {
            volume: Volume {
                guid_path: format!(r"\\?\Volume{{{}}}\", root.display()),
                mount_points: vec![root.to_path_buf()],
                label: None,
                filesystem: None,
                volume_serial: 0,
                removable: false,
                total_bytes: 0,
                free_bytes: 0,
            },
            dcim: root.join("DCIM"),
        }
    }

    #[test]
    fn two_cards_holding_the_same_listing_pass_the_gate() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        let frames = [("_50A0001.CR3", 100), ("_50A0002.CR3", 200)];

        let a = card_at(&scratch.path().join("a"), &frames);
        let b = card_at(&scratch.path().join("b"), &frames);

        assert!(gate(&a, &b).is_ok());
    }

    /// A slot that filled or died mid-day: one card has frames the other does not.
    #[test]
    fn a_card_missing_a_frame_is_refused_before_anything_moves() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");

        let a = card_at(
            &scratch.path().join("a"),
            &[("_50A0001.CR3", 100), ("_50A0002.CR3", 200)],
        );
        let b = card_at(&scratch.path().join("b"), &[("_50A0001.CR3", 100)]);

        let error = format!("{:#}", gate(&a, &b).unwrap_err());
        assert!(error.contains("DO NOT HOLD THE SAME FILES"), "{error}");
    }

    /// Equal names, unequal sizes — content divergence that costs no read to convict.
    #[test]
    fn same_names_at_different_sizes_are_refused() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");

        let a = card_at(&scratch.path().join("a"), &[("_50A0001.CR3", 100)]);
        let b = card_at(&scratch.path().join("b"), &[("_50A0001.CR3", 101)]);

        assert!(gate(&a, &b).is_err());
    }

    /// The gate pairs by card-relative path, so the same frame in a different DCIM
    /// folder is a divergence rather than a match.
    #[test]
    fn the_same_name_in_a_different_folder_does_not_pair() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");

        let a = card_at(&scratch.path().join("a"), &[("_50A0001.CR3", 100)]);

        let b_root = scratch.path().join("b");
        let elsewhere = b_root.join("DCIM").join("101CANON");
        std::fs::create_dir_all(&elsewhere).expect("a second folder");
        std::fs::write(elsewhere.join("_50A0001.CR3"), vec![0u8; 100]).expect("a frame");
        let b = Card {
            volume: a.volume.clone(),
            dcim: b_root.join("DCIM"),
        };

        assert!(gate(&a, &b).is_err());
    }

    fn configured() -> config::Body {
        config::Body {
            model: "Canon EOS R5".to_owned(),
            serial: "082021001047".to_owned(),
        }
    }

    fn observed(model: Option<&str>, serial: Option<&str>) -> BodyIdentity {
        BodyIdentity {
            make: Some("Canon".to_owned()),
            model: model.map(str::to_owned),
            serial: serial.map(str::to_owned),
        }
    }

    /// The agreeing case, including the padding cameras actually write.
    ///
    /// **Trimming is the only normalization allowed**, so this pins it from the permissive
    /// side; `a_body_that_is_not_his_is_never_reported_as_configured` pins the other.
    #[test]
    fn the_configured_body_matches_after_trimming() {
        let report = compare_body(
            observed(Some("Canon EOS R5 "), Some(" 082021001047")),
            &configured(),
        );

        assert_eq!(
            report,
            BodyReport::AsConfigured {
                model: "Canon EOS R5".to_owned(),
                serial: "082021001047".to_owned(),
            }
        );
    }

    /// **The test this feature exists for.** Every row is a body Terry has actually shot or
    /// could shoot, and every one MUST come out `Unexpected` — a false `AsConfigured` is
    /// indistinguishable from the check working, which is what makes it worth pinning by case.
    #[test]
    fn a_body_that_is_not_his_is_never_reported_as_configured() {
        let cases = [
            (
                "a rented R5 — same model, different serial, the case a model check misses",
                observed(Some("Canon EOS R5"), Some("212024001418")),
            ),
            (
                "a different model entirely",
                observed(Some("Canon EOS R6"), Some("082021001047")),
            ),
            (
                "a frame carrying no serial at all",
                observed(Some("Canon EOS R5"), None),
            ),
            (
                "a serial that differs only in its leading digit",
                observed(Some("Canon EOS R5"), Some("182021001047")),
            ),
        ];

        for (case, identity) in cases {
            assert!(
                matches!(
                    compare_body(identity, &configured()),
                    BodyReport::Unexpected { .. }
                ),
                "{case} was reported as the configured body"
            );
        }
    }

    /// A frame with no camera tags is **not** a mismatch, and conflating them would send the
    /// operator hunting for a body swap that did not happen.
    #[test]
    fn a_frame_with_no_camera_tags_is_not_a_mismatch() {
        assert_eq!(
            compare_body(BodyIdentity::default(), &configured()),
            BodyReport::FrameSaysNothing
        );
    }
}
