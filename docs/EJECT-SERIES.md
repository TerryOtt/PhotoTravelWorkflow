# The eject A/B series

**What this is for:** establishing whether `Prepare::FirstAttemptOnly` actually fixes the eject
veto, or whether it merely happened not to fail once. Started 2026-08-06.

**RFC 2119 keywords, and the capitals are load-bearing.**

[`DESIGN.md`](DESIGN.md) decision 22 has the cause, the traces and the reasoning. This file has
only the running tally, so a session that has to add a row does not need to re-read the argument.

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
