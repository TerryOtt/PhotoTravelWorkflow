//! Does this device slow down as it heats up?
//!
//! ```text
//! cargo run --release --example sustained -- D:\DCIM\100CANON 120
//! ```
//!
//! Every other probe here reports one average over a short burst, which cannot see
//! throttling at all — a device that starts at 250 MB/s and settles at 70 reports the
//! same single number as one that was always 70. **UHS-II cards run hot by design and a
//! passive reader has nowhere to put the heat**, so a sustained read is the only way to
//! tell a slow device from a hot one.
//!
//! This reads continuously and prints the rate for each window, so throttling shows up
//! as a curve rather than hiding inside a mean. **Read it cold**: a device that has just
//! absorbed a few hundred gigabytes is already at whatever temperature the last test
//! left it.
//!
//! Reads only — nothing here writes to a card. `CONOPS.md` makes that binding on
//! diagnostics as well as on the tool, after a write probe left a card's filesystem
//! dirty on 2026-08-04.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const SECTOR: usize = 4096;
const CHUNK: usize = 16 * 1024 * 1024;

/// How often to report. Short enough to see a knee, long enough to be steady.
const WINDOW: Duration = Duration::from_secs(10);

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next().map(PathBuf::from) else {
        eprintln!("usage: sustained <directory of CR3 files> [seconds]");
        std::process::exit(2);
    };
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(120);

    let files = cr3_files(&dir);
    assert!(!files.is_empty(), "no CR3 files in {}", dir.display());

    println!(
        "reading {} for {seconds}s, reporting every {}s\n",
        dir.display(),
        WINDOW.as_secs()
    );
    println!("  {:>6}  {:>9}  {:>8}", "at", "MB/s", "vs first");

    let layout = std::alloc::Layout::from_size_align(CHUNK, SECTOR).expect("a valid layout");
    // SAFETY: non-zero size, freed below exactly once.
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    // SAFETY: valid for CHUNK initialized bytes.
    let buffer = unsafe { std::slice::from_raw_parts_mut(raw, CHUNK) };

    let overall = Instant::now();
    let mut window_start = Instant::now();
    let mut window_bytes = 0u64;
    let mut first_rate = 0.0;

    'outer: loop {
        for path in &files {
            // The deadline is checked here as well as in the read loop below, because every
            // other exit lives inside that loop and a device removed mid-run fails every
            // `open` instead. Without this the `for` completes without reading a byte, the
            // outer loop restarts, and the run never consults the clock again. This probe
            // exists to be run across reader and card swaps, so that is the one thing it
            // must not do -- found 2026-08-07 when a reader was unplugged mid-measurement.
            if overall.elapsed().as_secs() >= seconds {
                break 'outer;
            }

            let Ok(mut file) = OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_NO_BUFFERING)
                .open(path)
            else {
                continue;
            };

            loop {
                match file.read(buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => window_bytes += n as u64,
                }

                if window_start.elapsed() >= WINDOW {
                    let rate = window_bytes as f64 / window_start.elapsed().as_secs_f64() / 1e6;
                    if first_rate == 0.0 {
                        first_rate = rate;
                    }
                    println!(
                        "  {:>5}s  {rate:>9.0}  {:>7.0}%",
                        overall.elapsed().as_secs(),
                        rate / first_rate * 100.0
                    );

                    window_start = Instant::now();
                    window_bytes = 0;
                }

                if overall.elapsed().as_secs() >= seconds {
                    break 'outer;
                }
            }
        }
    }

    // SAFETY: allocated above with this layout.
    unsafe { std::alloc::dealloc(raw, layout) };

    println!(
        "\n  A rate that decays across the windows is throttling — thermal, most likely,\n  \
         on a card that has nowhere to shed heat. A flat line is a genuinely slow device."
    );
}

fn cr3_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("cr3"))
        })
        .collect();
    files.sort();
    files
}
