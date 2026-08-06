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

**This file is the permanent record. The CLI checklist is a *working set*, and the two are not
the same list.** Standing order, Terry, 2026-08-06: *"as soon as something is blocked or complete,
remove it from the CLI UI checklist. BACKLOG is permanent memory, UI checklist is only stuff
that's both a) eligible to be worked, and b) not complete."*

| | Appears in the CLI checklist |
|---|---|
| Eligible to be worked, not complete — **by either of them** | **yes** |
| **Waiting on Terry** — a format, a shoot, a cable swap | **yes.** This is *not* blocked; it is his move, and the checklist is how he sees it |
| **BLOCKED** — outside both their control: hardware not delivered, a vendor, a release not shipped | **no.** It lives here until the world changes |
| Complete | **no.** It moves to the closed list below |

**"Blocked" is reserved and narrow.** Terry, 2026-08-06, correcting the first version of this
rule: *"blocked on Terry is not blocked. Blocked means blocked on some factor outside either of
our control — e.g. hardware not arrived yet."* **A task neither of us can advance is blocked;
a task one of us simply has not done yet is open.**

**That matters because the checklist is a working set for the pair, not a queue for Claude.** An
item needing a card reformatted or a reader swapped is exactly what Terry opens the list to find
— hiding it as "blocked" would take the thing he most needs to see and put it in a file he does
not have open.

**A short checklist is still the intended state**, and the reason stands: a list padded with
things *nobody* can act on is a list you stop reading — the same argument decisions 9 and 12 make
about warnings that fire when you cannot act.

**Each item carries its status in its heading** — `OPEN`, `IN PROGRESS`, `BLOCKED`, or moved to
the closed list — so what is *missing* from the checklist is explained here rather than simply
absent.

## 1. SanDisk 512 GB SD acceptance test — OPEN, TERRY'S MOVE — passed, one cold pass left

**It passed the bar decisively. One step remains and it needs Terry:**

1. **Reformat the card in the R5** — it currently holds 40 GiB of bench data.
2. Shoot frames onto it.
3. `cargo run --release --example sustained -- E:\DCIM\<folder> 150`

**Why that is not ceremony:** the 281 figure was taken on a **Windows-written layout immediately
after a bulk write**, so the card's SLC cache may still have been folding, and camera-written
geometry is what the card will actually carry. `REVIEWING.md` — *read the second pass*. **Nothing
is expected to change the verdict**; the card is 34 MB/s clear of the previous best.

Bar was the fleet range, **205–247 MB/s**; the known dud did 73.

| Step | State |
|---|---|
| Low-level format in the R5, before anything else touched it | **done** — Terry, 2026-08-06 |
| Card identity and capacity | **done** — `EOS_DIGITAL`, exFAT, 511,898,025,984 bytes ≈ 512 GB |
| PnP parent chain shows SuperSpeed | **done — passes**, see the topology below |
| Frames on the card | **done** — 748 real CR3s, 40 GiB, copied on the bench. Legitimate because no trip is in progress; see `CONOPS.md` on the two scopes |
| Sustained read | **PASSES, decisively — 281 MB/s** |
| A confirming second pass | **still wanted**, see the caveats below |

### The result

| | |
|---|---|
| **Sustained read** | **281 MB/s**, over 150 s, decaying to 273 — **97 %**, a mild and normal thermal droop |
| **Write** | **122 MB/s** average over 40 GiB, and flat: nine 4 GiB windows spanning 119.7–125.8 |
| **The bar** | fleet range 205–247. The known dud did 73 |

**This is the fleet's fastest SD by a wide margin**, displacing the Lexar Silver Pro 512 GB's 247.
Nothing else was touching the bus during the read.

> **It refutes a number this project had recorded.** The planned confound was that *"the SDDR-409's
> own ceiling is 247, so a ~247 result cannot separate card from reader."* **281 MB/s through that
> same reader retires the claim** — 247 was the *Lexar's* limit, never the reader's, and it had
> been written down as a property of the reader. **The confound dissolves rather than being
> controlled for**, and the Lexar cross-check is no longer needed to interpret this number.
>
> **How the error happened is the reusable part:** one card was measured through one reader, and
> the resulting figure was attributed to *the reader*. Nothing distinguished the two until a
> faster card arrived. Same shape as `REVIEWING.md`'s *when two runs agree, change the other
> variable*.

**Two caveats before it joins the travel case**, and neither is a reason to doubt the figure:

- **Measured on a Windows-written layout**, not a camera-written one — the same caveat the Lexar
  512 carries. The acceptance measurement that matters most is on frames the R5 wrote.
- **Read immediately after a bulk write**, so the card's SLC cache may still have been folding.
  `REVIEWING.md` — *read the second pass* — asks for a re-read cold.

**The chain, walked 2026-08-06** — every hop SuperSpeed, no USB 2 fallback:

```
SANDISK SDDR-409 USB Device            [DiskDrive]
  USB Mass Storage Device              Port_#0003.Hub_#0005
    Generic SuperSpeed USB Hub         Port_#0004.Hub_#0003
      Generic SuperSpeed USB Hub       Port_#0002.Hub_#0001
        USB Root Hub (USB 3.0)
          Intel(R) USB 3.20 eXtensible Host Controller
```

**A SuperSpeed hub in the path is real evidence rather than a hopeful reading**: a device that
negotiated USB 2.0 attaches to the *companion* hub, which enumerates as a plain "Generic USB
Hub". Seeing SuperSpeed hubs the whole way up means the reader came up at SuperSpeed.

> **New fact, and it matters more for the reader characterization than for this card: the SD
> reader sits behind TWO chained SuperSpeed hubs**, not directly on the laptop. That is shared
> bandwidth and a potential confound for any throughput number taken through it. **Establish
> whether it changes the figure before running the 2 × 3 matrix** — otherwise three readers get
> characterized through an untested variable, which is the mistake `REVIEWING.md`'s
> *when two runs agree, change the other variable* records.

## 2. Characterize all three UHS-II USB SD readers — OPEN, TERRY'S MOVE

One known-good card through all three, so Terry knows every reader in the bag is safe to travel
with. A slow reader is silent in the field — the card mounts, every file reads, nothing errors,
and you lose 5.8×. Combines with the acceptance test above into a 2 cards × 3 readers matrix.

**Unblocks when** a card with frames on it is available and Terry can swap readers. **The
SDDR-409 has already produced a number worth beating: 281 MB/s** — which retired the belief that
247 was that reader's ceiling, so the other two readers now have a real bar rather than an
assumed one.

## 3. Zoom out over the badge and verdict work — IN PROGRESS, roughly half done

**Started 2026-08-06. What has actually been swept, so nobody assumes the rest was:**

| Swept | Not yet swept |
|---|---|
| `CONOPS.md` verdict table — was naming phrases the tool no longer prints | `TRIP-HYGIENE.md` |
| `DESIGN.md` decision 14 — verdict table, layout rules, badge section | `REVIEWING.md` (grepped only, not read) |
| `progress.rs` — orphaned `clear()` doc | the rest of `main.rs`'s ~980 comment lines |
| `main.rs` — `step_badge`, `phase_heading`, `verdict()` | `eject.rs` (429 comment lines), `human.rs`, `winio.rs` |
| `WRITING.md` — gained the prose bar | `DESIGN.md`'s run records, which should move to their own file |
| `FULL-RUN.md` — **clean**, no verdict or badge claims | |
| `--eject-prepare` references repo-wide — **all five describe the removal**, none instruct | |

**Four defects found so far, none of them cosmetic:** `CONOPS.md` citing dead verdict phrases;
four rows of `DESIGN.md`'s verdict table describing output that never existed; two orphaned doc
comments; and a false mechanism written into `verdict()`'s own doc the same evening it was
corrected elsewhere.

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
read the result end to end.**

**Do this after the CLI signoff closes**, and treat it as a documentation review rather than a
code one — the code is tested; the prose is not.

## 4. Put the docs and tests on a diet — IN PROGRESS

**Terry raised the priority on 2026-08-06** and set the framing: *"pretty aggressive... this is a
hobby project, we aren't launching nuclear missiles, nobody's gonna die. Use a fresh pair of
skeptical eyes on what REALLY is justified."* **RawGeotag's tests are out of scope** — they passed
muster; this is about what this project grew.

**Measured before cutting:** 3,278 of 10,549 source lines are comments (**31 %**), plus a
4,593-line `DESIGN.md`. The bar now lives in [`WRITING.md`](WRITING.md) — *prose earns its place
or goes*.

**Done so far:**

- `step_badge`'s 60-line doc cut to 12; `phase_heading`'s doc restored; four redundant tests
  removed.
- **`DESIGN.md` split: 4,599 → 3,774 lines (−18 %).** The three full-run narratives moved to
  [`RUNS.md`](RUNS.md) — 834 lines, verified as an exact partition (3,765 + 834 = 4,599). **A
  structural win with zero findings lost**, which is the shape the rest of this item should take
  wherever possible.

**Remaining:** `main.rs` (~980 comment lines), `eject.rs` (429), `progress.rs` (270). These are
prose reduction rather than relocation, so they need the `WRITING.md` bar applied paragraph by
paragraph.

**The count is not the metric.** Most tests are regressions for defects that actually shipped, and
the "considered and rejected" material exists to stop re-proposals. **The real fat is the same
argument restated in three places**, so the likely answer is structure rather than deletion.

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
- **Eject vetoes — closed 2026-08-06.** Cause found (the retry re-dismounted before every
  attempt, so it never once asked about a *settled* volume; exFAT answers a freshly remounted one
  with `PNP_VETO_TYPE(6)`, which never yields). `Prepare::FirstAttemptOnly` shipped as the tool's
  **only** behavior, and `--eject-prepare` was deleted rather than left as a selectable
  known-hanging mode. **Closed on work, watched on evidence**: A's failure rate is established at
  2-for-2 and needs no more runs, and B now accrues on every ordinary run, so there is nothing
  left to schedule. Add a row to [`EJECT-SERIES.md`](EJECT-SERIES.md) if an eject ever behaves
  unusually — **a B run that takes several attempts and then succeeds is a different animal from
  A's unwinnable hang**, and would be worth knowing about
- **Terry's signoff on the CLI output — explicitly given 2026-08-06.** The badge column, the
  colours, the `Eject` restructure, the `SAFE TO STORE` defect and the verdict badge all landed
  and were reviewed on both the 4K monitor and the laptop. **The one leftover is prose and moved
  to the zoom-out**: `progress.rs` still argues with itself about erasing the Writing/Verifying
  block, while the code is now correct
