//! Dismount the camera cards and report whether Windows actually let go of them.
//!
//! ```text
//! cargo run --release --example dismount-cards
//! ```
//!
//! **Why this exists.** On 2026-08-04 a run reported both cards dismounted and the operator
//! then found both still sitting in the tray with drive letters. That is the end of a
//! 35-minute offload, which is a miserable place to learn what `FSCTL_DISMOUNT_VOLUME` does
//! and does not do — so this is the same code path the run uses, on the cards alone, in
//! about two seconds.
//!
//! **It writes nothing.** `eject::dismount_card` takes a lock and detaches a filesystem; it
//! creates, modifies and deletes nothing, and the tool's never-write-to-a-card non-goal is
//! intact. The worst outcome is a card that has to be reseated, which is what a *successful*
//! dismount is supposed to produce anyway.
//!
//! # The measurement hazard, which is the whole subtlety here
//!
//! **Looking at a dismounted volume can be what remounts it.** Windows remounts on next
//! access, and `GetVolumeInformationW` — which `storage::volumes()` calls on every volume it
//! enumerates — is an access. So a naive "did the letter go away" check is capable of
//! answering *no* precisely because it asked.
//!
//! That is not a reason to skip the check; it is the reason to report it honestly. Explorer,
//! the indexer and the tray icon all poll volumes continuously, so a state that any
//! observation destroys is a state the operator will never see either. **If the letter is
//! back by the time anything looks, it is back — and that is the finding, not an artifact.**
//! What this probe must not do is claim a durable dismount it did not establish, so it
//! reports what it saw and when.

use std::process::ExitCode;
use std::time::Instant;

use photoday::{cards, config, destinations, eject, storage};

fn main() -> ExitCode {
    let config = match config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    // The same exclusion pre-flight uses: a volume the config names is a destination and can
    // never be a card, which is what makes `DCIM` alone a safe discriminator (decision 7).
    let survey = match destinations::survey(&config) {
        Ok(survey) => survey,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    let volumes = match storage::volumes() {
        Ok(volumes) => volumes,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    let found = cards::find(&volumes, &survey.volume_guids());
    if found.is_empty() {
        eprintln!("no camera card found — a card is a volume with DCIM that is not a destination");
        return ExitCode::from(2);
    }

    println!();
    println!("=== before ===");
    for card in &found {
        describe(&card.volume);
    }

    println!();
    println!("=== dismounting ===");
    for card in &found {
        let started = Instant::now();
        let outcome = eject::dismount_card(&card.volume);
        let took = started.elapsed();

        match outcome {
            Ok(eject::CardOutcome::Dismounted) => {
                println!(
                    "  {:<12} dismounted in {:.1}s",
                    card.label(),
                    took.as_secs_f64()
                );
            }
            Ok(eject::CardOutcome::Held { reason }) => {
                println!(
                    "  {:<12} HELD after {:.1}s",
                    card.label(),
                    took.as_secs_f64()
                );
                println!("               {reason}");
            }
            Err(error) => println!("  {:<12} ERROR — {error:#}", card.label()),
        }
    }

    // Re-enumerated rather than reusing the volumes above, because the question is what
    // Windows says *now*. See the module note: this call is itself an access, so a letter
    // that reappears here is a letter that reappears for the tray icon too.
    println!();
    println!("=== after, re-enumerated ===");
    match storage::volumes() {
        Ok(after) => {
            for card in &found {
                match after
                    .iter()
                    .find(|volume| volume.guid_path == card.volume.guid_path)
                {
                    Some(volume) => describe(volume),
                    // The outcome the operator actually wants: the volume no longer
                    // enumerates at all.
                    None => println!("  {:<12} GONE — no longer enumerates", card.label()),
                }
            }
        }
        Err(error) => eprintln!("re-enumerating: {error:#}"),
    }

    println!();
    println!(
        "A drive letter still listed above is the bug: a dismount detaches the filesystem\n\
         and leaves the volume, so the tray icon still offers the device. Compare against\n\
         scripts\\eject-check.ps1, which is what the operator sees."
    );

    ExitCode::SUCCESS
}

/// One volume, in the terms this experiment turns on: does it still have a letter?
fn describe(volume: &storage::Volume) {
    let mounts: Vec<String> = volume
        .mount_points
        .iter()
        .map(|mount| mount.display().to_string())
        .collect();

    println!(
        "  {:<12} serial {}  mounted at [{}]",
        volume
            .mount_points
            .first()
            .map_or_else(|| volume.guid_path.clone(), |m| m.display().to_string()),
        volume.serial_text(),
        if mounts.is_empty() {
            "none".to_string()
        } else {
            mounts.join(", ")
        }
    );
}
