//! What does one device cost another?
//!
//! ```text
//! cargo run --release --example contention -- G:\...\2022-09-27 I:\...\2022-09-27 J:\...
//! ```
//!
//! Phase 3 reads and writes several devices at once, and the design's wall-clock table
//! assumes they are independent. They are not: a Thunderbolt hub tunnels PCIe for native
//! Thunderbolt devices but puts every USB device behind one internal USB host
//! controller, so "each stream is well under 10 Gbps" does not imply the streams do not
//! collide.
//!
//! Measures each device alone, then all of them together, and reports what each one lost.
//! **Alone-versus-together is the whole experiment** — a device that holds its solo rate
//! in company is independent, and one that halves is sharing something.
//!
//! Reads only, so it is safe to point at live archives and changes nothing.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const SECTOR: usize = 4096;
const CHUNK: usize = 16 * 1024 * 1024;

/// Per device, per phase. Large enough to be steady, small enough that the whole sweep
/// is a couple of minutes.
const SAMPLE: u64 = 3 * 1024 * 1024 * 1024;

fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if dirs.is_empty() {
        eprintln!("usage: contention <dir> [<dir> ...]");
        std::process::exit(2);
    }

    let sets: Vec<(PathBuf, Vec<PathBuf>)> = dirs
        .into_iter()
        .map(|dir| {
            let files = cr3_files(&dir);
            assert!(!files.is_empty(), "no CR3 files in {}", dir.display());
            (dir, files)
        })
        .collect();

    println!(
        "{} GB per device per phase, {} MiB unbuffered requests\n",
        SAMPLE / (1024 * 1024 * 1024),
        CHUNK / (1024 * 1024)
    );

    // Alone, one at a time.
    let mut alone = Vec::new();
    for (dir, files) in &sets {
        // Wake it first, so the number is the device rather than its power state.
        read(files, 128 * 1024 * 1024);

        let start = Instant::now();
        let bytes = read(files, SAMPLE);
        let rate = bytes as f64 / start.elapsed().as_secs_f64() / 1e6;
        alone.push(rate);
        println!("  alone      {:<28} {rate:>7.0} MB/s", label(dir));
    }

    println!();

    // All at once, started together.
    let together: Vec<f64> = std::thread::scope(|scope| {
        let handles: Vec<_> = sets
            .iter()
            .map(|(_, files)| {
                scope.spawn(move || {
                    let start = Instant::now();
                    let bytes = read(files, SAMPLE);
                    bytes as f64 / start.elapsed().as_secs_f64() / 1e6
                })
            })
            .collect();

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut alone_total = 0.0;
    let mut together_total = 0.0;

    for ((dir, _), (solo, shared)) in sets.iter().zip(alone.iter().zip(&together)) {
        alone_total += solo;
        together_total += shared;
        println!(
            "  together   {:<28} {shared:>7.0} MB/s   {:>5.0}% of its solo rate",
            label(dir),
            shared / solo * 100.0
        );
    }

    println!();
    println!("  sum alone      {alone_total:>7.0} MB/s");
    println!(
        "  sum together   {together_total:>7.0} MB/s   ({:.0}% of the sum of the parts)",
        together_total / alone_total * 100.0
    );
}

fn label(dir: &Path) -> String {
    dir.components().next().map_or_else(
        || dir.display().to_string(),
        |c| c.as_os_str().to_string_lossy().into_owned(),
    )
}

fn read(files: &[PathBuf], limit: u64) -> u64 {
    let layout = std::alloc::Layout::from_size_align(CHUNK, SECTOR).expect("a valid layout");
    // SAFETY: non-zero size, freed below exactly once.
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    // SAFETY: valid for CHUNK initialized bytes.
    let buffer = unsafe { std::slice::from_raw_parts_mut(raw, CHUNK) };

    let mut total = 0u64;
    'outer: for path in files {
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
                Ok(n) => total += n as u64,
            }
            if total >= limit {
                break 'outer;
            }
        }
    }

    // SAFETY: allocated above with this layout.
    unsafe { std::alloc::dealloc(raw, layout) };
    total
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
