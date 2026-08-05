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

impl Report {
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

    /// Clean means every recorded file was found and matched, and every manifest was
    /// readable. An unrecorded file is *not* damage and does not fail the disk — it is
    /// reported so it can be explained.
    pub fn clean(&self) -> bool {
        self.damaged() == 0 && self.missing() == 0 && self.unreadable_manifests.is_empty()
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
