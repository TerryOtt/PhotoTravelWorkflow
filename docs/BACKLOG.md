# Open work

**The durable copy of the working checklist.** A session's task list does not survive the
session, and on 2026-08-06 that list was the only record of four open items — Terry's own
words: *"that's my memory right now and that's dangerous."*

**Claude MUST update this file when an item opens, closes or materially changes**, in the same
turn, not at the end of a session. RFC 2119 keywords, and the capitals are load-bearing.

This is a *backlog*, not a design document. [`DESIGN.md`](DESIGN.md)'s **Still to build** list
is the scope of the product; this is what is in flight right now and what state it is in.

---

## 1. Eject vetoes — cause found, fix built, not yet proven

**Status: the most advanced and the least finished.** Full account in
[`DESIGN.md`](DESIGN.md) decision 22; the run tally is in [`EJECT-SERIES.md`](EJECT-SERIES.md).

- **Cause** — the retry re-dismounted before *every* attempt, so it never asked about a settled
  volume. exFAT refuses a freshly remounted one with `PNP_VETO_TYPE(6)`, which never yields.
- **Base rate** — established. **2 hangs from 2 runs** at `every-attempt`.
- **Fix** — `Prepare::FirstAttemptOnly`, built and pushed. Flushes once, then stops disturbing
  the volume.
- **Missing** — repeat runs. **One B run, zero hangs**, against a phenomenon that fires
  reliably. **The default MUST NOT move on that.**

**Next action:** more B runs, alternating with A. Each costs five cable replugs.

## 2. Terry's signoff on the CLI output

Every question he raised on 2026-08-06 is answered and shipped. **Two known open threads:**

- **The Writing/Verifying block vanishes before LANDED**, because `progress.clear()` erases it.
  He wants it kept. `progress.rs`'s `finish()` doc argues for keeping it and `clear()`'s doc
  argues for removing it — **the file contradicts itself** and the behavior follows `clear()`.
  Note the constraint: `MultiProgress` repaints wherever the cursor is, so a plain `println!`
  beside live bars *collides* — on 2026-08-05 that drew the LANDED banner inside eight progress
  rows. A static re-print after clearing is the achievable version.
- **Terminal width** — the geotag gap explanation is ~133 characters and has only been seen on
  a 4K display. **The laptop is what travels.** Check it wraps acceptably.

## 3. SanDisk 512 GB SD acceptance test

Card arrived 2026-08-06; **step 1 done** — Terry low-level formatted it in the R5 before
anything else touched it. Remaining: confirm the PnP parent chain shows SuperSpeed, get frames
on it, read the **second** sustained pass. Bar is the fleet range, 205–247 MB/s; the known dud
did 73.

**Planned confound:** the SDDR-409's own ceiling is 247, so a ~247 result cannot separate card
from reader. Read it in the Lexar LRWM04U as well.

## 4. Characterize all three UHS-II USB SD readers

One known-good card through all three, so Terry knows every reader in the bag is safe to travel
with. A slow reader is silent in the field — the card mounts, every file reads, nothing errors,
and you lose 5.8×. Combines with item 3 into a 2 cards × 3 readers matrix.

## 5. Put the docs and tests on a diet — LOW PRIORITY

186 tests and a 3,600-line `DESIGN.md`. **The count is not the metric**; most of those tests are
regressions for defects that actually shipped, and the "considered and rejected" material exists
to stop re-proposals. The real fat is the same argument restated in three places, and the likely
answer is structure rather than deletion — splitting run records out of `DESIGN.md` the way
`EJECT-SERIES.md` already was.

**Do this when the eject work is settled**, not during it.

---

## Closed 2026-08-06

Kept briefly so a resumed session does not re-open them.

- Display rounding — whole GiB, one decimal below 10 GiB, rates to one decimal
- `Released 5 devices` miscount when one device was vetoed
- Eject split — SSDs report the moment they are down, cards separately
- Build-chain freshness row in `full-run-check.ps1`, with BEHIND blocking
- Cards ejected sequentially, so Primary could starve Secondary
- A card already ejected reported as `still mounted`
- **OBE** — concurrency as the veto cause, overtaken by the settle-time explanation
