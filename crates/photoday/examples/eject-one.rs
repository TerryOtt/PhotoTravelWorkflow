//! Eject one configured destination, by label.
//!
//! ```text
//! cargo run --release --example eject-one -- OWC
//! ```
//!
//! **Eject is the one part of this tool that cannot be tested against real hardware from a
//! unit test.** Locking a volume, dismounting it and asking the configuration manager to
//! power the enclosure down are all operations on a live device; there is nothing to fake
//! that would prove anything. The alternative to this harness is exercising it through a
//! full 33-minute offload, which is a miserable way to learn that a wide-string buffer was
//! the wrong length.
//!
//! So: same code path the run uses, one device, thirty seconds.
//!
//! **It does not touch data.** A dismount flushes and detaches a filesystem; it deletes
//! nothing and modifies no file. The worst outcome is a drive that has to be unplugged and
//! reconnected — which is exactly what a successful eject produces anyway.
//!
//! Prints the same [`eject::Outcome`] the report does, plus the resolved device, so a
//! refusal can be read against *which* volume refused.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use photoday::{config, destinations, eject};

fn main() -> ExitCode {
    let Some(wanted) = std::env::args().nth(1) else {
        eprintln!("usage: eject-one <destination label>");
        return ExitCode::from(2);
    };

    let config = match config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    let survey = match destinations::survey(&config) {
        Ok(survey) => survey,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    let Some(resolved) = survey
        .found
        .iter()
        .find(|resolved| resolved.label.eq_ignore_ascii_case(&wanted))
    else {
        eprintln!(
            "no connected destination is labelled {wanted:?} — found: {}",
            survey
                .found
                .iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return ExitCode::from(2);
    };

    if !resolved.ejectable() {
        eprintln!(
            "{} is this machine's own disk — there is nothing to eject, and decision 22 \
             never touches it",
            resolved.label
        );
        return ExitCode::from(2);
    }

    println!(
        "{}  {}  disk {}",
        resolved.label,
        resolved.root.display(),
        resolved
            .device
            .as_ref()
            .map_or_else(|| "?".into(), |d| d.disk_number.to_string())
    );

    let Some(device) = resolved.device.as_ref() else {
        eprintln!("the volume reports no physical device to power down");
        return ExitCode::FAILURE;
    };

    // The run gives eject the rest of its hour (decision 22); a probe run by hand has
    // someone watching it, so it gets a window short enough to stay interactive while still
    // exercising the retry.
    let deadline = Instant::now() + Duration::from_secs(90);

    match eject::eject(&resolved.volume, device, deadline) {
        Ok(effort) => {
            println!(
                "\n  {:#?}\n  {} attempt(s) over {:.1}s",
                effort.outcome,
                effort.attempts,
                effort.waited.as_secs_f64()
            );
            if effort.outcome.is_ejected() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(error) => {
            eprintln!("\ncould not even attempt it: {error:#}");
            ExitCode::FAILURE
        }
    }
}
