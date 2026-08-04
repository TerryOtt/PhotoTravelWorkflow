//! Finding the camera cards, and deciding which one feeds phase 3.
//!
//! Decision 7, including its correction: **a card is a volume carrying `DCIM` that is
//! not a configured destination.** Nothing else identifies one. An in-camera format at
//! the start of every session assigns a new volume serial, so a card's identity changes
//! daily; the readers report fake hardware serials (`0123456789ABCDEF` on this rig); and
//! removability describes the reader's bridge rather than the medium — two identical
//! cards in two readers on one hub disagree about it.
//!
//! So which card is the fast one is settled by **measurement**, every run, costing about
//! two seconds. That needs no configuration, survives buying a new reader, and is
//! correct by construction: phase 3 runs off the faster card regardless of which reader
//! is in which port.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::pipeline::cr3_files;
use crate::storage::Volume;
use crate::winio::unbuffered_sample;

/// How much to read when timing a card.
///
/// Enough that the reader's steady-state rate dominates the first seek and the USB
/// negotiation, and small enough that two of them cost about two seconds rather than
/// being something you notice. Roughly one and a half frames from an R5.
pub const SAMPLE_BYTES: u64 = 64 * 1024 * 1024;

/// A camera card, as found.
#[derive(Debug, Clone)]
pub struct Card {
    pub volume: Volume,
    /// The `DCIM` directory that identified it.
    pub dcim: PathBuf,
}

impl Card {
    /// How the report and the refusals name this card.
    pub fn label(&self) -> String {
        match self.volume.mount_points.first() {
            Some(mount) => mount.display().to_string(),
            None => self.volume.guid_path.clone(),
        }
    }
}

/// What a timed read off one card produced.
#[derive(Debug, Clone, Copy)]
pub struct Speed {
    pub bytes: u64,
    pub elapsed: Duration,
}

impl Speed {
    pub fn bytes_per_second(&self) -> f64 {
        let seconds = self.elapsed.as_secs_f64();
        if seconds > 0.0 {
            self.bytes as f64 / seconds
        } else {
            f64::INFINITY
        }
    }
}

/// Every volume that carries a `DCIM` directory and is not one of the destinations.
///
/// **The destination exclusion is what makes `DCIM` alone safe.** Without it, an archive
/// disk that happened to hold a `DCIM` folder — a stray copy of someone's card, a
/// directory named by coincidence — would be offered as a source and could be *read from
/// and written to* in the same run. A volume the config already names is a destination,
/// full stop.
pub fn find(volumes: &[Volume], destination_guids: &[String]) -> Vec<Card> {
    let mut found: Vec<Card> = volumes
        .iter()
        .filter(|volume| !destination_guids.contains(&volume.guid_path))
        .filter_map(|volume| {
            volume
                .mount_points
                .iter()
                .map(|mount| mount.join("DCIM"))
                .find(|dcim| dcim.is_dir())
                .map(|dcim| Card {
                    volume: volume.clone(),
                    dcim,
                })
        })
        .collect();

    // Deterministic order, so a run's output does not depend on enumeration order and
    // two cards are always reported the same way round.
    found.sort_by_key(Card::label);
    found
}

/// Time an unbuffered read of about [`SAMPLE_BYTES`] from this card.
///
/// Unbuffered is the measurement rather than a refinement: the files may have been
/// written or read moments ago, and a cached read would report every reader as equally
/// and impossibly fast. What is timed is the device.
pub fn measure(card: &Card) -> Result<Speed> {
    let files = cr3_files(&card.dcim)
        .with_context(|| format!("listing {} to time it", card.dcim.display()))?;

    // Timing starts after the walk, so the directory enumeration — which is metadata
    // work on the filesystem, not throughput — stays out of the number.
    let start = Instant::now();
    let mut bytes = 0u64;

    for file in &files {
        if bytes >= SAMPLE_BYTES {
            break;
        }
        bytes += unbuffered_sample(file, SAMPLE_BYTES - bytes)?;
    }

    Ok(Speed {
        bytes,
        elapsed: start.elapsed(),
    })
}

/// The card phase 3 should read from, with both measurements for the report.
#[derive(Debug, Clone)]
pub struct Chosen {
    pub source: Card,
    pub source_speed: Speed,
    /// The other card and its speed, absent on a declared single-source run.
    pub other: Option<(Card, Speed)>,
}

/// Measure every card and pick the fastest as phase 3's source (decision 1).
///
/// Returns the cards in speed order rather than a bare winner, because decision 14's
/// report prints both rates — and because a *close* pair is worth seeing: CFexpress
/// against UHS-II should be unambiguous, and two cards that measure alike mean something
/// is not what it looks like.
pub fn choose(cards: &[Card]) -> Result<Chosen> {
    let mut measured: Vec<(Card, Speed)> = cards
        .iter()
        .map(|card| {
            measure(card)
                .map(|speed| (card.clone(), speed))
                .with_context(|| format!("timing {}", card.label()))
        })
        .collect::<Result<_>>()?;

    measured.sort_by(|a, b| {
        b.1.bytes_per_second()
            .partial_cmp(&a.1.bytes_per_second())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut measured = measured.into_iter();
    let (source, source_speed) = measured
        .next()
        .context("no camera card was found to read from")?;

    Ok(Chosen {
        source,
        source_speed,
        other: measured.next(),
    })
}

/// Whether `path` looks like a camera card's DCIM tree, for a caller that already has a
/// path rather than a volume.
pub fn has_dcim(mount: &Path) -> bool {
    mount.join("DCIM").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Volume;

    fn volume(guid: &str, mount: &str) -> Volume {
        Volume {
            guid_path: guid.to_string(),
            mount_points: vec![PathBuf::from(mount)],
            label: None,
            filesystem: None,
            volume_serial: 0,
            removable: false,
            total_bytes: 0,
            free_bytes: 0,
        }
    }

    /// The correction's core: a card is found by `DCIM`, and `removable` is not
    /// consulted at all. Built on volumes that claim *not* to be removable, which is what
    /// one of the two real cards reports — so a rule that filtered on it would find
    /// nothing here.
    #[test]
    fn a_card_is_found_by_dcim_regardless_of_what_removable_says() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        let card = scratch.path().join("card");
        std::fs::create_dir_all(card.join("DCIM")).expect("a DCIM tree");

        let volumes = vec![volume(r"\\?\Volume{card}\", &card.display().to_string())];
        let found = find(&volumes, &[]);

        assert_eq!(found.len(), 1);
        assert!(
            !found[0].volume.removable,
            "the fixture is deliberately not removable"
        );
        assert_eq!(found[0].dcim, card.join("DCIM"));
    }

    /// A volume with no `DCIM` is not a card, however removable it claims to be.
    #[test]
    fn a_volume_without_dcim_is_not_a_card() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        let plain = scratch.path().join("plain");
        std::fs::create_dir_all(&plain).expect("a directory");

        let mut volume = volume(r"\\?\Volume{plain}\", &plain.display().to_string());
        volume.removable = true;

        assert!(find(&[volume], &[]).is_empty());
    }

    /// The exclusion that makes `DCIM` alone safe. An archive disk holding a stray
    /// `DCIM` must never be offered as a source, or a run could read from and write to
    /// the same volume.
    #[test]
    fn a_configured_destination_is_never_a_card_even_with_dcim() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        let disk = scratch.path().join("archive");
        std::fs::create_dir_all(disk.join("DCIM")).expect("a stray DCIM");

        let guid = r"\\?\Volume{archive}\".to_string();
        let volumes = vec![volume(&guid, &disk.display().to_string())];

        assert_eq!(
            find(&volumes, &[]).len(),
            1,
            "without the exclusion it is a card"
        );
        assert!(
            find(&volumes, &[guid]).is_empty(),
            "a configured destination must never be offered as a source"
        );
    }
}
