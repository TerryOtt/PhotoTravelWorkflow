# Run records

**What a full end-to-end run actually cost, and what went wrong in it.** Newest first. Every
entry here is a real run on the real rig under [`FULL-RUN.md`](FULL-RUN.md) — clean build, cold
cache, wiped destinations — so the numbers are comparable to each other.

**Split out of [`DESIGN.md`](DESIGN.md) on 2026-08-06**, which had grown to 4,599 lines with a
third of that being run narratives. The design document is for someone changing the design; this
one is for someone asking *what happened last time*. **Nothing was cut in the move** — the split
is structure, not deletion, and the same reasoning that put the eject tally in
[`EJECT-SERIES.md`](EJECT-SERIES.md).

> **These narratives quote what the tool printed on the night they were written.** Verdict
> wording, badge shapes and flags have changed since — `--eject-prepare` no longer exists, and
> `EJECTED — SAFE TO STORE` is now a badge. **Do not read an old record as current behavior**;
> `DESIGN.md` decision 14 is the authority for what the report says today. Editing these to match
> would destroy the record.

---

### The 415 GB run, 2026-08-06 — LANDED 35 m 29 s, and the first eject that never recovered

**The largest day on record, run end to end under [`FULL-RUN.md`](FULL-RUN.md).** Cold boot,
24.7-minute settle, all four destinations empty, exit 0,
`EJECTED — SAFE TO STORE. every file from both cards is accounted for.` Whole run **89 m 59 s**,
which is one second inside the 90-minute budget.

| | |
|---|---|
| Corpus | 2024-10-02 — 7,395 frames, 386.6 GiB, both cards holding one identical listing |
| **LANDED** | **35 m 29 s** |
| I/O | 3,479.6 GiB at 1.63 GiB/s, 14.0 Gbps — **9N to within rounding** (9 × 386.6 = 3,479.4) |
| Every destination | `7,395 written · 0 skipped · 7,395 verified` — **29,580 pairs** |
| Corroboration | **7,395 matched · 0 mismatched**, and zero transient read errors |
| Geotag | 7,319 tagged · **0 outside track** · 76 in a gap · 29,276 sidecars |
| Eject | three SSDs first attempt in ~15 s; **the CFexpress never released** |

#### It scaled super-linearly, and that overturns the prediction this document recorded

**Decision 22's table predicted LANDED at ~22 m 30 s and eject opening at ~52 min. Both were
wrong, and they were recorded before the run precisely so this could be checked.**

| | 2026-08-05 baseline | This run | Ratio |
|---|---|---|---|
| Data | 187.5 GiB | 386.6 GiB | **2.06×** |
| LANDED | 10 m 55 s | **35 m 29 s** | **3.25×** |

**Time grew 58 % faster than data.** The tool's own estimator was wrong in the same direction
— it offered 18–31 min — so this is not one bad extrapolation, it is a real property of the
workload that nothing had measured, because no run at this size had ever happened.

**Three candidates, and this document deliberately records none of them as the cause:**

1. **Destination SLC exhaustion.** 386.6 GiB certainly exhausts caches that 187.5 GiB may sit
   inside, and *Throughput history* already records destination writes degrading 431 → 311 MB/s
   under exactly that pressure.
2. **The OWC holds a drive that has never been benchmarked.** The baseline ran on the FireCuda
   530; this ran on its replacement, a WD_BLACK SN850X. The session that produced the ~22 m 30 s
   figure scaled from a run containing a drive that no longer exists — a methodological error,
   not a hardware surprise.
3. **1.9× the file count** at a similar size each, and per-file pipeline drain scales with
   count rather than bytes (decision 17's remaining quarter).

**One candidate *was* eliminated, by a phase that behaved.** Corroboration — a pure sequential
read of all 386.6 GiB off the SD card — took ~31.5 minutes and scaled **linearly**. Had the
machine simply been slow today, that phase would have been slow too. **The super-linearity
lives on the write/verify path, not in the machine.**

#### The eject retried for 22 minutes and never recovered — a first

```text
    Cards
        Primary    dismounted, still listed — safe to pull anyway
            Windows declined to power the device down (CONFIGRET(23), PNP_VETO_TYPE(6),
            held by STORAGE\Volume\{3d2ab0c2-...})
        Secondary  ejected; remove card from reader
```

**Every prior veto in this project cleared on a second attempt within ~15 seconds. This one
consumed the entire remaining budget and still failed.** The veto names the volume device
object, which is the same signature decision 22 already documents and still says **where** the
obstruction is and never **who**.

**And it reverses the asymmetry measured on 2026-08-05.** That day the CFexpress released
cleanly through its Thunderbolt reader while the SD took its reader down with it. This day the
SD released fine and the **CFexpress** was the holdout. **Whichever card holds out looks like
luck rather than a property of the bus type**, which weakens the reasoning decision 22 currently
rests on.

**Nothing was lost and the verdict is correct**: the tool never wrote to that card, so it was
safe to pull before the attempt and after, and decision 22 forbids the result from touching the
verdict or the exit code. Both held.

> **✗ A claim made at LANDED and withdrawn an hour later, recorded because the reasoning was
> the error.** The session said raising the budget 60 → 90 that morning "was load-bearing, and
> this run proves it," reasoning that eject opened at ~68 min and would have had no window at
> 60. **The premise was right and the conclusion did not follow.** All three SSDs released on
> their *first* attempt, and decision 22 guarantees one attempt even when the budget is already
> spent — so at 60 minutes the outcome would have been identical. **The entire extra 22 minutes
> went to the one device whose eject result is declared meaningless.** The wider budget remains
> defensible; this run is not evidence for it.

#### ✔ Resolved the same afternoon — the cards release on their own, in about eleven minutes

**A small-day convergence run released both cards on the tool's own `CM_Request_Device_Eject`,
with nothing touching the tray:**

```text
Travel SSDs — all 3 put to bed in 13s. Safe to store.

    Cards
        Primary    ejected; remove card from reader
        Secondary  ejected; remove card from reader

Cards — all 2 put to bed in 11m 17s.
```

**This isolates time from mechanism, which the morning's run could not.** The tray eject that
worked at 36 minutes moved two variables together; here the same call the tool had been making
all along succeeded after eleven minutes with no tray involved. **The obstruction clears with
elapsed time, and the retry budget is the lever that matters.**

**It also kills the freshly-written-volume hypothesis for cards.** This run wrote **zero files**
— 23.4 GiB moved, every byte of it read-and-compare — and the tool never writes to a card under
any circumstance. Both cards still vetoed for eleven minutes. That hypothesis was borrowed from
the SSD case, where *a scanner is chewing freshly written files* at least has a mechanism;
applied to a volume nothing has written to, it never did.

> **↺ And it partly reverses the withdrawal above, which is why both are kept.** The correction
> stands **for the SSDs**: they needed one or two attempts and would have released under any
> budget. **It does not stand for the cards.** At ~11 minutes to release, a 60-minute budget on a
> run that reached eject at 67.7 minutes gives them one attempt and certain failure. **So the
> wider budget is load-bearing for exactly the device class this decision declares meaningless to
> the verdict** — a real argument for keeping it, and a weaker one than *it protects the data*.

**A consequence for iteration, worth knowing before someone waits it out:** eject now dominates a
small run — `LANDED in 0m 07s` against `11m 17s` of card release. **Use `--no-eject` for display
and report work**, or every tweak costs eleven minutes.

**Kernel-PnP event 225 is the instrument this investigation was missing**, found the same day. It
logs veto attribution to the System log, unelevated and **retroactively**, so it needs no live
specimen and no race against the obstruction clearing — which is how the 2026-08-05 experiment
was lost. Its first result splits one phenomenon in two: the SanDisk SSD's veto was logged and
attributed to PID 4 (the kernel, despite the message saying *application*), and **neither card
produced an event at all** across eleven minutes. That is consistent with `PNP_VETO_TYPE(6)`
naming an instance path rather than a process, and it means **the SSD veto and the card veto are
not the same fault and MUST NOT be reasoned about together.**

#### ✔ The backoff is right, measured — and asking LESS is 83× worse

**The operator asked whether the retry was self-defeating:** *"any chance the request to eject
compounds the problem and makes windows think the drive is under more contention than it is?"*
A good question with a real mechanism — every attempt dismounts and must close its handle, and
this module's own note says Windows remounts eagerly — so `--eject-gap-seconds` was added to
settle it. **The answer is no, and it is wrong in the opposite direction.**

| Cadence | SanDisk Extreme Pro released after |
|---|---|
| **2 s doubling to 60 s** — the default | **11 s**, attempt 2 |
| **300 s flat** | **15 m 12 s**, attempt 4 |

Same device, same machine, same afternoon. **The obstruction opens and closes on a short cycle,
so rapid retries catch a window a patient one sails past.** This decision's backoff had never
been compared against anything; it is now measured, and it stays. **A shorter first pause is the
only change worth considering; flattening or lengthening it MUST NOT be.**

**The veto type also changes mid-fight, which no completed run could show.** On that same
device, hands off the rig:

```text
#1  PNP_VETO_TYPE(6)       the device stack refused
#2  identical, unprinted   unchanged
#3  PNP_VETO_TYPE(5)       OUTSTANDING OPEN on the volume
#4  RELEASED
```

**Type 5 is `PNP_VetoOutstandingOpen`; type 6 is `PNP_VetoDevice`. Two different faults under
one `dismounted` label**, and `eject.rs` predicted the first of them in a comment written as
*reasoning* rather than observation — anything that reopens the volume between the dismount and
the power-down turns the refusal into exactly that. **Type 5 is a race this tool loses and can
therefore narrow.** Type 6 remains unexplained.

**And the device worth investigating is the SSD, not the cards.** The 11 m 17 s card hold has
not reproduced across four subsequent runs — 1 s, 2 s, 9 s, 10 s — while the SanDisk SSD vetoed
twice, logged Kernel-PnP event 225 both times against the same volume GUID, and is the class
that reaches the exit code. The cards produced the more spectacular single failure and have been
the wrong thing to chase.

#### ✔ THE VETOER IS THE exFAT DRIVER — named 2026-08-06 by an elevated ETW trace

**This closes three days of "the storage/PnP stack, cause unknown".** Verbose
`Microsoft-Windows-Kernel-PnP` tracing, captured while a CFexpress was mid-refusal:

```text
Device PCI\VEN_27D1&DEV_5216&... could not be query removed as the removal was vetoed.
Veto Type: 6
Veto Name: STORAGE\Volume\{3d2ab0c2-...}#0000000000040000\FileSystem\exfat
```

**The exFAT file system driver refuses the query-remove.** Not an application, not the search
indexer, not Defender, and not "the kernel" in the diffuse sense Kernel-PnP event 225's `PID 4`
suggested — a named driver attached to that volume.

**Why it took three days: this decision has been reasoning from a shorter name than Windows
has.** `CM_Request_Device_EjectW` returns a veto name ending at the volume; ETW's continues four
segments further, to the driver. Every earlier note here concluded *type 6 says **where**, never
**who*** — which was true of what the tool could see and false of what the OS knew.
**Whether that difference is truncation, a different field, or a different API contract is
UNTESTED and MUST NOT be recorded as any of them until it is checked** — the name is ~85
characters against a 260-wide buffer, so it is not a size limit.

**The method, which needs an elevated shell, takes ~90 seconds and touches no drive:**

```text
logman create trace pnpveto -p "Microsoft-Windows-Kernel-PnP" 0xffffffffffffffff 5 -o C:\Temp\pnpveto.etl -ets
   (wait for at least one attempt)
logman stop pnpveto -ets
tracerpt C:\Temp\pnpveto.etl -o C:\Temp\pnpveto.xml -of XML -y
```

**Keep the keyword mask wide.** PnP is low-volume — 752 events in 70 seconds on a quiet machine
— and narrowing risks filtering out the only event worth having.

**Two further facts from the same capture.** The device being ejected is `PCI\VEN_27D1&DEV_5216`,
the ProGrade Thunderbolt reader's bridge, so `CM_Get_Parent` walks up to the *reader* and what
exFAT refuses is the reader's removal rather than the card's. And **nothing hangs**: every
attempt logs `Begin attempting to eject` → `End attempting`, status `0x80000028`, cleanly. The
60-second rhythm is entirely this tool's.

**What it points at.** A *file system* driver vetoing means what is being protected is
filesystem state rather than device state, which puts this back inside what a run does to that
volume — reachable, unlike the storage stack.

#### The accepted query-remove is mechanically IDENTICAL to the refusals

**Captured the same evening.** The operator tray-ejected the CFexpress after 23 consecutive
tool refusals, with a trace running:

```text
VETOED (tool, 13:56:15)              ACCEPTED (tray, 13:56:56)
.142  CfgMgr_QueryRemove Start       .354  CfgMgr_QueryRemove Start
.142  DeviceEject Begin              .354  DeviceEject Begin
.142  DeviceRemoval 47 0x0 true 0x2  .354  DeviceRemoval 47 0x0 true 0x2
.142  Begin removal of PCI\VEN_27D1  .354  Begin removal of PCI\VEN_27D1
.326  VETO 6 \FileSystem\exfat              ← 184 ms
.328  End removal                  57.629  End removal            ← 1,275 ms
.329  DeviceEject End 0x80000028   57.629  Removal of STORAGE\Volume\{3d2ab0c2}
                                   57.629  DeviceEject End Status: 0x0
```

**Same device, same events, same order, same flags.** The tray used no different call and no
privileged one. **The only difference is that exFAT consented** — which restores this
decision's earlier conclusion that the tray is not special, after a same-day reversal that
rested on a coincidence rather than on evidence.

#### ⇒ The open hypothesis: this tool's lock-and-dismount may be counterproductive

**Kernel-PnP only sees from `CM_Request_Device_Eject` onward**, so everything either program
did *before* that call is invisible — and this tool does two things the tray almost certainly
does not, then asks.

**The timings fit that reading.** A refusal takes **184 ms**, because exFAT has nothing to do:
the volume was already dismounted. The success takes **1,275 ms**, which is exFAT performing
the flush and detach *itself*, as part of consenting — and only the successful path reaches the
step that removes `STORAGE\Volume\{...}` as its own device node.

**So the sequence may be defeating itself**: dismount, close the handle, watch Windows remount
the volume eagerly, then ask PnP to remove a volume that mounted milliseconds ago.

**`eject::power_down_disk` is already the bare call**, with no lock and no dismount, so the
test is small. **If it succeeds where the full sequence fails, steps 1 and 2 are the defect.**

> **This would overturn a claim at the top of `eject.rs`:** *"Three steps, in this order, and
> none of them is optional."* **That has never been tested** — it is reasoning, written before
> any of this was measured, and it is exactly the kind of asserted mechanism this project has
> spent a day disproving.

#### ✔ TESTED — `--eject-bare` released every device, including the card that never yielded

**Same corpus, same rig, same cadence, one hour apart. Only `Prepare` differs.**

| Device | `LockAndDismount` | **`Bare`** |
|---|---|---|
| SanDisk | 5 s · 1 ask | 7 s · 1 ask |
| WD | 7 s · 1 ask | 14 s · 1 ask |
| OWC | 13 s · 2 asks | 16 s · 2 asks |
| **Primary (CFexpress)** | **NEVER · 23 asks · 19 min** | **9 s · 1 ask** |
| Secondary | 2 s · 2 asks | 9 s · 1 ask |

**Whole eject stage: 16 seconds, against a run that could not finish it in nineteen minutes.**
The CFexpress has produced every long hold this project has recorded — 22 minutes never
released, 11 m 17 s, 23 attempts — and released on the first ask.

**The mechanism, and it is not that exFAT is more tolerant of the bare call.** It is that the
preparation puts exFAT into a state it will not release from:

| | What Windows is asked | What comes back |
|---|---|---|
| **Prepared** | remove a volume we dismounted, whose handle we had to close, and which Windows **remounted** | `PNP_VETO_TYPE(6)` `PNP_VetoDevice` on `\FileSystem\exfat`, refused in 184 ms, **never yields** |
| **Bare** | remove a normally-mounted volume | exFAT does its own flush and detach — or briefly `PNP_VETO_TYPE(5)` `PNP_VetoOutstandingOpen`, which **cleared on the 250 ms retry** |

**The remount is documented behavior, not a theory.** `FSCTL_DISMOUNT_VOLUME`'s own reference:
*"the operating system does not detect unmounted volumes, and if an attempt is made to access
an unmounted volume, the operating system then tries to mount the volume."*

**The type-5 fast retry fired in the wild for the first time here**, on OWC — built hours
earlier for precisely the veto that only appears when you do *not* prepare.

#### What the literature says: nothing, and that is worth recording

**Searched, and found no source describing this.** Not Microsoft's documentation, not Stack
Overflow. Two things did turn up, and both matter:

- **Microsoft cannot say whether `CM_Request_Device_Eject` flushes.** A Q&A asks exactly that
  and the staff answer is that the documentation is *vague*. **So forcing a dismount first was
  a reasonable defensive choice rather than a blunder** — given an undocumented flush and a
  guaranteed one, this project's posture has always been to force it.
- **The remount is documented; the consequence is not.** Nobody appears to have written up that
  the dismount-then-remount cycle produces a `PNP_VetoDevice` the retry cannot win.

**Why this rig may hit what others do not.** Three unusual things at once: the eject follows
heavy unbuffered I/O rather than an idle stick; **five devices are removed concurrently**; and
`CM_Get_Parent` walks up to a **PCIe-tunnelled reader's bridge** rather than a plain USB
mass-storage node. The ordinary "safely remove one idle USB stick" path probably never gets
there.

> **⚠ `n` = 1 each way, and this MUST NOT become a default on it.** The baseline is a single
> run, the bare result is a single run, and this card has been erratic all day.

#### ⇒ THE CAUSE: the retry re-dismounts before every attempt

**Found by the operator, and it is the first explanation that covers everything.** Each attempt
does lock → dismount → close handle → ask inside a few hundred milliseconds, so **the retry
never once asked about a settled volume** — it re-created the fresh-mount condition and
immediately asked about it, twenty-three times. The sixty-second gaps were spent waiting, not
settling, because the settling was destroyed at the start of the next attempt. **The tray
succeeded because it asked about a volume that had been mounted 41 seconds.**

| Observation | Volume state when asked |
|---|---|
| 23 refusals, prepared | freshly remounted, milliseconds old |
| tray, first try | settled 41 s |
| bare eject, first ask | never disturbed |
| cold harness 1 s · after-run harness 2 s | untouched |

**`Prepare::FirstAttemptOnly` follows directly**: lock and dismount on attempt one so the flush
still happens, then ask bare on every retry so the volume is never re-disturbed. **No
per-device-class branching is needed**, which the filesystem check below makes safe.

| Device | `EveryAttempt` | `Never` | `FirstAttemptOnly` |
|---|---|---|---|
| SanDisk | 5 s · 1 | 7 s · 1 | 3 s · 1 |
| WD | 7 s · 1 | 14 s · 1 | **29 s · 5** |
| OWC | 13 s · 2 | 16 s · 2 | 5 s · 1 |
| **Primary** | **NEVER · 23 · 19 min** | 9 s · 1 | 6 s · 1 |
| Secondary | 2 s · 2 | 9 s · 1 | 3 s · 2 |
| **Stage** | **19 m 20 s** | 16 s | 29 s |

#### ✗ Two results that undercut the story, recorded rather than smoothed

- **Primary released on a *prepared* attempt in the third run.** Attempt one under
  `FirstAttemptOnly` is byte-identical to the old behavior, and the call that failed 23
  consecutive times an hour earlier succeeded in six seconds. **The CFexpress is not reliably
  poisoned by preparation.**
- **The device that struggles changes run to run** — Primary once, WD once, neither twice, and
  WD had never fought before. That reads as per-night luck rather than any device being special.

**So the fix is plausible, costs nothing, and rests on `n` = 1 per mode against a phenomenon
firing roughly once in six runs. A base rate MUST be established before any default moves.**

#### The veto descends the stack, and its bottom rung is winnable

**Observed twice, both ending in release.** WD, third run:

```text
#1  type 6  SCSI\Disk&Ven_WD&Prod_My_Passport_264F   the disk object
#3  type 6  STORAGE\Volume\{0d6e2e37-...}            the volume object
#4  type 5  STORAGE\Volume\{0d6e2e37-...}            outstanding open → 250 ms retry
#5  RELEASED
```

**`PNP_VETO_TYPE(5)` has been the last stop before success both times it has appeared.** Visible
only because the reason prints *on change* — printing the last one shows no story, printing every
one buries it.

#### Filesystem check: the archives are NTFS, only the cards are exFAT

| | |
|---|---|
| `C:`, SanDisk, OWC, WD | **NTFS** |
| both cards | **exFAT** |

**So the flush guarantee is belt-and-braces on the archives and pointless on the cards.** NTFS
journals its metadata, so a surprise removal is recoverable; exFAT has no journal, and exFAT is
only where this tool never writes. **This removes the argument for treating the two device
classes differently** — which is why the fix can be one uniform rule.

#### Two things worth carrying

- **The gap rule on a clean track costs 1.0 %.** 76 of 7,395 frames refused, all inside one
  recording, **widest gap 27 s** — under the 60-second limit, so these were rejected on
  *distance*: fast movement opening 100 m inside half a minute. Against 38 % lost on the holey
  2022 track, this is the clearest evidence yet that keeping both corpora tests different things.
- **A new high for the CFexpress at pre-flight: 1,156 MB/s**, against a recorded burst range of
  913 / 842 / 975 for that card. The quietest machine this project has measured on is the obvious
  candidate and is **not** recorded as the cause.

**Declared, per this document's own standard:** `iSCSIAgent` (SYSTEM, 9.8 CPU-s) ran throughout
and could not be stopped unelevated; `AdobeIPCBroker` respawned once and was killed again;
`watch-rig.ps1` polled at 2 s throughout, metadata only. All five rig drives measured
**0.00 MB/s** at launch. The binary was built clean at `aacef77` and HEAD was `16057fb` — the
commits between are docs-only, so `cargo build --release` correctly had nothing to do.

### The interleaved verify run, 2026-08-05 — LANDED 10 m 55 s

**The fastest LANDED on record, and the first run exercising `unbuffered_sha256`'s
read/hash interleave against a real day.** Cold boot, 20-minute settle, fresh destinations,
exit 0, `EJECTED — SAFE TO STORE`.

| | This run | Cold-cache fresh run, 2026-08-04 |
|---|---|---|
| Write pass | 6 m 44 s | 7 m 47 s |
| **Verify pass** | **4 m 11 s** | 5 m 41 s |
| **LANDED** | **10 m 55 s** | 13 m 28 s |
| Whole run | 27 m 06 s | 34 m 51 s |

**The verify pass fell 26 %, and that is the interleave doing exactly what
`examples/verify-rate.rs` predicted.** Per-destination rates derived from `verified_utc` in
the run log — the run measuring itself, not a probe:

| Destination | In-run | Bench (interleaved) | Prior serialized |
|---|---|---|---|
| laptop | 1,511 MB/s | — | 1,150 |
| OWC | 1,478 | 1,670 | 1,077 |
| SanDisk | 1,104 | 1,134 | 409 *(on the dock)* |
| **WD** *(sets the pass)* | **798** | 828 | 590–597 |

**The WD came within 3.6 % of its bench figure and the SanDisk within 2.6 %**, under real
conditions with three other destinations verifying concurrently. A bench number that survives
contact with a live run is worth more than the bench.

> **The write pass also fell 63 s and that is *not* the interleave** — the interleave touches
> only the read side. It is recorded as **unattributed**. The strongest candidate is the
> Defender exclusions set by extension and process earlier the same day (decision 9), since
> freshly written files are exactly what Defender scans; the SanDisk's move to a hub TB5 port
> is a weaker second. **Neither has been measured, and this is the kind of gap that gets
> filled with a plausible story if it is not written down as open.**
>
> ⚠ **And then it was filled with a plausible story anyway, the same evening, by the session
> that wrote that sentence.** Recorded in full because the warning and the violation are hours
> apart in one conversation, which is more instructive than either alone.
>
> **The story:** live sampling during a run showed the four destinations writing at ~490 MB/s
> against this document's recorded ~292, and that 68 % gap was offered — repeatedly, and out
> loud — as evidence for the Defender exclusions. **Two things were wrong with it.** The ~490
> was an *instantaneous sample* of a few seconds, not a pass average. And the ~292 it was
> compared against is a **per-device probe figure** from the wall-clock table, gathered a
> different way entirely. Two numbers from two instruments, with a conclusion drawn from the
> gap between them — which is precisely what *Measurements are evidence* forbids.
>
> **What the run logs actually say**, derived from `verified_utc` and LANDED rather than from
> sampling:
>
> | | write pass | per destination |
> |---|---|---|
> | 2026-08-04, cold-cache measured run | 7 m 47 s | **431 MB/s** |
> | 2026-08-05, casual run with the exclusions in place | 8 m 30 s | **395 MB/s** |
>
> **Slower, not faster** — and that comparison is worthless too, because the second run had no
> reboot, warm caches, and a session compiling and running tests on the same machine
> throughout. It measures contention, which is the same error one level up.
>
> **So the honest state is unchanged from when it was first written: unattributed.** The
> Defender exclusions' effect on write throughput has never been measured, and settling it
> needs a [`FULL-RUN.md`](FULL-RUN.md) run with them removed. The 63 seconds remain unexplained.
>
> **The generalisable part is not about Defender.** A gap explicitly marked *open* is not
> protection against filling it — an open question is an itch, and a candidate written beside
> it reads as a lead. What protects it is refusing to *quote* the candidate until it has been
> measured, which is a discipline about speech rather than about record-keeping.

**Total I/O: 9N = 1,811.7 GB in 655 s — 2.77 GB/s, 22.1 Gbps sustained**, with 805 GB of it
read unbuffered and 805 GB written write-through, from a cold cache. Worth stating because
the design forbids every shortcut that would inflate it.

> **A property worth knowing: 9N holds for convergence runs too.** A skipped file still costs
> an `unbuffered_sha256` of the target in `place()` to prove the hash matches, plus its verify
> read — two units, the same as a written file's write-plus-read. **Convergence does not move
> less data; it moves the same data with 4N shifted from writes into reads.** That is why a
> convergence LANDED of 13 m 04 s beat a fresh 16 m 35 s without being a different amount of
> work.

**What the run also confirmed, each by an instrument that could have said no:**

- **`0 mismatched` across 15,532 pairs.** The rewritten verify function decides whether an
  archive is declared clean; it got that right 15,532 times on real data.
- **Progress reporting works redirected.** Every tick landed on its `⌈3883/10⌉` boundary in
  `Lines` mode — the path that renders nothing under `indicatif` and would have shipped
  invisible.
- **All five removable devices released**, and `scripts\watch-rig.ps1` reported five
  independent `- DETACHED` events from the storage stack. The tool's claim and the OS's
  agreement are different facts and this run has both.
- **The reader asymmetry held exactly**: the ProGrade Thunderbolt router stayed `OK`; the USB
  SD reader went to `Error`, powered down with its card.
- **Corroboration: 3,883 matched, 0 mismatched, and zero transient SD read errors** — the
  second consecutive clean pass on that card.

**Two caveats carried honestly.** Two `watch-rig.ps1` pollers were running throughout, hitting
the storage stack every 2 s — metadata only, the SD reader measured 0.00 MB/s, and almost
certainly immaterial, but *almost certainly* is a judgment and this project records those. And
**the earlier claim that all four baseline runs were fresh was asserted rather than checked**;
at least one recorded run was convergence, so the comparison above is anchored to the
2026-08-04 cold-cache run specifically, which *is* documented as written fresh.

**Still to build:**

- ~~wiring phase 4 to the CLI~~ — **done 2026-08-04.** `manifest::corroborate` resolves the
  pending entries, and a single-source run records *waived* rather than leaving the record
  ambiguous. The tombstone path is unit-tested and has never fired on real hardware, which is
  correct: forcing a mismatch would mean writing to a camera card
- ~~eject~~ — **built and proven on the rig, 2026-08-04/05.** Both bus types power down
  completely, the disk leaving the disk list rather than merely unmounting: Thunderbolt in
  **2.1 s**, USB in **2.9 s** on an idle drive. At the end of a real run it is less certain —
  **4 of 17 device ejects were vetoed on their first attempt** across five runs — and the
  whole-sequence retry has been observed recovering one in 15 s. **That is the three archive
  SSDs only** — the cards are covered separately below
- ~~**releasing the camera cards**~~ (decision 22) — **done 2026-08-05.** A dismount released
  nothing, so both cards sat in the tray after every run that claimed otherwise; they now take
  the full lock/dismount/power-down. Media eject turned out to be a dead end twice over — it
  reports success and releases neither card, and the physical-drive handle that might have
  behaved differently needs administrator rights. **Proven inside a real offload 2026-08-05**:
  both cards released at the end of a 201 GB run, confirmed by `scripts\eject-check.ps1`
  rather than by the report that had been lying about this the night before. The reader
  asymmetry held exactly as measured — the ProGrade router stayed enumerated, the USB SD
  reader powered down with its card
- **the report** of decision 14 — the verdict shape exists in outline, not in full
- ~~**progress output while a phase is running**~~ — **done 2026-08-05.** The run used to print
  `ingesting 3,883 files...` and then nothing for twelve minutes, then nothing again for the
  sixteen phase 4 takes; the operator resorted to watching the SD reader's LED to work out which
  phase was running, and said the thing that made it a defect rather than a polish item:
  *"I feel like I shouldn't need to guess at that."* **Decision 22 had already won this argument
  for a different stage** — eject became a *timed* stage because "an unlabeled twenty-minute
  silence reads as a hang while a timed one reads as persistence" — and the conclusion was never
  carried across to the phases that take twelve and sixteen minutes rather than fifteen seconds.

  **`crates/offload/src/progress.rs` is a three-way enum, not a display and a no-op**, and the
  reason is the trap it nearly shipped with: `indicatif` hides itself when its stream is not a
  terminal, so bars would have rendered *nothing* in exactly the mode this feature is most
  needed — captured to a log, which is how every run driven by Claude behaves. It would have
  looked correct every time the operator tested it by hand. So: `Bars` at a terminal, `Lines`
  (throttled plain text on stdout) when redirected, `Silent` for tests, with
  `examples/progress-demo.rs` as the harness because a display cannot be unit-tested. See
  [`REVIEWING.md`](REVIEWING.md), *A diagnostic that cannot fail*, and the memory note
  *verify in the mode it will run*.
- **`sync`** (decision 20), the recovery path `--without` implies
- **log-driven resume** (decision 13). Convergence already works, via decision 5's
  skip-on-identical-hash, but it re-reads what the run log could have told it
- ~~`source_card` records an assumption~~ — **fixed 2026-08-05.** It now carries the role
  (`primary`, or `sole` under `--allow-single-source`) beside a new `source_volume_serial`
  holding what was actually observed. Decision 12 has the reasoning and the schema-compat
  trap it walked into. **Proven on the rig 2026-08-05**: one unique `source_volume_serial`
  across all 3,883 manifest entries and every run-log record — `0E7A-0533`, the serial of the
  card pre-flight actually measured and chose as the source
- **the body check** (decision 34) — name the camera in the config, compare one frame per
  card at pre-flight, print it as INFO beside the timezone line. **Unblocked 2026-08-05 and
  still unbuilt**: the measurement it waited on came back the good way — the R5 writes
  `CameraSerialNumber` into standard EXIF, so `exif.get(ExifTag::CameraSerialNumber)` reaches
  it with no MakerNote decoding and no new dependency (`crates/geotag/examples/body-identity.rs`
  is the probe). What remains is a `body` field in the config, one frame read per card at
  pre-flight, and the INFO line — plus the standing instruction in [`../CLAUDE.md`](../CLAUDE.md)
  that Claude acts on that line every time it appears
- **the Defender exclusion check** (decision 9). **The exclusions themselves were set
  2026-08-05** — by extension and process rather than by path, for the reasons decision 9
  records — so what remains is the tool *reading* them at pre-flight and saying whether they
  are in place, including the unreadable-registry outcome. That is still the only consumer
  `windows-registry` has, which is why it stays a declared-but-unimported workspace dependency
  ([`TRIP-HYGIENE.md`](TRIP-HYGIENE.md))
- **naming whoever actually holds a vetoed volume.** Every claim that Defender or the
  indexer is responsible is inference: the veto names the *volume device object*, never a
  process, and the suspect has never been identified. `handle.exe -a -v <volume>` and
  `fltmc instances -v <volume>`, elevated, at the moment of a stuck eject would settle it.
  **Decision 22's retry made this possible** — it holds the failure open for up to an hour,
  where previously the moment passed before anyone could look. Do this before concluding
  anything about Defender
- **a third card is silently ignored** (decision 7). `preflight::phase1` refuses at zero
  cards and at one, and has no upper bound; `cards::choose` sorts by speed, takes the fastest
  as the source and `measured.next()` as the corroborator, and **drops every card after the
  second without a word.** Decision 7 is titled *there are always two* and that premise is
  simply unenforced at the top end.

  **Found 2026-08-05 while a spare SD card was being acceptance-tested**, which is what makes
  it worth recording rather than theoretical: loading a card with test frames gives it a
  `DCIM`, and a `DCIM` volume that is not a configured destination *is* a card as far as
  decision 7's rule is concerned. Three readers on one hub is not an exotic rig, and the
  spares list in memory holds two spare SD readers.

  > **Corrected within the hour, and the mistake is the more useful half.** This entry first
  > said *"there are three of them on this machine tonight."* There were **two**:
  > `full-run-check.ps1` reports `cards found — 2 volume(s) with DCIM`, because the test card
  > was sitting *in the corroborator's reader* rather than beside it. The hazard is **latent,
  > not present** — it needs a second SD reader and a loaded card at the same time.
  >
  > **The gap itself is unaffected**, which is why the entry stands rather than being deleted:
  > the code has no upper bound whatever the rig currently holds. What was wrong was a claim
  > about live state, asserted from an inference — *the test card has a DCIM, therefore three
  > cards* — without running the two-second check that would have answered it. That is the
  > same shape as the memory note *a comment explaining why is a claim*, and the same shape as
  > the disk-renumbering write-up two days earlier: **the reasoning was sound and the premise
  > was never looked up.**

  **It fails safe and that is the problem.** If the stray wins the corroborator slot, decision
  27's gate refuses on a listing mismatch — correct, but the refusal names a card the operator
  did not think was in play. If the real corroborator wins, the run proceeds normally and
  nothing ever says a third card was seen. Either way the tool knows something the operator
  does not, which is the shape [`REVIEWING.md`](REVIEWING.md) — *A diagnostic that cannot
  fail* — exists to catch.

  **The fix is decision 7's own argument applied at the other end**: one card is an equipment
  failure rather than a mode, and three is equally not a mode, so pre-flight should say what
  it found and refuse. What it must *not* do is pick two and carry on quietly
- **the card degradation check** (decision 32) — record pre-flight's existing speed
  measurement per card, warn when a card falls off its own history. Record first; the
  threshold is set from accumulated evidence, not chosen up front
- **the throughput history itself** (decision 33), which 32 depends on — one JSON file at
  `%APPDATA%\offload\history.json` covering cards *and* destinations, each sample carrying
  uptime and link generation. **Backfillable on day one**: every run log on the laptop
  already holds the per-destination rates, so this starts with history rather than empty
- ~~**overlapping the verify read with its hash**~~ — **done 2026-08-05**, 20–25 % across every
  destination and 20 % on the WD, which is the one that binds. Interleaved SHA-256 now beats
  serialized BLAKE3 on both USB drives. **What remains is per-file pipeline drain**: a ~52 MB
  raw is ~3.3 chunks, so each file starts with an unoverlapped read and ends with an
  unoverlapped hash. Pipelining across files would recover most of the remaining quarter —
  the OWC reaches 1,670 against a 2,380 theoretical
- *(superseded, kept for the reasoning)* **overlapping the verify read with its hash** — was
  described here as the binding constraint on LANDED and the largest remaining lever. With the archive SSDs on their own laptop ports
  they are no longer tunnel-limited: on 2026-08-04 every destination verified within ~12% of
  `1/(1/read + 1/hash)`, so drives that read at 934–980 MB/s verified at 590–597. Worth far
  more than the hash choice ever was; see decision 17 and the cold-cache run below
- **retiring RawGeotag** into `offload geotag` (decision 30), now unblocked by phase 5
- ~~proving decision 22's eject retry~~ — **done 2026-08-04.** The OWC was vetoed on its
  first attempt and powered down on its second, 15 s later, at the end of a full run; the
  other two succeeded first time. Decision 22 has the output. What is still worth
  accumulating is the *distribution* of attempts and wait times across many runs, which the
  report now prints per device

### The first cold-cache run, 2026-08-04 — LANDED 13 m 28 s

**The first run taken under [`FULL-RUN.md`](FULL-RUN.md)'s procedure**: rebooted for a cold
page cache, topology verified before launch, exit 0, all 15,532 pairs verified, all four
destinations written fresh. One caveat carried honestly — four destination directory trees
were walked before launch, which warmed some MFT entries. Metadata only, no file data.

| | This run | Previous, both USB SSDs on the dock |
|---|---|---|
| **LANDED** | **13 m 28 s** | 16 m 35 s |
| Verify pass | **5 m 41 s** | 8 m 12 s |
| Total | 34 m 51 s | 16 m 55 s (no corroboration) |

**Splitting the archive SSDs onto the laptop's own ports delivered 3 m 07 s of a predicted
4 m 30 s.** The shortfall is the finding: the probe measured raw reads of 980 + 934 MB/s,
the run delivered 597 + 590, and the gap is `unbuffered_sha256` serializing read and hash.
Against each device's measured read ceiling and SHA-NI's 2,380 MB/s, `1/(1/r + 1/h)` predicts
694 / 671 / 1,032 against 597 / 590 / 958 observed — every destination within ~12%. **The USB
drives are no longer tunnel-limited; they are limited by the verify loop.** This also weakens
the enclosure purchase, since PCIe would free them into a ceiling they can no longer reach.

**Decision 3's re-read safeguard fired on real hardware for the first time:** `3,878 matched ·
5 transient read error(s), re-read agreed · 0 mismatched`. Five SD reads failed, all five
agreed on re-read, nothing was quarantined and nothing deleted — exactly the reasoning that a
mismatch may be a transient reader error rather than media corruption. The destructive path
still remains unexercised outside unit tests, which is correct.

**And the run exposed a defect in its own exit code**, now fixed: decision 18 enumerates
about eight exit-2 conditions and the code tested one, so a run that ended with two archive
SSDs un-powered-down and a verdict saying to deal with them by hand still exited 0.

**Settled by measurement, 2026-08-04 — recorded because both were open for a while and
both changed a conclusion:**

- **The SD path was a faulty card, not the slow reader everyone blamed.** A second SDXC
  card in the same reader read **205 MB/s against 73**, cutting phase 4's corroboration on
  a big day from ~46 minutes to ~16. Since decision 22 holds the eject until corroboration
  finishes, that is **~30 minutes off when the disks can go in the safe**, for the price of
  retiring a card. **Travel with the AngelPro; the Lexar Silver Pro is out of service.**
  The reasoning failure that hid it for months is in
  [`REVIEWING.md`](REVIEWING.md) — *change the variable you have not changed*
- **`examples/contention.rs` is not measuring its own ceiling**, or not much of one. A
  thread-count sweep (`examples/threads.rs`) found the single-threaded probe understates by
  9–14% on fast devices and not at all on slow ones, so every contention number above
  stands as a slight underestimate. The apparent anomaly — two Thunderbolt devices summing
  below one alone — was a comparison error, not an instrument fault: the hub's USB traffic
  rides the same link, and everything together came to 3,344 MB/s against 3,065 for the OWC
  alone

**The travel case was fully exercised on 2026-08-04 and everything still in it passes.**
Three CFexpress cards read 834–1,135 MB/s and two SDXC cards 205–222, against a bar of
~292 MB/s — what a single destination absorbs, above which the card is not the constraint.
Both SD readers proved genuine UHS-II, delivering an identical 222 MB/s with the same card,
so the long-running suspicion that they were old and needed replacing was simply wrong. One
bad SD card explained the whole thing. Per-card figures live in the session memory.

**The TB5 hub was tested on 2026-08-04 and does not help — the last open hardware question,
closed by a negative result.** A CalDigit Element 5 was expected to raise USB tunnelling
from 10 to 20 Gbps and win perhaps 3 minutes of phase 3. Measured on a quiet bus, the USB
devices together reach **935 MB/s against the TB4 hub's 931** — the same tunnel.

**The constraint is the laptop, not the dock.** The hub enumerates as `USB4 Router (2.0)`
and is genuinely capable, but this machine's host is **`USB4 Root Router (1.0)`**; a link
negotiates to the lower end, so the tunnel stays USB4 v1. **No dock can lift this** — only
a host with a USB4 v2 router would, which means a different laptop. Every prior note on
this guessed the risk lay in the dock's USB controller, and named the wrong component.

The hub does deliver PCIe headroom on devices that were never binding: the OWC went
1,333 → 1,825 MB/s in company, the Thunderbolt CFexpress reader 1,112 → 1,153. Neither is
on the critical path, so neither moves the wall clock.

**Then the money question dissolved: split the drives across the laptop's own ports and buy
nothing.** Both archive SSDs had been on the dock, sharing one 10 Gbps USB4 tunnel with the
SD reader. Moving each to a laptop USB-C port gives it a native link on its own controller —
the SanDisk on the USB 3.2 xHCI, the WD on the 3.1, the OWC still on Thunderbolt, and the
card readers left with the tunnel to themselves.

| Read contention, *together* vs *together* | on the dock | split across ports |
|---|---|---|
| SanDisk | 360 MB/s | **980** |
| WD | 360 | **934** |
| **combined** | **720 MB/s** | **1,914 — 2.7×** |

Every device keeps ~100 % of its solo rate. **The verify pass should fall from 8 m 12 s to
roughly 3 m 45 s — about 4.5 minutes off a 16 m 35 s LANDED.** The TB5 hub bought nothing and
a new laptop would have bought ~3.5 minutes; **two cables bought more than either, for free.**

The cost is operational rather than financial, and it is the operator's call: it trades the
hub's single-connector hotel ritual for three cables to the laptop each night.

> ### ✗ Corrected 2026-08-05: Gen 2x2 *is* available — on the hub's TB5 ports, not the laptop's
>
> **Measured with `examples/sustained.rs`, 60 s each, quiet bus, same drive and same probe.**
> The operator plugged the SanDisk into an Element 5 **TB5 port** rather than a USB-A one — a
> configuration nobody had tried, since every earlier hub measurement used USB-A:
>
> | Path | Sustained |
> |---|---|
> | **Element 5 TB5 port (USB-C)** | **1,435 → 1,398 MB/s** — flat, ~11.2 Gbps |
> | Element 5 USB-A port | 967 → 957 |
> | Laptop's own USB-C port | ~980 |
> | Both SSDs on hub USB-A | 360 each |
>
> **Everything is pinned at the practical 10 Gbps ceiling except the TB5 port**, which beats a
> laptop port by **43 %** on the one drive bought for Gen 2x2 in the first place.
>
> **The USB-A control used a USB-C-to-USB-A adapter, and that confound does not matter** — a
> USB-A connector has a single lane pair and *cannot* carry Gen 2x2 whatever the adapter does,
> so the port was capped by its connector before the adapter entered the picture. The adapter
> and the port share one ceiling, and 964 MB/s is it.
>
> **What is measured and what is not.** The numbers are solid: flat across 60 s, ~84 GB read,
> no cache can fake that. **The mechanism is not** — the paragraph below argues Gen 2x2 should
> be impossible on a USB4 port because "USB4 claims those same lanes", and the parent chain
> shows the drive *tunnelled* through the laptop's xHCI rather than natively attached. So
> either USB4 v2 tunnels Gen 2x2, or the port does something else. **Recorded as an
> unexplained measurement rather than dressed in a mechanism**, which is the error this
> document made about a 4K display earlier the same day.
>
> ### Both remaining questions answered, 2026-08-05 — and the answer is a new standard rig
>
> **The WD is not Gen 2x2.** On the same TB5 port that gives the SanDisk ~1,400, it sustains
> **943 · 928 · 941 · 951 · 944 MB/s** — flat, the same ~10 Gbps ceiling it hits on a laptop
> port and on USB-A. **A TB5 port buys it nothing**, which collapses the "do two SSDs contend"
> question: only one drive here ever wants that port.
>
> **The TB5 ports do not contend**, which was the question that actually mattered — the
> CFexpress reader is the phase 3 *source* while a destination writes, so that pair genuinely
> coexists in a run. Reading both together, 4K display still attached:
>
> | | Alone | Together |
> |---|---|---|
> | CFexpress (source) | 1,266 MB/s | **1,294 — 102 %** |
> | SanDisk (destination) | 1,464 | **1,487 — 102 %** |
> | **Sum** | 2,730 | **2,781 MB/s ≈ 22.2 Gbps** |
>
> Both *gained* marginally in company, reproducibly across two passes — noise rather than a
> real gain, but decisively not a loss. **22.2 Gbps of storage plus a ~12.5 Gbps display tunnel
> on one 40 Gbps link, and nothing gives.**
>
> **The display stayed attached on purpose.** Its cost was measured this morning at 0.7 %,
> inside the noise, and it is part of the standard rig — so a number taken without it would
> describe a configuration nobody runs. **Removing a settled variable makes a measurement less
> representative, not more.**
>
> ### And laptop ports are not interchangeable, which this document had been flattening
>
> **The left ports are Thunderbolt 4; the right is USB-only with DP out**, which Dell's
> documentation recommends for projectors. The discriminator in `Get-PnpDevice` is which
> controller a device lands on: **`Intel USB 3.10` is the right-hand USB-only port, `3.20` is
> the Thunderbolt side.** Only a left port can carry the OWC's PCIe tunnel.
>
> Classifying a device by *what it negotiated* rather than by *what the port is* hides that
> distinction, and it is the same error shape as the parent-chain probe that could not see the
> hub. **Every earlier "laptop port" figure in this document should be read as "whichever port
> it happened to be in."**

> **The consequence is real and not yet acted on.** `CONOPS.md` says the two USB SSDs go in the
> laptop's own ports and *not* the hub — a rule measured against the hub's **USB-A** ports and
> silent about TB5. **For the SanDisk that advice is now backwards.** What is still unknown:
> whether the WD is Gen 2x2 at all (My Passport SSDs are typically Gen 2x1, so probably not),
> and whether two devices on TB5 ports would contend. Both are one measurement each, and the
> wiring should not change until they are taken.

**Gen 2x2 was tested separately and is not available on this machine.** The SanDisk supports
it; on a native USB 3.2 controller port it still measured 1,051 MB/s best-of-threads against
1,034 through the tunnel — ≈8.4 Gbps, the practical 10 Gbps ceiling. The link negotiates
Gen 2x1 regardless of what the drive can do.

**The superseded recommendation, kept because the reasoning still holds if the ports are ever
needed elsewhere: buy an enclosure, not a laptop.** The OWC
is a `USB4 Router (1.0)` carrying a **PCIe** tunnel instead of the USB one, which is why it
verified at 1,077 MB/s and kept 95 % of its solo write rate under load while the two USB
drives sat at 409. TB4 offers ~3.3 GB/s of aggregate PCIe and one device uses it. Putting
both archive SSDs in OWC Express 1M2-class enclosures takes them off the contended tunnel
entirely — the same ~3.5 minutes a new laptop would buy, for a fraction of the price, and it
sidesteps the USB4 v1 host rather than trying to outspend it.

**One correction worth keeping, because it sounds wrong:** Thunderbolt 4 is **not** a superset
of USB 3.2. USB4/TB4 tunnels USB 3.2 **Gen 2x1 — 10 Gbps** — and Gen 2x2 is not tunnelled at
all. The two are mutually exclusive in the connector: Gen 2x2 reaches 20 Gbps by using *both*
USB-C lane pairs for USB3 signaling, and USB4 claims those same lanes for its own link. The
SanDisk here supports Gen 2x2 and cannot use it on this machine — the one native USB-C port
enumerates on the **USB 3.1** controller, whose ceiling is 10 Gbps, and the TB4 ports tunnel
at 10.

**One open hardware question remains, and it is blocked on an instrument rather than on
hardware:**

- **Are the archive SSDs tunnel-limited or drive-limited when *writing*?** The verify half is
  settled — 409 MB/s each against 724/703 solo reads, so that half is the tunnel. The write
  half is not, and `examples/write-contention.rs` cannot currently answer it: it reported
  every device keeping **>100 %** in company, and its untouched control drifted **24 %**
  between runs. It measures the solo and together phases with the drives in different states,
  and its 12 GB sample fits inside an SLC cache that the real 201 GB run exhausts. **Fix the
  instrument before quoting it.** This decides whether enclosures buy ~3.5 minutes or ~7.

**A run measured on the desk dock is not a measurement of this tool.** Terry travels with the
Element 5 and had been testing on the desk dock out of convenience, so every wall-clock figure
before 2026-08-04 described a rig that never leaves the house. The two hubs turned out to be
equivalent for USB, so nothing had to be retracted — **that was luck, not method. State which
hub a timing came from.**

**Two gaps recorded rather than closed.** Neither is a defect today. *No test can prove
the two file flags are still set*: removing `FILE_FLAG_NO_BUFFERING` changes where bytes
come from, not what they are, so every assertion still passes — the constants are
load-bearing on inspection only, and the comment in `winio.rs` says so where someone
might delete one. And *a file that rots after a clean run* is not phase 3's to notice; it
is what `offload verify` exists for (decision 20).
