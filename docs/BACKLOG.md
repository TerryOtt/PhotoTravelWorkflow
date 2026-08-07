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

### Every checklist item MUST be prefixed `C: ` or `T: `

**Standing order, Terry, 2026-08-07.** The prefix names **who can advance the item right now**,
and the list sorts with all `C:` items before all `T:` ones.

| Prefix | Means |
|---|---|
| **`C: `** | Claude can move it without him. He can skip the line |
| **`T: `** | It needs Terry — a cable, a card, a decision, a signoff |

**It marks the *current* blocker, not ownership, so it flips as work moves.** The reader matrix is
`T:` while a reader needs swapping and becomes `C:` the moment the hardware is in and only the
measuring is left. **An item whose prefix never changes across a long task is probably mislabeled.**

**Why it earns its place on a list that is deliberately short:** the checklist is the first thing
he reads, and until now every line had to be *parsed* to find out whether it was waiting on him.
Two characters turn that into a scan — which is the same argument as the badge column, applied to
the backlog instead of the report.

> **The ordering is by task ID, not by subject**, so the prefix alone does not guarantee the sort.
> When a new `C:` item lands after a `T:` one, the grouping breaks and the fix is to recreate the
> tasks in the intended order. **Check the grouping after adding an item** rather than assuming
> the prefix did it.


## Characterize all three UHS-II USB SD readers — CLOSED 2026-08-07

> **All three cleared: 280, 276, 275 MB/s — one population inside ±2 %.** Every reader in the bag
> is safe to travel with. Full record below; it is kept in place rather than moved because it is
> long and the protocol is worth re-reading before the next matrix.

One known-good card through all three, so Terry knows every reader in the bag is safe to travel
with. A slow reader is silent in the field — the card mounts, every file reads, nothing errors,
and you lose 5.8×.

**Started 2026-08-07.** The three readers are the **SanDisk SDDR-409** (USB-C, the incumbent),
a **UGreen** (USB-C) and a **Lexar** (USB-A). The card is the SanDisk 512 GB, which is the
fleet's fastest at **279–281 MB/s** and therefore the one most likely to expose a reader that
caps — a slow card would hide the difference under its own ceiling.

**The baseline is re-taken rather than quoted.** The 281 was measured on 2026-08-06 during
acceptance; re-running it now under the same conditions as the other two is what makes the
three numbers comparable. A figure carried across from a different day is exactly the
cross-variable comparison `REVIEWING.md` refuses.

### The protocol, written down before the numbers

**The port MUST be held constant across all three readers**, because reader and topology
otherwise move together and neither can be blamed. The current chain, walked 2026-08-07:

```
SANDISK SDDR-409 USB Device
  USB Mass Storage Device            Port_#0003.Hub_#0005
    Generic SuperSpeed USB Hub       Port_#0004.Hub_#0003
      Generic SuperSpeed USB Hub     Port_#0002.Hub_#0001
        USB Root Hub (USB 3.0)
          Intel(R) USB 3.20 eXtensible Host Controller
```

**Two chained SuperSpeed hubs, so the reader is not on the laptop directly** — and that is the
*travel* configuration rather than a defect, since the hotel ritual is one connector to the
Element 5. Measuring all three there measures the rig he actually carries.

**The Lexar is USB-A and the XPS 15 9530 has no USB-A port**, so it can only be reached through
the hub. That settles the design rather than constraining it: the hub is the one place all
three readers can meet, so the hub is where the matrix runs.

### Reader 1 of 3 — SanDisk SDDR-409, the baseline: 280 MB/s

**Measured 2026-08-07**, clean build, quiet bus (nothing above 1 MB/s), 31 h 52 m uptime,
`sustained.rs` over `E:\burnin\100CANON` — 798 CR3s, 42.6 GiB, a working set far too large to
cache. Rig watcher armed throughout at its 2 s metadata poll, which is how the acceptance
figure was also taken.

```
at      10s  20s  30s  40s  50s  60s  70s  80s  90s 100s 110s 120s 130s 140s
MB/s    277  278  279  280  281  277  279  279  281  282  283  282  281  281
```

**Mean 280 MB/s, range 277–283, spread 2.1 %, no decay.** The *first* window is the slowest and
the curve drifts mildly upward, which is the opposite signature to thermal throttling.

**It reproduces the acceptance number within 0.4 %** — 279 → 277 cold on 2026-08-06 against
280 mean today — comfortably inside this project's ±2 % band for reads. **So the baseline is
a re-measurement rather than a citation**, and the other two readers can be compared against it
directly.

| Reader | Link | Sustained read | State |
|---|---|---|---|
| **SanDisk SDDR-409** (USB-C) | SuperSpeed, `Hub_#0005` port 3 | **280 MB/s** (277–283) | **done** |
| **Lexar** (USB-A) | SuperSpeed, **same hub**, port 5 | **276 MB/s** (273–278) | **done** |
| **UGreen** (USB-C) | SuperSpeed, **`Hub_#0005` port 3 — the baseline's own socket** | **275 MB/s** (272–279) | **done** |

> **The UGreen row is the tightest comparison in the matrix and it happened by luck rather than
> design.** It went into `Port_#0003.Hub_#0005` — *the same physical socket* the SDDR-409's
> 280 MB/s came from — so reader is genuinely the only variable, with not even a sibling port
> between them. **Record which socket each row used**; the protocol asked for the same hub and
> this row happens to do better than that.

### All three readers are indistinguishable — 2026-08-07

| Reader | Mean | Range | Spread |
|---|---|---|---|
| SanDisk SDDR-409 | **280** | 277–283 | 2.1 % |
| Lexar | **276** | 273–278 | 1.8 % |
| UGreen | **275** | 272–279 | 2.5 % |

**The widest gap between any two readers is 1.8 %, inside the ±2 % band — so this is one
population, not three.** All three flat over 150 s, none throttling, none capping the card.
**Every reader in the bag is safe to travel with**, which is the question the item was opened to
answer, and the answer is boring in the best available way.

> **What it does NOT establish: any reader's ceiling.** Three readers agreeing at ~277 with one
> card means they all clear *that card*, not that 277 is anybody's limit. **A faster card would be
> needed to separate them**, and there is no reason to buy one — the fleet's fastest SD is the card
> under test, so nothing in the bag can expose a difference that matters.

**The like-for-like turned out better than the protocol demanded.** The two readers landed on the
**same hub** — identical chain from `Hub_#0005` up through both SuperSpeed hubs to the same Intel
3.20 controller, differing only in which downstream port of that hub they occupied. Same upstream
bandwidth, same controller. **The port-constancy rule was satisfied more tightly than by holding
one socket**, because holding one socket across a USB-C and a USB-A reader was never possible.

> **The Lexar's caveat is retired rather than carried.** It was written expecting USB-A to force
> a foreign port; it forced a *sibling* port on the same hub instead. Nothing needs subtracting.

> **The 222 MB/s recorded for the Lexar reader was the CARD's limit, not the reader's** — it just
> read 276 with a faster card. **That is the second instance of this exact misattribution**, after
> the SDDR-409's "247 ceiling" turned out to belong to the Lexar Silver Pro card. One card through
> one reader yields one number, and this project has now filed it under the reader twice.
> **A reader's ceiling is only established by the fastest card that has ever been through it.**

### The UGreen read 93 MB/s first, and the reader was innocent

**A badly seated card negotiates UHS-I and looks exactly like UHS-I hardware.** The UGreen's first
two runs were **flat at 92–93 MB/s** — 89 % of UHS-I's 104 MB/s SDR104 ceiling — with the USB side
confirmed SuperSpeed in the baseline's own socket. Terry found it: the slot was sticky, and the
card clicked in deeper when pushed. **A full remove-and-reinsert took it to 275.**

**The mechanism is worth keeping because it is invisible and cheap to hit.** A UHS-II card slot has
**two rows of pins** — the standard row plus a second row behind it, which *is* the UHS-II
interface. A card seated far enough to contact the first row and not the second enumerates
normally, mounts, reads every file, errors at nothing, and runs at exactly UHS-I speed.

> **⚠ A push is not a reseat. SD bus speed is negotiated when the card INITIALIZES.** The first
> reseat improved contact without re-enumerating anything — same serial, same disk numbers, same
> drive letter — so the link stayed at whatever it had already agreed to and the number did not
> move. **Only a full removal and reinsertion renegotiates.** Verified both ways here.

> **The reasoning failure, recorded because the objection was raised and then argued past.** The
> not-re-enumerated caveat was written down *before* the repeat run, and then a flat 93 was read as
> a spec limit and a confident lean toward "the UGreen is UHS-I" was stated anyway.
>
> **Flat at a spec boundary establishes WHICH spec is in force. It says nothing about WHY.** A
> UHS-I ceiling is exactly as consistent with UHS-I hardware as with UHS-II hardware that
> negotiated down, and those two were collapsed while the note separating them was still on screen.
> **A caveat only helps if it survives the next result.**

## 1. Put the docs and tests on a diet — IN PROGRESS

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

- **Code comments, in progress.** `main.rs` 997 → **862**, `eject.rs` 429 → **404**. Seven doc
  blocks cut, each one restating a `DESIGN.md` decision in full rather than citing it, or quoting
  Terry at paragraph length where a clause carried it.
- **A decorative test found and repaired rather than deleted.** `the_spacer_template_...`
  asserted that a *literal* parses, re-typing the value instead of reading it — so mutating the
  real one would not have failed it. Now reads a named `SPACER` const, and is mutation-checked.

- **Two more run records found and moved.** *Corroboration ran for the first time* (247 lines)
  and *16 m 55 s on the dock* were sitting in the **architecture** section, not at the end, so
  the first split walked past them. `DESIGN.md` **4,599 → 3,482 (−24 %)**, none of it deleted.
- **Prose deleted, not just moved.** `CLAUDE.md` **528 → 455**: the no-drift rule was 109 lines
  written the same day across four edits, now 38; and the build-chain section stopped restating
  the global config, which is loaded in the same session. **Total docs 8,411 → 7,713.**

**Remaining:** `progress.rs`, `human.rs`, `winio.rs` and the smaller `main.rs` blocks — all
prose reduction rather than relocation.

> **Moving does not shrink the total, only deletion does**, and it is worth being honest about
> which is which. The `RUNS.md` splits took **1,117 lines** out of `DESIGN.md` and **zero** out
> of the repository — they buy navigability. The `CLAUDE.md` cuts are the first real deletions.
>
- **Decision 22 split, 455 → 256 lines**, on Terry's go-ahead: *"we have code that works, so
  trimming docs is much less risky."* The **decision** stayed in `DESIGN.md`; the **working out**
  — the card-release correction with its reproduction and trace, and the run that first proved
  the retry — moved to [`EJECT-SERIES.md`](EJECT-SERIES.md), which is now the eject record rather
  than only the tally. Both blocks moved **whole and unedited**, and `DESIGN.md` gained two
  pointers so the evidence is one click away rather than gone.

**`DESIGN.md` is now 3,284 lines, from 4,599 — down 29 %.**

> **What is deliberately NOT being cut.** Tests that guard a defect which actually shipped, and
> comments carrying a mechanism a reader would get wrong — `estimate()`'s warning that
> corroboration is *added* rather than overlapped is the type case, since decision 2 describes an
> overlap that is not built and estimating as though it were would understate every run by a
> quarter of an hour. **The target is duplication, not volume.**

**The count is not the metric.** Most tests are regressions for defects that actually shipped, and
the "considered and rejected" material exists to stop re-proposals. **The real fat is the same
argument restated in three places**, so the likely answer is structure rather than deletion.

## 2. Settle the USB-C→USB-A adapter — OPEN, TERRY'S MOVE

**Opened 2026-08-07 out of the reader matrix, as a side quest rather than a blocker** — the
matrix does not need an adapter, because the Lexar is natively USB-A and the other two are
natively USB-C.

**What happened:** a UGREEN passive adapter, blue-flagged and advertised at 10 Gbps, put the
SanDisk SDDR-409 on **USB 2.0** — 40 MB/s flat against its 280 MB/s baseline. **The port is
innocent**, proven by substitution: the Lexar reader in that same front USB-A port landed on the
SuperSpeed hub while the adapted SanDisk landed on the USB 2.0 companion hub.

**Why it is worth closing rather than shrugging at.** Terry has three of these, and the question
is whether he can *ever* put a USB-C device on a USB-A port at speed. A yes buys the travel case
a genuine option; a no means three adapters that look useful, carry a 10 Gbps label, and would
silently cost 7× the first night someone reached for one in a hotel.

**The test, and it is two minutes each.** SDDR-409 in the same Element 5 front USB-A port through
each spare, then read the PnP parent chain — **`Generic SuperSpeed USB Hub` means it works,
plain `Generic USB Hub` means USB 2.0.** No throughput run needed; the chain answers it, and the
40 MB/s only ever confirmed what the chain already said.

> **Three causes were never separated and MUST NOT be collapsed into "counterfeit":** a defective
> unit, a design wired for USB 2.0 only, and a plug seated 90 % of the way — deep enough for the
> USB 2.0 contacts, short of the SuperSpeed pins. **Reseat firmly and try both orientations before
> condemning any of the three**, since the cheapest explanation costs nothing to rule out.

> **⚠ START WITH THE SAME ADAPTER, RESEATED — not a different one.** The seating explanation was
> raised when this item opened, never tested, and then **the identical failure was proven on the
> card slot an hour later**: flat at a lower spec's ceiling, on a path confirmed good, fixed by
> pulling the thing out and pushing it home. **This rig has now demonstrated a seating tell twice
> in one session**, and the adapter is the one place it was hypothesized and never checked.
>
> **Swapping adapters first would waste the evidence.** If a *different* adapter works, that reads
> as "the first one was faulty" — when the live alternative is that any of them works once seated.
> Reseat the original, both orientations, before introducing a second variable.

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
- **`offload sync` — withdrawn 2026-08-06, not deferred.** It was advertised in `--help` and was
  a stub that exited with an error. **The rig's own specification had already absorbed the
  problem it solved**: four destinations is N+1, so a dead drive is one config edit and the
  remaining three still clear the *three copies or you have none* bar, with no hole to backfill.
  Terry: *"it's why I bring four."* Subcommand removed, design replaced by the reasoning, and
  `preflight`'s DESTINATION MISSING message no longer names a command that does not exist
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

### SanDisk 512 GB SD acceptance test — CLOSED 2026-08-06, ACCEPTED at 279–281 MB/s

**Accepted. The fleet's fastest SD by 32 MB/s**, and confirmed over two independent passes.

| Pass | Result |
|---|---|
| Straight after a 40 GiB write | 281 → 273 MB/s over 150 s (**2.8 %** decay) |
| Cold, hours idle | **279 → 277 MB/s** over 150 s (**0.7 %** — essentially flat) |
| Write | 122 MB/s, flat across 40 GiB |
| The bar | fleet range 205–247; the known dud did 73 |

**The two passes agree within 0.7 %**, inside this project's ±2 % band for reads. **And the
difference between them is itself the finding: the first pass's droop was the SLC cache folding,
not heat.** That is exactly what `REVIEWING.md`'s *read the second pass* exists to separate, and
it separated cleanly — a thermal problem would have got *worse* on the second run, not vanished.

> **RETIRED 2026-08-06: the "camera-written layout" caveat was never real.** It had been carried
> since the Lexar acceptance and it does not survive being stated plainly. Terry: *"it's an exFAT
> file system. Windows won't write to it any differently than the camera."*
>
> **Filesystem geometry is set at *format* time, not write time.** Both cards are exFAT with a
> **262,144-byte allocation unit**, both formatted in the R5, and copying files into a
> camera-formatted volume uses the clusters the camera would have used. The caveat would only
> bite if *Windows had formatted the card*, which has never happened here.
>
> **And it proves too much.** `D:`'s 7,395 frames were also copied on by Windows onto a
> camera-formatted card — that is how the 390 corpus was loaded — so **every throughput figure
> this project has ever taken sits on Windows-written files**, including the fleet baselines the
> caveat was meant to protect. A caveat that invalidates its own reference points is not a
> caveat.
>
> **What survives is the SLC caveat**, which is about *timing* rather than provenance: a read
> taken straight after a bulk write can be flattered by the card's cache still folding.
> `REVIEWING.md` — *read the second pass*.

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

### Zoom out over the badge and verdict work — CLOSED 2026-08-06, every document swept

**Started 2026-08-06. What has actually been swept, so nobody assumes the rest was:**

**Every document is now swept.** What remains is code comments.

| Swept | Not yet swept |
|---|---|
| `CONOPS.md` verdict table — was naming phrases the tool no longer prints | the rest of `main.rs`'s ~980 comment lines |
| `DESIGN.md` decision 14 — verdict table, layout rules, badge section | `eject.rs` (429 comment lines) |
| `DESIGN.md` run records → [`RUNS.md`](RUNS.md), −18 % | `progress.rs` (270), `human.rs`, `winio.rs` |
| `progress.rs` — orphaned `clear()` doc | |
| `main.rs` — `step_badge`, `phase_heading`, `verdict()` | |
| `WRITING.md` — gained the prose bar; `RUNS.md` registered | |
| `FULL-RUN.md` — **clean**, no verdict or badge claims | |
| `TRIP-HYGIENE.md` — **`console` was still filed as "cosmetic"** | |
| `REVIEWING.md` — **still described a four-test project**; there are 122 | |
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

