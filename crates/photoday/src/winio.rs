//! The two Windows file flags decision 2 rests on, and the aligned buffer one of them
//! demands.
//!
//! **The flags are deliberately different on each side and neither substitutes for the
//! other.** Writes are `FILE_FLAG_WRITE_THROUGH`, which makes LANDED mean *durably on
//! media* rather than *somewhere in RAM* — a plain buffered write would let the process
//! exit with gigabytes still in the page cache, which makes the primary metric
//! unmeasurable. Verify reads are `FILE_FLAG_NO_BUFFERING`, which is what stops a
//! read-back from being served out of the page cache and comparing a buffer to itself.
//!
//! Unbuffered *writes* were considered and rejected (decision 2): they demand
//! sector-multiple writes, and a raw file's partial final sector cannot meet that
//! without a pad-and-truncate dance that buys no guarantee write-through does not
//! already give. The read side keeps the flag precisely because the constraint never
//! bites there — a short read at end-of-file is legal.
//!
//! Nothing here needs the `windows` crate: `OpenOptionsExt::custom_flags` is std.

use std::alloc::{self, Layout};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::ptr::NonNull;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tempfile::Builder;

use crate::hash::Digest32;

/// Durably on media before it is claimed.
const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;

/// Bypasses the OS page cache, so a verify read describes the device.
const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;

/// Buffer alignment and read granularity for `FILE_FLAG_NO_BUFFERING`.
///
/// Windows requires the buffer address, the file offset and the request length to be
/// multiples of the volume's physical sector size. 4 KiB satisfies both 512e and 4Kn
/// devices, which is every device in this rig — asking each volume for its real value
/// would cost a `DeviceIoControl` to learn a number that is never larger than this.
const SECTOR: usize = 4096;

/// How much to pull per unbuffered read. A sector multiple, as the flag requires.
///
/// **16 MiB, chosen by measurement rather than by feel.** `FILE_FLAG_NO_BUFFERING`
/// disables OS read-ahead, so nothing is in flight behind a request and throughput
/// becomes a function of request size against device latency. Swept on all four real
/// destinations, 1 MiB against 16 MiB:
///
/// | Device | 1 MiB | 16 MiB |
/// |---|---|---|
/// | SanDisk, 10 Gbps USB | 662 MB/s | 947 MB/s |
/// | WD, 10 Gbps USB | 663 MB/s | 899 MB/s |
/// | OWC, Thunderbolt | 1,976 MB/s | 2,540 MB/s |
/// | laptop NVMe | 2,130 MB/s | 3,044 MB/s |
///
/// A third to nearly half again, on every device, for a constant. 32 MiB measured no
/// better than 16 and sometimes worse, so this is the knee rather than the ceiling.
///
/// The same sweep also settled a question decision 2 left open: **unbuffered is not a
/// throughput sacrifice.** Buffered reads with full OS read-ahead came in *below* even
/// the 1 MiB unbuffered figure on all four devices — 634, 625, 1,609 and 1,718 MB/s.
/// The cache bypass that makes verification mean something is also the faster path.
const VERIFY_CHUNK: usize = 16 * 1024 * 1024;

/// The prefix every in-flight write carries, and the one pre-flight's orphan sweep
/// keys on (decision 13). A partial file never wears the real name, so anything left
/// under this prefix is debris from a killed run and is safe to delete.
pub const TEMP_PREFIX: &str = ".photoday-tmp-";

/// Copy `bytes` to `target`, durably, without ever letting a partial file wear the
/// real name.
///
/// Temp-then-rename, with the temp file in the destination directory so the rename
/// stays on one filesystem and is therefore atomic. `tempfile` supplies the unique name
/// — so two runs over one directory cannot collide — and deletes it on drop, so a
/// failure anywhere below leaves nothing behind. The flags are ours rather than
/// `tempfile`'s defaults, which is what `Builder::make_in` exists for.
pub fn write_through(target: &Path, bytes: &[u8]) -> Result<()> {
    let directory = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };

    fs::create_dir_all(directory).with_context(|| format!("creating {}", directory.display()))?;

    let mut temp = Builder::new()
        .prefix(TEMP_PREFIX)
        .make_in(directory, |path| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .custom_flags(FILE_FLAG_WRITE_THROUGH)
                .open(path)
        })
        .with_context(|| format!("creating a temporary file for {}", target.display()))?;

    temp.as_file_mut()
        .write_all(bytes)
        .with_context(|| format!("writing {}", target.display()))?;

    temp.persist(target)
        // The error hands the temp file back so a caller could retry; nothing here
        // does, and dropping it is what removes the file.
        .map_err(|error| error.error)
        .with_context(|| format!("renaming into place: {}", target.display()))?;

    Ok(())
}

/// Re-read `path` past every OS cache and return what is actually on the device.
///
/// This is the read half of decision 2, and the reason a verify pass means anything.
/// What it cannot defeat is the SSD's own onboard DRAM cache on a small day, which the
/// design accepts and records rather than engineering away — write-through softens even
/// that, since a device-cached read then describes data the device has already
/// committed.
pub fn unbuffered_sha256(path: &Path) -> Result<Digest32> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING)
        .open(path)
        .with_context(|| format!("opening {} for verification", path.display()))?;

    let mut buffer = SectorBuf::new(VERIFY_CHUNK);
    let mut hasher = Sha256::new();

    loop {
        // The *request* is always a sector multiple, which is what the flag demands.
        // The returned count may be short at end-of-file, which is legal and is exactly
        // why the partial final sector that rules this flag out on the write side is a
        // non-issue here.
        let read = file
            .read(buffer.as_mut_slice())
            .with_context(|| format!("reading {} back", path.display()))?;

        if read == 0 {
            break;
        }
        hasher.update(&buffer.as_mut_slice()[..read]);
    }

    Ok(hasher.finalize().into())
}

/// Read up to `limit` bytes off the media, bypassing every cache, and return how many.
///
/// For the card speed test of decision 7, where the *content* is irrelevant and the only
/// thing being measured is how fast the device delivers bytes. Unbuffered is not an
/// optimization here, it is the whole measurement: a buffered second read would come out
/// of the page cache at RAM speed and report both readers as equally, impossibly fast.
pub fn unbuffered_sample(path: &Path, limit: u64) -> Result<u64> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING)
        .open(path)
        .with_context(|| format!("opening {} to sample it", path.display()))?;

    let mut buffer = SectorBuf::new(VERIFY_CHUNK);
    let mut total = 0u64;

    while total < limit {
        let read = file
            .read(buffer.as_mut_slice())
            .with_context(|| format!("sampling {}", path.display()))?;

        if read == 0 {
            break;
        }
        total += read as u64;
    }

    Ok(total)
}

/// A heap buffer whose address is sector-aligned, which `FILE_FLAG_NO_BUFFERING`
/// requires and `Vec<u8>` does not promise.
///
/// Hand-rolled rather than taking `aligned-vec` (decision 29): one allocation carrying
/// one invariant, and the invariant is worth seeing at the place it is established.
struct SectorBuf {
    ptr: NonNull<u8>,
    len: usize,
}

impl SectorBuf {
    fn new(len: usize) -> Self {
        assert!(
            len > 0 && len.is_multiple_of(SECTOR),
            "an unbuffered read buffer must be a non-zero multiple of {SECTOR} bytes"
        );

        let layout = Layout::from_size_align(len, SECTOR).expect("a valid sector layout");

        // Zeroed rather than raw: `alloc` hands back uninitialized memory, and building
        // a `&mut [u8]` over that is undefined behavior even before anything reads it.
        // One memset per verify pass — not per file — is not a cost worth reasoning
        // around.
        //
        // SAFETY: `layout` has a non-zero size, asserted above.
        let ptr = unsafe { alloc::alloc_zeroed(layout) };

        Self {
            ptr: NonNull::new(ptr).unwrap_or_else(|| alloc::handle_alloc_error(layout)),
            len,
        }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` is valid for `len` initialized bytes from construction until
        // `Drop`, and `&mut self` guarantees this is the only live reference to it.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for SectorBuf {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.len, SECTOR).expect("a valid sector layout");

        // SAFETY: allocated in `new` with this exact layout, and freed exactly once.
        unsafe { alloc::dealloc(self.ptr.as_ptr(), layout) }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::hash::sha256;

    /// **Held to decision 18's own criterion rather than exceeding its budget.** That
    /// decision tests where a defect is irreversible, and a verify read that is quietly
    /// wrong is the worst such defect this tool can carry: it would report an SSD clean,
    /// the SSD would go in the safe, and nothing would ever say otherwise. These three
    /// cases are the ones where that could happen silently.
    ///
    /// **What none of these can prove, stated so nobody assumes otherwise: that the
    /// flags are still on.** Deleting `FILE_FLAG_NO_BUFFERING` leaves every test here
    /// passing, because bypassing the cache changes *where the bytes come from* and not
    /// what they are — which is the entire reason decision 2 had to reason about it
    /// rather than measure it. The same goes for `FILE_FLAG_WRITE_THROUGH`. Treat both
    /// constants as load-bearing on inspection; no assertion is standing behind them.
    ///
    /// The round trip, both flags at once. A file written through and read back
    /// unbuffered has to hash to what went in.
    #[test]
    fn a_file_written_through_reads_back_unbuffered_to_the_same_hash() {
        let dir = TempDir::new().expect("a scratch directory");
        let target = dir
            .path()
            .join("2026")
            .join("2026-08-03")
            .join("1422Z_1.CR3");

        let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        write_through(&target, &bytes).expect("writing through");

        assert_eq!(unbuffered_sha256(&target).unwrap(), sha256(&bytes));
    }

    /// The partial final sector — the exact shape decision 2 says makes unbuffered
    /// *writes* unimplementable, and which the read side must nevertheless handle. A
    /// raw file is never a sector multiple, so if this were broken it would be broken
    /// for every photograph rather than for an edge case.
    #[test]
    fn a_file_that_is_not_a_whole_number_of_sectors_verifies() {
        let dir = TempDir::new().expect("a scratch directory");

        for size in [1, SECTOR - 1, SECTOR + 1, VERIFY_CHUNK + 7] {
            let target = dir.path().join(format!("{size}.CR3"));
            let bytes: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();

            write_through(&target, &bytes).expect("writing through");
            assert_eq!(
                unbuffered_sha256(&target).unwrap(),
                sha256(&bytes),
                "{size} bytes"
            );
        }
    }

    /// A wrong byte must be caught. Without this the two tests above would still pass
    /// against a `unbuffered_sha256` that hashed nothing at all and returned a constant.
    #[test]
    fn a_single_flipped_byte_changes_the_verified_hash() {
        let dir = TempDir::new().expect("a scratch directory");
        let target = dir.path().join("flipped.CR3");

        let mut bytes: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        write_through(&target, &bytes).expect("writing through");
        let before = unbuffered_sha256(&target).unwrap();

        bytes[50_000] ^= 0x01;
        fs::remove_file(&target).expect("clearing the target");
        write_through(&target, &bytes).expect("rewriting");

        assert_ne!(before, unbuffered_sha256(&target).unwrap());
    }

    /// Temp-then-rename's whole purpose: nothing is left under the temp prefix, and the
    /// real name exists only once the write finished.
    #[test]
    fn no_temporary_file_survives_a_completed_write() {
        let dir = TempDir::new().expect("a scratch directory");
        let target = dir.path().join("done.CR3");

        write_through(&target, b"whatever").expect("writing through");

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("listing the directory")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(TEMP_PREFIX))
            .collect();

        assert!(target.exists());
        assert!(leftovers.is_empty(), "left behind {leftovers:?}");
    }
}
