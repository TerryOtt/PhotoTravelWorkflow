# Open work

**The durable copy of the working checklist.** A session's task list does not survive the
session, and on 2026-08-06 that list was the only record of four open items — Terry's own
words: *"that's my memory right now and that's dangerous."*

**Claude MUST update this file when an item opens, closes or materially changes**, in the same
turn, not at the end of a session. RFC 2119 keywords, and the capitals are load-bearing.

> **⚠ THIS FILE IS NOT THE FIRST THING TO READ. If `RUN-STATE.json` exists at the repository
> root, [`FULL-RUN.md`](FULL-RUN.md) governs and MUST be read before any other tool call** — a
> measured run is staged or in flight, and nothing here matters until it is finished.
>
> **The reason is that a staged run is perishable and this file is not.** A cold page cache
> bought with a reboot, wiped destinations, a settled machine: picking up an interesting item
> from this list spends all of it on a probe or a walk of the archive trees, and the reboot was
> for nothing. **The backlog will still be here afterwards.**

This is a *backlog*, not a design document. [`DESIGN.md`](DESIGN.md)'s **Still to build** list
is the scope of the product; this is what is in flight right now and what state it is in.

> **The numbers below are positions, not identifiers, and they renumber when an item closes.**
> Cite an item by its **title** anywhere that has to survive — a commit message, a task
> description, another document. "Backlog item 5" was accurate for four hours.

> **The CLI checklist shows at most five items**, so on a list this length the newest one is
> invisible to Terry. **This file is therefore the only complete copy**, not a backup of his
> screen — see [`../CLAUDE.md`](../CLAUDE.md) on keeping the two synced.

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

## 2. SanDisk 512 GB SD acceptance test

Card arrived 2026-08-06; **step 1 done** — Terry low-level formatted it in the R5 before
anything else touched it. Remaining: confirm the PnP parent chain shows SuperSpeed, get frames
on it, read the **second** sustained pass. Bar is the fleet range, 205–247 MB/s; the known dud
did 73.

**Planned confound:** the SDDR-409's own ceiling is 247, so a ~247 result cannot separate card
from reader. Read it in the Lexar LRWM04U as well.

## 3. Characterize all three UHS-II USB SD readers

One known-good card through all three, so Terry knows every reader in the bag is safe to travel
with. A slow reader is silent in the field — the card mounts, every file reads, nothing errors,
and you lose 5.8×. Combines with the acceptance test above into a 2 cards × 3 readers matrix.

## 4. Zoom out over the badge and verdict work — NOT YET

**Opened 2026-08-06, deliberately deferred by Terry while the CLI work is still moving:** *"we're
gonna have some GOOD doc comments and doc changes flowing out of this. We've made a LOT of
substantive changes that need a zoom out, but not yet."*

**What accumulated in one evening**, all of it committed and none of it yet reviewed as a whole:

- The badge column as a single go/no-go on unplugging, and yellow as a stop signal rather than a
  severity
- Red banned outright, including the `LANDED` block's last carve-out
- `#FFFF00` true colour, never bold, and *why* — two causes were dulling the same badge
- `Eject` reclassified as a container rather than a step, with `Progress Log`, `Travel SSDs`,
  `Cards` and `Safe to Unhook` as its steps
- **`SAFE TO STORE` reserved for when nothing is mounted**, which was a real defect

**The risk this item exists to catch:** each change was argued in its own commit and its own doc
comment, and several of them *supersede* text elsewhere rather than adding to it. **Nobody has
read the result end to end.** That is exactly how decision 14's layout rules came to contradict
`progress.rs`, which item 2 is still carrying.

**Do this after the CLI signoff closes**, and treat it as a documentation review rather than a
code one — the code is tested; the prose is not.

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
- **Terry's signoff on the CLI output — explicitly given 2026-08-06.** The badge column, the
  colours, the `Eject` restructure, the `SAFE TO STORE` defect and the verdict badge all landed
  and were reviewed on both the 4K monitor and the laptop. **The one leftover is prose and moved
  to the zoom-out**: `progress.rs` still argues with itself about erasing the Writing/Verifying
  block, while the code is now correct
