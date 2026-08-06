//! Eject — the only code here that dismounts a live volume.
//!
//! Decision 22. The nightly ritual used to end with three trips to the tray icon, usually
//! twice per device out of fear that bytes were still sitting in a cache. **By the time
//! this runs that fear is structurally dead**: writes were write-through and every byte was
//! read back off the media (decision 2), so ejection *confirms* persistence rather than
//! providing it.
//!
//! # Three steps, in this order, and none of them is optional
//!
//! 1. **Lock** (`FSCTL_LOCK_VOLUME`), retried with backoff. Windows refuses the lock while
//!    anything holds an open handle, and immediately after a run something usually does —
//!    Defender scanning freshly written files, or the search indexer. The retry window is
//!    what turns "eject failed" into "eject waited".
//! 2. **Dismount** (`FSCTL_DISMOUNT_VOLUME`), which flushes and detaches the filesystem.
//! 3. **Power down** (`CM_Request_Device_Eject`), which is exactly what the tray icon does.
//!
//! **All three run for camera cards too, which is a correction.** They used to get steps 1
//! and 2 only, on the reasoning that a card is pulled from a reader that stays put. A
//! dismount releases nothing — it detaches a filesystem and leaves the volume and its drive
//! letter, and Windows remounts on next access — so both cards sat in the tray after every
//! run that claimed to have settled them. Decision 22 has the measurement.
//!
//! # Why this is not simply `IOCTL_STORAGE_EJECT_MEDIA`
//!
//! That control code ejects *media* from a drive — a disc from an optical drive, a card
//! from a reader. A USB or Thunderbolt SSD reports non-removable media in a removable
//! enclosure, so it succeeds at nothing. Powering the enclosure down is a *device* operation
//! and belongs to the configuration manager, which is why this walks to the device node.
//!
//! **That paragraph is right about SSDs and was wrong to stop there, which cost real time.**
//! It names a card in a reader as the case where media eject *does* work, so the card path
//! inherited an exclusion argued for a different device — and nobody checked. Measured
//! 2026-08-05: media eject reports success and releases neither card, and the physical-drive
//! handle that might have behaved differently needs administrator rights (binding constraint
//! 4). See [`eject_media`], which is kept as a harness rather than a code path.
//!
//! # A refused eject is a result, not a failure
//!
//! The data guarantees were settled before eject was attempted, so nothing here can
//! downgrade them. A volume something else is holding is named per device and the verdict
//! says *eject it by hand* (decision 14). This module therefore returns an [`Outcome`] and
//! reserves `Err` for the cases where it could not even ask.

use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Parent, CM_Request_Device_EjectW, CR_SUCCESS, PNP_VETO_TYPE,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME};

use crate::storage::{Device, Volume};

/// Lock is retried for this long. Defender finishing with a few hundred freshly written
/// gigabytes is the case this exists for, and it is measured in seconds rather than
/// milliseconds.
const LOCK_WINDOW: Duration = Duration::from_secs(30);

/// Between lock attempts. Long enough not to spin, short enough that a lock released early
/// is taken promptly.
const LOCK_RETRY: Duration = Duration::from_millis(500);

/// The first pause between whole-sequence attempts, doubling from here.
///
/// **Dismounting releases the volume, and Windows remounts it eagerly.** The lock lives on
/// the handle, so closing the handle — which must happen, or this process is itself the
/// outstanding open — also drops the exclusivity that made the volume ejectable. Anything
/// that reopens it in that window turns the power-down into `PNP_VetoOutstandingOpen`
/// naming the volume itself rather than any application. Retrying only
/// `CM_Request_Device_Eject` would then ask the same question of a volume that has since
/// remounted; the lock and dismount have to be redone with it.
///
/// On 2026-08-04 two of three archive SSDs were vetoed on their single attempt, and the
/// operator's own long-standing workaround — recorded in decision 22 — was pressing the
/// tray icon a second time.
const FIRST_BACKOFF: Duration = Duration::from_secs(2);

/// The pause stops doubling here.
///
/// What the backoff waits out is a scanner working through a few hundred freshly written
/// gigabytes, which is a minutes-scale problem — but an unbounded doubling would spend the
/// last hour of the budget asleep, so it flattens into steady polling instead. Sixty
/// seconds is frequent enough that a volume released early is taken promptly, and rare
/// enough that a long wait costs a handful of attempts rather than thousands.
const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// What happened to one device.
///
/// **Not a `Result`**, because a refusal is an outcome the report describes rather than an
/// error that aborts anything — decision 22 is explicit that a refused eject downgrades
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Locked, dismounted, powered down. Safe to unplug and put in the safe.
    Ejected,
    /// Dismounted — the filesystem is flushed and detached, so the bytes are safe — but the
    /// enclosure declined to power down. Distinguished from [`Outcome::Held`] because the
    /// data is in a different state, even though both end with the operator at the tray.
    Dismounted { reason: String },
    /// Something held the volume for the whole retry window. Still mounted, still safe,
    /// still to be ejected by hand.
    Held { reason: String },
}

impl Outcome {
    /// Whether Windows powered the device down, which is the only case the verdict may
    /// call `EJECTED`.
    pub fn is_ejected(&self) -> bool {
        matches!(self, Outcome::Ejected)
    }
}

// **There is deliberately no `CardOutcome` type any more, and its deletion is the tidy half
// of a bug.** It existed because a card was thought to end somewhere a destination does not:
// an SSD is unplugged whole, so success meant the enclosure powered down, while a card was
// pulled from a reader that stayed put, so success supposedly meant *dismounted and nothing
// more*. Sharing one enum would then have made `Dismounted` mean failure for one and success
// for the other — a real argument, resting on a premise measured false on 2026-08-05.
//
// A dismount releases nothing. Both kinds of device reach the same three states by the same
// three calls, so they share [`Outcome`]: `Ejected` is released for either. What genuinely
// differs is the *instruction* — unplug an enclosure, pull a card, replug a reader — and that
// belongs in the report, which is where it now lives.

/// Lock and dismount, and stop there. **A harness step, not a code path.**
///
/// Nothing in the run calls this: it is the *first* rung of the escalation
/// `examples/release-cards.rs` walks, kept so the bug it reproduces stays reproducible.
/// **It releases nothing** — a dismount detaches the filesystem and leaves the volume and its
/// drive letter, and Windows remounts on next access — which is precisely what made it look
/// like a working card eject for as long as it did.
///
/// [`eject`] is the real sequence; this is the same first two steps without the third.
pub fn dismount_only(volume: &Volume) -> Result<Outcome> {
    let file = open_for_control(volume.device_path())
        .with_context(|| format!("opening {} to dismount", volume.guid_path))?;

    let handle = HANDLE(file.as_raw_handle());

    if let Err(error) = lock_with_backoff(handle) {
        return Ok(Outcome::Held {
            reason: format!("{error:#}"),
        });
    }

    match control(handle, FSCTL_DISMOUNT_VOLUME) {
        Ok(()) => Ok(Outcome::Dismounted {
            reason: "dismounted only — no device eject was attempted".into(),
        }),
        Err(error) => Ok(Outcome::Held {
            reason: format!("locked, but dismount failed: {error:#}"),
        }),
    }
}

/// Ask the drive to eject its **media**. **Measured useless on this rig, and kept anyway.**
///
/// > **Nothing in the run calls this.** It was written as the obvious fix for cards that a
/// > dismount would not release, and on 2026-08-05 it **returned success on both cards and
/// > released neither** — including the SD, which advertises `Supports Removable Media`. That
/// > is the "succeeds at nothing" outcome this module's own doc predicted for SSDs, observed
/// > on the device the same sentence held up as the case where it works.
/// >
/// > The obvious objection — that a media operation belongs on the physical drive rather than
/// > on a volume — is chased by [`eject_media_on_disk`], and closes the question from the
/// > other side: that handle needs administrator rights, which binding constraint 4 forbids.
/// > **So `CM_Request_Device_Eject` is the only mechanism an unelevated process has.**
///
/// It stays in the tree, exercised by `examples/release-cards.rs`, for the same reason the
/// hash experiments do: a future rig with a different reader can **re-measure** the claim
/// rather than re-argue it. Delete it only alongside that harness.
///
/// **The lock is released first, deliberately.** `IOCTL_STORAGE_MEDIA_REMOVAL` with
/// `PreventMediaRemoval = FALSE` clears any software lock a previous holder set — a lock
/// nothing here sets, but which a card reader or another application may have — and a
/// failure to clear it is not worth aborting over, so it is attempted and ignored.
pub fn eject_media(volume: &Volume) -> Result<()> {
    let file = open_for_control(volume.device_path())
        .with_context(|| format!("opening {} to eject its media", volume.guid_path))?;

    eject_media_on(HANDLE(file.as_raw_handle()))
        .with_context(|| format!("ejecting the media in {}", volume.guid_path))
}

/// The same request, addressed to the **physical drive** rather than to a volume on it.
///
/// **Which object receives this is not a detail, and assuming it was cost a wrong
/// conclusion.** On 2026-08-05 the volume-handle form above returned success on both cards
/// and released neither, and that was nearly written up as *media eject does nothing here*
/// before anyone asked whether the request had been addressed to the right thing.
/// `\\.\PhysicalDriveN` is the conventional target, since ejecting a medium is a property of
/// the drive rather than of any filesystem mounted from it. **Both spellings exist so the
/// difference can be measured instead of argued** — the same reason `examples/` carries the
/// harnesses for every other number in this project.
pub fn eject_media_on_disk(disk_number: u32) -> Result<()> {
    let path = format!(r"\\.\PhysicalDrive{disk_number}");

    let file =
        open_for_control(&path).with_context(|| format!("opening {path} to eject its media"))?;

    eject_media_on(HANDLE(file.as_raw_handle()))
        .with_context(|| format!("ejecting the media in {path}"))
}

/// Allow removal, then eject — the media sequence, on whichever handle the caller opened.
fn eject_media_on(handle: HANDLE) -> Result<()> {
    use windows::Win32::System::Ioctl::{
        IOCTL_STORAGE_EJECT_MEDIA, IOCTL_STORAGE_MEDIA_REMOVAL, PREVENT_MEDIA_REMOVAL,
    };

    // Best effort, for the reason in the doc comment: this clears somebody else's lock, and
    // its absence is the normal case rather than a problem.
    let allow = PREVENT_MEDIA_REMOVAL {
        PreventMediaRemoval: false,
    };
    let mut returned = 0u32;
    // SAFETY: `allow` is a fully initialized input of exactly the size given, it outlives
    // the call as a local, and this control code writes no output.
    let _ = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_MEDIA_REMOVAL,
            Some(std::ptr::from_ref(&allow).cast()),
            size_of::<PREVENT_MEDIA_REMOVAL>() as u32,
            None,
            0,
            Some(&mut returned),
            None,
        )
    };

    control(handle, IOCTL_STORAGE_EJECT_MEDIA)
}

/// Open a volume or a physical drive for the control codes in this module.
///
/// **Write access is what `FSCTL_LOCK_VOLUME` requires**, so the query-only handle
/// `storage::Volume::open` hands out cannot be reused here. Sharing stays permissive because
/// the *lock*, not the open, is what has to win exclusivity — failing the open instead would
/// report the wrong thing entirely.
fn open_for_control(path: &str) -> Result<std::fs::File> {
    const GENERIC_READ_WRITE: u32 = 0x8000_0000 | 0x4000_0000;
    const FILE_SHARE_READ_WRITE: u32 = 0x0000_0001 | 0x0000_0002;

    Ok(std::fs::OpenOptions::new()
        .access_mode(GENERIC_READ_WRITE)
        .share_mode(FILE_SHARE_READ_WRITE)
        .open(path)?)
}

/// Power down the device behind a physical disk number — the tray icon's own call.
///
/// Public because releasing a **card** may need it: a CFexpress behind a Thunderbolt reader
/// enumerates as a fixed NVMe disk with no separable medium, so a device eject is the only
/// call that removes it. Decision 22 treats that as a deliberate exception to
/// *cards are never powered down*, not as the general rule, because the device it reaches is
/// the reader.
pub fn power_down_disk(disk_number: u32) -> Result<()> {
    power_down(disk_number)
}

/// How one device's eject ended, and what it cost to get there.
///
/// **The cost is reported rather than discarded** because eject is the least predictable
/// part of the run and the only way to make it better is to know how hard it actually had
/// to work. A device that powers down on the second attempt after nine seconds and one
/// that takes forty attempts over twenty minutes are the same [`Outcome`] and very
/// different facts.
#[derive(Debug, Clone)]
pub struct Effort {
    pub outcome: Outcome,
    /// Full lock → dismount → power-down passes made, always at least one.
    pub attempts: u32,
    /// Wall clock across all of them.
    pub waited: Duration,
}

/// Lock, dismount and power down one archive SSD, retrying the whole sequence until
/// `deadline`.
///
/// `Err` only when the volume could not be opened at all — everything after that is an
/// [`Outcome`], and the one reported is the last attempt's. Retrying stops the moment a
/// device powers down, so the common case costs a single pass and no waiting.
///
/// **One attempt always happens, even past the deadline.** A run that has already spent the
/// whole budget still gets its ejects tried once, because refusing to attempt at all would
/// turn a slow night into a manual one for no gain.
///
/// The caller owns the deadline because the caller owns the budget — see decision 22 and
/// `RUN_BUDGET` in the binary. This module deliberately knows nothing about dinner.
pub fn eject(volume: &Volume, device: &Device, deadline: Instant) -> Result<Effort> {
    let started = Instant::now();
    let mut backoff = FIRST_BACKOFF;
    let mut attempts = 0;

    loop {
        attempts += 1;
        let outcome = attempt(volume, device)?;

        // Tested after the attempt rather than before, so the deadline bounds how long this
        // keeps *trying* rather than whether it tries at all. Adding `backoff` is what stops
        // it sleeping past the deadline only to give up on waking.
        if outcome.is_ejected() || Instant::now() + backoff >= deadline {
            return Ok(Effort {
                outcome,
                attempts,
                waited: started.elapsed(),
            });
        }

        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// One pass: open, lock, dismount, release the handle, ask the enclosure to leave.
///
/// **Public so a diagnostic can time the veto without reimplementing the sequence.**
/// [`eject`] reports only its *last* attempt, which is the right thing for a report and
/// useless for the open question of what holds a camera card for eleven minutes — whether the
/// veto changes over that window or the same one is simply eventually won.
/// `examples/card-veto-watch.rs` is the caller, and it exists because a harness that
/// approximated this sequence would be measuring itself rather than the tool.
pub fn attempt(volume: &Volume, device: &Device) -> Result<Outcome> {
    let file = open_for_control(volume.device_path())
        .with_context(|| format!("opening {} for eject", volume.guid_path))?;

    let handle = HANDLE(file.as_raw_handle());

    if let Err(error) = lock_with_backoff(handle) {
        return Ok(Outcome::Held {
            reason: format!("{error:#}"),
        });
    }

    if let Err(error) = control(handle, FSCTL_DISMOUNT_VOLUME) {
        return Ok(Outcome::Held {
            reason: format!("locked, but dismount failed: {error:#}"),
        });
    }

    // The handle must go before the device is asked to leave, or this process is itself the
    // thing holding it open.
    drop(file);

    match power_down(device.disk_number) {
        Ok(()) => Ok(Outcome::Ejected),
        Err(error) => Ok(Outcome::Dismounted {
            reason: format!("{error:#}"),
        }),
    }
}

/// `FSCTL_LOCK_VOLUME`, retried until [`LOCK_WINDOW`] is spent.
///
/// **The retry is the whole point.** A single attempt immediately after phase 3 wrote 200 GB
/// will usually lose to Defender, and reporting that as a refusal would send the operator to
/// the tray icon for a lock that was about to be available anyway.
fn lock_with_backoff(handle: HANDLE) -> Result<()> {
    let deadline = Instant::now() + LOCK_WINDOW;
    let mut last = None;

    while Instant::now() < deadline {
        match control(handle, FSCTL_LOCK_VOLUME) {
            Ok(()) => return Ok(()),
            Err(error) => last = Some(error),
        }
        std::thread::sleep(LOCK_RETRY);
    }

    Err(anyhow!(
        "the volume was still held after {}s — something has files open on it ({})",
        LOCK_WINDOW.as_secs(),
        last.map_or_else(|| "no error reported".into(), |error| format!("{error:#}"))
    ))
}

/// A `DeviceIoControl` that takes and returns nothing, which is both FSCTLs used here.
fn control(handle: HANDLE, code: u32) -> Result<()> {
    let mut returned = 0u32;

    // SAFETY: both control codes take no input and write no output, so passing `None` for
    // both buffers is the documented shape. `handle` is a live volume handle.
    unsafe { DeviceIoControl(handle, code, None, 0, None, 0, Some(&mut returned), None) }
        .map_err(Into::into)
}

/// `CM_Request_Device_Eject` on the device node behind `disk_number` — the tray icon's own
/// call.
///
/// **The disk's own device node is not the one to eject.** That node is the volume's storage
/// device; the thing that can be powered down is its parent, the USB or Thunderbolt
/// enclosure. So this walks up from the disk and asks the parent to leave, which is what
/// "safely remove hardware" does.
fn power_down(disk_number: u32) -> Result<()> {
    let disk = devnode_for_disk(disk_number)?;

    let mut parent = 0u32;
    // SAFETY: `disk` is a devnode this process just located; `parent` is written on success.
    let status = unsafe { CM_Get_Parent(&mut parent, disk, 0) };
    if status != CR_SUCCESS {
        return Err(anyhow!(
            "could not find the enclosure behind disk {disk_number} (CM_Get_Parent: {status:?})"
        ));
    }

    // Windows fills this in with *what* vetoed the eject, which is far more useful to an
    // operator than a status code — it names the process or driver holding on.
    let mut veto_type = PNP_VETO_TYPE::default();
    let mut veto_name = [0u16; 260];

    // SAFETY: both out-parameters are correctly sized for the lengths given.
    let status =
        unsafe { CM_Request_Device_EjectW(parent, Some(&mut veto_type), Some(&mut veto_name), 0) };

    if status == CR_SUCCESS {
        return Ok(());
    }

    let name = String::from_utf16_lossy(&veto_name);
    let name = name.trim_end_matches('\0');
    Err(anyhow!(
        "Windows declined to power the device down ({status:?}, {veto_type:?}{})",
        if name.is_empty() {
            String::new()
        } else {
            format!(", held by {name}")
        }
    ))
}

/// The device node for a physical disk number.
///
/// Windows exposes no direct disk-number-to-devnode call, so this goes the way every
/// safely-remove implementation goes: enumerate the disk device interfaces, ask each one
/// which disk number it is, and take the devnode of the one that matches.
fn devnode_for_disk(disk_number: u32) -> Result<u32> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVINFO_DATA,
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW,
    };
    use windows::Win32::System::Ioctl::GUID_DEVINTERFACE_DISK;

    // SAFETY: a null enumerator and parent are the documented "everything present" query.
    let set = unsafe {
        SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_DISK),
            None,
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    }
    .context("enumerating disk device interfaces")?;

    let mut found = None;
    for index in 0.. {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };

        // SAFETY: `set` is live and `interface` is correctly sized; a false return ends the
        // enumeration, which is the documented termination condition.
        if unsafe {
            SetupDiEnumDeviceInterfaces(set, None, &GUID_DEVINTERFACE_DISK, index, &mut interface)
        }
        .is_err()
        {
            break;
        }

        // The detail struct is a header followed by a variable-length path, so it is asked
        // for twice: once for the size, once for the data.
        let mut needed = 0u32;
        // SAFETY: passing no output buffer is how the required size is requested; the error
        // this returns is expected and ignored.
        let _ = unsafe {
            SetupDiGetDeviceInterfaceDetailW(set, &interface, None, 0, Some(&mut needed), None)
        };
        if needed == 0 {
            continue;
        }

        let mut buffer = vec![0u8; needed as usize];
        let detail = buffer.as_mut_ptr().cast::<
            windows::Win32::Devices::DeviceAndDriverInstallation::SP_DEVICE_INTERFACE_DETAIL_DATA_W,
        >();
        // SAFETY: `buffer` is at least `needed` bytes and 4-aligned by `Vec<u8>`'s
        // allocation; `cbSize` is the header size the API expects, not the buffer size.
        unsafe {
            (*detail).cbSize =
                size_of::<windows::Win32::Devices::DeviceAndDriverInstallation::SP_DEVICE_INTERFACE_DETAIL_DATA_W>()
                    as u32;
        }

        let mut info = SP_DEVINFO_DATA {
            cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };

        // SAFETY: `detail` points at a correctly sized, correctly initialized header.
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                Some(detail),
                needed,
                None,
                Some(&mut info),
            )
        }
        .is_err()
        {
            continue;
        }

        // SAFETY: `DevicePath` is a NUL-terminated wide string inside `buffer`.
        let path = unsafe { widestring_at(std::ptr::addr_of!((*detail).DevicePath).cast()) };

        if disk_number_of(Path::new(&path)).is_ok_and(|number| number == disk_number) {
            found = Some(info.DevInst);
            break;
        }
    }

    // SAFETY: `set` came from `SetupDiGetClassDevsW` and is not used after this.
    let _ = unsafe { SetupDiDestroyDeviceInfoList(set) };

    found.ok_or_else(|| anyhow!("no present disk device reports disk number {disk_number}"))
}

/// Read a NUL-terminated wide string.
///
/// # Safety
///
/// `ptr` must point at a NUL-terminated UTF-16 sequence.
unsafe fn widestring_at(ptr: *const u16) -> String {
    let mut length = 0;
    // SAFETY: the caller guarantees a NUL terminator, which bounds this walk.
    while unsafe { *ptr.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: `length` units are initialized, per the walk above.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(ptr, length) })
}

/// Which physical disk a device interface path belongs to.
fn disk_number_of(path: &Path) -> Result<u32> {
    use windows::Win32::System::Ioctl::{IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER};

    let file = std::fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(0x0000_0001 | 0x0000_0002)
        .open(path)?;

    let mut number = STORAGE_DEVICE_NUMBER::default();
    let mut returned = 0u32;

    // SAFETY: the output buffer is a correctly sized, correctly aligned
    // `STORAGE_DEVICE_NUMBER`, which is what this control code writes.
    unsafe {
        DeviceIoControl(
            HANDLE(file.as_raw_handle()),
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            None,
            0,
            Some(std::ptr::from_mut(&mut number).cast()),
            size_of::<STORAGE_DEVICE_NUMBER>() as u32,
            Some(&mut returned),
            None,
        )
    }?;

    Ok(number.DeviceNumber)
}
