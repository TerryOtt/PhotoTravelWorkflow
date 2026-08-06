//! `offload verify <DEST>` — prove a disk, years later, on any machine.
//!
//! Decision 20: **this reads nothing but the destination itself** — its marker and its
//! manifests. No config, no run log, no network, no memory of the rig that wrote it.
//! That is the promise the whole archive format exists to keep, and it is why the
//! manifest carries its own checksum and why every schema this tool has ever written
//! stays readable (decisions 12, 28).
//!
//! # The order matters
//!
//! **The manifest's own integrity is checked before any photograph is read.** A rotted
//! manifest holds every hash in the archive, so verifying photographs against it would
//! report hundreds of perfectly intact files as damaged. *I cannot read this* must never
//! wear the costume of *your archive is rotting* — so the two are separate outcomes and
//! the manifest is settled first.
//!
//! # Every bit, every time
//!
//! Decision 19: no sampling, ever. These are the most emotionally valuable files the
//! archive holds and the check is performed rarely and trusted completely. Reads are
//! unbuffered, so what is hashed is what is on the media rather than what is in RAM.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::hash::hex;
use crate::manifest::{self, Manifest, ManifestError, Status};
use crate::marker;
use crate::winio::unbuffered_sha256;

/// What one date folder turned out to be.
#[derive(Debug, Default)]
pub struct FolderReport {
    pub folder: PathBuf,
    pub checked: usize,
    /// Deliberately deleted in phase 4 and correctly absent. Reported as *clean*, not as
    /// missing — the whole reason tombstones are kept (decision 12).
    pub tombstoned: usize,
    /// Hash did not match. These are the files this command exists to find.
    pub damaged: Vec<String>,
    /// The manifest lists it and the disk does not have it.
    pub missing: Vec<String>,
    /// On the disk, absent from the manifest. Not damage, but not explained either.
    pub unrecorded: Vec<String>,
}

/// What the whole disk turned out to be.
#[derive(Debug, Default)]
pub struct Report {
    pub label: Option<String>,
    pub created_utc: Option<String>,
    pub last_run_utc: Option<String>,
    /// Folders whose manifest could not be trusted, with why. **Kept apart from
    /// `damaged`** so a rotted manifest can never be reported as rotted photographs.
    pub unreadable_manifests: Vec<(PathBuf, String)>,
    pub folders: Vec<FolderReport>,
}

/// What a verification actually established.
///
/// **An enum rather than a `bool`, because the boolean could not say "I could not check".**
/// `clean()` used to be three `== 0` tests — no damage, nothing missing, no unreadable
/// manifest — and **every one of them is vacuously true on a disk holding no manifests at
/// all.** So an archive that had been wiped reported `CLEAN — every recorded file is present
/// and matches`, which is true in the same way that every unicorn in this room is purple.
///
/// Found 2026-08-06 on a real drive, and it is the shape [`REVIEWING.md`] collects: a check
/// that answers the reassuring thing when it cannot answer at all. The failure has no
/// backstop anywhere — decision 20's whole promise is a disk proving itself years later on a
/// machine that has never seen this tool, and nobody re-checks a `CLEAN`.
///
/// **The type is the fix, not a fourth `if`.** A `bool` lets a caller keep asking the old
/// question; a non-exhaustive `match` is a compile error, which is this project's stated
/// preference for making a mistake impossible rather than merely unlikely.
///
/// [`REVIEWING.md`]: ../../../docs/REVIEWING.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every recorded file was found and matched, and every manifest was readable.
    Clean,
    /// **No manifest anywhere on the disk**, so nothing here claims to be an archive. Not a
    /// pass and not a failure — the check had nothing to examine, and that is its own answer.
    NothingToVerify,
    /// At least one manifest could not be read, so what it covers was not checked. Says
    /// nothing whatever about the photographs beside it (decisions 12, 28).
    Incomplete,
    /// Files are damaged or missing. The thing this command exists to find.
    Damaged,
}

impl Report {
    /// What this verification established — see [`Verdict`].
    pub fn verdict(&self) -> Verdict {
        // **Checked first, because every test below is a statement *about* manifests and all
        // of them are vacuously true when there are none.** `manifest_folders` only yields
        // directories that actually carry a `.photoday-manifest.json`, so an empty list means
        // the walk found no such file anywhere under the root.
        if self.folders.is_empty() {
            return Verdict::NothingToVerify;
        }

        // Damage outranks an unreadable manifest: a disk with both has a definite problem and
        // a partial view, and the definite one is what the operator must act on.
        if self.damaged() > 0 || self.missing() > 0 {
            return Verdict::Damaged;
        }

        // Note for whoever touches this next: a missing or unreadable *destination marker*
        // also lands in `unreadable_manifests`, so it reaches this branch too. The module doc
        // says a marker is "information, not a gate" while this makes it one — a real
        // disagreement, older than this function, deliberately left alone rather than widened
        // into an unrequested behavior change.
        if !self.unreadable_manifests.is_empty() {
            return Verdict::Incomplete;
        }

        Verdict::Clean
    }

    pub fn checked(&self) -> usize {
        self.folders.iter().map(|f| f.checked).sum()
    }

    pub fn damaged(&self) -> usize {
        self.folders.iter().map(|f| f.damaged.len()).sum()
    }

    pub fn missing(&self) -> usize {
        self.folders.iter().map(|f| f.missing.len()).sum()
    }

    pub fn unrecorded(&self) -> usize {
        self.folders.iter().map(|f| f.unrecorded.len()).sum()
    }

    pub fn tombstoned(&self) -> usize {
        self.folders.iter().map(|f| f.tombstoned).sum()
    }
}

/// Verify a destination root.
pub fn destination(root: &Path) -> Result<Report> {
    let mut report = Report::default();

    // The marker first, so the report can name what it is checking (decision 20). A disk
    // without one is still verifiable — the manifests are what carry the proof — so this
    // is information, not a gate.
    match marker::read(root) {
        Ok(marker) => {
            report.label = Some(marker.label);
            report.created_utc = Some(marker.created_utc);
            report.last_run_utc = Some(marker.last_run_utc);
        }
        Err(error) => {
            report
                .unreadable_manifests
                .push((marker::path_in(root), format!("{error}")));
        }
    }

    for folder in manifest_folders(root)? {
        report
            .folders
            .push(check_folder(&folder, &mut report.unreadable_manifests)?);
    }

    Ok(report)
}

/// Every directory under `root` that carries a manifest.
///
/// Found by walking for the manifests themselves rather than by assuming a `YYYY\` shape,
/// so `_unfiled` is included exactly like a date folder (decisions 20, 21) with no
/// special case — and so a future layout change cannot silently leave folders unchecked.
fn manifest_folders(root: &Path) -> Result<Vec<PathBuf>> {
    let mut folders: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == ".photoday-manifest.json")
        .filter_map(|entry| entry.path().parent().map(Path::to_path_buf))
        .collect();

    folders.sort();
    Ok(folders)
}

fn check_folder(folder: &Path, unreadable: &mut Vec<(PathBuf, String)>) -> Result<FolderReport> {
    let mut report = FolderReport {
        folder: folder.to_path_buf(),
        ..Default::default()
    };

    let path = manifest::path_in(folder);
    let manifest = match Manifest::read(&path) {
        Ok(manifest) => manifest,
        Err(error) => {
            // Reported and skipped, never guessed at. Without a trustworthy manifest
            // there is nothing to verify these photographs *against*, and inventing an
            // answer is the one thing decision 12 forbids.
            unreadable.push((path, describe(&error)));
            return Ok(report);
        }
    };

    for entry in &manifest.body.files {
        let file = folder.join(&entry.name);

        if entry.status == Status::Deleted {
            // Absent is correct here; present means the tombstone is wrong about what
            // happened, which is worth knowing.
            if file.exists() {
                report
                    .unrecorded
                    .push(format!("{} (tombstoned but present)", entry.name));
            } else {
                report.tombstoned += 1;
            }
            continue;
        }

        if !file.exists() {
            report.missing.push(entry.name.clone());
            continue;
        }

        let actual =
            unbuffered_sha256(&file).with_context(|| format!("re-reading {}", file.display()))?;

        if hex(&actual) == entry.sha256 {
            report.checked += 1;
        } else {
            report.damaged.push(entry.name.clone());
        }
    }

    // Raw files on the disk that the manifest does not mention. Sidecars are excluded:
    // decision 12 does not cover them, so their presence or absence says nothing.
    let recorded: Vec<&str> = manifest
        .body
        .files
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    for entry in
        std::fs::read_dir(folder).with_context(|| format!("listing {}", folder.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();

        let is_raw = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cr3"));

        if is_raw && !recorded.contains(&name.as_str()) {
            report.unrecorded.push(name);
        }
    }

    Ok(report)
}

/// Phrase a manifest failure for someone holding a disk and worrying about their photos.
fn describe(error: &ManifestError) -> String {
    match error {
        ManifestError::SchemaTooNew { .. } | ManifestError::ChecksumMismatch => {
            format!("{error}")
        }
        other => format!("{other} — your photographs are probably fine"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{hex, sha256};
    use crate::manifest::{Entry, Run, Status};

    /// A destination root holding one date folder with one verified raw in it.
    fn archive_with_one_frame(root: &Path) {
        let folder = root.join("2026").join("2026-08-06");
        let bytes = b"a photograph, for the purposes of argument".to_vec();
        std::fs::create_dir_all(&folder).expect("a date folder");
        std::fs::write(folder.join("1402Z_0001.CR3"), &bytes).expect("a raw");

        crate::manifest::update(
            &folder,
            "2026-08-06",
            "TEST",
            Run {
                run_id: "R".into(),
                files_added: 1,
                bytes_added: bytes.len() as u64,
            },
            vec![Entry {
                name: "1402Z_0001.CR3".into(),
                status: Status::Present,
                sha256: hex(&sha256(&bytes)),
                bytes: bytes.len() as u64,
                captured_utc: None,
                source_card: "primary".into(),
                source_volume_serial: "A4E2-91CC".into(),
                run_id: "R".into(),
                verified_utc: "2026-08-06T14:02:00Z".into(),
                corroborated: None,
                deletion: None,
            }],
        )
        .expect("writing the manifest");
    }

    /// **The defect this enum exists for.** A disk with no manifests reported
    /// `CLEAN — every recorded file is present and matches`, because every test behind the
    /// old boolean was vacuously true. Found on a real drive 2026-08-06.
    ///
    /// **Mutation-checked:** restoring the old body — `damaged() == 0 && missing() == 0 &&
    /// unreadable_manifests.is_empty()` — makes this case return `Clean` and fails here,
    /// naming the empty root. Nothing else in the suite notices.
    #[test]
    fn a_disk_with_no_manifests_is_not_clean() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");

        // A marker and nothing else — exactly the state the WD was found in: it *had* been an
        // archive, so the marker reads fine and only the photographs are gone. That readable
        // marker is what kept `unreadable_manifests` empty and let the old test pass.
        crate::marker::write(scratch.path(), "WD", Some("SERIAL"), "2026-08-06T00:00:00Z")
            .expect("a destination marker");

        let report = destination(scratch.path()).expect("verification runs");

        assert_eq!(
            report.verdict(),
            Verdict::NothingToVerify,
            "an empty disk must not be spelled the same way as a verified one: {report:?}"
        );
        assert_eq!(report.checked(), 0);
    }

    /// The other half of the same assertion, and the reason the fix is not simply "return
    /// NothingToVerify when nothing was checked": a real archive still has to pass.
    #[test]
    fn a_disk_with_a_matching_manifest_is_clean() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        crate::marker::write(scratch.path(), "WD", Some("SERIAL"), "2026-08-06T00:00:00Z")
            .expect("a destination marker");
        archive_with_one_frame(scratch.path());

        let report = destination(scratch.path()).expect("verification runs");

        assert_eq!(report.verdict(), Verdict::Clean, "{report:?}");
        assert_eq!(report.checked(), 1);
    }

    /// **A folder whose files were all tombstoned is a real archive, not an empty one**, and
    /// this is the case that makes `folders.is_empty()` the right discriminator rather than
    /// `checked() == 0`. Phase 4 deleting every frame of a day leaves manifests that verify
    /// perfectly and check zero files — that disk is clean, and must not be told it holds
    /// nothing.
    #[test]
    fn a_folder_of_tombstones_is_clean_rather_than_empty() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        crate::marker::write(scratch.path(), "WD", Some("SERIAL"), "2026-08-06T00:00:00Z")
            .expect("a destination marker");
        archive_with_one_frame(scratch.path());

        let folder = scratch.path().join("2026").join("2026-08-06");
        std::fs::remove_file(folder.join("1402Z_0001.CR3")).expect("phase 4 deleted it");
        crate::manifest::corroborate(
            &folder,
            &[crate::manifest::Outcome {
                name: "1402Z_0001.CR3".into(),
                corroborated: crate::manifest::Corroborated::Mismatched,
                deletion: Some(crate::manifest::Deletion {
                    source_sha256: "aaaa".into(),
                    other_sha256: "bbbb".into(),
                    reason: "the two cards disagreed".into(),
                    deleted_utc: "2026-08-06T00:00:00Z".into(),
                }),
            }],
        )
        .expect("tombstoning");

        let report = destination(scratch.path()).expect("verification runs");

        assert_eq!(report.checked(), 0, "the fixture must check nothing");
        assert_eq!(report.tombstoned(), 1);
        assert_eq!(
            report.verdict(),
            Verdict::Clean,
            "a fully tombstoned day is a proven archive, not an absent one: {report:?}"
        );
    }

    /// Damage still outranks everything, so the new early return cannot mask a real fault.
    #[test]
    fn a_damaged_file_still_reports_damaged() {
        let scratch = tempfile::TempDir::new().expect("a scratch directory");
        crate::marker::write(scratch.path(), "WD", Some("SERIAL"), "2026-08-06T00:00:00Z")
            .expect("a destination marker");
        archive_with_one_frame(scratch.path());

        let raw = scratch
            .path()
            .join("2026")
            .join("2026-08-06")
            .join("1402Z_0001.CR3");
        std::fs::write(&raw, b"not what the manifest recorded").expect("corrupting it");

        let report = destination(scratch.path()).expect("verification runs");
        assert_eq!(report.verdict(), Verdict::Damaged, "{report:?}");
        assert_eq!(report.damaged(), 1);
    }
}
