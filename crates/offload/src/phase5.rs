//! Phase 5 — geotag. The gravy, and it is allowed to take its time.
//!
//! Decision 14: only phase 3 may change the verdict. **A geotag miss is a count in the
//! body, never a downgrade at the top** — otherwise you learn to read past the verdict
//! line, which is the same failure the raws-only manifest exists to avoid.
//!
//! # A tag is earned, never assumed
//!
//! The project mantra, inherited with the engine: **a geotag off by more than 5 m is
//! worse than no geotag.** A missing tag is visibly missing; a wrong one looks
//! authoritative and silently corrupts a photograph's provenance. So [`geotag::track`]
//! refuses to interpolate across a hole too wide in *either* time or distance, refuses
//! to bridge a `<trkseg>` break at all, and never clamps or extrapolates past the ends
//! of a track. Decision 16 keeps both limits because neither implies the other: a
//! 140-second hole with 8 m between its ends is untrustworthy and only the time limit
//! rejects it, while a short hole with wide separation is genuine fast movement and only
//! the distance limit rejects that.
//!
//! # It re-reads nothing
//!
//! Decision 10: phase 3 already held every file in RAM to hash it, so capture times were
//! taken then and carried here. This phase correlates timestamps against an index and
//! writes a few thousand 3 KB sidecars; it never opens a raw file.

use std::path::PathBuf;

use crate::progress::Progress;
use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use geotag::track::{GapLimits, Lookup, Track};
use geotag::xmp;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::pipeline::Destination;

/// How this tool identifies itself in a sidecar's `x:xmptk`.
const WRITER: &str = concat!("offload ", env!("CARGO_PKG_VERSION"));

/// One frame phase 3 landed, with the capture time it stashed.
#[derive(Debug, Clone)]
pub struct Landed {
    /// Relative to each destination root — identical across all four (decision 5).
    pub relative: PathBuf,
    pub captured: DateTime<Utc>,
}

/// What phase 5 did.
#[derive(Debug, Default)]
pub struct Report {
    pub tagged: usize,
    /// Before the first track point or after the last. No clamping, no extrapolation.
    pub outside_track: usize,
    /// Inside the track's span but between two points too far apart to bridge.
    pub in_gap: usize,
    /// Already had a sidecar, so left alone — decision 16's invariant.
    pub skipped: usize,
    /// Sidecars written, counted across all destinations.
    pub written: usize,
    /// Shot before the track's first point — the logger was started late.
    pub before_track: usize,
    /// Shot after the track's last point. **The most common real cause is forgetting to
    /// restart the logger**, and it is a standing risk of this workflow rather than a
    /// freak event, so the report names the boundary instead of only counting.
    pub after_track: usize,
    /// The track's own span, so a miss can be described relative to it.
    pub track_span: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Frames that fell between two *recording sessions* — the logger stopped and
    /// started. Never bridged at any width, because nothing is known about the path in
    /// between (decision 16).
    pub across_segments: usize,
    /// Frames inside one continuous recording, but between points too far apart in time
    /// or distance to interpolate honestly.
    pub within_segment: usize,
    /// The widest hole any frame fell into, for naming the worst of it.
    pub widest_gap: Option<TimeDelta>,
    /// How far outside the track each miss fell, for the clock heuristic below.
    misses: Vec<TimeDelta>,
}

impl Report {
    /// Decision 23's heuristic: **a scattered miss pattern is a logging gap, a uniform
    /// one is a camera clock.**
    ///
    /// A camera set to UTC whose clock reads two hours off derives a wrong UTC from
    /// honest arithmetic — wrong date folders, shifted geotags, and no error anywhere,
    /// because the metadata itself is lying. Nothing in the pipeline can catch that. What
    /// *can* be caught is its signature: every frame missing the track by nearly the same
    /// amount. That is the difference between finding out tonight and finding out on
    /// Lightroom's map three weeks later.
    ///
    /// Returns the offset only when the misses are both numerous and tightly clustered,
    /// because saying "check your clock" on a scattered pattern is the false alarm that
    /// teaches you to ignore the real one.
    pub fn systematic_offset(&self) -> Option<TimeDelta> {
        const ENOUGH_TO_JUDGE: usize = 10;
        // A real clock error puts every frame the same distance out; a logging gap does
        // not. One minute of spread is generous and still nowhere near a gap's scatter.
        const TIGHT: i64 = 60;

        if self.misses.len() < ENOUGH_TO_JUDGE || self.tagged > 0 {
            return None;
        }

        let seconds: Vec<i64> = self.misses.iter().map(TimeDelta::num_seconds).collect();
        let (low, high) = (seconds.iter().min()?, seconds.iter().max()?);
        if high - low > TIGHT {
            return None;
        }

        let median = seconds[seconds.len() / 2];
        Some(TimeDelta::seconds(median))
    }

    /// One sentence naming *why* frames went untagged, when the pattern says so.
    ///
    /// A count alone is not actionable: 1,383 outside a track could be a dead logger, a
    /// late start, or a day of scattered dropouts, and the operator's response differs
    /// for each. When every miss sits on one side of the track, the boundary time is the
    /// thing worth printing — it is what makes "ah, the sunset shoot" click.
    pub fn boundary_note(&self) -> Option<String> {
        let (first, last) = self.track_span?;

        if self.after_track > 0 && self.before_track == 0 {
            return Some(format!(
                "all after {}, where the track ends — the logger stopped before these were shot",
                format_utc(last)
            ));
        }

        if self.before_track > 0 && self.after_track == 0 {
            return Some(format!(
                "all before {}, where the track starts — the logger was started after these were shot",
                format_utc(first)
            ));
        }

        if self.before_track > 0 && self.after_track > 0 {
            return Some(format!(
                "{} before {} and {} after {}",
                self.before_track,
                format_utc(first),
                self.after_track,
                format_utc(last)
            ));
        }

        None
    }

    /// Why frames fell into holes, when the pattern says something useful.
    ///
    /// **A break between recording sessions is the common real cause and the one worth
    /// naming**: the logger stopped and restarted, so the track arrives in fragments and
    /// every frame between two fragments is unbridgeable at any gap limit. That is not a
    /// tuning problem — widening `--max-gap-seconds` will not recover a single one of
    /// them — so telling the operator to adjust limits would be actively misleading.
    pub fn gap_note(&self) -> Option<String> {
        if self.in_gap == 0 {
            return None;
        }

        let widest = self
            .widest_gap
            .map(|gap| format!(", widest {}", humanise(gap)))
            .unwrap_or_default();

        // **Capitalized because these begin a line of the report.** Terry, 2026-08-06: the
        // output was inconsistent about it, and he wants sentences to start like sentences.
        // A line opening with a count — `49 tagged`, `196 sidecars` — keeps its digit; the
        // rule is about words, not about forcing a capital onto a number.
        if self.across_segments > 0 && self.within_segment == 0 {
            return Some(format!(
                "All of them across breaks in the recording{widest} — the logger stopped \
                 and restarted, and no gap limit can bridge that"
            ));
        }

        if self.within_segment > 0 && self.across_segments == 0 {
            return Some(format!(
                "All of them inside one recording but past the limits{widest} — these are \
                 what --max-gap-seconds and --max-gap-meters govern"
            ));
        }

        Some(format!(
            "{} across breaks in the recording, {} inside one but past the limits{widest}",
            self.across_segments, self.within_segment
        ))
    }
}

fn humanise(gap: TimeDelta) -> String {
    let seconds = gap.num_seconds();
    if seconds >= 3600 {
        format!("{}h{:02}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m{:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn format_utc(at: DateTime<Utc>) -> String {
    at.format("%H:%M:%SZ").to_string()
}

/// Write sidecars for everything the track genuinely supports.
///
/// `force` overwrites existing sidecars, and is decision 16's single door through the
/// never-rewrite-a-sidecar invariant.
pub fn run(
    landed: &[Landed],
    destinations: &[Destination],
    tracks: &[PathBuf],
    limits: GapLimits,
    force: bool,
    jobs: usize,
    progress: &Progress,
) -> Result<Report> {
    let track = Track::load(tracks).context("loading the GPX tracks")?;
    let mut report = Report::default();

    let (first, last) = track.span();
    report.track_span = Some((first, last));

    // Heading plus one row, matching phase 3 and 4. `frames` rather than `Geotag`: the
    // heading says the phase, so the row says what is being counted.
    let _section = progress.section("Geotagging", crate::progress::PHASE);
    let bar = progress.bar("Frames", landed.len(), crate::progress::PHASE);
    bar.set_message("correlating and writing sidecars");

    // **A pool built here, not rayon's global one.** `--jobs` has to size *this phase*, and
    // the global pool can be configured only once per process — a rule that turns any future
    // second caller into a silent no-op. Building locally also keeps the width visible at the
    // one place it means something.
    let pool = ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .context("building the geotag thread pool")?;

    let outcomes: Vec<Outcome> = pool.install(|| {
        landed
            .par_iter()
            .map(|photo| {
                let outcome = correlate(photo, &track, destinations, limits, force, first, last);
                bar.inc();
                outcome
            })
            .collect::<Result<Vec<_>>>()
    })?;

    // **Folded sequentially, in input order, and that is not fussiness.** `Report::misses`
    // feeds `systematic_offset`, which takes `seconds[len / 2]` *without sorting* — safe only
    // because the spread is bounded first, but a value that would drift run to run if the
    // vector arrived in completion order. `par_iter().map(..).collect()` preserves input
    // order, so this report is identical to the one the serial loop produced.
    for outcome in outcomes {
        match outcome {
            Outcome::Tagged { skipped, written } => {
                report.tagged += 1;
                report.skipped += skipped;
                report.written += written;
            }

            // Which side matters more than the count. Everything after the last point is the
            // signature of a logger that stopped -- forgotten, out of battery, or never
            // restarted after a break -- and that is a named risk of this workflow.
            Outcome::Outside { miss, before } => {
                report.outside_track += 1;
                if before {
                    report.before_track += 1;
                } else {
                    report.after_track += 1;
                }
                report.misses.push(miss);
            }

            // *Which kind* of hole is the actionable part. A break between recording sessions
            // means the logger stopped — signal lost, app backgrounded, battery — and that is
            // a different conversation from a logger that ran continuously but sparsely.
            Outcome::InGap {
                across_segments,
                duration,
            } => {
                report.in_gap += 1;
                if across_segments {
                    report.across_segments += 1;
                } else {
                    report.within_segment += 1;
                }
                if report.widest_gap.is_none_or(|widest| duration > widest) {
                    report.widest_gap = Some(duration);
                }
            }
        }
    }

    bar.finish();
    Ok(report)
}

/// One frame's result, carried out of the parallel pass so the counting stays sequential.
///
/// The counters are all commutative and could have been atomics; `misses` is not, and mixing
/// the two would leave a reader guessing which fields were order-sensitive. One value per
/// frame keeps that answer in the type.
enum Outcome {
    Tagged {
        skipped: usize,
        written: usize,
    },
    Outside {
        miss: TimeDelta,
        before: bool,
    },
    InGap {
        across_segments: bool,
        duration: TimeDelta,
    },
}

/// Correlate one frame and write its sidecars. **The whole of the parallel work.**
fn correlate(
    photo: &Landed,
    track: &Track,
    destinations: &[Destination],
    limits: GapLimits,
    force: bool,
    first: DateTime<Utc>,
    last: DateTime<Utc>,
) -> Result<Outcome> {
    match track.lookup(photo.captured, limits) {
        Lookup::Found(fix) => {
            // Rendered once and written to every destination, so all four copies carry
            // byte-identical sidecars (decision 11).
            let packet = xmp::render(&fix, photo.captured, WRITER);
            let mut skipped = 0;
            let mut written = 0;

            for destination in destinations {
                let sidecar = xmp::sidecar_path(&destination.root.join(&photo.relative));

                // The invariant: an existing sidecar is never rewritten without being asked
                // (decision 16). Phase 5 tags what is untagged and skips the rest, which is
                // also what makes a re-run converge.
                if sidecar.exists() && !force {
                    skipped += 1;
                    continue;
                }

                xmp::write_atomic(&sidecar, &packet)?;
                written += 1;
            }

            Ok(Outcome::Tagged { skipped, written })
        }

        Lookup::OutsideTrack => Ok(if photo.captured < first {
            Outcome::Outside {
                miss: photo.captured - first,
                before: true,
            }
        } else {
            Outcome::Outside {
                miss: photo.captured - last,
                before: false,
            }
        }),

        Lookup::InGap(gap) => Ok(Outcome::InGap {
            across_segments: gap.across_segments,
            duration: gap.duration,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn miss(seconds: i64) -> TimeDelta {
        TimeDelta::seconds(seconds)
    }

    /// The signature of a wrong camera clock: many misses, all nearly the same distance
    /// out. This is the one thing that can catch a lie the metadata tells honestly.
    #[test]
    fn a_tight_cluster_of_misses_reads_as_a_clock_error() {
        let report = Report {
            tagged: 0,
            outside_track: 12,
            misses: (0..12).map(|n| miss(7200 + n)).collect(),
            ..Default::default()
        };

        let offset = report.systematic_offset().expect("a systematic offset");
        assert!(
            (offset.num_seconds() - 7200).abs() < 60,
            "expected about +2h, got {offset:?}"
        );
    }

    /// **The three ways it MUST stay quiet, which matter as much as the one way it fires.** A
    /// false "check your clock" is what teaches an operator to ignore the real one.
    #[test]
    fn a_clock_error_is_only_claimed_for_a_tight_cluster_of_many_misses() {
        let scattered = Report {
            tagged: 0,
            outside_track: 12,
            misses: vec![
                miss(30),
                miss(900),
                miss(4000),
                miss(75),
                miss(12000),
                miss(60),
                miss(300),
                miss(8000),
                miss(45),
                miss(2500),
                miss(150),
                miss(6000),
            ],
            ..Default::default()
        };
        let too_few = Report {
            tagged: 0,
            outside_track: 3,
            misses: vec![miss(7200), miss(7201), miss(7202)],
            ..Default::default()
        };
        let anything_tagged = Report {
            tagged: 1,
            outside_track: 20,
            misses: (0..20).map(|n| miss(7200 + n)).collect(),
            ..Default::default()
        };

        for (why, report) in [
            ("a logging gap scatters rather than clustering", scattered),
            ("three misses are not evidence about a clock", too_few),
            (
                "anything tagged means the track covers the day",
                anything_tagged,
            ),
        ] {
            assert!(report.systematic_offset().is_none(), "{why}");
        }
    }

    /// The Moraine Lake case, and a named risk of this workflow: the logger stopped and
    /// the sunset shoot came after. The count alone is not actionable — the boundary is,
    /// because it is what makes the operator recognize the evening.
    #[test]
    fn misses_all_after_the_track_name_the_moment_the_logger_stopped() {
        let report = Report {
            tagged: 2_500,
            outside_track: 1_383,
            after_track: 1_383,
            track_span: Some((
                instant("2022-09-27T09:47:45Z"),
                instant("2022-09-27T17:19:36Z"),
            )),
            ..Default::default()
        };

        let note = report.boundary_note().expect("a boundary note");
        assert!(note.contains("17:19:36Z"), "{note}");
        assert!(note.contains("logger stopped"), "{note}");
    }

    /// The mirror case: started shooting before the logger was running.
    #[test]
    fn misses_all_before_the_track_name_the_late_start() {
        let report = Report {
            tagged: 900,
            outside_track: 40,
            before_track: 40,
            track_span: Some((
                instant("2022-09-27T09:47:45Z"),
                instant("2022-09-27T17:19:36Z"),
            )),
            ..Default::default()
        };

        let note = report.boundary_note().expect("a boundary note");
        assert!(note.contains("09:47:45Z"), "{note}");
        assert!(note.contains("started after"), "{note}");
    }

    /// A day that ran off both ends gets both numbers rather than a story that fits
    /// neither. Guessing a single cause here would be the false diagnosis that makes the
    /// true ones stop being believed.
    #[test]
    fn misses_on_both_sides_are_reported_as_both() {
        let report = Report {
            tagged: 900,
            outside_track: 50,
            before_track: 10,
            after_track: 40,
            track_span: Some((
                instant("2022-09-27T09:47:45Z"),
                instant("2022-09-27T17:19:36Z"),
            )),
            ..Default::default()
        };

        let note = report.boundary_note().expect("a boundary note");
        assert!(
            note.contains("10 before") && note.contains("40 after"),
            "{note}"
        );
    }

    /// Nothing outside the track: no note, because there is nothing to explain.
    #[test]
    fn a_fully_covered_day_says_nothing_about_boundaries() {
        let report = Report {
            tagged: 3_883,
            track_span: Some((
                instant("2022-09-27T09:47:45Z"),
                instant("2022-09-27T17:19:36Z"),
            )),
            ..Default::default()
        };

        assert!(report.boundary_note().is_none());
    }

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().expect("a valid instant")
    }

    /// The 2022-09-27 case, once the track was actually read: seven recording segments,
    /// and every unbridgeable frame sitting between two of them. Telling the operator to
    /// widen a limit here would be wrong — no limit bridges a break.
    #[test]
    fn gaps_across_recording_breaks_say_so_and_do_not_blame_the_limits() {
        let report = Report {
            tagged: 2_394,
            in_gap: 1_489,
            across_segments: 1_489,
            widest_gap: Some(TimeDelta::seconds(1694)),
            ..Default::default()
        };

        let note = report.gap_note().expect("a gap note");
        // The whole phrase, not the fragment before the line break. A `\` continuation
        // that loses its leading space — or gains eighteen — reads fine in the source and
        // prints a hole; asserting either side of the seam alone cannot see it, which is
        // exactly how this string shipped broken on 2026-08-04.
        assert!(note.contains("the logger stopped and restarted"), "{note}");
        assert!(note.contains("28m"), "{note}");
        assert!(
            !note.contains("--max-gap"),
            "must not suggest tuning a limit that cannot help: {note}"
        );
    }

    /// The opposite case, where the limits *are* the thing in play and naming them is
    /// the useful advice rather than a red herring.
    #[test]
    fn gaps_inside_one_recording_point_at_the_limits() {
        let report = Report {
            tagged: 900,
            in_gap: 40,
            within_segment: 40,
            widest_gap: Some(TimeDelta::seconds(140)),
            ..Default::default()
        };

        let note = report.gap_note().expect("a gap note");
        // Spanning the line break for the reason above.
        assert!(
            note.contains("these are what --max-gap-seconds and --max-gap-meters govern"),
            "{note}"
        );
    }

    #[test]
    fn a_day_with_no_gaps_says_nothing_about_them() {
        let report = Report {
            tagged: 3_883,
            ..Default::default()
        };
        assert!(report.gap_note().is_none());
    }
}
