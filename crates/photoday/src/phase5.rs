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

use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use geotag::track::{GapLimits, Lookup, Track};
use geotag::xmp;

use crate::pipeline::Destination;

/// How this tool identifies itself in a sidecar's `x:xmptk`.
const WRITER: &str = concat!("photoday ", env!("CARGO_PKG_VERSION"));

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
) -> Result<Report> {
    let track = Track::load(tracks).context("loading the GPX tracks")?;
    let mut report = Report::default();

    let (first, last) = track.span();

    for photo in landed {
        match track.lookup(photo.captured, limits) {
            Lookup::Found(fix) => {
                report.tagged += 1;

                // Rendered once and written to every destination, so all four copies
                // carry byte-identical sidecars (decision 11).
                let packet = xmp::render(&fix, photo.captured, WRITER);

                for destination in destinations {
                    let sidecar = xmp::sidecar_path(&destination.root.join(&photo.relative));

                    // The invariant: an existing sidecar is never rewritten without
                    // being asked (decision 16). Phase 5 tags what is untagged and
                    // skips the rest, which is also what makes a re-run converge.
                    if sidecar.exists() && !force {
                        report.skipped += 1;
                        continue;
                    }

                    xmp::write_atomic(&sidecar, &packet)?;
                    report.written += 1;
                }
            }

            Lookup::OutsideTrack => {
                report.outside_track += 1;
                // How far out, so a systematic offset can be recognised later.
                report.misses.push(if photo.captured < first {
                    photo.captured - first
                } else {
                    photo.captured - last
                });
            }

            Lookup::InGap(_) => report.in_gap += 1,
        }
    }

    Ok(report)
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

    /// A logging gap scatters. Saying "check your clock" here is the false alarm that
    /// teaches an operator to ignore the real one, so it must stay quiet.
    #[test]
    fn scattered_misses_are_a_logging_gap_and_stay_quiet() {
        let report = Report {
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

        assert!(report.systematic_offset().is_none());
    }

    /// Too few misses to judge. Two frames shot before the logger started are not
    /// evidence about the camera's clock.
    #[test]
    fn a_handful_of_misses_is_not_evidence() {
        let report = Report {
            tagged: 0,
            outside_track: 3,
            misses: vec![miss(7200), miss(7201), miss(7202)],
            ..Default::default()
        };

        assert!(report.systematic_offset().is_none());
    }

    /// If anything tagged, the track plainly covers the day and the clock is fine —
    /// whatever missed, missed for its own reasons.
    #[test]
    fn a_run_that_tagged_anything_never_blames_the_clock() {
        let report = Report {
            tagged: 1,
            outside_track: 20,
            misses: (0..20).map(|n| miss(7200 + n)).collect(),
            ..Default::default()
        };

        assert!(report.systematic_offset().is_none());
    }
}
