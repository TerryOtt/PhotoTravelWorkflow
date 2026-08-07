# Eject — the run series and the investigation record

**What this file is for:** everything eject has *cost* to work out — the A/B tally, and the two
investigations that produced the current design. [`DESIGN.md`](DESIGN.md) decision 22 holds the
**decision**; this holds the **working out**, so neither has to carry the other.

**RFC 2119 keywords, and the capitals are load-bearing.**

## The two modes

| | `eject::Prepare` | What it does |
|---|---|---|
| **A** | `EveryAttempt` | Locks and dismounts before **every** attempt. The behavior every run before 2026-08-06 used, and the one that produced 23 consecutive unwinnable refusals. |
| **B** | `FirstAttemptOnly` | Locks and dismounts **once** — so the flush still happens — then asks bare on every retry, so the volume is never re-disturbed. |

A third mode, `Never`, drops the flush entirely. **It is a reference point and MUST NOT be run
as a candidate**: the archives are NTFS and journal their metadata, but giving up a guarantee
this project already has, to fix something `FirstAttemptOnly` also fixes, is a bad trade.

> **`--eject-prepare` no longer exists.** B is the built-in behavior as of 2026-08-06 and A is
> **not reachable from `offload`** — see `CLAUDE.md`, *a config item that is never used MUST NOT
> exist*. To run A, use `cargo run --release --example eject-one`, which drives
> [`eject::Prepare`] directly. **The experiment did not lose anything; the product stopped
> offering a mode known to hang.**

## How the evidence accumulates now

**The original plan was to alternate A and B**, on the reasoning that the pathological case fired
"roughly once in six runs", so a clean B streak would prove little. **That estimate is retired and
this section replaced it on 2026-08-06.** The tally below shows **A failing two times out of
two** — a reproducible fault, not a 1-in-6 lottery.

> **The 1-in-6 figure sat in this file beside the 2-for-2 tally that refuted it**, and the
> alternation rule was still resting on it. Superseding a number MUST include deleting the
> arguments built on it; see [`WRITING.md`](WRITING.md).

**So the design changes, and it gets cheaper:**

- **A needs no more runs.** Its failure rate is established and each further run costs 19+ minutes
  and five cable replugs to re-prove a known result.
- **B accumulates for free.** Every ordinary run is now a B run, so the sample grows without
  anyone scheduling anything. **Add a row for any run whose eject behaved unusually**, and for
  every full run.
- **The first B hang is the interesting event**, and it would be immediately informative rather
  than ambiguous — A hangs *unwinnably*, so a B run that merely takes several attempts and then
  succeeds is a different animal and worth recording as such.

**Compare attempts per device, not just whether anything hung.** Attempt counts are continuous and
say something after four runs; waiting for another 23-attempt event could take all night and still
prove nothing.

## The tally

Every row is one run of the 50-frame corpus on the full rig. `NEVER` means the device was still
refusing when the run ended.

| # | Mode | SanDisk | WD | OWC | Primary | Secondary | Stage | Note |
|---|---|---|---|---|---|---|---|---|
| 1 | A | 5s · 1 | 7s · 1 | 13s · 2 | **NEVER · 23** | 2s · 2 | **19m 20s** | the run that started this; ended by a tray eject |
| — | `never` | 7s · 1 | 14s · 1 | 16s · 2 | 9s · 1 | 9s · 1 | 16s | reference only, no flush |
| 2 | B | 3s · 1 | **29s · 5** | 5s · 1 | 6s · 1 | 3s · 2 | 29s | first B run; WD fought for the first time ever |
| 3 | A | 5s · 1 | 16s · 3 | 2s · 1 | **HUNG · 8+** | 0s · 1 | stopped | stopped by hand at 8 attempts rather than wait out the 90-minute budget |

**Running totals — A: 2 runs, 2 hangs. B: 1 run, 0 hangs.**

> **The base rate is real, and that is the point of run 3.** Before it, the 23-attempt event
> was a single occurrence and there was a live possibility that it was a fluke and nothing
> needed fixing. **Two for two at `every-attempt` retires that.** The phenomenon reproduces.
>
> **A run MAY be stopped by hand once its outcome is established.** Riding a hang to the full
> budget upgrades *"never released in 3 minutes"* to *"never released in 90 minutes"*, which is
> marginal information for 87 minutes of an evening. Record that it was stopped and at which
> attempt, so the row is not mistaken for a device that recovered.

### The type 6 → type 5 → released descent has now been seen three times

WD in runs 2 and 3, and once earlier. **Every device that has descended to
`PNP_VetoOutstandingOpen` has then released.** The devices that hang — Primary, twice — stay on
`PNP_VetoDevice` and never descend.

**That is the sharpest predictor found so far**, and it suggests the two veto types are not two
symptoms of one problem: type 5 is a transient this tool can out-wait, and type 6 on a card may
be something else entirely.

## What each outcome would mean

- **A hangs again** — the base rate is real and worth fixing. This is the outcome that makes
  the whole exercise worthwhile.
- **B hangs** — the settle-time explanation is wrong and `first-attempt-only` is not a fix.
  **Record it and stop**; do not reach for the next variant without a new explanation.
- **Neither hangs across several runs** — then the 23-attempt event was rarer than believed.
  **Compare attempt counts, and if B is not clearly better, say so** rather than shipping a
  change that bought nothing. A fix for a phenomenon that does not recur is not a fix.

## Two results already on record that argue for caution

- **Primary released on a *prepared* attempt in run 2.** Attempt one under B is byte-identical
  to A, and the call that failed 23 times in run 1 succeeded in six seconds. **So preparation
  does not reliably poison that card.**
- **The struggling device changes run to run** — Primary once, WD once, neither twice. That
  reads as per-night luck rather than any device being special, and "the CFexpress is special"
  was asserted several times on 2026-08-06 on the strength of one spectacular failure.

---

## The investigation record

**Moved out of `DESIGN.md` decision 22 on 2026-08-06**, which had grown to 455 lines by keeping
its own evidence inline. **The decision stayed there; the working out came here.** Both blocks are
kept whole and unedited — one is a correction the project had to make against itself, the other a
run record, and both are the kind of thing that gets re-proposed once nobody can find the refutation.

> ### ✗ Lock-and-dismount does not release a card, and the report has been overstating it
>
> **Found by the operator on 2026-08-04 and reproduced deterministically on 2026-08-05.** He
> put the laptop to sleep after a run that reported both cards dismounted, checked the tray
> out of habit, and found **both cards still attached with drive letters**.
> `cargo run --release --example release-cards` reproduces it in two seconds — its first
> rung is exactly the old behavior:
>
> ```text
> === before ===
>   D:\  serial 1E68-1046  mounted at [D:\]
>   E:\  serial 0E7A-0533  mounted at [E:\]
> === dismounting ===
>   D:\  dismounted in 0.0s
>   E:\  dismounted in 0.0s
> === after, re-enumerated ===
>   D:\  serial 1E68-1046  mounted at [D:\]
>   E:\  serial 0E7A-0533  mounted at [E:\]
> ```
>
> **Both calls succeed and neither achieves anything the operator can see.**
> `FSCTL_DISMOUNT_VOLUME` detaches the filesystem; it does not remove the volume or its drive
> letter, and Windows remounts on next access — which the tray icon, Explorer and the indexer
> supply continuously. This is not intermittent and was never a race we lost: it is the
> guaranteed outcome of what the code asks for.
>
> **What is and is not damaged by this, stated precisely.** *Safe to pull* was true before the
> attempt and is true after it — the tool never wrote to the card, which is the whole reason
> this operation is cosmetic in the first place. **What fails is the only thing the feature was
> for.** This decision added card dismount because *"an asymmetry the operator has to remember
> is a cost paid at the end of a long day"*; the asymmetry is still there, and the report now
> prints a line claiming otherwise. **A feature that delivers nothing but a reassuring line is
> worse than no feature**, by decision 12's standard — it is a warning you learn to read past,
> pointed at the wrong end of the run.
>
> **The call that was missing was named in `eject.rs`'s own module doc, describing this exact
> case.** That doc explains why the module does not use `IOCTL_STORAGE_EJECT_MEDIA` — *"that
> control code ejects media from a drive — a disc from an optical drive, **a card from a
> reader**"* — an argument written about **SSDs**, where it is correct. The card path inherited
> the exclusion without anyone noticing that cards are the case the sentence endorses.
>
> **And the two cards are not one problem.** Measured 2026-08-05 with
> `Win32_DiskDrive.CapabilityDescriptions`:
>
> | | SD, via a USB reader | CFexpress, via a Thunderbolt reader |
> |---|---|---|
> | `BusType` | USB | **NVMe** |
> | `MediaType` | **Removable Media** | **Fixed hard disk media** |
> | `Supports Removable Media` | **yes** | **no** |
>
> **The PCIe tunnel exposes the card *as* the device** — the disk is named `AV PRO CFexpress
> SE` and carries the card's own hardware serial — so there is no separable medium to eject,
> and `IOCTL_STORAGE_EJECT_MEDIA` should be expected to do for it exactly what this module's
> doc says it does for an SSD: nothing. **So the fix is asymmetric, and the CFexpress half is
> an open design question rather than an implementation gap:** the only call that releases it
> is a device eject, whose parent is the reader — which is the thing this decision forbids.
> Resolving that needs the operator's call on whether powering down the ProGrade reader is
> acceptable, and it is deliberately not assumed here.
>
> ### Measured 2026-08-05 — and both predictions above were wrong, in opposite directions
>
> `cargo run --release --example release-cards` escalates lock+dismount → eject-medium →
> device-eject, re-enumerating after each. The operator authorized step 3 in advance.
>
> | | SD, USB reader | CFexpress, Thunderbolt reader |
> |---|---|---|
> | 1. lock + dismount | still mounted | still mounted |
> | 2. **eject medium** | **returns `ok`, releases nothing** | **returns `ok`, releases nothing** |
> | 3. device eject | **RELEASED** | **RELEASED** |
> | ...and the reader? | **powered down too** | **still enumerated** |
>
> **`IOCTL_STORAGE_EJECT_MEDIA` is not the answer, and the prediction that it would be was
> mine.** It reports success on *both* cards and releases neither — the "succeeds at nothing"
> outcome this module's doc predicted for SSDs, now observed on a device that advertises
> `Supports Removable Media`.
>
> **The obvious objection was chased rather than left standing, and it closed the question
> from an unexpected direction.** That call was issued on the *volume* handle, where the
> conventional target for a media operation is the physical drive — so the result was
> consistent with *we asked the wrong object* rather than *the device ignores this*. Asking
> `\\.\PhysicalDriveN` instead returns **`Access is denied` (os error 5)**, and an unelevated
> process cannot open a physical drive **at any access level, not even read** — verified
> against an archive SSD, so it needs no card and no reseat to reproduce.
>
> **So the drive-handle route is not merely untested, it is unavailable — binding constraint
> 4 forbids it.** *Nothing in a run may come to need administrator rights*, and this would.
> Whether media-eject would work there is now moot, which is a better resolution than an
> answer: the tool may not take that path regardless. **`CM_Request_Device_Eject` is the only
> mechanism an unelevated `offload` has that actually releases a card.**
>
> **The reader asymmetry is the reverse of what this decision feared.** The worry was that
> device-ejecting a card would take its reader down. That is exactly true of the **SD** — the
> SDDR-409 disappears from the device tree entirely and needs a physical reseat — and exactly
> false of the **CFexpress**, where the ProGrade router stays enumerated and only the card's
> own PCIe device goes. The mechanism is `CM_Get_Parent`: the SD disk's parent *is* the reader,
> while the NVMe disk's parent is a PCIe downstream port beneath a router that survives it.
>
> **So the cost lands on the card this decision cared least about, and the exception it was
> braced for is not needed.** The CFexpress — the card on the LANDED path — releases cleanly
> with its reader intact, which is precisely the behavior decision 22 asks for and assumed
> impossible. The SD is the one that forces a choice, and it is sharpened by a fact from the
> *Inputs* section: **multiple offloads a day are normal**, so a reader that must be reseated
> between them is a real cost paid at lunchtime, not once at bedtime.
>
> ### ✔ Settled by the operator, 2026-08-05: eject both, and say what it costs
>
> **Both cards are released, and the report names the consequence rather than leaving it to be
> discovered.** Three options were put to him — eject both; eject only the CFexpress and report
> the SD honestly as still mounted; or eject both and warn. He took the third, in his words:
> *"I'll catch it when we try the next run and SD is missing."*
>
> **That sentence is the whole justification, and it is a statement about the system rather
> than about the reader.** The forgotten replug is not a silent failure — pre-flight refuses
> with `ONLY ONE CARD FOUND` in the first ten seconds (decision 7), while the fix is a reach to
> a cable. So the cost of the worst case is ten seconds at the desk, against a tray icon that
> would otherwise be wrong after every single run. **A degradation that the next run refuses to
> proceed past is not the same kind of thing as one nobody notices**, and choosing between them
> is what decision 9's whole pre-flight exists to make possible.
>
> The rejected middle option is worth recording too: ejecting only the CFexpress would have
> reinstated exactly the asymmetry this decision added card handling to remove — one card
> settled, one in the tray, and the operator remembering which is which at the end of a long
> day.
>
> **What the report says**, printed once when anything was released:
>
> ```text
>   Cards
>            primary   released — pull the card
>            secondary released — pull the card
>
>   !  A USB card reader powers down with its card and needs a replug before the
>      next offload. The Thunderbolt reader does not. If you forget, pre-flight
>      refuses with ONLY ONE CARD FOUND rather than running short.
> ```
>
> **It still may not touch the verdict or the exit code**, which is unchanged and load-bearing:
> the tool never wrote to a card, so this was always tidiness. A card that will not release
> leaves the night exactly as safe as one that does.

> **Proven on the rig, 2026-08-04 — the retry fired and worked.** Third full run of the
> evening, all four destinations freshly written:
>
> ```
>   Eject    (15s)
>            OWC      powered down after 2 attempts over 15s
>            SanDisk  powered down
>            WD       powered down
> ```
>
> **The OWC's first attempt was vetoed and its second succeeded**, fifteen seconds later.
> Under the previous single-attempt code that drive would have ended `dismounted, not
> powered down` and sent the operator to the tray icon. Two of three still powered down on
> the first ask, so the veto is intermittent rather than the norm — which is precisely why a
> single attempt was enough to look correct for as long as it did.
>
> **Fifteen seconds, against a budget of an hour.** The obstruction here was brief, and the
> generous window cost nothing to have. One observation is not a distribution: keep
> recording `attempts` and `waited` per device (the report prints them) and let the real
> spread accumulate before tuning anything.
>
> **The rate so far, across four runs of the same evening — twelve device ejects, three
> vetoed:**
>
> | Run | Vetoed on first attempt | Eject stage |
> |---|---|---|
> | 1 (single-attempt code) | OWC, WD — both left dismounted, not powered down | — |
> | 2 | none | — |
> | 3 | OWC — powered down on its second attempt | 15 s |
> | 4 | none | 9 s |
> | 5 — *cards included for the first time* | OWC — powered down on its second attempt | 11 s |
>
> **Updated 2026-08-05: 4 of 17 across five runs, once the two cards joined the count.** The
> fifth run vetoed the OWC and recovered it on the second attempt in 11 s — the same device
> and the same shape as run 3. **Both cards released on their first attempt**, so nothing
> so far suggests a card is more prone to this than a freshly written SSD; if anything the
> reverse, which fits, since a card was never written to and has no scanner working through
> it.
>
> **One in four device ejects, and it clusters rather than spreading evenly** — two on one
> run, none on the next two, one on the fourth. That is why a single-attempt implementation
> looked fine for as long as it did, and why no conclusion about eject should ever rest on
> one run. It is also why the retry earns its place at a cost of 9–15 seconds on a normal
> night.
>
> **The mechanism, since "race condition" is too vague to act on.** `FSCTL_LOCK_VOLUME`'s
> exclusivity belongs to *the handle*. The handle must be closed before
> `CM_Request_Device_Eject`, or PnP walks the device tree, finds this process's own
> outstanding open, and vetoes naming us. But closing it releases the lock, and a dismounted
> volume remounts on next access. **So the two steps that need exclusivity are separated by
> the step that gives it up** — the eject structurally cannot hold the claim it requires.
> That is a tension in the Win32 API rather than a defect here, and it is why widening or
> narrowing the gap changes nothing while re-running the sequence does.
>
> Two explanations fit every observation and both are addressed by the retry, so neither has
> been isolated: the volume remounted in the gap and something grabbed it, or a scanner held
> files throughout and the lock merely succeeded in a lull. The veto name is the *volume
> device object*, which says **where** the obstruction is and never **who**.
>
> **The candidate that would close it properly rather than racing it: `IOCTL_VOLUME_OFFLINE`,**
> which takes a volume offline in a way that survives the handle close — what
> `diskpart offline volume` does. If it holds, the eject becomes deterministic instead of
> probabilistic. Untested, and unknown whether PnP still finds something to veto. Given this
> decision is the operator's number one risk, it is the most promising lead on file.
