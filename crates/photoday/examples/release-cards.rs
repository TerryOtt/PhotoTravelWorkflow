//! Find out *which* call actually releases a camera card, one escalating step at a time.
//!
//! ```text
//! cargo run --release --example release-cards
//! ```
//!
//! **The experiment decision 22 needs.** `dismount_card` succeeds and releases nothing —
//! `examples/dismount-cards.rs` reproduces that in two seconds — so the open question is what
//! does, and whether the answer is the same for both cards. It should not be: an SD card
//! reports removable media, while a CFexpress behind a Thunderbolt reader enumerates as a
//! fixed NVMe disk and *is* the device rather than sitting in one.
//!
//! Three steps, escalating, checked after each:
//!
//! 1. **Lock + dismount** — today's behavior, expected to release nothing.
//! 2. **Eject the medium** — expected to work on removable media and to fail on fixed.
//! 3. **Power the device down** — the tray icon's own call, and the only remaining option
//!    for a card with no separable medium. **This reaches the reader**, which decision 22
//!    forbids in the general case; it is run here to find out empirically whether the reader
//!    comes back cleanly, which is the fact that decision cannot be made without.
//!
//! **It writes nothing to any card**, so the never-write non-goal is intact. The worst case
//! is a reader that has to be unplugged and reseated — which is the operator's own remedy
//! and was agreed before this was run.

use std::process::ExitCode;

use photoday::storage::Volume;
use photoday::{cards, config, destinations, eject, storage};

fn main() -> ExitCode {
    let Ok(config) = config::load().map_err(|e| eprintln!("{e:#}")) else {
        return ExitCode::FAILURE;
    };
    let Ok(survey) = destinations::survey(&config).map_err(|e| eprintln!("{e:#}")) else {
        return ExitCode::FAILURE;
    };
    let Ok(volumes) = storage::volumes().map_err(|e| eprintln!("{e:#}")) else {
        return ExitCode::FAILURE;
    };

    let found = cards::find(&volumes, &survey.volume_guids());
    if found.is_empty() {
        eprintln!("no camera card found");
        return ExitCode::from(2);
    }

    for card in &found {
        let label = card.label();
        let device = storage::device_of(&card.volume).ok();

        println!();
        println!("=== {label} ===");
        println!(
            "  disk {}  ·  {}",
            device
                .as_ref()
                .map_or_else(|| "?".into(), |d| d.disk_number.to_string()),
            // Removability is what predicts whether step 2 can work at all, and it is the
            // one property that differs between the two cards on this rig.
            if card.volume.removable {
                "reports removable media"
            } else {
                "reports FIXED media"
            }
        );

        // Step 1 — today's behavior.
        match eject::dismount_card(&card.volume) {
            Ok(eject::CardOutcome::Dismounted) => println!("  1. dismount      ok"),
            Ok(eject::CardOutcome::Held { reason }) => {
                println!("  1. dismount      HELD — {reason}")
            }
            Err(error) => println!("  1. dismount      ERROR — {error:#}"),
        }
        if report_released(&card.volume, "     after 1") {
            continue;
        }

        // Step 2 — the call the module doc named and did not use, addressed to the volume.
        match eject::eject_media(&card.volume) {
            Ok(()) => println!("  2. eject media   ok  (volume handle)"),
            Err(error) => println!("  2. eject media   FAILED — {error:#}"),
        }
        if report_released(&card.volume, "     after 2") {
            continue;
        }

        // Step 2b — the same request addressed to the physical drive, which is the
        // conventional target. Step 2 returning `ok` while releasing nothing is exactly the
        // shape that means the request went to the wrong object, so this separates "the
        // device ignores media eject" from "we asked the wrong thing".
        match device.as_ref() {
            Some(device) => match eject::eject_media_on_disk(device.disk_number) {
                Ok(()) => println!("  2b. eject media  ok  (physical drive handle)"),
                Err(error) => println!("  2b. eject media  FAILED — {error:#}"),
            },
            None => println!("  2b. eject media  skipped — no device behind this volume"),
        }
        if report_released(&card.volume, "     after 2b") {
            continue;
        }

        // Step 3 — reaches the reader. Only run because nothing above released it.
        match device.as_ref() {
            Some(device) => match eject::power_down_disk(device.disk_number) {
                Ok(()) => println!("  3. device eject  ok — NOTE: this powered down the reader"),
                Err(error) => println!("  3. device eject  FAILED — {error:#}"),
            },
            None => println!("  3. device eject  skipped — no device behind this volume"),
        }
        report_released(&card.volume, "     after 3");
    }

    println!();
    println!("Re-run scripts\\eject-check.ps1 for the operator's-eye view, and reseat any");
    println!("reader that step 3 powered down before the next run.");

    ExitCode::SUCCESS
}

/// Whether the volume has stopped enumerating, printed either way.
///
/// Re-enumerates rather than trusting the handle we had: the question is what Windows says
/// now. This call is itself an access, so a volume that reappears here is one that reappears
/// for the tray icon too — which is the only definition of "released" that matters.
fn report_released(volume: &Volume, tag: &str) -> bool {
    let Ok(now) = storage::volumes() else {
        println!("{tag}   could not re-enumerate");
        return false;
    };

    match now.iter().find(|v| v.guid_path == volume.guid_path) {
        Some(still) => {
            let mounts: Vec<String> = still
                .mount_points
                .iter()
                .map(|m| m.display().to_string())
                .collect();
            println!(
                "{tag}   still present, mounted at [{}]",
                if mounts.is_empty() {
                    "none".into()
                } else {
                    mounts.join(", ")
                }
            );
            false
        }
        None => {
            println!("{tag}   RELEASED — no longer enumerates");
            true
        }
    }
}
