//! What does `FILE_FLAG_WRITE_THROUGH` actually cost on this rig?
//!
//! ```text
//! cargo run --release --example write-probe -- G:\Travel\Images
//! ```
//!
//! Decision 2 chose write-through so that LANDED means *durably on media* rather than
//! *somewhere in RAM*, and priced it at nothing in particular. The first real run came
//! in at 56 MB/s per destination against an expected 400–800, so the price needs a
//! number rather than an assumption.
//!
//! Writes frames the size of a real CR3, the same way phase 3 does — temp file, write,
//! rename — and then the same volume of plain buffered writes for comparison. Cleans up
//! after itself.

use std::path::{Path, PathBuf};
use std::time::Instant;

use offload::cards;
use offload::winio::write_through;

/// A realistic frame: the day's average is 48.5 MB.
const FRAME_BYTES: usize = 48 * 1024 * 1024;
const FRAMES: usize = 20;

fn main() {
    let Some(root) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: write-probe <directory>");
        std::process::exit(2);
    };

    // **Never a camera card.** CONOPS makes in-camera formatting the only way a card is
    // ever written to by anything, and that binds diagnostics as well as the tool. This
    // probe ignored that on 2026-08-04 and wrote a gigabyte to a live card. The guard it
    // then grew was defeated hours later by a deeper path, so the check now lives in
    // `cards::is_on_camera_card` — one copy, tested, asked of the whole ancestry.
    if cards::is_on_camera_card(&root) {
        eprintln!(
            "refusing to write to {} — it is on a camera card, and nothing here writes to those (see docs/CONOPS.md). Point this at a destination instead.",
            root.display()
        );
        std::process::exit(2);
    }

    let probe = root.join("_write-probe");
    std::fs::create_dir_all(&probe).expect("creating the probe directory");

    let frame: Vec<u8> = (0..FRAME_BYTES)
        .map(|i| (i.wrapping_mul(31) % 251) as u8)
        .collect();
    let total_mb = (FRAME_BYTES * FRAMES) as f64 / 1e6;

    println!(
        "{FRAMES} frames of {} MB each = {total_mb:.0} MB, into {}\n",
        FRAME_BYTES / (1024 * 1024),
        probe.display()
    );

    // The tool's path: write-through to a temp name, then rename.
    let start = Instant::now();
    for n in 0..FRAMES {
        write_through(&probe.join(format!("wt_{n:03}.CR3")), &frame).expect("write-through");
    }
    let write_through_rate = total_mb / start.elapsed().as_secs_f64();

    // Plain buffered writes, for contrast. Not a candidate — decision 2 explains why a
    // buffered write makes the primary metric unmeasurable — but it is the yardstick
    // that says whether write-through is the cost or the device is.
    let start = Instant::now();
    for n in 0..FRAMES {
        std::fs::write(probe.join(format!("plain_{n:03}.CR3")), &frame).expect("plain write");
    }
    let plain_rate = total_mb / start.elapsed().as_secs_f64();

    println!("  write-through + rename : {write_through_rate:>7.0} MB/s");
    println!("  plain buffered write   : {plain_rate:>7.0} MB/s");
    println!(
        "  write-through costs     {:>7.1}x",
        plain_rate / write_through_rate
    );

    cleanup(&probe);
}

fn cleanup(probe: &Path) {
    match std::fs::remove_dir_all(probe) {
        Ok(()) => println!("\ncleaned up {}", probe.display()),
        Err(error) => eprintln!("\ncould not clean up {}: {error}", probe.display()),
    }
}
