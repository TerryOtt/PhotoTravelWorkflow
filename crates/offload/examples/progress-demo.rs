//! Watch the progress display do its thing, without a 33-minute offload.
//!
//! ```text
//! cargo run --release --example progress-demo            # bars, at a terminal
//! cargo run --release --example progress-demo > out.txt  # plain lines, redirected
//! ```
//!
//! **A display is the one thing a unit test cannot judge**, and the alternative to this
//! harness is exercising it through a real run — the same argument `eject-one.rs` carries,
//! and the same 33-minute price. It moves no data and touches no device.
//!
//! **It earns its place by checking the mode that is easy to get wrong.** `indicatif`
//! disables itself when its stream is not a terminal, so the bars were briefly written in a
//! form that would have rendered *nothing* whenever a run was captured to a file — which is
//! how every run driven by Claude behaves (`CONOPS.md`). Redirect this example's stdout and
//! the `Lines` mode has to produce something; run it at a terminal and the bars have to.
//!
//! Both are one command, which is the point: a check nobody runs is not a check.

use std::thread::sleep;
use std::time::Duration;

use offload::progress::Progress;

fn main() {
    // The real thing: bars when stderr is a terminal, throttled lines when it is not.
    let progress = Progress::detect();

    match &progress {
        Progress::Bars(_) => eprintln!("\nstderr is a terminal — expect four live bars\n"),
        Progress::Lines => println!("\nstderr is redirected — expect plain lines on stdout\n"),
        Progress::Silent => println!("\nsilent\n"),
    }

    // Four destinations, as phase 3 has, so the *stacking* is visible rather than just one
    // bar moving. A small N on purpose: the throttle is a fraction of the pass, so ten
    // updates land whether the day holds 40 frames or 3,883.
    const FRAMES: usize = 40;
    let labels = ["laptop", "OWC", "SanDisk", "WD"];

    let bars: Vec<_> = labels
        .iter()
        .map(|label| {
            let bar = progress.bar(label, FRAMES);
            bar.set_message("writing");
            bar
        })
        .collect();

    // Staggered rates, because the single most useful thing this display shows is one
    // destination finishing while another is still going — decision 14 names the slowest
    // device as the most useful number in the report for the same reason.
    for step in 0..FRAMES {
        for (n, bar) in bars.iter().enumerate() {
            if step % (n + 1) == 0 {
                bar.inc();
            }
        }
        sleep(Duration::from_millis(40));
    }

    // The pass change: counters rewind rather than running on to 2N.
    for bar in &bars {
        bar.restart();
        bar.set_message("verifying");
    }

    for _ in 0..FRAMES {
        for bar in &bars {
            bar.inc();
        }
        sleep(Duration::from_millis(25));
    }

    for bar in &bars {
        bar.finish();
    }

    println!("\ndone — the report would print here, below four bars left standing at 100%");
}
