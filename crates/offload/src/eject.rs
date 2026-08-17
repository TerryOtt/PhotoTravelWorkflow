//! Eject — the only code here that dismounts a live volume.
//!
//! Decision 22. **By the time this runs, the fear it used to answer is structurally dead**:
//! writes were write-through and every byte was read back off the media (decision 2), so
//! ejection *confirms* persistence rather than providing it.
//!
//! # Three steps, and when each one happens
//!
//! 1. **Lock** (`FSCTL_LOCK_VOLUME`), retried with backoff — Windows refuses while anything
//!    holds an open handle, and immediately after a run something usually does. The retry
//!    window is what turns "eject failed" into "eject waited".
//! 2. **Dismount** (`FSCTL_DISMOUNT_VOLUME`), which flushes and detaches the filesystem.
//! 3. **Power down** (`CM_Request_Device_Eject`), which is what the tray icon does.
//!
//! **Steps 1 and 2 run on the FIRST attempt only** — see [`Prepare`]. Repeating them means never
//! asking about a *settled* volume, and exFAT refuses a freshly remounted one forever.
//!
//! **All three apply to camera cards too**, which once got steps 1 and 2 only on the reasoning
//! that a card is pulled from a reader that stays put. A dismount releases nothing, so both cards
//! sat in the tray after every run that claimed to have settled them.
//!
//! # Why this is not simply `IOCTL_STORAGE_EJECT_MEDIA`
//!
//! That ejects *media* from a drive. An SSD reports non-removable media in a removable
//! enclosure, so it succeeds at nothing; powering the enclosure down is a *device* operation
//! belonging to the configuration manager, which is why this walks to the device node.
//!
//! **A card in a reader looks like the exception and is not.** Measured 2026-08-05: media eject
//! reports success and releases neither card, and the handle that might have behaved differently
//! needs administrator rights (binding constraint 4). [`eject_media`] is a harness, not a code
//! path.
//!
//! # A refused eject is a result, not a failure
//!
//! The data guarantees were settled before eject was attempted, so nothing here can downgrade
//! them. This module returns an [`Outcome`] and reserves `Err` for the cases where it could not
//! even ask.

// Denied here and allowed workspace-wide — see the note in `storage.rs`. This module calls
// `DeviceIoControl`, `SetupDi*` and `CM_Request_Device_Eject`, so a truncated length is a
// wrong-sized buffer aimed at a disk on its way to the safe.
#![deny(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

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

use crate::storage::{Device, Volume, size_u32};

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

/// The pause after losing the dismount-to-eject race, rather than the usual backoff.
///
/// **[`Veto::OutstandingOpen`] is not a device refusing; it is this tool arriving a moment
/// late.** [`attempt`] must close its handle before asking the device to leave — the lock lives
/// on that handle — and in the window between the two, Windows remounts the volume eagerly and
/// anything at all may open it. Waiting seconds to try again is backwards: nothing is *busy*,
/// a moment was *missed*, and the next moment is along directly.
///
/// **Measured support, 2026-08-06:** the same SanDisk released in 11 s on the 2 s backoff and
/// took 15 m 12 s at a flat 300 s gap — 83× worse for asking less often. Whatever holds these
/// volumes opens and closes on a short cycle, so the fast path is the one that catches it.
const RACE_RETRY: Duration = Duration::from_millis(250);

/// How many consecutive race retries before the normal backoff resumes.
///
/// **Bounded, because "we lost a race" and "something reopens this volume constantly" produce
/// the same veto.** Unbounded fast retries against the second case would spin the whole budget
/// away at four attempts a second and starve the patient path that actually works on
/// [`Veto::Device`]. Eight is two seconds of trying hard before going back to waiting.
const RACE_RETRIES: u32 = 8;

/// The pause stops doubling here.
///
/// What the backoff waits out is a scanner working through a few hundred freshly written
/// gigabytes, which is a minutes-scale problem — but an unbounded doubling would spend the
/// last hour of the budget asleep, so it flattens into steady polling instead. Sixty
/// seconds is frequent enough that a volume released early is taken promptly, and rare
/// enough that a long wait costs a handful of attempts rather than thousands.
const MAX_BACKOFF: Duration = Duration::from_mins(1);

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
    ///
    /// **`veto` says *which* refusal it was**, and the retry branches on it:
    /// [`Veto::OutstandingOpen`] is a race this tool lost and can retry almost immediately,
    /// where [`Veto::Device`] is a device stack that wants waiting out.
    Dismounted { veto: Veto, reason: String },
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
        // `Veto::Other` because nothing vetoed: no eject was asked for. The field describes a
        // refusal, and there was none to describe.
        Ok(()) => Ok(Outcome::Dismounted {
            veto: Veto::Other,
            reason: "dismounted only — no device eject was attempted".into(),
        }),
        Err(error) => Ok(Outcome::Held {
            reason: format!("locked, but dismount failed: {error:#}"),
        }),
    }
}

/// Ask the drive to eject its **media**. **Measured useless on this rig, and kept anyway.**
///
/// > **Nothing in the run calls this.** Written as the obvious fix for cards a dismount would not
/// > release, it **returned success on both cards and released neither** (2026-08-05) — including
/// > the SD, which advertises `Supports Removable Media`.
/// >
/// > The obvious objection — that a media operation belongs on the physical drive rather than on
/// > a volume — is chased by [`eject_media_on_disk`] and closes the question from the other side:
/// > that handle needs administrator rights, which binding constraint 4 forbids. **So
/// > `CM_Request_Device_Eject` is the only mechanism an unelevated process has.**
///
/// It stays, exercised by `examples/release-cards.rs`, so a future rig with a different reader
/// can **re-measure** the claim rather than re-argue it. Delete it only alongside that harness.
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
            size_u32::<PREVENT_MEDIA_REMOVAL>(),
            None,
            0,
            Some(&raw mut returned),
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
/// Whether this failure means the volume no longer exists — i.e. that it is already gone.
///
/// **Separate and named because the distinction is the whole defect.** *Could not open it* and
/// *it is not there* arrive through the same `Err`, and collapsing them made a released device
/// report as held.
fn missing_volume(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
}

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
    power_down(disk_number).map_err(|refusal| anyhow!("{}", refusal.detail))
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
    /// Full lock → dismount → power-down passes made.
    ///
    /// At least one from [`eject`], which always tries once even past the deadline. A caller
    /// that could not resolve a device to attempt against at all reports **zero**, and the
    /// distinction matters: *asked once and refused* and *never asked* look identical in a
    /// report that collapses them.
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
/// One attempt, handed to the caller the moment it resolves.
///
/// **The retry is otherwise invisible while it happens.** [`Effort`] reports the last attempt
/// and a total, both only once the fight is over — so an eleven-minute hold looks identical to
/// a hang, and the forty vetoes that preceded the win are never seen at all. Terry asked for
/// the live version on 2026-08-06, and it earns its keep twice: he gets to watch the tool
/// out-wait Windows, and the transcript answers **whether the veto changes across the window**,
/// which is the open question no completed run has been able to speak to.
#[derive(Debug)]
pub struct Attempt<'a> {
    /// 1-based.
    pub number: u32,
    pub outcome: &'a Outcome,
    /// Since the first attempt began.
    pub elapsed: Duration,
    /// The pause before the next attempt, or `None` when this was the last one.
    pub retry_in: Option<Duration>,
}

/// How long to wait between whole-sequence attempts.
///
/// **This exists to ask a question the retry loop otherwise makes unaskable.** Attempts and
/// elapsed time are welded together by that loop — it increments one *by* spending the other —
/// so no production run can separate *a long hold needs many attempts* from **many attempts
/// cause the long hold**. Terry raised the second possibility on 2026-08-06 and it has a
/// documented mechanism sitting in this very module: every attempt dismounts the volume and
/// must close its handle, Windows remounts eagerly, and the next attempt then puts its question
/// to a volume that has just come back online — which is exactly when a scanner takes interest.
///
/// **Two runs over the same corpus at different cadences separate them.** If asking is neutral,
/// both take about the same wall clock and the patient one merely has coarser resolution. If
/// asking is harmful, the patient one releases *sooner* while asking a fraction as often, and
/// nothing else produces that signature.
#[derive(Debug, Clone, Copy)]
pub enum Cadence {
    /// [`FIRST_BACKOFF`] doubling to [`MAX_BACKOFF`]. The default, and what every recorded run
    /// so far used — so it is the baseline any comparison has to be made against.
    Backoff,
    /// A fixed pause between attempts. Diagnostic; see the type note.
    Every(Duration),
}

impl Cadence {
    /// The first pause, before any doubling.
    fn first(self) -> Duration {
        match self {
            Cadence::Backoff => FIRST_BACKOFF,
            Cadence::Every(gap) => gap,
        }
    }

    /// The pause after `previous`.
    fn next(self, previous: Duration) -> Duration {
        match self {
            Cadence::Backoff => (previous * 2).min(MAX_BACKOFF),
            Cadence::Every(gap) => gap,
        }
    }
}

/// `watch` sees every attempt as it resolves — see [`Attempt`]. Pass `|_| {}` to ignore them.
pub fn eject(
    volume: &Volume,
    device: &Device,
    deadline: Instant,
    cadence: Cadence,
    prepare: Prepare,
    mut watch: impl FnMut(Attempt<'_>),
) -> Result<Effort> {
    let started = Instant::now();
    let mut backoff = cadence.first();
    let mut attempts = 0;
    let mut races = 0;

    loop {
        attempts += 1;
        let outcome = attempt(volume, device, prepare.before(attempts))?;

        let (pause, still_racing) = pause_after(&outcome, races, backoff);
        races = still_racing;

        // Tested after the attempt rather than before, so the deadline bounds how long this
        // keeps *trying* rather than whether it tries at all. Adding the pause is what stops
        // it sleeping past the deadline only to give up on waking.
        let last = outcome.is_ejected() || Instant::now() + pause >= deadline;

        // Reported before the return, so the final attempt gets a line like every other one.
        // A watcher that fell silent exactly when the fight ended would be the least useful
        // moment to stop talking.
        watch(Attempt {
            number: attempts,
            outcome: &outcome,
            elapsed: started.elapsed(),
            retry_in: (!last).then_some(pause),
        });

        if last {
            return Ok(Effort {
                outcome,
                attempts,
                waited: started.elapsed(),
            });
        }

        std::thread::sleep(pause);

        // **The backoff only advances when it was actually used**, which `races == 0` is exactly
        // the test for. A burst of race retries must not push the patient path out to a minute,
        // or losing eight races early would leave a genuinely busy device asked once a minute
        // from then on.
        if races == 0 {
            backoff = cadence.next(backoff);
        }
    }
}

/// How long to wait after `outcome`, and the consecutive-race count to carry forward.
///
/// **Extracted from the loop so the bound can be tested**, which the loop itself cannot be
/// without real hardware — every path through [`attempt`] needs a live volume and an enclosure
/// willing to refuse. An untested spin guard is the shape `docs/REVIEWING.md` calls a check
/// that cannot fail.
///
/// **The counter resets after any patient wait, and that is deliberate rather than incidental.**
/// A device that keeps losing the race gets a fresh burst each cycle — a burst is two seconds,
/// attempts are cheap, and 2026-08-06 measured that asking *more* often is what wins. What the
/// bound prevents is the pathological case: something reopening the volume continuously, which
/// would otherwise spin at four attempts a second for the whole budget and never once let the
/// patient path run.
fn pause_after(outcome: &Outcome, races: u32, backoff: Duration) -> (Duration, u32) {
    let lost_race = matches!(
        outcome,
        Outcome::Dismounted {
            veto: Veto::OutstandingOpen,
            ..
        }
    );

    if lost_race && races < RACE_RETRIES {
        (RACE_RETRY, races + 1)
    } else {
        (backoff, 0)
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
pub fn attempt(volume: &Volume, device: &Device, prepare: bool) -> Result<Outcome> {
    if prepare {
        let file = match open_for_control(volume.device_path()) {
            Ok(file) => file,
            // **A volume that is not there has been released, and saying otherwise is a lie the
            // report has already told.** On 2026-08-06 the operator ejected a card from the
            // tray mid-run; the next attempt could not open it, that became `Held`, and the
            // run announced `Primary still mounted` about the most thoroughly ejected device
            // in the room — then excluded it from the count of devices put to bed.
            //
            // **Matched on the error kind, not on the message.** `ERROR_FILE_NOT_FOUND` on a
            // volume path means the volume object is gone, which is exactly what success looks
            // like from here; a message match would break on a non-English Windows.
            Err(error) if missing_volume(&error) => return Ok(Outcome::Ejected),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("opening {} for eject", volume.guid_path));
            }
        };

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
    }

    match power_down(device.disk_number) {
        Ok(()) => Ok(Outcome::Ejected),
        Err(refusal) => Ok(Outcome::Dismounted {
            veto: refusal.veto,
            reason: refusal.detail,
        }),
    }
}

/// Whether to lock and dismount the volume before asking PnP to remove the device.
///
/// **Settled 2026-08-06: [`Prepare::FirstAttemptOnly`] is what the tool does, and the
/// `--eject-prepare` flag that compared these was deleted once it had.** `DESIGN.md` decision 22
/// carries the traces and the argument. This enum stays because the losing arms are still worth
/// running from `examples/eject-one.rs`, and because their tests document the divergence.
///
/// **The finding, in one line:** re-dismounting before every attempt means never asking about a
/// *settled* volume, and exFAT answers a freshly remounted one with `PNP_VETO_TYPE(6)`, which
/// never yields.
///
/// **Why the flush is kept on attempt one rather than dropped entirely.** Lock-and-dismount is
/// what *guarantees* the filesystem is flushed before the device leaves, and that protects data
/// this tool wrote — four verified copies on the SSDs. **Going fully bare would trade a
/// data-safety property for speed this project does not need.**
///
/// **The variant names say *when*, because when was the whole finding.** The original
/// `LockAndDismount` hid that it happened before *every* attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prepare {
    /// Lock, dismount and close the handle before **every** attempt. What every run before
    /// 2026-08-06 did, and what produced 23 consecutive unwinnable refusals.
    EveryAttempt,
    /// Lock and dismount on the **first** attempt only; ask bare from then on.
    ///
    /// **Terry's idea, 2026-08-06, and it dominates both alternatives.** The first dismount is
    /// what flushes the filesystem, so the data guarantee is paid for once and kept. Every
    /// attempt after it asks about a volume nobody has disturbed — which is the only state any
    /// eject has ever succeeded from on this rig.
    ///
    /// **The alternative considered and rejected: dismount, sleep a few seconds, ask.** It
    /// would test the same idea but buys the settling with wall clock, and it would still
    /// re-dismount on the next retry. This costs nothing and re-disturbs nothing.
    FirstAttemptOnly,
    /// Never prepare — ask PnP directly and let the file system do its own teardown.
    ///
    /// **Diagnostic**, and it drops the flush guarantee entirely. Defensible for cards, which
    /// this tool never writes to and which are formatted in-body next session; **not** for an
    /// archive SSD that just received four verified copies.
    Never,
}

impl Prepare {
    /// Whether attempt number `n` (1-based) locks and dismounts first.
    fn before(self, n: u32) -> bool {
        match self {
            Prepare::EveryAttempt => true,
            Prepare::FirstAttemptOnly => n == 1,
            Prepare::Never => false,
        }
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
    unsafe {
        DeviceIoControl(
            handle,
            code,
            None,
            0,
            None,
            0,
            Some(&raw mut returned),
            None,
        )
    }
    .map_err(Into::into)
}

/// `CM_Request_Device_Eject` on the device node behind `disk_number` — the tray icon's own
/// call.
///
/// **The disk's own device node is not the one to eject.** That node is the volume's storage
/// device; the thing that can be powered down is its parent, the USB or Thunderbolt
/// enclosure. So this walks up from the disk and asks the parent to leave, which is what
/// "safely remove hardware" does.
fn power_down(disk_number: u32) -> std::result::Result<(), Refusal> {
    let disk = devnode_for_disk(disk_number).map_err(|error| Refusal {
        veto: Veto::Other,
        detail: format!("{error:#}"),
    })?;

    let mut parent = 0u32;
    // SAFETY: `disk` is a devnode this process just located; `parent` is written on success.
    let status = unsafe { CM_Get_Parent(&raw mut parent, disk, 0) };
    if status != CR_SUCCESS {
        return Err(Refusal {
            veto: Veto::Other,
            detail: format!(
                "could not find the enclosure behind disk {disk_number} (CM_Get_Parent: {status:?})"
            ),
        });
    }

    // Windows fills this in with *what* vetoed the eject, which is far more useful to an
    // operator than a status code — it names the process or driver holding on.
    let mut veto_type = PNP_VETO_TYPE::default();
    let mut veto_name = [0u16; 260];

    // SAFETY: both out-parameters are correctly sized for the lengths given.
    let status = unsafe {
        CM_Request_Device_EjectW(parent, Some(&raw mut veto_type), Some(&mut veto_name), 0)
    };

    if status == CR_SUCCESS {
        return Ok(());
    }

    let name = String::from_utf16_lossy(&veto_name);
    let name = name.trim_end_matches('\0');
    Err(Refusal {
        veto: Veto::from(veto_type),
        detail: format!(
            "Windows declined to power the device down ({status:?}, {veto_type:?}{})",
            if name.is_empty() {
                String::new()
            } else {
                format!(", held by {name}")
            }
        ),
    })
}

/// Why Windows refused, **as a value rather than as prose**.
///
/// **The retry has to branch on this, and until 2026-08-06 it could not.** The veto type was
/// formatted straight into a message and discarded, so every refusal looked alike to the code
/// that had to decide what to do next — and the two that matter want opposite responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Veto {
    /// `PNP_VetoOutstandingOpen` (5) — a handle was open on the volume.
    ///
    /// **This one is ours.** The lock lives on the handle, so [`attempt`] must close the handle
    /// before asking the device to leave — and in that window the volume is unlocked, mounted
    /// again by an eager Windows, and anything at all can open it. Losing that race is not a
    /// device refusing; it is us arriving a moment late.
    OutstandingOpen,
    /// `PNP_VetoDevice` (6) — a device in the stack refused. Cause unknown as of 2026-08-06.
    Device,
    /// Any other veto type, or none reported.
    Other,
}

impl From<PNP_VETO_TYPE> for Veto {
    fn from(raw: PNP_VETO_TYPE) -> Self {
        // The numbers are from `cfgmgr32.h`'s `PNP_VETO_TYPE`, matched by value because the
        // `windows` crate exposes the enum as a newtype rather than as named variants.
        match raw.0 {
            5 => Veto::OutstandingOpen,
            6 => Veto::Device,
            _ => Veto::Other,
        }
    }
}

/// A refused power-down, carrying both the machine-readable reason and the operator-facing one.
#[derive(Debug)]
struct Refusal {
    veto: Veto,
    detail: String,
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
            cbSize: size_u32::<SP_DEVICE_INTERFACE_DATA>(),
            ..Default::default()
        };

        // SAFETY: `set` is live and `interface` is correctly sized; a false return ends the
        // enumeration, which is the documented termination condition.
        if unsafe {
            SetupDiEnumDeviceInterfaces(
                set,
                None,
                &GUID_DEVINTERFACE_DISK,
                index,
                &raw mut interface,
            )
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
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &raw const interface,
                None,
                0,
                Some(&raw mut needed),
                None,
            )
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
            (*detail).cbSize = size_u32::<
                windows::Win32::Devices::DeviceAndDriverInstallation::SP_DEVICE_INTERFACE_DETAIL_DATA_W,
            >();
        }

        let mut info = SP_DEVINFO_DATA {
            cbSize: size_u32::<SP_DEVINFO_DATA>(),
            ..Default::default()
        };

        // SAFETY: `detail` points at a correctly sized, correctly initialized header.
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &raw const interface,
                Some(detail),
                needed,
                None,
                Some(&raw mut info),
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
            size_u32::<STORAGE_DEVICE_NUMBER>(),
            Some(&raw mut returned),
            None,
        )
    }?;

    Ok(number.DeviceNumber)
}

/// The retry's branching, which is the only part of this module that can be tested without
/// hardware — every other path needs a live volume and an enclosure willing to refuse.
#[cfg(test)]
mod tests {
    use super::*;

    const BACKOFF: Duration = Duration::from_secs(32);

    /// **The defect this guard exists for**: on 2026-08-06 a card was ejected from the tray
    /// mid-run, the next attempt could not open the volume, and the run reported
    /// `Primary still mounted` about a device that was unambiguously gone — then left it out
    /// of the count of devices put to bed.
    #[test]
    fn a_volume_that_no_longer_exists_reads_as_gone() {
        let vanished = anyhow::Error::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "The system cannot find the file specified.",
        ));

        assert!(missing_volume(&vanished));
    }

    /// **Everything else is still a real failure.** A volume that is present and refusing must
    /// not be waved through as released — that would turn this fix into a worse version of the
    /// bug it repairs, since the operator would be told to store a drive still holding a lock.
    #[test]
    fn other_failures_are_not_mistaken_for_a_released_volume() {
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::Other,
            std::io::ErrorKind::InvalidInput,
        ] {
            let error = anyhow::Error::from(std::io::Error::new(kind, "nope"));
            assert!(!missing_volume(&error), "{kind:?} must not read as gone");
        }

        // And an error carrying no `io::Error` at all — the downcast must fail closed.
        assert!(!missing_volume(&anyhow::anyhow!("something else entirely")));
    }

    fn raced() -> Outcome {
        Outcome::Dismounted {
            veto: Veto::OutstandingOpen,
            reason: "PNP_VETO_TYPE(5)".to_owned(),
        }
    }

    fn refused() -> Outcome {
        Outcome::Dismounted {
            veto: Veto::Device,
            reason: "PNP_VETO_TYPE(6)".to_owned(),
        }
    }

    /// **The distinction the whole change rests on.** Before 2026-08-06 both of these were
    /// `Dismounted` with a prose reason, so the retry could not tell them apart and waited the
    /// same seconds for each — including for a race where nothing was busy at all.
    #[test]
    fn a_lost_race_retries_fast_and_a_refusing_device_waits() {
        assert_eq!(pause_after(&raced(), 0, BACKOFF), (RACE_RETRY, 1));
        assert_eq!(pause_after(&refused(), 0, BACKOFF), (BACKOFF, 0));
    }

    /// A veto with no type, and the success case, must both take the patient path rather than
    /// falling into the fast one by accident.
    #[test]
    fn only_an_outstanding_open_takes_the_fast_path() {
        let other = Outcome::Dismounted {
            veto: Veto::Other,
            reason: "no type reported".to_owned(),
        };
        let held = Outcome::Held {
            reason: "never got the lock".to_owned(),
        };

        assert_eq!(pause_after(&other, 0, BACKOFF), (BACKOFF, 0));
        assert_eq!(pause_after(&held, 0, BACKOFF), (BACKOFF, 0));
        assert_eq!(pause_after(&Outcome::Ejected, 0, BACKOFF), (BACKOFF, 0));
    }

    /// **The spin guard, which is the reason this function was extracted.** Something reopening
    /// the volume continuously produces an unbroken run of type 5, and without a bound that
    /// would retry four times a second for the whole 90-minute budget — never once letting the
    /// patient path run against a device that might simply need waiting out.
    #[test]
    fn a_burst_of_lost_races_is_bounded_and_then_falls_back_to_waiting() {
        let mut races = 0;
        let mut run = 0;
        let mut longest = 0;
        let mut waits = 0;

        // An unbroken stream of type 5 — the pathological case, where something reopens the
        // volume on every single attempt rather than once by accident.
        for _ in 0..RACE_RETRIES * 3 {
            let (pause, next) = pause_after(&raced(), races, BACKOFF);
            races = next;
            if pause == RACE_RETRY {
                run += 1;
                longest = longest.max(run);
            } else {
                run = 0;
                waits += 1;
            }
        }

        assert_eq!(
            longest, RACE_RETRIES,
            "a burst must cap at RACE_RETRIES consecutive fast retries"
        );
        assert!(
            waits >= 2,
            "an unbroken run of lost races must keep yielding to the patient path, got {waits}"
        );
    }

    /// **The counter resets after a patient wait, so bursts recur** — deliberate, since a burst
    /// costs two seconds and asking more often is what was measured to win. Asserted because it
    /// reads like an oversight and is not.
    #[test]
    fn the_burst_counter_resets_after_a_patient_wait() {
        let (_, after_burst) = pause_after(&raced(), RACE_RETRIES, BACKOFF);
        assert_eq!(
            after_burst, 0,
            "the bound must reset the counter, not freeze it"
        );

        let (pause, races) = pause_after(&raced(), after_burst, BACKOFF);
        assert_eq!(
            (pause, races),
            (RACE_RETRY, 1),
            "a fresh burst must be able to start"
        );
    }

    /// The backoff is only advanced when it was the pause actually used, and `races == 0` is
    /// the loop's test for that. If this drifted, a burst would push the patient path straight
    /// to its ceiling.
    #[test]
    fn a_fast_retry_does_not_advance_the_backoff() {
        let (pause, races) = pause_after(&raced(), 0, BACKOFF);
        assert_eq!(pause, RACE_RETRY);
        assert_ne!(
            races, 0,
            "a fast retry must leave races non-zero so the backoff is held"
        );
    }
}
