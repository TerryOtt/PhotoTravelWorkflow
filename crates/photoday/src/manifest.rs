//! The durable artifact: what a date folder holds, and proof of it.
//!
//! Decision 12 splits the two records by durability requirement. The run log is the
//! fragile one — append-only, survives a crash mid-phase. **This is the durable one**,
//! written atomically at the end of a run, one per *date folder* so a day is
//! self-contained: `2026\2026-08-03\` can be copied anywhere and still verify itself,
//! and no run has to rewrite a manifest spanning years.
//!
//! # Raw files only
//!
//! Sidecars are regenerable derived data and are deliberately not covered. A re-run of
//! phase 5 against a corrected track rewrites every sidecar, and a `verify` that hashed
//! them would then report four healthy copies as damaged. **A verification tool whose
//! warnings you learn to ignore is worse than one that checks less and means it.**
//!
//! # Readable forever
//!
//! Decision 28: `verify` reads every schema version this tool has ever written,
//! permanently — dropping schema 1 would not degrade an old archive, it would strip it
//! of the one thing that makes it self-describing, and the disk in the safe cannot be
//! regenerated. The other direction fails loudly instead of guessing: an older binary
//! meeting a newer manifest says exactly that, and never reports the photos as damaged.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::hash::{hex, sha256};

/// The newest schema this build writes.
///
/// Bumped only when an old reader would be **wrong**, not when it would be incomplete
/// (decision 28). Adding a field an old `verify` ignores while still checking every hash
/// correctly is not a bump; redefining a field, removing one, or making a new one
/// load-bearing for verification is.
pub const CURRENT_SCHEMA: u32 = 1;

/// A manifest as it sits on disk.
///
/// **The checksum covers `body`, not the whole file**, which is what makes it checkable
/// at all: the reader hashes the same bytes the writer did by routing both through
/// `serde_json::Value`, whose maps are key-sorted. Hashing "the file minus this one
/// field" is the alternative, and it is the fragile spelling of the same idea — any
/// difference in field order or spacing breaks it silently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// First, so a reader knows what it is holding before it reads anything else
    /// (decision 28).
    pub schema: u32,
    /// SHA-256 of `body`, so `verify` can tell *this manifest is damaged, your photos are
    /// probably fine* from *your photos are damaged* (decision 12).
    pub checksum: String,
    pub body: Body,
}

/// Everything the checksum covers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub date_utc: String,
    pub destination: String,
    /// One entry per offload that contributed, which is what makes several offloads a
    /// day legible after the fact.
    pub runs: Vec<Run>,
    pub files: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub run_id: String,
    pub files_added: usize,
    pub bytes_added: u64,
}

/// One raw file. **These five fields are the stable core no schema bump may redefine**
/// (decision 28) — `name`, `status`, `sha256`, `bytes`, `captured_utc` — because a
/// cross-check between two copies will eventually span two schema versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub status: Status,
    pub sha256: String,
    pub bytes: u64,
    /// Absent for a file in `_unfiled`, which is there precisely because it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_utc: Option<String>,
    /// Which card fed the run — the *role*, `primary` or `sole`, never a card type
    /// (decision 12). The tool cannot know a card is a CFexpress.
    pub source_card: String,
    /// The source volume's serial, `XXXX-XXXX`, as observed at pre-flight.
    ///
    /// **Both serde attributes are load-bearing, and the second one is the subtle one.**
    /// `default` lets a schema-1 manifest — written before this field existed — still parse,
    /// which decision 28 promises forever. `skip_serializing_if` is what stops it *breaking
    /// its own checksum*: the manifest's self-hash covers `body` canonicalized through
    /// `serde_json::Value`, so writing `"source_volume_serial": ""` into a re-read schema-1
    /// manifest changes those bytes and the checksum stops validating. Every archive already
    /// in the safe would have begun reporting as damaged — the exact false alarm on
    /// irreplaceable data that decision 12 exists to prevent.
    ///
    /// Caught by the committed schema-1 fixture on the first run after this field was added,
    /// which is what that test is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_volume_serial: String,
    pub run_id: String,
    pub verified_utc: String,
    /// Phase 4's verdict, `None` while still pending.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corroborated: Option<Corroborated>,
    /// Set only on a tombstone: the two hashes that disagreed, and why it was deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion: Option<Deletion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Present,
    /// A **tombstone**: deleted in phase 4 for a genuine mismatch, and kept in the list
    /// so a `verify` years later reports *clean* rather than flagging a missing file
    /// nobody remembers deleting (decision 12).
    Deleted,
}

/// Why two copies of one frame disagreed, kept with the tombstone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deletion {
    pub source_sha256: String,
    pub other_sha256: String,
    pub reason: String,
    pub deleted_utc: String,
}

/// Phase 4's verdict per file (decision 12).
///
/// The three non-matched outcomes are deliberately distinct, because conflating them is
/// how a record stops meaning anything years later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Corroborated {
    /// The other card agreed.
    Matched,
    /// **The two cards disagreed**, confirmed on a re-read of both copies, and the frame
    /// was quarantined and deleted everywhere. Always paired with [`Status::Deleted`] and
    /// a [`Deletion`].
    ///
    /// This exists because the alternatives both lie. `Matched` would claim an agreement
    /// that never happened, and `None` means *pending* — so without it a deleted frame is
    /// indistinguishable from one phase 4 never examined, which is the exact conflation
    /// this enum was written to prevent.
    Mismatched,
    /// **By declaration**: a single-source run had no second card to consult
    /// (decision 7).
    Waived,
    /// **By loss**: the card generation that could have answered was reformatted before
    /// phase 4 examined it (decision 13).
    Forfeited,
}

/// Why a manifest could not be used.
///
/// A typed error rather than a string, because `verify` must *branch* on these and must
/// never let the second wear the costume of the third — decision 28's whole point.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error(
        "this manifest is schema {found}, and this build understands up to {understood}. \
         Use a newer photoday — your photographs are fine."
    )]
    SchemaTooNew { found: u32, understood: u32 },

    #[error(
        "this manifest's own checksum does not match its contents, so the manifest is \
         damaged. Your photographs are probably fine — check them against another copy's \
         manifest."
    )]
    ChecksumMismatch,

    #[error("this manifest could not be parsed: {0}")]
    Unreadable(#[from] serde_json::Error),

    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Manifest {
    /// Build a manifest and seal it with its own checksum.
    pub fn seal(body: Body) -> Result<Self> {
        Ok(Self {
            schema: CURRENT_SCHEMA,
            checksum: checksum_of(&body)?,
            body,
        })
    }

    /// Read and check a manifest.
    ///
    /// The order matters and is decision 20's: **the manifest's own integrity first**, so
    /// a rotted manifest is reported as a rotted manifest rather than as damaged
    /// photographs.
    pub fn read(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path)?;
        Self::parse(&text)
    }

    /// The same, from text already in hand.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        // Read the version before anything else, so a manifest from the future is
        // reported as such rather than failing somewhere deep in a field that moved.
        #[derive(Deserialize)]
        struct Probe {
            schema: u32,
        }

        let probe: Probe = serde_json::from_str(text)?;
        if probe.schema > CURRENT_SCHEMA {
            return Err(ManifestError::SchemaTooNew {
                found: probe.schema,
                understood: CURRENT_SCHEMA,
            });
        }

        let manifest: Manifest = serde_json::from_str(text)?;

        if checksum_of(&manifest.body).map_err(|_| ManifestError::ChecksumMismatch)?
            != manifest.checksum
        {
            return Err(ManifestError::ChecksumMismatch);
        }

        Ok(manifest)
    }

    /// Write atomically — temp then rename, so an interrupted run cannot leave a
    /// half-written manifest where a whole one used to be.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).context("serializing the manifest")?;
        crate::winio::write_through(path, text.as_bytes())
    }
}

/// Where a date folder's manifest lives.
///
/// Dotted and prefixed so it cannot collide with anything the camera or Lightroom puts
/// there, and so it sorts out of the way of the photographs.
pub fn path_in(folder: &Path) -> PathBuf {
    folder.join(".photoday-manifest.json")
}

/// Add this run's files to the manifest in `folder`, creating it if there is none.
///
/// **Merging rather than replacing is the whole point** (decision 12): several offloads
/// a day is the normal rhythm, so an evening run must not erase what the lunchtime run
/// recorded. Entries are keyed by name — a file ingested twice updates in place rather
/// than appearing twice — and the `runs` array grows, which is what makes several
/// offloads legible after the fact.
///
/// **An unreadable existing manifest is a hard error, deliberately.** The alternative is
/// to overwrite it, which would silently discard the only record of everything that
/// landed before tonight. A damaged manifest is a thing to be told about while the
/// source card is still in your hand.
pub fn update(
    folder: &Path,
    date_utc: &str,
    destination: &str,
    run: Run,
    entries: Vec<Entry>,
) -> Result<()> {
    let path = path_in(folder);

    let mut body = match Manifest::read(&path) {
        Ok(existing) => existing.body,
        Err(ManifestError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Body {
            date_utc: date_utc.to_owned(),
            destination: destination.to_owned(),
            runs: Vec::new(),
            files: Vec::new(),
        },
        Err(error) => {
            return Err(anyhow::Error::new(error).context(format!(
                "refusing to overwrite the existing manifest at {} — it is the only \
                 record of what landed here before tonight",
                path.display()
            )));
        }
    };

    for entry in entries {
        match body.files.iter_mut().find(|held| held.name == entry.name) {
            Some(held) => *held = entry,
            None => body.files.push(entry),
        }
    }

    // Sorted so two destinations holding the same day produce byte-identical manifests
    // apart from the `destination` field, which is what makes the cross-check of
    // decision 12 a simple comparison.
    body.files.sort_by(|a, b| a.name.cmp(&b.name));
    body.runs.push(run);

    Manifest::seal(body)?.write(&path)
}

/// What phase 4 concluded about one file, ready to be written into a manifest.
///
/// A struct rather than a bare `(String, Corroborated)` because a mismatch carries the two
/// competing hashes with it, and a tombstone without them records that something was
/// deleted while losing the only evidence of why.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The file name as the manifest holds it — a bare `1402Z_0001.CR3`, not a path.
    pub name: String,
    pub corroborated: Corroborated,
    /// Present only for a confirmed mismatch, which is what turns the entry into a
    /// tombstone.
    pub deletion: Option<Deletion>,
}

/// Record phase 4's verdicts against the manifest already in `folder`.
///
/// **Phase 3 writes every entry with `corroborated: None`, meaning genuinely *pending***
/// (decision 12), and this is what resolves it. Kept separate from [`update`] because the
/// two do different jobs: `update` merges a run's new files, while this revisits files
/// already recorded and answers a question that was left open. Folding them together would
/// mean phase 4 had to reconstruct entries it has no business knowing about — bytes,
/// capture time, verified timestamp — purely to avoid a second write.
///
/// A named file that is not in the manifest is skipped rather than treated as an error: the
/// manifest holds only what *verified*, so a frame that failed read-back is legitimately
/// absent while still having been ingested.
///
/// Returns how many entries were changed, which the caller compares against what it asked
/// for — a silent shortfall would mean the manifest and the run disagree about what landed.
pub fn corroborate(folder: &Path, outcomes: &[Outcome]) -> Result<usize> {
    let path = path_in(folder);
    let mut body = Manifest::read(&path)
        .with_context(|| {
            format!(
                "reading {} to record corroboration — phase 3 wrote it minutes ago, so a \
                 manifest that cannot be read now means something changed underneath the run",
                path.display()
            )
        })?
        .body;

    let mut changed = 0;
    for outcome in outcomes {
        let Some(entry) = body.files.iter_mut().find(|held| held.name == outcome.name) else {
            continue;
        };

        entry.corroborated = Some(outcome.corroborated);
        if let Some(deletion) = &outcome.deletion {
            // The tombstone of decision 12: the entry stays in the list so a `verify`
            // years from now reports *clean* rather than flagging a missing file nobody
            // remembers deleting.
            entry.status = Status::Deleted;
            entry.deletion = Some(deletion.clone());
        }
        changed += 1;
    }

    // Re-sealed rather than edited in place: the checksum covers the body, so any change
    // to a verdict has to re-derive it or the manifest fails its own integrity check.
    Manifest::seal(body)?.write(&path)?;
    Ok(changed)
}

/// SHA-256 of the body, routed through `Value` so writer and reader hash the same bytes.
fn checksum_of(body: &Body) -> Result<String> {
    let value = serde_json::to_value(body).context("canonicalizing the manifest body")?;
    let canonical = serde_json::to_string(&value).context("serializing the manifest body")?;
    Ok(hex(&sha256(canonical.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Body {
        Body {
            date_utc: "2026-08-03".into(),
            destination: "OWC".into(),
            runs: vec![Run {
                run_id: "2026-08-03T18-22-04Z".into(),
                files_added: 1_247,
                bytes_added: 60_236_492_800,
            }],
            files: vec![Entry {
                name: "1422Z_0001.CR3".into(),
                status: Status::Present,
                sha256: "9f2b".into(),
                bytes: 47_185_920,
                captured_utc: Some("2026-08-03T14:22:37Z".into()),
                source_card: "primary".into(),
                source_volume_serial: "A4E2-91CC".into(),
                run_id: "2026-08-03T18-22-04Z".into(),
                verified_utc: "2026-08-03T18:23:31Z".into(),
                corroborated: Some(Corroborated::Matched),
                deletion: None,
            }],
        }
    }

    #[test]
    fn a_sealed_manifest_reads_back() {
        let sealed = Manifest::seal(body()).expect("sealing");
        let text = serde_json::to_string_pretty(&sealed).unwrap();

        let back = Manifest::parse(&text).expect("a sealed manifest must read back");
        assert_eq!(back.schema, 1);
        assert_eq!(back.body.files[0].name, "1422Z_0001.CR3");
    }

    /// The self-checksum earning its keep: the manifest rotted, and `verify` must say so
    /// rather than reporting the photographs as damaged.
    #[test]
    fn a_tampered_body_is_caught_as_a_damaged_manifest() {
        let sealed = Manifest::seal(body()).expect("sealing");
        let text = serde_json::to_string_pretty(&sealed)
            .unwrap()
            .replace("47185920", "47185921");

        match Manifest::parse(&text) {
            Err(ManifestError::ChecksumMismatch) => {}
            other => panic!("expected a checksum mismatch, got {other:?}"),
        }
    }

    /// Decision 28's other direction: an older binary meeting a newer manifest says
    /// exactly that, and never reports the photographs as damaged.
    #[test]
    fn a_newer_schema_is_reported_as_such_and_not_as_damage() {
        let sealed = Manifest::seal(body()).expect("sealing");
        let text = serde_json::to_string_pretty(&sealed)
            .unwrap()
            .replace("\"schema\": 1", "\"schema\": 99");

        match Manifest::parse(&text) {
            Err(ManifestError::SchemaTooNew {
                found: 99,
                understood: 1,
            }) => {}
            other => panic!("expected a schema-too-new error, got {other:?}"),
        }

        // And the message must send the reader to a newer binary rather than to a
        // recovery procedure.
        let rendered = ManifestError::SchemaTooNew {
            found: 99,
            understood: 1,
        }
        .to_string();
        assert!(rendered.contains("your photographs are fine"), "{rendered}");
    }

    /// The checksum must not depend on how the writer happened to order or space its
    /// JSON, or a manifest would fail to verify on a build with a reordered struct.
    #[test]
    fn the_checksum_survives_reformatting_and_key_reordering() {
        let sealed = Manifest::seal(body()).expect("sealing");

        let compact = serde_json::to_string(&sealed).unwrap();
        let pretty = serde_json::to_string_pretty(&sealed).unwrap();
        assert_ne!(compact, pretty, "the two spellings must actually differ");

        assert!(Manifest::parse(&compact).is_ok());
        assert!(Manifest::parse(&pretty).is_ok());

        // Round-tripping through `Value` reorders keys; the checksum must not notice.
        let reordered: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert!(Manifest::parse(&serde_json::to_string(&reordered).unwrap()).is_ok());
    }

    /// A tombstone reports clean rather than missing, which is the whole reason it is
    /// kept in the list (decision 12).
    #[test]
    fn a_tombstone_round_trips_with_both_competing_hashes() {
        let mut body = body();
        body.files[0].status = Status::Deleted;
        body.files[0].deletion = Some(Deletion {
            source_sha256: "aaaa".into(),
            other_sha256: "bbbb".into(),
            reason: "the two cards disagreed".into(),
            deleted_utc: "2026-08-03T18:44:02Z".into(),
        });

        let sealed = Manifest::seal(body).expect("sealing");
        let back = Manifest::parse(&serde_json::to_string(&sealed).unwrap()).expect("reading");

        assert_eq!(back.body.files[0].status, Status::Deleted);
        let deletion = back.body.files[0].deletion.as_ref().expect("a tombstone");
        assert_eq!(deletion.source_sha256, "aaaa");
        assert_eq!(deletion.other_sha256, "bbbb");
    }

    /// The verdicts stay distinct on disk, because conflating them is how a record stops
    /// meaning anything years later.
    #[test]
    fn the_corroboration_verdicts_are_distinct_strings() {
        for (verdict, spelled) in [
            (Corroborated::Matched, "\"matched\""),
            (Corroborated::Mismatched, "\"mismatched\""),
            (Corroborated::Waived, "\"waived\""),
            (Corroborated::Forfeited, "\"forfeited\""),
        ] {
            assert_eq!(serde_json::to_string(&verdict).unwrap(), spelled);
        }
    }

    /// Phase 3 leaves corroboration genuinely pending; phase 4 is what resolves it.
    #[test]
    fn corroborate_resolves_a_pending_entry_and_reseals() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");

        // As phase 3 writes it: corroboration genuinely pending, not merely absent.
        let mut files = body().files;
        files[0].corroborated = None;
        let name = files[0].name.clone();

        update(
            scratch.path(),
            "2026-08-03",
            "OWC",
            body().runs[0].clone(),
            files,
        )
        .expect("writing the manifest");

        assert!(
            Manifest::read(&path_in(scratch.path())).unwrap().body.files[0]
                .corroborated
                .is_none(),
            "the fixture must start pending or this proves nothing"
        );

        let changed = corroborate(
            scratch.path(),
            &[Outcome {
                name: name.clone(),
                corroborated: Corroborated::Matched,
                deletion: None,
            }],
        )
        .expect("recording corroboration");

        assert_eq!(changed, 1);
        // Re-read rather than inspected in memory: the seal has to survive the rewrite, so
        // this also proves the checksum was re-derived rather than left stale.
        let back = Manifest::read(&path_in(scratch.path())).expect("the manifest still verifies");
        assert_eq!(back.body.files[0].corroborated, Some(Corroborated::Matched));
        assert_eq!(back.body.files[0].status, Status::Present);
    }

    /// A mismatch must leave a tombstone that still carries *why*, and must not be
    /// recordable as `Matched` or left looking pending.
    #[test]
    fn a_mismatch_becomes_a_tombstone_carrying_both_hashes() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        update(
            scratch.path(),
            "2026-08-03",
            "OWC",
            body().runs[0].clone(),
            body().files,
        )
        .expect("writing the manifest");

        corroborate(
            scratch.path(),
            &[Outcome {
                name: body().files[0].name.clone(),
                corroborated: Corroborated::Mismatched,
                deletion: Some(Deletion {
                    source_sha256: "aaaa".into(),
                    other_sha256: "bbbb".into(),
                    reason: "the two cards disagreed".into(),
                    deleted_utc: "2026-08-04T18:44:02Z".into(),
                }),
            }],
        )
        .expect("recording the mismatch");

        let back = Manifest::read(&path_in(scratch.path())).expect("the manifest still verifies");
        assert_eq!(back.body.files[0].status, Status::Deleted);
        assert_eq!(
            back.body.files[0].corroborated,
            Some(Corroborated::Mismatched)
        );
        let deletion = back.body.files[0].deletion.as_ref().expect("a tombstone");
        assert_eq!(deletion.source_sha256, "aaaa");
        assert_eq!(deletion.other_sha256, "bbbb");
    }

    /// A frame that failed read-back never reached the manifest, so phase 4 naming it is
    /// not an error — but the count must reveal the shortfall rather than hide it.
    #[test]
    fn a_name_the_manifest_does_not_hold_is_skipped_and_counted() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        update(
            scratch.path(),
            "2026-08-03",
            "OWC",
            body().runs[0].clone(),
            body().files,
        )
        .expect("writing the manifest");

        let changed = corroborate(
            scratch.path(),
            &[
                Outcome {
                    name: body().files[0].name.clone(),
                    corroborated: Corroborated::Matched,
                    deletion: None,
                },
                Outcome {
                    name: "never_verified.CR3".into(),
                    corroborated: Corroborated::Matched,
                    deletion: None,
                },
            ],
        )
        .expect("recording corroboration");

        assert_eq!(changed, 1, "the absent file must not be counted as changed");
    }
}
