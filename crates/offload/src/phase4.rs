//! Phase 4 — corroboration. The second opinion, and the only code here that deletes.
//!
//! Decision 1 keeps the SDXC off the critical path: in the normal run it contributes no
//! bytes to the output, it is a corroborating hash, and reading it costs real minutes. So
//! the guarantee is **preserved but deferred** — any disagreement is still detected, just
//! after LANDED rather than before it.
//!
//! # Two outcomes, because decision 27 settled the rest
//!
//! Pre-flight already proved both cards hold the same names at the same sizes before a
//! byte moved, so phase 4 is left with the one question a listing cannot answer:
//!
//! | The other card's hash | Action |
//! |---|---|
//! | matches the canonical hash | keep, mark `matched` |
//! | differs | delete from all four, tombstone, quarantine |
//!
//! A file the gate saw that now cannot be read at all is **not** a corroboration
//! outcome — a card that changes under a run is environmental, and environmental is
//! fatal (decision 18).
//!
//! # This is the only path that destroys data
//!
//! Decision 18 makes it test #1 for exactly that reason: *a bug here deletes
//! photographs.* Two safeguards stand in front of the deletion, and neither is optional.
//!
//! **Both copies are re-read before anything is acted on.** A hash mismatch may be a
//! transient read error in a card reader rather than media corruption. At the observed
//! 1–2 incidents per run this costs ~90 MB and almost never happens.
//!
//! **Quarantine rather than unlink.** Both variants are copied out before the file is
//! removed, which makes the irreversible step reversible for the window where that
//! matters. Losing one frame of thirty on a scene is not measurable data loss; losing it
//! *and* the evidence would be.

use std::path::{Path, PathBuf};

use crate::progress::Progress;
use anyhow::{Context, Result, bail};

use crate::hash::{Digest32, hex};
use crate::pipeline::{Destination, Ingested};
use crate::winio::unbuffered_sha256;

/// What phase 4 decided about one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The other card agreed.
    Matched,
    /// Confirmed on a second read of both copies. The file has been quarantined and
    /// removed from every destination.
    Mismatched {
        source_sha256: String,
        other_sha256: String,
    },
    /// The first read disagreed and the second agreed — a transient reader error, which
    /// is exactly what the re-read safeguard exists to catch. Kept, and worth reporting:
    /// a reader producing these is on its way out.
    Transient,
}

/// What phase 4 did.
#[derive(Debug, Default)]
pub struct Report {
    pub matched: usize,
    pub transient: usize,
    /// One entry per confirmed mismatch: the destination-relative name, quarantined and
    /// removed everywhere.
    pub mismatched: Vec<(PathBuf, String, String)>,
    pub quarantine: PathBuf,
}

impl Report {
    /// Decision 4: **a run reporting far more mismatches than the 1–2 baseline is a dying
    /// card, and the summary should say so in those words** rather than making the number
    /// speak for itself. A photographer reading "47 mismatches" should not have to know
    /// what normal looks like.
    pub fn suspect_card(&self) -> bool {
        const BASELINE: usize = 2;
        self.mismatched.len() > BASELINE * 5
    }
}

/// Corroborate every ingested frame against the other card.
///
/// `fail_on_mismatch` is decision 3's `--fail-on-source-mismatch`: abort rather than
/// delete. It changes nothing about detection, only about what happens next.
pub fn run(
    ingested: &[Ingested],
    source_card_root: &Path,
    other_card_root: &Path,
    destinations: &[Destination],
    quarantine: &Path,
    fail_on_mismatch: bool,
    progress: &Progress,
) -> Result<Report> {
    let mut report = Report {
        quarantine: quarantine.to_path_buf(),
        ..Default::default()
    };

    // The longest silent stretch in the whole run before this: phase 4 reads the entire
    // corroborating card, which is ~16 minutes on a 201 GB day even at a healthy 205 MB/s.
    // Heading plus one row, the same shape phase 3 uses. The row is labelled by what is
    // being read rather than by the phase — the heading already says the phase, and
    // `secondary` is the word the card block and the eject block both use for this card.
    let _section = progress.section("Corroborating", crate::progress::PHASE);
    let bar = progress.bar("Secondary", ingested.len(), crate::progress::PHASE);

    for frame in ingested {
        bar.inc();
        // Paired by card-relative path, never by basename and never by content
        // (decision 4). The camera writes the same tree to both slots, so the same path
        // is the same frame; matching by hash instead would read a genuine mismatch as
        // two unrelated files and lose the quarantine that makes it recoverable.
        let other = other_card_root.join(&frame.card_relative);

        let other_hash = unbuffered_sha256(&other).with_context(|| {
            format!(
                "reading {} from the corroborating card. The gate saw this file before \
                 phase 3 started, so a card that can no longer produce it has changed \
                 under the run — that is environmental, not a corroboration outcome",
                other.display()
            )
        })?;

        if other_hash == frame.sha256 {
            report.matched += 1;
            continue;
        }

        // Safeguard one: never act on a single read. A card reader glitch is far more
        // likely than media corruption, and this is the cheap way to tell them apart.
        let source = source_card_root.join(&frame.card_relative);
        let source_again = unbuffered_sha256(&source)
            .with_context(|| format!("re-reading {} to confirm a mismatch", source.display()))?;
        let other_again = unbuffered_sha256(&other)
            .with_context(|| format!("re-reading {} to confirm a mismatch", other.display()))?;

        if source_again == other_again {
            report.transient += 1;
            continue;
        }

        if fail_on_mismatch {
            bail!(
                "SOURCE MISMATCH on {} — the two cards disagree about this frame.\n\n\
                 Refusing to go further because --fail-on-source-mismatch was given. \
                 Nothing has been deleted.\n\n\
                 source card: {}\n other card: {}",
                frame.card_relative.display(),
                hex(&source_again),
                hex(&other_again)
            );
        }

        // Safeguard two: both variants are preserved *before* anything is removed, so
        // the irreversible step is reversible for as long as it matters.
        quarantine_both(quarantine, &frame.card_relative, &source, &other)?;
        remove_everywhere(destinations, &frame.destination_relative)?;

        report.mismatched.push((
            frame.destination_relative.clone(),
            hex(&source_again),
            hex(&other_again),
        ));
    }

    bar.finish();
    Ok(report)
}

/// Copy both disputed variants somewhere outside the archive tree.
///
/// Outside `YYYY\` deliberately, so Lightroom never sees them and the archives stay
/// exactly what they claim to be (decision 3).
fn quarantine_both(
    quarantine: &Path,
    card_relative: &Path,
    source: &Path,
    other: &Path,
) -> Result<()> {
    std::fs::create_dir_all(quarantine)
        .with_context(|| format!("creating {}", quarantine.display()))?;

    let stem = card_relative.file_name().map_or_else(
        || "unnamed".into(),
        |name| name.to_string_lossy().into_owned(),
    );

    for (label, from) in [("source", source), ("other", other)] {
        let to = quarantine.join(format!("{label}_{stem}"));
        std::fs::copy(from, &to).with_context(|| {
            format!(
                "quarantining {} to {} — refusing to delete anything until both \
                 variants are safely copied out",
                from.display(),
                to.display()
            )
        })?;
    }

    Ok(())
}

/// Remove a frame from every destination.
///
/// Only ever called after both variants are quarantined. A destination that does not
/// have the file is not an error — the same run may have been interrupted, or a
/// `--without` destination may never have received it.
fn remove_everywhere(destinations: &[Destination], relative: &Path) -> Result<()> {
    for destination in destinations {
        let target = destination.root.join(relative);
        match std::fs::remove_file(&target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("removing the disputed frame from {}", target.display())
                });
            }
        }

        // The sidecar is derived data and means nothing without its raw, so it goes too.
        let sidecar = geotag::xmp::sidecar_path(&target);
        let _ = std::fs::remove_file(sidecar);
    }

    Ok(())
}

/// The canonical hash a frame is corroborated against, for callers assembling
/// [`Ingested`] from a run log rather than from a live phase 3.
pub fn canonical(sha256_hex: &str) -> Option<Digest32> {
    let bytes = (0..sha256_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(sha256_hex.get(i..i + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;

    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha256;

    struct Rig {
        _scratch: tempfile::TempDir,
        source: PathBuf,
        other: PathBuf,
        quarantine: PathBuf,
        destinations: Vec<Destination>,
        ingested: Vec<Ingested>,
    }

    /// Two cards holding the same frame, already landed on two destinations.
    fn rig(source_bytes: &[u8], other_bytes: &[u8]) -> Rig {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        let root = scratch.path();

        let card_relative = PathBuf::from("DCIM").join("100CANON").join("_50A0001.CR3");
        let destination_relative = PathBuf::from("2022")
            .join("2022-09-27")
            .join("1402Z_0001.CR3");

        for (card, bytes) in [("source", source_bytes), ("other", other_bytes)] {
            let path = root.join(card).join(&card_relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }

        let destinations: Vec<Destination> = ["a", "b"]
            .iter()
            .map(|label| Destination {
                label: (*label).to_string(),
                root: root.join(label),
            })
            .collect();

        for destination in &destinations {
            let path = destination.root.join(&destination_relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, source_bytes).unwrap();
            std::fs::write(geotag::xmp::sidecar_path(&path), b"<x:xmpmeta/>").unwrap();
        }

        Rig {
            source: root.join("source"),
            other: root.join("other"),
            quarantine: root.join("_runs").join("quarantine"),
            ingested: vec![Ingested {
                card_relative,
                destination_relative,
                sha256: sha256(source_bytes),
            }],
            destinations,
            _scratch: scratch,
        }
    }

    fn frame(seed: u8) -> Vec<u8> {
        (0..200_000u32).map(|i| (i as u8) ^ seed).collect()
    }

    /// The ordinary case: both cards agree, nothing is touched.
    #[test]
    fn agreeing_cards_corroborate_and_delete_nothing() {
        let rig = rig(&frame(0), &frame(0));

        let report = run(
            &rig.ingested,
            &rig.source,
            &rig.other,
            &rig.destinations,
            &rig.quarantine,
            false,
            &crate::progress::Progress::silent(),
        )
        .expect("corroboration");

        assert_eq!(report.matched, 1);
        assert!(report.mismatched.is_empty());
        for destination in &rig.destinations {
            assert!(
                destination
                    .root
                    .join(&rig.ingested[0].destination_relative)
                    .exists(),
                "an agreeing frame must not be removed"
            );
        }
    }

    /// **Decision 18's test #1.** One byte differs between the cards, and the frame must
    /// be quarantined *and* removed from every destination — not silently dropped, and
    /// not wrongly kept.
    #[test]
    fn a_disagreeing_frame_is_quarantined_and_removed_everywhere() {
        let mut other = frame(0);
        other[100_000] ^= 0x01;
        let rig = rig(&frame(0), &other);

        let report = run(
            &rig.ingested,
            &rig.source,
            &rig.other,
            &rig.destinations,
            &rig.quarantine,
            false,
            &crate::progress::Progress::silent(),
        )
        .expect("corroboration");

        assert_eq!(report.matched, 0);
        assert_eq!(report.mismatched.len(), 1, "the disagreement must be found");

        let (_, source_hash, other_hash) = &report.mismatched[0];
        assert_ne!(
            source_hash, other_hash,
            "both competing hashes are recorded"
        );

        // Gone from every destination, sidecar included.
        for destination in &rig.destinations {
            let target = destination.root.join(&rig.ingested[0].destination_relative);
            assert!(!target.exists(), "{} still holds it", destination.label);
            assert!(!geotag::xmp::sidecar_path(&target).exists());
        }

        // And recoverable: both variants preserved, byte for byte.
        let kept_source = std::fs::read(rig.quarantine.join("source__50A0001.CR3"))
            .expect("the source variant must be quarantined");
        let kept_other = std::fs::read(rig.quarantine.join("other__50A0001.CR3"))
            .expect("the other variant must be quarantined");

        assert_eq!(kept_source, frame(0));
        assert_eq!(kept_other, other);
    }

    /// `--fail-on-source-mismatch` detects the same thing and refuses to act. **Nothing
    /// may be deleted**, which is the entire point of asking for it.
    #[test]
    fn fail_on_mismatch_aborts_without_deleting_anything() {
        let mut other = frame(0);
        other[100_000] ^= 0x01;
        let rig = rig(&frame(0), &other);

        let error = run(
            &rig.ingested,
            &rig.source,
            &rig.other,
            &rig.destinations,
            &rig.quarantine,
            true,
            &crate::progress::Progress::silent(),
        )
        .expect_err("it must refuse");

        assert!(format!("{error:#}").contains("Nothing has been deleted"));
        for destination in &rig.destinations {
            assert!(
                destination
                    .root
                    .join(&rig.ingested[0].destination_relative)
                    .exists(),
                "aborting must leave every copy in place"
            );
        }
    }

    /// A card that cannot produce a file the gate already saw has changed under the run.
    /// That is environmental, and decision 18 makes environmental fatal — it must never
    /// be mistaken for a corroboration verdict and must never delete anything.
    #[test]
    fn an_unreadable_corroborating_file_is_fatal_rather_than_a_verdict() {
        let rig = rig(&frame(0), &frame(0));
        std::fs::remove_file(rig.other.join(&rig.ingested[0].card_relative)).unwrap();

        let error = run(
            &rig.ingested,
            &rig.source,
            &rig.other,
            &rig.destinations,
            &rig.quarantine,
            false,
            &crate::progress::Progress::silent(),
        )
        .expect_err("it must be fatal");

        assert!(format!("{error:#}").contains("changed under the run"));
        for destination in &rig.destinations {
            assert!(
                destination
                    .root
                    .join(&rig.ingested[0].destination_relative)
                    .exists()
            );
        }
    }

    /// Decision 4: a run far above the 1–2 baseline is a dying card, and the report has
    /// to be able to say so in words.
    #[test]
    fn a_flood_of_mismatches_reads_as_a_failing_card() {
        let mut report = Report::default();
        assert!(!report.suspect_card(), "two is the normal baseline");

        report.mismatched = (0..2)
            .map(|n| (PathBuf::from(format!("{n}.CR3")), "a".into(), "b".into()))
            .collect();
        assert!(!report.suspect_card());

        report.mismatched = (0..47)
            .map(|n| (PathBuf::from(format!("{n}.CR3")), "a".into(), "b".into()))
            .collect();
        assert!(report.suspect_card(), "47 is not a normal night");
    }

    #[test]
    fn a_hex_digest_round_trips_through_canonical() {
        let digest = sha256(b"abc");
        assert_eq!(canonical(&hex(&digest)), Some(digest));
        assert_eq!(canonical("not hex"), None);
    }
}
