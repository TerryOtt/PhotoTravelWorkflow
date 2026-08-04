//! The file that lets a disk say what it is.
//!
//! Decision 6: each destination carries a `.photoday-destination.json` at its root, so
//! **an archive pulled from the safe in 2031 can prove what it is on a machine that has
//! never seen this configuration.** `verify` reads this before anything else on the disk
//! (decision 20), which makes it the first thing that has to survive a decade in a safe —
//! so it is schema-versioned under the same permanent-readability rule as the manifest
//! (decision 28).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::manifest::ManifestError;

/// The newest marker schema this build writes. Same bump rule as the manifest: only when
/// an old reader would be *wrong*, never when it would merely be incomplete.
pub const CURRENT_SCHEMA: u32 = 1;

/// What a destination says about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    pub schema: u32,
    /// The config label, so a report and a refusal can name this disk the way its owner
    /// does.
    pub label: String,
    /// When this disk first became a destination. Never updated, so it records the
    /// archive's age rather than the last run.
    pub created_utc: String,
    /// The device's serial as of the most recent run, for cross-checking against a config
    /// years later. Absent when the enclosure reports none — which decision 6 has to
    /// tolerate, since some USB bridges do exactly that.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_serial: Option<String>,
    /// Updated every run, which is what makes a marker also say *when this disk was last
    /// current* — the question you actually have when you pull one from the safe.
    pub last_run_utc: String,
}

/// `.photoday-destination.json` at the destination root.
pub fn path_in(root: &Path) -> PathBuf {
    root.join(".photoday-destination.json")
}

/// Read a destination's marker.
pub fn read(root: &Path) -> Result<Marker, ManifestError> {
    let text = std::fs::read_to_string(path_in(root))?;

    #[derive(Deserialize)]
    struct Probe {
        schema: u32,
    }

    let probe: Probe = serde_json::from_str(&text)?;
    if probe.schema > CURRENT_SCHEMA {
        return Err(ManifestError::SchemaTooNew {
            found: probe.schema,
            understood: CURRENT_SCHEMA,
        });
    }

    Ok(serde_json::from_str(&text)?)
}

/// Write or refresh the marker, preserving `created_utc` from any marker already there.
pub fn write(root: &Path, label: &str, disk_serial: Option<&str>, now_utc: &str) -> Result<()> {
    let created_utc = match read(root) {
        Ok(existing) => existing.created_utc,
        // A disk with no readable marker is being adopted now. That includes one whose
        // marker is damaged: the photographs and their manifests are the archive, and a
        // corrupt marker is not worth refusing a night's backup over.
        Err(_) => now_utc.to_owned(),
    };

    let marker = Marker {
        schema: CURRENT_SCHEMA,
        label: label.to_owned(),
        created_utc,
        disk_serial: disk_serial.map(str::to_owned),
        last_run_utc: now_utc.to_owned(),
    };

    let text = serde_json::to_string_pretty(&marker).context("serializing the marker")?;
    crate::winio::write_through(&path_in(root), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_round_trips() {
        let dir = tempfile::TempDir::new().expect("a scratch directory");

        write(
            dir.path(),
            "OWC",
            Some("6479_A751_AF00_3CFF."),
            "2026-08-04T03:05:36Z",
        )
        .expect("writing");

        let back = read(dir.path()).expect("reading");
        assert_eq!(back.schema, 1);
        assert_eq!(back.label, "OWC");
        assert_eq!(back.disk_serial.as_deref(), Some("6479_A751_AF00_3CFF."));
    }

    /// `created_utc` records the archive's age, so a later run must not move it — that is
    /// the one fact in the marker a re-run could silently destroy.
    #[test]
    fn a_later_run_refreshes_last_run_but_never_created() {
        let dir = tempfile::TempDir::new().expect("a scratch directory");

        write(dir.path(), "OWC", None, "2026-01-01T00:00:00Z").expect("first run");
        write(dir.path(), "OWC", None, "2026-08-04T03:05:36Z").expect("second run");

        let back = read(dir.path()).expect("reading");
        assert_eq!(
            back.created_utc, "2026-01-01T00:00:00Z",
            "age must not move"
        );
        assert_eq!(back.last_run_utc, "2026-08-04T03:05:36Z");
    }

    /// Decision 28's rule, applied to the first thing `verify` reads: a marker from a
    /// newer build says so rather than being guessed at.
    #[test]
    fn a_newer_marker_schema_is_reported_rather_than_guessed() {
        let dir = tempfile::TempDir::new().expect("a scratch directory");
        write(dir.path(), "OWC", None, "2026-08-04T03:05:36Z").expect("writing");

        let path = path_in(dir.path());
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"schema\": 1", "\"schema\": 99");
        std::fs::write(&path, text).unwrap();

        match read(dir.path()) {
            Err(ManifestError::SchemaTooNew { found: 99, .. }) => {}
            other => panic!("expected a schema-too-new error, got {other:?}"),
        }
    }
}
