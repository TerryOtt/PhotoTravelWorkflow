//! Phase 3 — ingest and verify. This phase is the product.
//!
//! The shape, from `DESIGN.md`'s *Phase 3 in detail*: one reader pulls each source file
//! into memory **exactly once**, takes its SHA-256 and its EXIF capture time from that
//! buffer, and hands it to one queue per destination. Each destination writes everything
//! through to media, then re-reads everything back unbuffered and compares. A record per
//! `(file, destination)` is appended to the run log as each verify read completes —
//! never before.
//!
//! # Why there is no rayon here
//!
//! Decision 15 sizes phase 3's hashing at 5N and says it must spread across cores. It
//! does, structurally, without a thread pool: the reader hashes the 1N it reads, and the
//! four destination threads each hash their own 1N verify stream, which is five
//! concurrent hash streams on a machine with twenty threads. At the measured
//! 2,380 MB/s per core (decision 17) a single stream already outruns the CFexpress card
//! and every SSD in the rig several times over, so a pool would add scheduling and take
//! nothing off the critical path. `--jobs` governs phase 5, where thousands of small
//! sidecars actually are CPU- and metadata-bound.
//!
//! # Why the channels are `std`
//!
//! Decision 29 declined `crossbeam-channel`. `sync_channel(DEPTH)` blocks the sender
//! when a queue is full, so handing each photo to the four queues in turn makes the
//! reader wait on the slowest destination — which is the backpressure the design asks
//! for, falling out of the types rather than being arranged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread;

use crate::progress::{Bar, Progress};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use geotag::format::RawFormat;
use geotag::raw::{Capture, MediaParser, capture_time_in_memory};

use crate::hash::{Digest32, hex, sha256};
use crate::manifest;
use crate::naming::{destination_path, unfiled_path, with_collision_suffix};
use crate::runlog::{RunLog, Verified};
use crate::winio::{unbuffered_sha256, write_through};

/// How many photos may sit in one destination's queue before the reader blocks.
///
/// The memory bound is this times the frame size — the same buffer is shared by all
/// four queues, so the live set is the *slowest* queue's backlog, not the sum. Four
/// 45 MB frames is ~180 MB, which buys enough slack for an SSD to stutter without
/// letting one lag far enough to matter.
const DEPTH: usize = 4;

/// A place a copy goes. Resolved by hardware identity in phase 2 (decision 6); by the
/// time it reaches phase 3 it is a label and a path, which is all this phase needs and
/// is what makes the phase testable over ordinary directories.
#[derive(Debug, Clone)]
pub struct Destination {
    pub label: String,
    pub root: PathBuf,
}

/// What one destination did.
#[derive(Debug, Clone)]
pub struct DestinationOutcome {
    pub label: String,
    pub written: usize,
    /// Already present with an identical hash, so not written again (decision 5).
    pub skipped: usize,
    pub verified: usize,
    /// Files whose read-back did not match. Non-empty means `NOT SAFE` (decision 14).
    pub failed: Vec<String>,
    /// Bytes this destination actually moved, counted rather than assumed.
    ///
    /// **A fresh file and a converged one cost the same two units**, which is not obvious:
    /// a written file pays one write plus one verify read, and a *skipped* one still pays an
    /// `unbuffered_sha256` of the target in [`place`] to prove the hash matches before
    /// skipping, plus the same verify read. So convergence does not move less data — it moves
    /// the same data with the write half shifted into reads.
    ///
    /// Summed rather than derived from `files × size × 2` because a collision retry, a
    /// mismatch, or a partial pass all make the derivation quietly wrong, and a headline
    /// throughput figure that is quietly wrong is worse than none.
    pub bytes_moved: u64,
}

/// What the phase did, in the terms decision 14's report is built from.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub files: usize,
    pub bytes: u64,
    /// Files that landed in `_unfiled` because their capture time was unreadable
    /// (decisions 21, 23).
    pub unfiled: Vec<String>,
    /// Every frame that *has* a capture time, carried forward so phase 5 correlates
    /// against the GPX without reopening a single raw file (decision 10).
    pub landed: Vec<crate::phase5::Landed>,
    /// Every frame ingested, with what phase 4 needs to corroborate it.
    pub ingested: Vec<Ingested>,
    pub destinations: Vec<DestinationOutcome>,
}

impl Outcome {
    /// LANDED: every file verified on every destination. The one question the verdict
    /// line answers.
    pub fn landed(&self) -> bool {
        self.destinations
            .iter()
            .all(|d| d.failed.is_empty() && d.verified == self.files)
    }
}

/// One frame as phase 3 ingested it, carried to phase 4.
///
/// **Paired by card-relative path** (decision 4): the camera writes the same tree to
/// both slots, so the same path is the same frame. The basename alone is not the key —
/// the counter is four digits and a 512 GB card holds more frames than it spans, so one
/// session can legitimately hold two `_50A0001.CR3` in different DCIM folders.
#[derive(Debug, Clone)]
pub struct Ingested {
    /// Relative to the card root, e.g. `DCIM/100CANON/_50A0001.CR3`.
    pub card_relative: PathBuf,
    /// Relative to each destination root, e.g. `2022/2022-09-27/1402Z_0001.CR3`.
    pub destination_relative: PathBuf,
    /// The canonical hash, taken from the source card's bytes in phase 3.
    pub sha256: Digest32,
}

/// One photo, read once and shared by every destination.
struct Photo {
    /// Where this goes inside every destination root — identical across all four,
    /// because the name is a pure function of the photo (decision 5).
    relative: PathBuf,
    sha256: Digest32,
    captured: Option<DateTime<Utc>>,
    bytes: Vec<u8>,
}

/// Which card fed the run, as *observed* rather than assumed (decision 12).
///
/// **Two fields in one struct rather than two adjacent `&str` parameters**, because those
/// are trivially swapped at a call site and the compiler would never notice — this project
/// prefers a mistake that cannot compile to one that surfaces in a manifest years later.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    /// `primary` when two cards were present, `sole` under `--allow-single-source`.
    ///
    /// **Not a card type.** The tool cannot know a card is a CFexpress: decision 7 identifies
    /// cards by measurement precisely because serial, removability and bus type all fail, and
    /// a CFexpress in a bridge reader enumerates as USB.
    pub role: &'a str,
    /// The source volume's serial, `XXXX-XXXX`. Reassigned by every in-camera format, so it
    /// identifies the card *generation* that fed this run — which is what a run is a property
    /// of (decision 13).
    pub volume_serial: &'a str,
}

/// Run phase 3.
///
/// `sources` are the CR3 paths phase 1 enumerated on the ingest card, already filtered
/// to `*.CR3` (decision 24). `source` is which card fed the run, recorded per file
/// so a single-source night is legible afterwards (decision 7).
pub fn run(
    sources: &[PathBuf],
    destinations: &[Destination],
    run_id: &str,
    source: Source<'_>,
    card_root: &Path,
    log: &RunLog,
    progress: &Progress,
) -> Result<Outcome> {
    if destinations.is_empty() {
        return Err(anyhow!("phase 3 needs at least one destination"));
    }

    let mut senders: Vec<SyncSender<Arc<Photo>>> = Vec::with_capacity(destinations.len());
    let mut receivers = Vec::with_capacity(destinations.len());

    for _ in destinations {
        let (sender, receiver) = sync_channel::<Arc<Photo>>(DEPTH);
        senders.push(sender);
        receivers.push(receiver);
    }

    let total = sources.len();

    // **Two sections, both created before any thread starts**, so `MultiProgress` draws the
    // `Writing` rows and then the `Verifying` rows underneath in a stable order rather than in
    // whatever order the destinations happen to reach each pass.
    //
    // A destination appears in both, and that is deliberate: there is no barrier between the
    // passes (see below), so the screen shows the laptop already reading back while a USB
    // drive is still being written. **That overlap is the single most useful thing on the
    // display** — decision 14 names the slowest device as the report's most useful number for
    // the same reason, and this is that, live.
    progress.section("Writing");
    let write_bars: Vec<Bar> = destinations
        .iter()
        .map(|destination| {
            let bar = progress.bar(&destination.label, total);
            bar.set_pass("writing");
            bar
        })
        .collect();

    progress.section("Verifying");
    let verify_bars: Vec<Bar> = destinations
        .iter()
        .map(|destination| {
            let bar = progress.bar(&destination.label, total);
            bar.set_pass("verifying");
            bar
        })
        .collect();

    thread::scope(|scope| {
        let workers: Vec<_> = destinations
            .iter()
            .zip(receivers)
            .zip(write_bars.into_iter().zip(verify_bars))
            .map(|((destination, receiver), (write_bar, verify_bar))| {
                scope.spawn(move || {
                    let mut landed = Vec::new();

                    // Write pass. Everything, before anything is read back — decision 2
                    // wants two clean sequential passes rather than a mixed stream.
                    for photo in receiver {
                        let outcome = place(destination, &photo)?;
                        landed.push(outcome);
                        write_bar.inc();
                    }
                    write_bar.finish();

                    // Verify pass. Every byte, off the media, with the page cache
                    // bypassed. **Starts the moment this destination's writes finish**, so
                    // the laptop's NVMe verifies while the slowest SSD is still writing.
                    // Introducing a barrier here to tidy the display would idle the fast
                    // drives and cost wall clock against the primary metric.
                    let done = verify(destination, landed, run_id, source, log, &verify_bar);
                    verify_bar.finish();
                    done
                })
            })
            .collect();

        let read = feed(sources, &senders, run_id, card_root);

        // Dropped explicitly and before the joins: the destination threads iterate their
        // receiver until every sender is gone, so holding these would deadlock the join
        // below no matter what the reader did.
        drop(senders);

        let mut outcome = read?;
        for worker in workers {
            let done = worker
                .join()
                .map_err(|_| anyhow!("a destination thread panicked"))??;
            outcome.destinations.push(done);
        }

        Ok(outcome)
    })
}

/// The reader: each source file read exactly once, hashed and named from that buffer.
fn feed(
    sources: &[PathBuf],
    senders: &[SyncSender<Arc<Photo>>],
    run_id: &str,
    card_root: &Path,
) -> Result<Outcome> {
    let mut parser = MediaParser::new();
    let mut outcome = Outcome {
        files: 0,
        bytes: 0,
        unfiled: Vec::new(),
        landed: Vec::new(),
        ingested: Vec::new(),
        destinations: Vec::new(),
    };

    for source in sources {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("{} has no usable file name", source.display()))?
            .to_owned();

        let bytes = std::fs::read(source)
            .with_context(|| format!("reading {} from the card", source.display()))?;

        // Both from the buffer, so the file is never read twice (decision 10).
        let sha256 = sha256(&bytes);
        let captured = capture_instant(&mut parser, &bytes);

        let relative = match captured {
            Some(at) => {
                let relative = destination_path(at, &name);
                outcome.landed.push(crate::phase5::Landed {
                    relative: relative.clone(),
                    captured: at,
                });
                relative
            }
            // Decision 21: a defective file is kept, not dropped. It is still hashed,
            // still written everywhere, still verified — only its placement is
            // unknowable, and `_unfiled` is where the unnameable go.
            None => {
                outcome.unfiled.push(name.clone());
                unfiled_path(run_id, &name)
            }
        };

        outcome.files += 1;
        outcome.bytes += bytes.len() as u64;

        outcome.ingested.push(Ingested {
            card_relative: source
                .strip_prefix(card_root)
                .unwrap_or(source)
                .to_path_buf(),
            destination_relative: relative.clone(),
            sha256,
        });

        let photo = Arc::new(Photo {
            relative,
            sha256,
            captured,
            bytes,
        });

        // In turn, so a full queue blocks here. That is the backpressure.
        for sender in senders {
            if sender.send(Arc::clone(&photo)).is_err() {
                return Err(anyhow!("a destination stopped accepting work"));
            }
        }
    }

    Ok(outcome)
}

/// The capture instant, or `None` for anything that cannot supply one.
///
/// Every failure mode collapses to the same answer here, and that is not laziness: with
/// the bytes already in RAM there is no I/O error left to distinguish, so unreadable
/// EXIF, EXIF with no capture tag, and EXIF with no UTC offset are three ways of saying
/// *this file cannot be named*, and decision 21 sends all three to the same place.
fn capture_instant(parser: &mut MediaParser, bytes: &[u8]) -> Option<DateTime<Utc>> {
    match capture_time_in_memory(parser, bytes, RawFormat::Cr3, None) {
        Ok(Capture::Resolved { at, .. }) => Some(at),
        _ => None,
    }
}

/// What one destination did with one photo, carried into the verify pass.
struct Placed {
    relative: PathBuf,
    sha256: Digest32,
    captured: Option<DateTime<Utc>>,
    bytes: u64,
    skipped: bool,
}

/// Write one photo to one destination, or establish that it is already there.
fn place(destination: &Destination, photo: &Photo) -> Result<Placed> {
    let mut relative = photo.relative.clone();
    let mut target = destination.root.join(&relative);

    // Decision 5's one content check. Deciding on the file name alone would get the
    // common case wrong in the silent direction — a genuinely different photo skipped
    // because its name matched — so the hash is what decides.
    for attempt in 1..=MAX_COLLISION_ATTEMPTS {
        if !target.exists() {
            break;
        }

        if unbuffered_sha256(&target)? == photo.sha256 {
            return Ok(Placed {
                relative,
                sha256: photo.sha256,
                captured: photo.captured,
                bytes: photo.bytes.len() as u64,
                skipped: true,
            });
        }

        // Two distinct photos sharing a basename within one minute. Pathological, and
        // this branch should effectively never fire.
        let name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("{} has no usable file name", relative.display()))?;
        let renamed = with_collision_suffix(name, attempt);

        relative.set_file_name(renamed);
        target = destination.root.join(&relative);
    }

    write_through(&target, &photo.bytes)?;

    Ok(Placed {
        relative,
        sha256: photo.sha256,
        captured: photo.captured,
        bytes: photo.bytes.len() as u64,
        skipped: false,
    })
}

/// How many suffixed names to try before giving up. Decision 5 calls this branch
/// pathological; a run that reaches the limit is not a collision, it is a bug.
const MAX_COLLISION_ATTEMPTS: u32 = 999;

/// The verify pass: every file re-read off the media and compared to the hash taken
/// from the source buffer.
fn verify(
    destination: &Destination,
    landed: Vec<Placed>,
    run_id: &str,
    source: Source<'_>,
    log: &RunLog,
    bar: &crate::progress::Bar,
) -> Result<DestinationOutcome> {
    let mut outcome = DestinationOutcome {
        label: destination.label.clone(),
        written: landed.iter().filter(|p| !p.skipped).count(),
        skipped: landed.iter().filter(|p| p.skipped).count(),
        verified: 0,
        failed: Vec::new(),
        // The write-or-skip-check half, already paid by the time this pass starts.
        bytes_moved: landed.iter().map(|placed| placed.bytes).sum(),
    };

    let mut landed_by_folder: BTreeMap<PathBuf, Vec<manifest::Entry>> = BTreeMap::new();

    for placed in landed {
        bar.inc();
        let target = destination.root.join(&placed.relative);
        let name = placed.relative.to_string_lossy().replace('\\', "/");

        // Counted before the comparison: a file that fails still came off the media, and a
        // throughput figure describes what the hardware did rather than what it proved.
        outcome.bytes_moved += placed.bytes;

        if unbuffered_sha256(&target)? != placed.sha256 {
            outcome.failed.push(name);
            continue;
        }

        outcome.verified += 1;

        let verified_utc = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let captured_utc = placed
            .captured
            .map(|at| at.format("%Y-%m-%dT%H:%M:%SZ").to_string());

        // Only now. A record that preceded its verify read would let resume trust work
        // that was never proven (decision 13).
        log.append(&Verified {
            run_id: run_id.to_owned(),
            name: name.clone(),
            destination: destination.label.clone(),
            sha256: hex(&placed.sha256),
            bytes: placed.bytes,
            captured_utc: captured_utc.clone(),
            source_card: source.role.to_owned(),
            source_volume_serial: source.volume_serial.to_owned(),
            verified_utc: verified_utc.clone(),
        })?;

        // Collected per folder so each manifest is written once at the end of the pass
        // rather than rewritten per file — decision 12 wants one atomic write.
        if let Some(folder) = placed.relative.parent() {
            landed_by_folder
                .entry(folder.to_path_buf())
                .or_default()
                .push(manifest::Entry {
                    name: placed
                        .relative
                        .file_name()
                        .map_or_else(|| name.clone(), |n| n.to_string_lossy().into_owned()),
                    status: manifest::Status::Present,
                    sha256: hex(&placed.sha256),
                    bytes: placed.bytes,
                    captured_utc,
                    source_card: source.role.to_owned(),
                    source_volume_serial: source.volume_serial.to_owned(),
                    run_id: run_id.to_owned(),
                    verified_utc,
                    // Phase 4 has not run, so corroboration is genuinely *pending*
                    // rather than absent (decision 12).
                    corroborated: None,
                    deletion: None,
                });
        }
    }

    // The durable artifact (decision 12). Only files that *verified* reach here, so a
    // manifest can never claim something the read-back could not confirm.
    for (folder, entries) in landed_by_folder {
        let date = folder
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());

        manifest::update(
            &destination.root.join(&folder),
            &date,
            &destination.label,
            manifest::Run {
                run_id: run_id.to_owned(),
                files_added: entries.len(),
                bytes_added: entries.iter().map(|entry| entry.bytes).sum(),
            },
            entries,
        )?;
    }

    Ok(outcome)
}

/// Every `*.CR3` under `root`, sorted, as phase 1 hands them over.
///
/// Sorted so a run is deterministic and two cards can be compared listing to listing
/// (decision 27). The filter is decision 24's: this tool ingests CR3 and nothing else,
/// and a stray is the report's business rather than the pipeline's.
pub fn cr3_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("cr3"))
        })
        .collect();

    found.sort();
    Ok(found)
}
