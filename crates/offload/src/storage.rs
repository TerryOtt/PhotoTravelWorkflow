//! Windows storage identity: what is plugged in, and which physical device it is.
//!
//! This is what decisions 6 and 7 rest on, and decision 17 calls the genuinely new part
//! of this tool — it exists in no other language here and had to be written from the
//! Win32 API up.
//!
//! **The problem it solves: a drive letter is not an identity.** Windows reassigns
//! letters to external SSDs freely, so passing three letters that happen to be right
//! today is how two "copies" silently land on one physical disk. So a destination is
//! found by its **disk serial** — which survives a reformat, a letter change and a move
//! to another machine — with the **volume GUID** as a fast local index (decision 6). And
//! a card is found by *measurement* rather than by identity at all, because an in-camera
//! format assigns a new volume serial at the start of every session (decision 7).
//!
//! # Everything here runs unelevated
//!
//! Deliberate, and it constrains the implementation: the volume handles opened below ask
//! for **no access rights at all** (`access_mode(0)`), which is enough for the two
//! IOCTLs used here and needs no administrator. Asking for read access would work on
//! this machine and fail on a locked-down one, for a query that never reads a byte of
//! data.
//!
//! Binding constraint 4 is not a preference, and decision 9's Defender check is what it
//! costs: that check was **withdrawn** on 2026-08-07 because both ways of reading the
//! exclusion list throw on an unelevated process, and elevating was never on the table.
//! A capability that only works elevated does not exist for this tool's purposes — so the
//! access rights get designed down to what the query actually needs, here and everywhere.

// Denied here and allowed workspace-wide, because this is one of the two modules where a
// truncated integer becomes a wrong buffer length handed to `DeviceIoControl` against a real
// disk. Everywhere else the family reports display math. Use `size_u32` for a Win32 record
// and `u32::try_from` for a runtime length; both fail loudly instead of silently.
#![deny(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FILE_SHARE_READ, FILE_SHARE_WRITE, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose,
    GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW, GetVolumePathNamesForVolumeNameW,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
    STORAGE_DEVICE_DESCRIPTOR, STORAGE_DEVICE_NUMBER, STORAGE_PROPERTY_QUERY,
    StorageDeviceProperty,
};
use windows::core::PCWSTR;

/// The byte size of a Win32 record, as the `u32` its `cbSize` and buffer-length parameters
/// want.
///
/// Every caller passes a fixed Win32 struct of a few dozen bytes, so the conversion cannot
/// fail. It is written as a checked conversion anyway, because the failure it replaces is
/// the bad one: `size_of::<T>() as u32` truncates in silence, and a truncated length handed
/// to `DeviceIoControl` is a wrong buffer size against a disk bound for the safe. A panic
/// here would be loud, immediate, and impossible to mistake for success.
///
/// It also keeps `clippy::cast_possible_truncation` live at every other call site instead of
/// being switched off across two modules of unsafe FFI, which is where the lint earns its
/// place.
pub(crate) fn size_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("a Win32 record is far smaller than 4 GiB")
}

/// `GetDriveTypeW`'s answer for removable media — a card reader with a card in it.
const DRIVE_REMOVABLE: u32 = 2;

/// One mounted volume, as Windows sees it.
#[derive(Debug, Clone)]
pub struct Volume {
    /// `\\?\Volume{...}\`, with the trailing backslash Windows requires on the path
    /// forms that take a root. This is decision 6's fast local index.
    pub guid_path: String,
    /// Drive letters and mount points, usually one and sometimes none.
    pub mount_points: Vec<PathBuf>,
    pub label: Option<String>,
    pub filesystem: Option<String>,
    /// **Reassigned by every format**, which is exactly what makes it the identity of a
    /// *card generation* (decision 13) and useless as the identity of a disk. Rendered for
    /// humans by [`Volume::serial_text`].
    pub volume_serial: u32,
    /// `GetDriveTypeW` said `DRIVE_REMOVABLE`.
    ///
    /// **Do not use this to find camera cards, and do not trust it to mean anything
    /// about the medium.** Measured on the real rig: of two identical Canon cards in two
    /// readers on one hub, one enumerates removable and the other fixed — and all three
    /// archive SSDs enumerate fixed. It describes the enclosure's firmware. Decision 7
    /// finds cards by the presence of `DCIM`, and its correction note records what
    /// filtering on this field would have cost.
    pub removable: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

impl Volume {
    /// The volume serial as Windows itself prints it — `A4E2-91CC`.
    ///
    /// **A string rather than the raw `u32`, deliberately**, despite this project's habit of
    /// keeping types in JSON. This is an identifier, not a quantity: nothing sums or compares
    /// it arithmetically, and the hex-pair form is what `vol` shows, so an operator can
    /// cross-check a manifest against the machine. `2749763532` is technically typed and
    /// practically useless.
    pub fn serial_text(&self) -> String {
        format!(
            "{:04X}-{:04X}",
            self.volume_serial >> 16,
            self.volume_serial & 0xFFFF
        )
    }

    /// The form `CreateFileW` wants: the GUID path **without** its trailing backslash.
    ///
    /// The two forms are not interchangeable and the failure is not obvious — with the
    /// backslash you get a handle to the volume's root *directory*, and every IOCTL
    /// below then fails with a parameter error rather than anything that names the
    /// cause.
    pub fn device_path(&self) -> &str {
        self.guid_path.trim_end_matches('\\')
    }

    /// Open the volume for querying, asking for no access rights — see the module note.
    fn open(&self) -> Result<File> {
        OpenOptions::new()
            .access_mode(0)
            .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0)
            .open(self.device_path())
            .with_context(|| format!("opening volume {}", self.guid_path))
    }
}

/// The physical device a volume sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Windows' index for the physical disk. Two volumes reporting the same number are
    /// two partitions of one device, which is what makes decision 6's
    /// four-distinct-devices assertion exact rather than hopeful.
    pub disk_number: u32,
    /// The manufacturer's serial. `None` when the device or its bridge declines to
    /// report one — cheap USB enclosures do this, and the caller has to cope rather
    /// than assume.
    pub serial: Option<String>,
}

/// Every volume currently mounted, skipping any that cannot answer.
///
/// **A volume that fails to describe itself is skipped, not an error.** An empty card
/// reader slot enumerates as a volume and then reports `ERROR_NOT_READY` to everything —
/// it is the normal state of a two-slot reader with one card in it, and failing the run
/// over it would make the tool refuse to start on the rig it was built for.
pub fn volumes() -> Result<Vec<Volume>> {
    let mut found = Vec::new();
    let mut buffer = [0u16; 260];

    // SAFETY: `buffer` is a valid, writable slice for the length passed with it.
    let search = unsafe { FindFirstVolumeW(&mut buffer) }.context("enumerating volumes")?;
    let search = VolumeSearch(search);

    loop {
        if let Some(volume) = describe(&wide_to_string(&buffer)) {
            found.push(volume);
        }

        buffer.fill(0);
        // SAFETY: `search` is a live find handle and `buffer` is valid for its length.
        // `FindNextVolumeW` reports the end of the enumeration as an error, which is
        // the loop's exit rather than a failure.
        if unsafe { FindNextVolumeW(search.0, &mut buffer) }.is_err() {
            break;
        }
    }

    Ok(found)
}

/// Which physical device this volume is on, and its serial if it has one.
pub fn device_of(volume: &Volume) -> Result<Device> {
    let file = volume.open()?;
    let handle = HANDLE(file.as_raw_handle());

    Ok(Device {
        disk_number: disk_number(handle)
            .with_context(|| format!("reading the disk number of {}", volume.guid_path))?,
        serial: serial_number(handle),
    })
}

/// `IOCTL_STORAGE_GET_DEVICE_NUMBER` — cheap, and the whole of the distinctness answer.
///
/// `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` would be the thorough version, reporting every
/// device a volume spans. It is not used because a spanned or striped volume is not a
/// thing this rig has, and the extra call would buy a case that cannot occur while
/// costing a variable-length struct to unpack.
fn disk_number(handle: HANDLE) -> Result<u32> {
    let mut number = STORAGE_DEVICE_NUMBER::default();
    let mut returned = 0u32;

    // SAFETY: the output buffer is a correctly sized, correctly aligned
    // `STORAGE_DEVICE_NUMBER`, which is what this control code writes.
    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            None,
            0,
            Some(std::ptr::from_mut(&mut number).cast()),
            size_u32::<STORAGE_DEVICE_NUMBER>(),
            Some(&raw mut returned),
            None,
        )
    }?;

    Ok(number.DeviceNumber)
}

/// The device serial, or `None` if it does not report one.
///
/// Never an error: a missing serial is a fact about the enclosure, not a failure of the
/// run, and decision 6 already has to cope with it — it is why the volume GUID is kept
/// as a second identifier rather than being merely a fast path.
fn serial_number(handle: HANDLE) -> Option<String> {
    // `STORAGE_DEVICE_DESCRIPTOR` is a header followed by its strings, so the buffer has
    // to be larger than the struct and 8-aligned for the header to be read out of it.
    #[repr(C, align(8))]
    struct Descriptor([u8; 1024]);

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };

    let mut buffer = Descriptor([0; 1024]);
    let mut returned = 0u32;

    // SAFETY: `query` is a fully initialized input of the size given, and `buffer` is
    // valid and writable for its length.
    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(std::ptr::from_ref(&query).cast()),
            size_u32::<STORAGE_PROPERTY_QUERY>(),
            Some(buffer.0.as_mut_ptr().cast()),
            u32::try_from(buffer.0.len()).expect("the descriptor buffer is 1 KiB"),
            Some(&raw mut returned),
            None,
        )
    }
    .ok()?;

    // SAFETY: the call above filled at least a descriptor header, and `Descriptor` is
    // aligned for it.
    let descriptor = unsafe { &*buffer.0.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>() };

    // Zero means "this device reports no serial", which is a legal answer rather than
    // an offset to read.
    let offset = descriptor.SerialNumberOffset as usize;
    if offset == 0 || offset >= buffer.0.len() {
        return None;
    }

    // Trimmed of surrounding whitespace, which is padding, and **normalized no further**.
    // A serial is an opaque identity, and the temptation to tidy it is a real one: this
    // machine's NVMe reports
    // `0000_0000_0000_0000_0026_B768_6A0F_9005.` — underscores and a trailing period
    // included. Stripping punctuation would read nicer and could map two genuinely
    // different devices onto one string, which is the exact mistake decision 6 uses
    // serials to avoid. Whatever the device says is what gets stored and compared.
    let tail = &buffer.0[offset..];
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(tail.len());
    let serial = std::str::from_utf8(&tail[..end]).ok()?.trim();

    // Some bridges answer with padding and nothing else, which is a missing serial
    // wearing a costume.
    (!serial.is_empty()).then(|| serial.to_owned())
}

/// Everything Windows will say about one volume, or `None` if it will not say anything.
fn describe(guid_path: &str) -> Option<Volume> {
    let root = wide(guid_path);

    let mut label = [0u16; 256];
    let mut filesystem = [0u16; 64];
    let mut volume_serial = 0u32;

    // SAFETY: every buffer is valid for the length implied by the slice passed with it,
    // and `root` outlives the call.
    unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            Some(&mut label),
            Some(&raw mut volume_serial),
            None,
            None,
            Some(&mut filesystem),
        )
    }
    .ok()?;

    // SAFETY: `root` is a NUL-terminated wide string that outlives the call.
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };

    let mut total_bytes = 0u64;
    let mut free_bytes = 0u64;
    // SAFETY: both out-pointers are valid for the duration of the call.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            None,
            Some(&raw mut total_bytes),
            Some(&raw mut free_bytes),
        )
    }
    .ok()?;

    Some(Volume {
        guid_path: guid_path.to_owned(),
        mount_points: mount_points(guid_path),
        label: non_empty(wide_to_string(&label)),
        filesystem: non_empty(wide_to_string(&filesystem)),
        volume_serial,
        removable: drive_type == DRIVE_REMOVABLE,
        total_bytes,
        free_bytes,
    })
}

/// Drive letters and mount points for a volume — often one, sometimes none.
///
/// A volume with no mount point is normal rather than broken: Windows keeps recovery and
/// EFI partitions unmounted, and they enumerate here like anything else.
fn mount_points(guid_path: &str) -> Vec<PathBuf> {
    let name = wide(guid_path);
    let mut buffer = vec![0u16; 512];
    let mut needed = 0u32;

    // SAFETY: `name` is NUL-terminated and outlives the call; `buffer` is valid for its
    // length, and `needed` receives the length actually required.
    let queried = unsafe {
        GetVolumePathNamesForVolumeNameW(
            PCWSTR(name.as_ptr()),
            Some(&mut buffer),
            std::ptr::from_mut(&mut needed),
        )
    };

    if queried.is_err() {
        return Vec::new();
    }

    // A double-NUL-terminated list of NUL-terminated strings.
    buffer
        .split(|unit| *unit == 0)
        .take_while(|part| !part.is_empty())
        .map(|part| PathBuf::from(OsString::from_wide(part)))
        .collect()
}

/// Closes the volume enumeration handle however the loop above leaves it.
struct VolumeSearch(HANDLE);

impl Drop for VolumeSearch {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from `FindFirstVolumeW` and is closed exactly once.
        let _ = unsafe { FindVolumeClose(self.0) };
    }
}

/// A NUL-terminated wide string, as every `W` function here expects.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A wide buffer up to its first NUL.
fn wide_to_string(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

/// The volume whose GUID path or mount point matches `path`, for resolving a configured
/// destination that is identified by path rather than by hardware (decision 11).
pub fn volume_containing(path: &std::path::Path) -> Result<Volume> {
    let volumes = volumes()?;

    volumes
        .into_iter()
        .filter(|volume| {
            volume
                .mount_points
                .iter()
                .any(|mount| path.starts_with(mount))
        })
        // The longest mount point wins, so a volume mounted inside another's directory
        // is preferred over the one it is nested in.
        .max_by_key(|volume| {
            volume
                .mount_points
                .iter()
                .filter(|mount| path.starts_with(mount))
                .map(|mount| mount.as_os_str().len())
                .max()
                .unwrap_or(0)
        })
        .ok_or_else(|| anyhow!("no mounted volume contains {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `cbSize` and buffer-length argument in this module and in `eject` now goes
    /// through [`size_u32`], so a wrong answer here is a wrong length handed to
    /// `DeviceIoControl` against a real disk. Asserted against the records actually passed
    /// rather than a stand-in type, because the point is that these specific structs
    /// convert exactly and cannot make the `expect` fire.
    #[test]
    fn size_u32_converts_every_win32_record_it_is_used_on_exactly() {
        assert_eq!(
            usize::try_from(size_u32::<STORAGE_DEVICE_NUMBER>()).expect("a u32 fits a usize"),
            size_of::<STORAGE_DEVICE_NUMBER>(),
        );
        assert_eq!(
            usize::try_from(size_u32::<STORAGE_PROPERTY_QUERY>()).expect("a u32 fits a usize"),
            size_of::<STORAGE_PROPERTY_QUERY>(),
        );
        // A zero-length buffer would be accepted by `DeviceIoControl` and return nothing,
        // which is the failure this whole helper exists to make impossible to reach quietly.
        assert!(size_u32::<STORAGE_DEVICE_NUMBER>() > 0);
    }

    /// **These run against whatever is plugged into the machine**, which is unusual for
    /// a unit test and is the point: this module's entire job is to describe real
    /// hardware, and a mock would only prove the mock. They assert invariants that hold
    /// on any Windows machine rather than anything about this rig, so they stay true on
    /// a CI runner with one virtual disk.
    #[test]
    fn every_enumerated_volume_describes_itself_consistently() {
        for volume in volumes().expect("enumeration must succeed") {
            assert!(
                volume.guid_path.starts_with(r"\\?\Volume{") && volume.guid_path.ends_with('\\'),
                "{:?} is not a GUID path with its trailing backslash",
                volume.guid_path
            );
            assert!(
                !volume.device_path().ends_with('\\'),
                "the CreateFileW form must not keep the trailing backslash"
            );
            assert!(
                volume.free_bytes <= volume.total_bytes,
                "{} reports more free than total",
                volume.guid_path
            );
        }
    }

    /// The system volume is always present, always mounted, and always on a device that
    /// reports a disk number — so this exercises the full path through `DeviceIoControl`
    /// on any machine, including a CI runner.
    #[test]
    fn the_volume_holding_the_current_directory_resolves_to_a_device() {
        let here = std::env::current_dir().expect("a current directory");
        let volume = volume_containing(&here).expect("its volume must be found");

        assert!(
            !volume.mount_points.is_empty(),
            "a volume we resolved by path must have a mount point"
        );

        let device = device_of(&volume).expect("its device must be queryable unelevated");
        assert_eq!(
            device,
            device_of(&volume).expect("a second query"),
            "device identity must be stable across calls"
        );
    }
}
