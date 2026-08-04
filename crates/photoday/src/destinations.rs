//! Turning config entries into places on real hardware.
//!
//! Decision 6: **a drive letter is not an identity.** A destination is found by its disk
//! serial, with the volume GUID as a fast local index, and the two disagreeing is
//! information rather than an error — a disk whose serial matches at a new GUID has been
//! reformatted, and that is something to hear about loudly rather than to silently
//! accept.
//!
//! Nothing here refuses a run. A destination that cannot be found is *reported* as
//! missing, and whether that is fatal is decision 25's question: the default is refusal,
//! `--without <LABEL>` is the declared exception. Keeping that decision out of this
//! module is what lets pre-flight state the whole situation at once rather than dying on
//! the first absence.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::{Config, Destination, Located};
use crate::storage::{self, Device, Volume};

/// How a destination's stored identity lined up with what is plugged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match {
    /// Both the serial and the volume GUID matched. The ordinary case.
    Exact,

    /// The serial matched at a volume GUID the config does not have.
    ///
    /// **The disk was reformatted, or the config's GUID was always stale.** The serial is
    /// the authority (decision 6), so the run proceeds — but the config wants updating
    /// and the operator wants telling, because a reformat of an archive disk is either
    /// something they just did or something they very much did not.
    SerialAtNewVolume {
        stored: Option<String>,
        found: String,
    },

    /// Only the GUID matched, because the device reports no serial at all.
    ///
    /// Weaker than the design would like and not an error: some USB bridges decline to
    /// report one. The GUID does not survive a reformat, so this destination silently
    /// loses its identity the day it is reformatted — which is exactly why the serial is
    /// preferred wherever it exists.
    VolumeOnly,

    /// Found by path on this machine's own disk, where hardware identity does not arise.
    ThisMachine,
}

/// A destination, located.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub label: String,
    /// Where this copy's `YYYY\` tree lives.
    pub root: PathBuf,
    pub volume: Volume,
    /// Absent when the volume's device declined to identify itself.
    pub device: Option<Device>,
    pub matched: Match,
}

impl Resolved {
    /// Whether this destination is a removable device, and therefore whether it is
    /// ejected when the run completes (decision 22).
    pub fn ejectable(&self) -> bool {
        self.matched != Match::ThisMachine
    }
}

/// A destination the config names and the hardware does not have.
#[derive(Debug, Clone)]
pub struct Missing {
    pub label: String,
    /// Phrased for decision 25's refusal, which prints it and names `--without`.
    pub reason: String,
}

/// What pre-flight found when it looked for the rig.
#[derive(Debug, Clone, Default)]
pub struct Survey {
    pub found: Vec<Resolved>,
    pub missing: Vec<Missing>,
}

impl Survey {
    /// Volume GUID paths of everything found, which is what excludes a destination from
    /// being mistaken for a camera card (decision 7's correction).
    pub fn volume_guids(&self) -> Vec<String> {
        self.found
            .iter()
            .map(|resolved| resolved.volume.guid_path.clone())
            .collect()
    }

    /// Distinct physical disks among the destinations found.
    ///
    /// **This is decision 6's exactness, and it is real work rather than ceremony**: four
    /// volume GUIDs can be four partitions of one disk, and the laptop alone presents
    /// exactly that shape. A device that reports no disk number cannot be counted, so it
    /// is reported separately rather than assumed distinct.
    pub fn distinct_disks(&self) -> (usize, usize) {
        let mut disks: Vec<u32> = self
            .found
            .iter()
            .filter_map(|resolved| resolved.device.as_ref())
            .map(|device| device.disk_number)
            .collect();

        let unidentified = self.found.len() - disks.len();

        disks.sort_unstable();
        disks.dedup();

        (disks.len(), unidentified)
    }
}

/// Locate every configured destination against what is currently plugged in.
pub fn survey(config: &Config) -> Result<Survey> {
    let volumes = storage::volumes().context("enumerating volumes to find the destinations")?;
    Ok(survey_against(config, &volumes))
}

/// The same, against a volume list the caller already has.
pub fn survey_against(config: &Config, volumes: &[Volume]) -> Survey {
    let mut survey = Survey::default();

    for destination in &config.destinations {
        match locate(destination, volumes) {
            Ok(resolved) => survey.found.push(resolved),
            Err(reason) => survey.missing.push(Missing {
                label: destination.label.clone(),
                reason,
            }),
        }
    }

    survey
}

/// One destination, or why it could not be found.
///
/// The error is a plain `String` rather than an `anyhow::Error` because it is not a
/// failure — it is one line of a report about the rig, and every caller wants to keep
/// going and collect the rest.
fn locate(destination: &Destination, volumes: &[Volume]) -> Result<Resolved, String> {
    let located = destination
        .located()
        .map_err(|error| format!("{error:#}"))?;

    match located {
        Located::ThisMachine(path) => {
            let volume = storage::volume_containing(path)
                .map_err(|_| format!("no mounted volume holds {}", path.display()))?;

            Ok(Resolved {
                label: destination.label.clone(),
                root: path.to_path_buf(),
                device: storage::device_of(&volume).ok(),
                volume,
                matched: Match::ThisMachine,
            })
        }

        Located::Device {
            serial,
            volume_guid,
            subpath,
        } => {
            // The serial is the authority, so it is tried first and a hit at an
            // unexpected GUID is a *result* rather than a miss.
            let by_serial = volumes.iter().find_map(|volume| {
                let device = storage::device_of(volume).ok()?;
                (device.serial.as_deref() == Some(serial)).then_some((volume, device))
            });

            let (volume, device, matched) = match by_serial {
                Some((volume, device)) => {
                    let matched = if volume_guid == Some(volume.guid_path.as_str()) {
                        Match::Exact
                    } else {
                        Match::SerialAtNewVolume {
                            stored: volume_guid.map(str::to_owned),
                            found: volume.guid_path.clone(),
                        }
                    };
                    (volume, Some(device), matched)
                }

                // No serial matched. A device that reports none can still be found by
                // its GUID, which is weaker and is labelled as such.
                None => {
                    let guid = volume_guid
                        .ok_or_else(|| format!("no connected device reports serial {serial}"))?;

                    let volume = volumes
                        .iter()
                        .find(|volume| volume.guid_path == guid)
                        .ok_or_else(|| {
                            format!(
                                "no connected device reports serial {serial}, and no volume \
                                 matches {guid} either"
                            )
                        })?;

                    (volume, storage::device_of(volume).ok(), Match::VolumeOnly)
                }
            };

            let mount = volume
                .mount_points
                .first()
                .ok_or_else(|| format!("the volume for serial {serial} is not mounted"))?;

            Ok(Resolved {
                label: destination.label.clone(),
                root: mount.join(subpath),
                volume: volume.clone(),
                device,
                matched,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn config(json: &str) -> Config {
        serde_json::from_str(json).expect("valid test config")
    }

    /// A device the config names and the hardware does not have is *reported*, not
    /// fatal — decision 25 owns whether the run continues, and it needs the whole list
    /// to say `--without` about.
    #[test]
    fn a_missing_device_is_collected_rather_than_raised() {
        let config = config(
            r#"{
              "destinations": [
                { "label": "gone", "disk_serial": "NOT-PLUGGED-IN",
                  "volume_guid": "{nope}", "subpath": "Travel\\Images" }
              ],
              "gpx_dir": "C:\\Travel\\GPX"
            }"#,
        );

        let survey = survey_against(&config, &[volume(r"\\?\Volume{other}\", r"Z:\")]);

        assert!(survey.found.is_empty());
        assert_eq!(survey.missing.len(), 1);
        assert_eq!(survey.missing[0].label, "gone");
        assert!(
            survey.missing[0].reason.contains("NOT-PLUGGED-IN"),
            "the reason must name what was looked for: {}",
            survey.missing[0].reason
        );
    }

    /// A config entry that is malformed rather than absent lands in the same list, so
    /// pre-flight reports one situation instead of failing two different ways.
    #[test]
    fn a_malformed_entry_is_reported_by_label_too() {
        let config = config(
            r#"{
              "destinations": [{ "label": "broken" }],
              "gpx_dir": "C:\\Travel\\GPX"
            }"#,
        );

        let survey = survey_against(&config, &[]);
        assert_eq!(survey.missing[0].label, "broken");
    }

    /// The count that decision 6 exists for. Four destinations on two disks is two
    /// distinct disks, and the assertion has to see that rather than counting entries.
    #[test]
    fn distinct_disks_counts_devices_and_not_destinations() {
        let survey = Survey {
            found: vec![
                resolved("a", 0),
                resolved("b", 0),
                resolved("c", 1),
                resolved("d", 2),
            ],
            missing: Vec::new(),
        };

        assert_eq!(
            survey.distinct_disks(),
            (3, 0),
            "four copies on three disks"
        );
    }

    /// A device with no disk number cannot be counted as distinct from anything, so it
    /// is reported separately rather than quietly assumed to be its own disk.
    #[test]
    fn a_destination_with_no_device_is_counted_as_unidentified() {
        let mut unknown = resolved("x", 0);
        unknown.device = None;

        let survey = Survey {
            found: vec![resolved("a", 0), unknown],
            missing: Vec::new(),
        };

        assert_eq!(survey.distinct_disks(), (1, 1));
    }

    fn resolved(label: &str, disk_number: u32) -> Resolved {
        Resolved {
            label: label.to_string(),
            root: PathBuf::from(r"Z:\Travel\Images"),
            volume: volume(&format!(r"\\?\Volume{{{label}}}\"), r"Z:\"),
            device: Some(Device {
                disk_number,
                serial: Some(format!("SERIAL-{disk_number}")),
            }),
            matched: Match::Exact,
        }
    }
}
