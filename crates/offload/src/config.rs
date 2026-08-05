//! The rig, described once so the nightly command can be bare.
//!
//! Decision 8: "one intuitive CLI command" and "six paths that shuffle between sessions"
//! are in tension, and typing destination paths at 11pm after a day of shooting is how a
//! destination ends up pointed at the wrong disk. So the rig lives here and `offload`
//! takes almost no arguments.
//!
//! **Read from `%APPDATA%\offload\config.json`** and never generated: a tool that
//! invented a config would be inventing a rig. A missing one is a pre-flight fatal that
//! names the path it looked in.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

/// The whole rig.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub destinations: Vec<Destination>,
    /// Where the GPS logger's tracks are copied to. Pre-flight refuses an empty
    /// directory unless `--no-gpx` declares the night genuinely trackless (decision 26).
    pub gpx_dir: PathBuf,
}

/// One of the four copies.
///
/// **There is no `role` field** (decision 11): all four copies are backups and none is a
/// working copy. What distinguishes them is how they are *found*, which is the shape of
/// this struct rather than a value in it — see [`Destination::located`].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Destination {
    /// Short name, used in the report and by `--without`.
    pub label: String,

    /// A location on this machine's own disk. Mutually exclusive with `disk_serial`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,

    /// The physical device's serial — the true identity, surviving a reformat, a drive
    /// letter change and a move to another machine (decision 6). Stored **verbatim**:
    /// real serials carry underscores and trailing punctuation, and normalizing them
    /// could map two devices onto one string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_serial: Option<String>,

    /// The volume GUID, a fast local index that the serial is the authority over. It
    /// changes when the disk is reformatted, which pre-flight reports loudly rather than
    /// silently accepting (decision 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_guid: Option<String>,

    /// Where under the device's root the archive lives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<PathBuf>,
}

/// How a destination is found, once the config has been checked.
///
/// The two ways are genuinely different — one is a path that either exists or does not,
/// the other is a hardware search that can succeed at a different drive letter than last
/// time — so they are separate variants rather than a struct with optional everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located<'a> {
    /// A path on the disk this program is running from. Cannot be unplugged, so
    /// `--without` may not name it (decision 25), and there is nothing to eject.
    ThisMachine(&'a Path),
    /// A removable device on the hub, found by serial and ejected when the run
    /// completes (decision 22).
    Device {
        serial: &'a str,
        volume_guid: Option<&'a str>,
        subpath: &'a Path,
    },
}

impl Destination {
    /// Which of the two ways this destination is found, or why the entry is unusable.
    ///
    /// The errors name the label, because the operator reading them is looking at a file
    /// with four entries in it and needs to know which one to fix.
    pub fn located(&self) -> Result<Located<'_>> {
        match (&self.path, &self.disk_serial) {
            (Some(path), None) => Ok(Located::ThisMachine(path)),

            (None, Some(serial)) => Ok(Located::Device {
                serial,
                volume_guid: self.volume_guid.as_deref(),
                // A device without a subpath means the archive is the volume root, which
                // is legal and is what an empty string would have meant anyway.
                subpath: self.subpath.as_deref().unwrap_or(Path::new("")),
            }),

            (Some(_), Some(_)) => bail!(
                "destination {:?} has both `path` and `disk_serial`; a copy is found one \
                 way or the other, and which one it is decides whether it gets ejected",
                self.label
            ),

            (None, None) => bail!(
                "destination {:?} has neither `path` nor `disk_serial`, so nothing can \
                 find it",
                self.label
            ),
        }
    }
}

impl Config {
    /// Check what can be checked without touching the hardware.
    ///
    /// Deliberately *not* the whole of pre-flight — that resolves every entry against
    /// what is actually plugged in and is decision 9's job. This catches the config
    /// being wrong on its own terms, which is worth separating because the fix is
    /// different: an editor rather than a cable.
    pub fn validate(&self) -> Result<()> {
        if self.destinations.is_empty() {
            bail!("the config lists no destinations");
        }

        for destination in &self.destinations {
            destination.located()?;
        }

        // Duplicate labels would make `--without` ambiguous and the report unreadable.
        let mut labels: Vec<&str> = self.destinations.iter().map(|d| d.label.as_str()).collect();
        labels.sort_unstable();
        let before = labels.len();
        labels.dedup();
        if labels.len() != before {
            bail!("two destinations share a label; every label must be unique");
        }

        Ok(())
    }
}

/// `%APPDATA%\offload\config.json` (decision 8).
pub fn default_path() -> Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| anyhow!("APPDATA is not set, so the config location is unknown"))?;

    Ok(PathBuf::from(appdata).join("offload").join("config.json"))
}

/// Load and validate the config from its standard location.
pub fn load() -> Result<Config> {
    load_from(&default_path()?)
}

/// Load and validate a config from an explicit path.
pub fn load_from(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path).with_context(|| {
        format!(
            "reading the config at {}. It describes the rig and is never generated — \
             create it by hand",
            path.display()
        )
    })?;

    let config: Config = serde_json::from_str(&text)
        .with_context(|| format!("parsing the config at {}", path.display()))?;

    config
        .validate()
        .with_context(|| format!("in the config at {}", path.display()))?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<Config> {
        let config: Config = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    const REAL: &str = r#"{
      "destinations": [
        { "label": "laptop", "path": "C:\\Travel\\Images" },
        { "label": "OWC", "disk_serial": "6479_A751_AF00_3CFF.",
          "volume_guid": "{d10e8b02-e4f2-476a-af89-78eacb8a7f38}",
          "subpath": "Travel\\Images" }
      ],
      "gpx_dir": "C:\\Travel\\GPX"
    }"#;

    /// The two shapes, and that each resolves to the variant that decides whether it
    /// gets ejected.
    #[test]
    fn a_path_destination_and_a_device_destination_both_resolve() {
        let config = parse(REAL).expect("the real config shape must parse");

        assert_eq!(
            config.destinations[0].located().unwrap(),
            Located::ThisMachine(Path::new(r"C:\Travel\Images"))
        );
        assert_eq!(
            config.destinations[1].located().unwrap(),
            Located::Device {
                serial: "6479_A751_AF00_3CFF.",
                volume_guid: Some("{d10e8b02-e4f2-476a-af89-78eacb8a7f38}"),
                subpath: Path::new(r"Travel\Images"),
            }
        );
    }

    /// The serial is kept exactly as the device reported it — underscores, trailing
    /// period and all. Normalizing it is how two different devices become one string.
    #[test]
    fn a_serial_with_punctuation_survives_a_round_trip() {
        let config = parse(REAL).unwrap();
        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains(r"6479_A751_AF00_3CFF."), "{json}");
        assert_eq!(
            parse(&json).unwrap().destinations[1].disk_serial.as_deref(),
            Some("6479_A751_AF00_3CFF.")
        );
    }

    /// A `role` field is not merely unused — it must not silently reappear carrying a
    /// distinction decision 11 removed. `serde` ignores unknown fields by default, so
    /// this pins that the *resolution* ignores it rather than that the parse rejects it.
    #[test]
    fn a_stale_role_field_changes_nothing() {
        let with_role = r#"{
          "destinations": [
            { "label": "laptop", "role": "working", "path": "C:\\Travel\\Images" }
          ],
          "gpx_dir": "C:\\Travel\\GPX"
        }"#;

        let config = parse(with_role).expect("an old config still loads");
        assert_eq!(
            config.destinations[0].located().unwrap(),
            Located::ThisMachine(Path::new(r"C:\Travel\Images"))
        );
    }

    #[test]
    fn a_destination_found_two_ways_at_once_is_refused_by_name() {
        let both = r#"{
          "destinations": [
            { "label": "confused", "path": "C:\\x", "disk_serial": "ABC" }
          ],
          "gpx_dir": "C:\\Travel\\GPX"
        }"#;

        let error = format!("{:#}", parse(both).unwrap_err());
        assert!(error.contains("confused"), "{error}");
    }

    #[test]
    fn a_destination_found_no_way_at_all_is_refused_by_name() {
        let neither = r#"{
          "destinations": [{ "label": "orphan" }],
          "gpx_dir": "C:\\Travel\\GPX"
        }"#;

        let error = format!("{:#}", parse(neither).unwrap_err());
        assert!(error.contains("orphan"), "{error}");
    }

    #[test]
    fn duplicate_labels_are_refused() {
        let duplicate = r#"{
          "destinations": [
            { "label": "SSD", "path": "C:\\a" },
            { "label": "SSD", "path": "C:\\b" }
          ],
          "gpx_dir": "C:\\Travel\\GPX"
        }"#;

        assert!(parse(duplicate).is_err());
    }

    /// A device with no `subpath` archives at the volume root, which is legal.
    #[test]
    fn a_device_without_a_subpath_archives_at_the_volume_root() {
        let bare = r#"{
          "destinations": [{ "label": "SSD", "disk_serial": "ABC" }],
          "gpx_dir": "C:\\Travel\\GPX"
        }"#;

        assert_eq!(
            parse(bare).unwrap().destinations[0].located().unwrap(),
            Located::Device {
                serial: "ABC",
                volume_guid: None,
                subpath: Path::new(""),
            }
        );
    }
}
