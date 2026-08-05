//! Is the probe the limit, or the device?
//!
//! ```text
//! cargo run --release --example threads -- J:\Travel\Images\2022\2022-09-27
//! ```
//!
//! `examples/contention.rs` reads each device with **one thread** issuing sequential
//! `FILE_FLAG_NO_BUFFERING` requests. That flag disables OS read-ahead, so nothing is in
//! flight behind a request and a single thread is latency-bound: throughput becomes
//! request size divided by round-trip time, whatever the device could actually deliver.
//!
//! **The tell that prompted this:** two Thunderbolt devices measured 2,445 MB/s together
//! while one of them alone measured 2,582. Two devices sharing a fabric cannot sum to
//! less than one of them unless something other than the fabric is the constraint.
//!
//! So: read one device with 1, 2, 4 and 8 threads over disjoint files, same total bytes
//! each time. **If the total climbs with thread count, the probe was the ceiling** — and
//! every "device saturates at X" conclusion drawn from the single-threaded instrument is
//! a lower bound rather than a limit.
//!
//! This is the request-size finding one level up: there an apparent device limit was
//! really how a request was made, and a bigger request lifted it by a third. Here the
//! question is how many requests are made at once.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const SECTOR: usize = 4096;

/// The tool's own request size, so this measures what the verify pass does.
const CHUNK: usize = 16 * 1024 * 1024;

/// Total per configuration, split across however many threads are running.
const SAMPLE: u64 = 6 * 1024 * 1024 * 1024;

fn main() {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: threads <directory of CR3 files>");
        std::process::exit(2);
    };

    let files = cr3_files(&dir);
    assert!(!files.is_empty(), "no CR3 files in {}", dir.display());

    println!(
        "{} GB per configuration, {} MiB unbuffered requests, from {}\n",
        SAMPLE / (1024 * 1024 * 1024),
        CHUNK / (1024 * 1024),
        dir.display()
    );

    // Wake the device so the first row does not pay for it.
    read(&files[..4.min(files.len())], 256 * 1024 * 1024);

    println!("  {:<8} {:>10}  {:>9}", "threads", "MB/s", "vs 1");

    let mut single = 0.0;
    for threads in [1usize, 2, 4, 8] {
        // Disjoint slices, so the threads never contend for the same file — the point is
        // to have several requests in flight at the device, not to race on one handle.
        let per_thread = SAMPLE / threads as u64;
        let chunks: Vec<&[PathBuf]> = files.chunks(files.len().div_ceil(threads)).collect();

        let start = Instant::now();
        let total: u64 = std::thread::scope(|scope| {
            let handles: Vec<_> = chunks
                .iter()
                .take(threads)
                .map(|slice| scope.spawn(move || read(slice, per_thread)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum()
        });
        let rate = total as f64 / start.elapsed().as_secs_f64() / 1e6;

        if threads == 1 {
            single = rate;
        }
        println!("  {threads:<8} {rate:>10.0}  {:>8.2}x", rate / single);
    }
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
