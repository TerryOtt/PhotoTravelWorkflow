//! Is a slow read the device, or the way we are asking for it?
//!
//! ```text
//! cargo run --release --example read-probe -- D:\DCIM\100CANON
//! ```
//!
//! The SDXC card measured 60 MB/s read while having been *written* at 117 MB/s, which
//! is backwards for flash and points at the request pattern rather than the medium.
//!
//! `FILE_FLAG_NO_BUFFERING` disables OS read-ahead, so each request is a synchronous
//! round trip with nothing in flight behind it. That makes throughput a function of
//! request size divided by device latency — and on a high-latency USB bridge, a 1 MiB
//! request size can cap a perfectly fast card at a fraction of its bandwidth. This
//! sweeps the request size to find out, and compares against a buffered read where the
//! OS is free to prefetch.
//!
//! **It matters well beyond the card speed test**: the verify pass of decision 2 reads
//! every destination through the same unbuffered path, so if request size is the
//! constraint here it is the constraint on 4N of verification too.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::Instant;

const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const SECTOR: usize = 4096;

/// Read this much per measurement — enough to be steady, quick enough to sweep.
const SAMPLE: u64 = 256 * 1024 * 1024;

fn main() {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: read-probe <directory of CR3 files>");
        std::process::exit(2);
    };

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("reading the directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("cr3"))
        })
        .collect();
    files.sort();

    if files.is_empty() {
        eprintln!("no CR3 files in {}", dir.display());
        std::process::exit(1);
    }

    println!(
        "{} MB per measurement from {}\n",
        SAMPLE / 1_000_000,
        dir.display()
    );
    println!("  {:<22} {:>10}", "request size", "MB/s");

    // Wake the device so the first row is not paying for it — the lesson from the card
    // speed test, which billed a reader's wake-up to the transfer and picked the wrong
    // card because of it.
    let _ = unbuffered(&files, 1024 * 1024, 8 * 1024 * 1024);

    for chunk_mib in [1usize, 2, 4, 8, 16, 32] {
        let chunk = chunk_mib * 1024 * 1024;
        let start = Instant::now();
        let read = unbuffered(&files, chunk, SAMPLE);
        let rate = read as f64 / start.elapsed().as_secs_f64() / 1e6;
        println!("  unbuffered {chunk_mib:>3} MiB{:<8} {rate:>10.0}", "");
    }

    let start = Instant::now();
    let read = buffered(&files, SAMPLE);
    let rate = read as f64 / start.elapsed().as_secs_f64() / 1e6;
    println!("  {:<22} {rate:>10.0}", "buffered (read-ahead)");
}

/// Read `limit` bytes across `files` with `FILE_FLAG_NO_BUFFERING`, `chunk` at a time.
fn unbuffered(files: &[PathBuf], chunk: usize, limit: u64) -> u64 {
    assert!(chunk.is_multiple_of(SECTOR));

    // One aligned buffer, reused — the alignment the flag requires.
    let layout = std::alloc::Layout::from_size_align(chunk, SECTOR).expect("a valid layout");
    // SAFETY: non-zero size, and freed below exactly once.
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    // SAFETY: `raw` is valid for `chunk` initialized bytes.
    let buffer = unsafe { std::slice::from_raw_parts_mut(raw, chunk) };

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

    // SAFETY: allocated above with this exact layout.
    unsafe { std::alloc::dealloc(raw, layout) };
    total
}

/// The same volume, read normally, so the OS may prefetch.
fn buffered(files: &[PathBuf], limit: u64) -> u64 {
    let mut total = 0u64;
    for path in files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        total += bytes.len() as u64;
        if total >= limit {
            break;
        }
    }
    total
}
