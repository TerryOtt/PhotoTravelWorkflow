//! Time the camera-card eject veto, one production attempt at a time.
//!
//! ```text
//! cargo run --release --example card-veto-watch
//! ```
//!
//! **The open question decision 22 cannot answer from a run.** On 2026-08-06 both cards
//! released on the tool's own `CM_Request_Device_Eject` after **11 m 17 s**, with nothing
//! touching the tray — which established that the obstruction clears with *elapsed time*
//! rather than needing a different mechanism. What it did not establish is **why**, and the
//! report cannot help: [`eject::eject`] returns only its last attempt, so a run prints one
//! veto and discards the forty before it.
//!
//! This calls [`eject::attempt`] — the same sequence the tool uses, not an approximation of
//! it — and prints every result with a running clock. Two things fall out that a run cannot
//! give:
//!
//! - **Whether the veto changes.** A `PNP_VETO_TYPE` that stays identical for eleven minutes
//!   and then succeeds means a state cleared somewhere below the filesystem. One that changes
//!   shape near the end means something else, and the two want different fixes.
//! - **Whether our own instrument is the cause.** `scripts/watch-rig.ps1` polls `Get-Disk` and
//!   `Get-Partition` every two seconds, and its doc comment *asserts* it never opens a volume.
//!   That is an untested claim about the exact stack under investigation. Run this once with
//!   the watcher armed and once with it stopped; nothing else changes.
//!
//! **It writes nothing to any card** — binding constraint 2 is untouched, and every call here
//! is one a normal run already makes.
//!
//! # Run it AFTER something that reads the cards, or it measures an empty room
//!
//! **Measured 2026-08-06, first use: run cold, with no offload beforehand, both cards released
//! on the first attempt in ONE SECOND.** So the veto is not a property of the cards, the
//! readers, the bus or the idle machine — all of which were unchanged from the runs that took
//! 11 m 17 s and 22 m 16 s. **What differs is that a run had just read every file on the card.**
//!
//! The intended sequence is therefore:
//!
//! ```text
//! offload --no-eject          # reads every file, ejects nothing
//! cargo run --release --example card-veto-watch
//! ```
//!
//! A cold run of this harness is still worth something — it is what exonerated
//! `scripts/watch-rig.ps1` — but it MUST NOT be read as evidence that a veto has stopped
//! happening.
//!
//! **What it costs the operator: one cable.** Releasing a card ejects its device, and for the
//! USB SD reader that device *is* the reader, so it powers down and needs a replug. The
//! Thunderbolt CFexpress reader is untouched. **The three archive SSDs are never touched at
//! all**, which is the whole reason this is a card-only harness rather than a full run.

use std::io::Write;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use offload::{cards, config, destinations, eject, storage};

/// Long enough to cover the 11 m 17 s observation with room either side, short enough that a
/// card which is never going to release does not hold the desk all afternoon.
const CAP: Duration = Duration::from_secs(25 * 60);

/// Between rounds. The production path backs off geometrically to spread attempts across a
/// 90-minute budget; a fixed short gap is right here because the *shape over time* is the
/// measurement, and geometric backoff would sample the interesting minutes least.
const GAP: Duration = Duration::from_secs(15);

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
        eprintln!("no camera card found — is the SD reader replugged?");
        return ExitCode::from(2);
    }

    // Resolved once, up front. `device_of` walks the storage stack, and doing that inside the
    // retry loop would put an extra enumeration between every attempt — an instrument change
    // masquerading as a measurement.
    let mut watching: Vec<_> = found
        .iter()
        .filter_map(|card| match storage::device_of(&card.volume) {
            Ok(device) => Some((card.label(), &card.volume, device)),
            Err(error) => {
                eprintln!("{}: no device behind it — {error:#}", card.label());
                None
            }
        })
        .collect();

    for (label, volume, device) in &watching {
        say(format!(
            "watching {label} · disk {} · {} · {}",
            device.disk_number,
            if volume.removable {
                "removable media"
            } else {
                "FIXED media"
            },
            volume.guid_path
        ));
    }
    say(format!(
        "cap {}s, {}s between rounds, using the production attempt sequence",
        CAP.as_secs(),
        GAP.as_secs()
    ));

    let started = Instant::now();
    let mut round = 0u32;

    while !watching.is_empty() && started.elapsed() < CAP {
        round += 1;

        // Retain rather than index: a card that releases mid-round leaves the set immediately,
        // so the next round's timings are not padded by attempts on a device already gone.
        watching.retain(|(label, volume, device)| {
            // The production preparation, so this measures the tool rather than an alternative
            // to it. `Prepare::Bare` is the other arm of that experiment and belongs to a run.
            let outcome = eject::attempt(volume, device, true);
            let at = started.elapsed();

            let line = match &outcome {
                Ok(eject::Outcome::Ejected) => "RELEASED".to_owned(),
                Ok(eject::Outcome::Dismounted { veto, reason }) => {
                    format!("dismounted [{veto:?}] — {reason}")
                }
                Ok(eject::Outcome::Held { reason }) => format!("held — {reason}"),
                Err(error) => format!("ERROR — {error:#}"),
            };
            say(format!(
                "[{:>3}m {:02}s] round {round:<3} {label:<10} {line}",
                at.as_secs() / 60,
                at.as_secs() % 60
            ));

            !matches!(outcome, Ok(eject::Outcome::Ejected))
        });

        if !watching.is_empty() {
            std::thread::sleep(GAP);
        }
    }

    let total = started.elapsed();
    say(String::new());
    if watching.is_empty() {
        say(format!(
            "ALL CARDS RELEASED after {}m {:02}s over {round} rounds",
            total.as_secs() / 60,
            total.as_secs() % 60
        ));
    } else {
        // Deliberately not phrased as a failure: the tool never wrote to a card, so this is a
        // measurement that did not converge rather than data at risk.
        let stuck: Vec<&str> = watching.iter().map(|(label, ..)| label.as_str()).collect();
        say(format!(
            "STILL HELD at the {}m cap after {round} rounds: {}",
            CAP.as_secs() / 60,
            stuck.join(", ")
        ));
    }
    say("replug the USB SD reader before the next offload".to_owned());

    ExitCode::SUCCESS
}

/// Print and flush.
///
/// **Flushed per line because this is watched live.** Rust block-buffers stdout when it is a
/// pipe, so a `Monitor` on this would otherwise receive the whole session in one burst at the
/// end — which is exactly the information this harness exists to spread out over time.
fn say(line: String) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}
