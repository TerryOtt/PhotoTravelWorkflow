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
| Complete | **no — and `TaskUpdate status: deleted`, not `completed`.** It moves to the closed list below |

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

> **A finished item MUST be *deleted* from the CLI checklist, never left showing `completed`.**
> Standing order, Terry, 2026-08-07: *"anything completed in the checklist should be removed from
> the checklist — I only want to see work either eligible to be worked or in progress."*
>
> **`TaskUpdate status: completed` is not the end of the job; `status: deleted` is.** Marking an
> item completed leaves it on screen, and the checklist shows at most five — so a finished item
> is occupying a slot that a workable one needs. **This file is what makes deleting safe**: the
> permanent record lives here, so nothing is lost when the line disappears from his view.

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

## Put the docs and tests on a diet — CLOSED 2026-08-07

> **Signed off by Terry**, who read all docs and code comments and accepted the changes.
>
> **Tests 114 → 106** in `offload`'s lib (182 across the workspace). **Comments 32 % → 29 %.**
> `DESIGN.md` **3,245 → 3,218**. **`docs/` grew overall**, 7,670 → 7,881 — the corrections and
> this record outweighed the cuts, and **the pass bought accuracy rather than size.**
>
> **Eight defects found, all one class: claims about output, data shapes, and what exists.**
> Four sweeps came back clean, which is what located the class. `WRITING.md` rule 5 and
> `scripts/doc-claims-check.py` are what carry it forward.
>
> **The stopping argument, since "aggressive" invites more:** the largest remaining blockquotes
> are measurements plus a live trap (the 26 MB figure is per *file*, not per frame). **Cutting
> those would be deleting evidence to hit a number**, which the standing order does not license.
> Any remaining fat is structural, not lexical.

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

### The fresh pass, 2026-08-07 — tests, then comments, then docs

Ordered by Terry, no preconceptions carried in. **Tests 114 → 106**, merging only where several
tests exercised one function over different inputs; `main.rs`'s were already deduplicated by the
earlier pass. **Comments 32 % → 29 %** of `crates/offload/src`. **`winio.rs` deliberately
untouched** — flag semantics, measurement tables and SAFETY blocks.

**The docs half stopped being a diet and became an audit**, because grepping quoted output against
the source found four defects in one file:

| Found in `DESIGN.md` | |
|---|---|
| Decision 14 contradicted its own standing order | The opening blockquote described a badge as *"white on red"*; the red ban landed a day later and was never carried back. `.red()` appears nowhere in `crates/` |
| Decision 29's dependency table omitted three of nineteen crates | `blake3`, `xxhash-rust` (optional, `hash-experiments`) and `sha3` (dev-only). `windows-registry` was listed with an active role while imported by nothing, and `rayon`'s role read as the binary's when it is `geotag`'s |
| **Decision 34 is designed and unbuilt** | No `body` field in `config.rs`, no `Body` line printed — **and `CLAUDE.md` instructed Claude to act on that line "every time it appears."** Now marked in three places, with a MUST NOT against reading a missing line as agreement |
| A verbatim duplicate | *"Both badges are five cells wide"* stated twice, 90 lines apart, inside one section |

**Then the sweep widened, and found four more:**

| Also found | |
|---|---|
| **The manifest sample was flat** | Decision 12 showed `schema`/`date_utc`/`destination`/`runs`/`files` at top level. The real shape is `{ schema, checksum, body: { ... } }`. **`checksum` covers `body` and nothing else**, so a flat sample did not omit a field — it contradicted the mechanism the next paragraph explains. **The highest-stakes sample in the repo**: decision 28 promises a stranger reads these disks in 2031 with this block |
| **Decisions 9 and 33 read as built** | The Defender check and the throughput history, both present tense, both absent. `history.json`, `uptime_min`, `read_mb_s`, `write_mb_s`, `verify_mb_s` appear nowhere in `crates/` |
| **`Still to build` was the only correction** | A reader arriving at decision 9, 33 or 34 directly had nothing telling them the behavior was absent. **A list of exceptions only works if the exceptions know they are on it** — all four now carry a marker at the decision |
| **Where this stands** | Module inventory re-checked mechanically. **Clean** — every `offload` module named, geotag's four covered by its crate row |

**And one more, from sweeping every file path the docs cite:** four resolved nowhere, and the
reason is that **RawGeotag is not cloned on this machine** — `CLAUDE.md` linked `..\RawGeotag`
and told every session to read it. All four files exist at
<https://github.com/TerryOtt/RawGeotag>, so decision 30's migration list was right and only the
locality was wrong. Recorded with the instruction to read it via `gh` rather than clone, since a
stale clone answers authoritatively from whatever state it was left in.

### What was swept, 2026-08-07 — so it is not re-done blind

| Category | Method | Result |
|---|---|---|
| Output strings | Every all-caps phrase in backticks vs `crates/` | **4 defects** (decision 14's red badge, the four verdict suffixes re-confirmed absent, decisions 33/34 unbuilt) |
| JSON samples | Each `json` fence vs its struct and fixture | **1 defect** — the manifest was flat |
| Present tense vs `Still to build` | Each listed item's own decision | **3 defects** — decisions 9, 33, 34 unmarked |
| CLI flags | Every `--flag` in docs vs clap's derive | **clean** — all ten doc-only flags legitimate |
| Config sample | `config.json` block vs `config.rs` | **clean** |
| Module inventory | *Where this stands* vs `crates/*/src/*.rs` | **clean** |
| Cited file paths | 29 distinct paths vs disk | **4 defects**, one root cause (RawGeotag) |
| Relative markdown links | 131 links across 11 files | **clean** — after removing the `..\RawGeotag` one |

**Two sweeps came back clean before anything was changed**, which is worth as much as the finds:
the CLI surface and the module inventory were accurate. **The defect class is claims about output
and data shapes, and claims about what exists — never the interface itself.**

**Coverage is complete: every file in `docs/` plus `CLAUDE.md` was swept.** `CONOPS.md` gave up
two unbuilt promises in the **shooting-day contract**, which is the worst place for one since it
is what Terry reads. `FULL-RUN.md`, `REVIEWING.md` and `EJECT-SERIES.md` came back clean —
`EJECT-SERIES.md` cites `IOCTL_VOLUME_OFFLINE`, which is absent from the source and correctly
labeled a candidate and *Untested*. **That is the shape a forward-looking reference should have**,
and it is the counter-example to the three decisions that read as built.

**The same sweep then ran over code comments**, since a comment is a claim too. Two stale ones:
`storage.rs` cited the Defender check that had just been withdrawn, and `body-identity.rs` still
called itself *"the one measurement decision 34 is blocked on"* after answering that question
twice.

**`scripts/doc-claims-check.py` makes the mechanical corner repeatable** — links, cited paths,
invisible dependencies, the red-badge ban. **It is explicitly not the rule**: it cannot read an
output string out of a format argument or tell a built decision from a designed one, which is
where every real defect today actually lived. **Its first version reported eight defects that
were its own**, resolving paths only from the repo root so `../CLAUDE.md` looked broken — so it
was given a negative control before being trusted, per rule 5's own instruction. Not in the
pre-commit hook, deliberately.

**The docs got BIGGER, and that is the honest result.** `DESIGN.md` **3,245 → 3,218** despite
decision 14 losing 41, decision 22 losing 38 and decision 20 losing 19 — the corrections added
back most of what the narrative cuts removed. Across `docs/` the total went **7,670 → 7,791**,
almost all of it `BACKLOG.md` recording this work. **The pass bought accuracy, not size**, and
that trade was not the one the item was opened for.

> **What to conclude for the rest of the item:** the remaining prose is mostly *earned*. The
> largest blockquotes left — decision 17's interleaved-verify table, *Where the wall clock goes*
> — are measurements plus a live trap (the 26 MB figure is per **file**, not per frame, and
> already cost a session half its card-capacity answers). **Cutting those would be deleting
> evidence to hit a number**, which the standing order explicitly does not license. The real
> remaining fat, if any, is structural rather than lexical. `WRITING.md` rule 5 gained what would have caught
all of it: a document stating what a program prints or writes MUST have those strings grepped
against the source; **the pattern MUST be proven able to find something before its silence is
believed**; and a phrase with a variable in it is never present literally, so search the invariant
half.

## Decision 9's Defender check — CLOSED 2026-08-07, WITHDRAWN

> **Terry accepted the recommendation the same hour it was raised: withdrawn, not deferred.**
> `windows-registry` removed from the workspace, decision 9's section struck, *Still to build*
> updated, `TRIP-HYGIENE.md`'s invisible-dependency list now **empty for the first time**.
> Workspace builds clean, **182 tests passing**.
>
> **The exclusions were already set on 2026-08-05**, so what was withdrawn is the *reporting*,
> never the protection — which is what made it cheap rather than a trade.

**Opened 2026-08-07** when Terry asked whether this needed an item. **It needed a decision, not a
build, and the recommendation was to withdraw it.**

**Both mechanisms are closed on this rig, re-measured today, unelevated:**

| Probe | Result |
|---|---|
| `Get-MpPreference` | `N/A: Must be an administrator to view exclusions`, all three lists |
| `HKLM:\...\Windows Defender\Exclusions\{Paths,Extensions,Processes}` | `SecurityException` on every one |
| **Control** — `HKLM:\...\Windows NT\CurrentVersion` | **readable**, so the instrument works and the denial is real |

**Binding constraint 4 forbids elevation**, so the check could only ever return its *could not
confirm* outcome — every run, forever. **That is a diagnostic that cannot succeed**, the mirror of
[`REVIEWING.md`](REVIEWING.md)'s objection to one that cannot fail.

**The substitute already ships, and decision 9 names it**: *"the real check is one the report
already prints — the per-destination sustained rates, where a Defender tax shows as every
destination running far below its known ability."*

**Withdrawing also closes a real hole.** `windows-registry` is the **last** declared-but-unused
workspace dependency, and [`TRIP-HYGIENE.md`](TRIP-HYGIENE.md) records that those never resolve,
never reach `Cargo.lock`, and are **invisible to `cargo outdated`**. `indicatif` and `console`
both left that list as their features got built; this one cannot.

**Precedent:** `offload sync` was withdrawn rather than deferred on 2026-08-06 — a capability
nobody can use is worse than one that is absent.

> **The alternative, if kept, is to re-scope rather than build as designed:** report that
> exclusions *could not be verified* and point at the sustained rates. That is what it would do
> every night anyway, so the honest version is one line, not a registry read.

**Why it is his call:** the engineering is not in doubt; withdrawing a designed feature is a scope
decision.

## Implement the phase 5 pool and wire `--jobs` — CLOSED 2026-08-07

> **Terry chose to build it, and it is built and measured.** `phase5::run` takes `jobs`, builds
> its own `rayon` pool, and correlates frames in parallel; `--jobs` is read at exactly one call
> site. **188 tests still pass**, output is unchanged.
>
> **~1.5–1.8× on local NVMe, knee at four threads.** 7,395 frames × 4 destinations against a
> real track: 10.45 s / 8.88 s single-threaded against 6.01 s / 5.40 s at `-j 4`.
> **Quoted as a range because the single-thread baseline differs 18 % between two runs** — this
> is 26,900 small writes, and writes on this rig have never reproduced like reads.
> `cargo run --release --example geotag-rate` re-runs it.
>
> **It is not RawGeotag's 12×, and decision 15 predicted exactly that**: NTFS serializes
> metadata within a directory and a day is one directory, so local work plateaus where SMB's
> latency-bound case kept scaling.
>
> **Two things this did not do**, both recorded at decision 15: the **NAS case is unmeasured**,
> so decision 30 cannot yet claim parity with RawGeotag; and `Progress::Bar` had to become
> thread-safe — `Cell`/`RefCell` became one `Mutex`, because splitting the print decision
> across atomics would let two threads emit the same row.

## ~~Decide `--jobs`: implement the phase 5 pool, or delete the flag~~ — the finding

**Found 2026-08-07 in the first ten minutes of decision 30, which is what makes it urgent
rather than tidy.** `--jobs` is **parsed and never read**: `main.rs` declares it with an
`available_parallelism` default, and **no other line in `crates/` mentions it.** `phase5::run`
is a sequential `for photo in landed` loop — no pool, no threads, no `rayon` (`offload` does
not depend on `rayon`; that is `crates/geotag`'s).

**`pipeline.rs` claimed *"`--jobs` governs phase 5"***, which is the sentence that made it look
settled. Now corrected, along with decision 15.

**It blocks decision 30 from being a clean win.** RawGeotag's `-j` is *load-bearing* —
**3,883 CR3s in 5.8 s at `-j 20` against 48 s at `-j 2`**, and ~12× over SMB. Retiring it into
a **sequential** `offload geotag` would make the workflow Terry actually runs *slower*, on
exactly the storage that measurement was taken against.

| | |
|---|---|
| **A — implement the pool** | Makes decision 15 true and decision 30 a straight win. Phase 5 is where a pool pays: thousands of 3 KB sidecars into one directory, CPU- and metadata-bound. **Must be measured on the rig afterwards**, not merely built |
| **B — delete the flag** | The standing order that *a config item never used MUST NOT exist* — *"a dangerously unused code path waiting to bite us."* Accepts a sequential tagger, so decision 30 either regresses the NAS case or stays open |

**Recommendation: A**, because decision 30 is already authorized and B makes it worse. **His
call because it is scope**, not because the engineering is unclear.

## Retire RawGeotag into `offload geotag` (decision 30) — IN PROGRESS

**Opened 2026-08-07: the precondition is met and nobody had noticed.** `CLAUDE.md` said
retirement *"cannot happen until phase 5 works."* **Phase 5 works** — `main.rs:316` calls
`geotag_phase` → `phase5::run` on the ordinary run path, and `DESIGN.md`'s *Where this stands*
lists it wired and running.

**So the engine duplication is open by choice rather than by dependency**, and the standing
warning in `CLAUDE.md` is real: `crates/geotag` here and RawGeotag's own four modules are
separate copies, so **a fix made here does not reach the tool Terry actually travels with.**

**RawGeotag is <https://github.com/TerryOtt/RawGeotag>** — public, last pushed 2026-08-03, which
is the lift itself. **Not cloned on this machine; read it with `gh`**, since a stale clone would
answer authoritatively from whatever state it was left in.

**The work:** add the `geotag` subcommand (RawGeotag's CLI is a strict subset of what phase 5
already does); migrate `docs/LIGHTROOM-XMP.md`, `docs/FIXTURES.md` + `scripts/verify-fixtures.ps1`,
`docs/TESTING.md`'s one load-bearing principle, and the duplicated `fixture-manifests/`; archive
the repository.

**`--utc-offset` MUST NOT come across.** It existed for a body that recorded no timezone; decision
23 removed the need, and reintroducing it would reintroduce the gate it implies.

> **TERRY'S MOVE at the end:** archiving the repository is his call, since it is the tool he
> currently travels with. Everything before that is buildable without him.

## Boomerang pass: code → docs → code — OPEN, TERRY'S MOVE

**Requested 2026-08-07, mid-build.** He will explain the intent when he authorizes work on it.

**Claude MUST NOT start this without his brief.** The name suggests a round trip — read the code
to correct the docs, then let the corrected docs drive changes back into the code — **but that is
inference rather than instruction**, and guessing the scope is exactly what this project's writing
rules exist to prevent.

> **Context that may or may not be what he means.** The 2026-08-07 audit ran in **one direction
> only**: docs checked against code, eight defects found. **The return leg was never run** — docs
> revealing something the *code* should change — and two candidates surfaced incidentally:
> decision 34 specifying a body read from *"each card"* where decision 27's gate arguably makes
> the source card sufficient, and `DESIGN.md`'s sample report, whose lines have not all been
> matched to print sites. **Ask before acting on any of it.**

## Build decision 34: the body check — CODE COMPLETE 2026-08-07, one check outstanding

> **Built in 13 minutes of the two-hour window.** Optional `body` in the config,
> `preflight::check_body` on the first frame, a `Body` row in pre-flight's card block, INFO in
> every arm. **182 → 188 tests**, clippy clean.
>
> **Two deliberate departures from the written decision**, both recorded at decision 34:
> the **source card only** (decision 27's gate makes the second read redundant) and
> **pre-flight's card block** rather than beside decision 23's timezone line, *which does not
> exist and never did*.
>
> ### ⚠ TERRY'S MOVE — the row has not been seen on screen
>
> **The comparison and the rendering are unit-tested, and `raw::body_identity` is validated
> against nine real frames**, but the wiring from `phase1` to the printed row has only been
> **type-checked**. This project's own memory is that *a feature checked only in the convenient
> mode is unchecked*.
>
> **Two things close it, and both need him:**
>
> 1. Add `"body": { "model": "Canon EOS R5", "serial": "082021001047" }` to `config.json` —
>    Claude will make the edit on request; the value is confirmed from nine frames.
> 2. Insert a card and run `offload --dry-run`. The row should read
>    `Body    Canon EOS R5 · 082021001047 — as configured`, aligned with the card labels above it.
>
> **Until then the honest status is code complete, not done.**

**Opened 2026-08-07 at Terry's request**, off the diet pass's finding that decision 34 is fully
designed and **entirely unbuilt**: `config.rs` has no `body` field and `main.rs` prints no `Body`
line. Three places now say so — the decision's own header, *Still to build*, and
[`../CLAUDE.md`](../CLAUDE.md), whose *Report lines you must act on* row had been instructing
Claude to act on a line that has never appeared.

**What ships:**

1. `config.json` gains `"body": { "model": "Canon EOS R5", "serial": "..." }`.
2. Pre-flight reads `Make`, `Model` and `CameraSerialNumber` from the **first frame on each card**
   and compares.
3. The report prints a `Body` line beside decision 23's timezone line.

**INFO only — it MUST NOT touch the verdict or the exit code.** Decision 34 rejected exit 2 on the
grounds that a mismatch *persists*: replace the body or shoot a rental, and it is true on every run
until the config is edited, which is exactly how a scarce signal stops meaning anything.

**The hard question is already answered, so this is not research.** The R5 writes the serial into
**standard EXIF**, not MakerNotes — `crates/geotag/examples/body-identity.rs` reads it through the
same call shape `raw.rs` already makes for capture time. No MakerNote decoding, no new dependency,
no strain on binding constraint 1.

> **✔ The config value is `082021001047`, established 2026-08-07 and no longer TERRY'S MOVE.**
> Read from nine frames on `Q:\Lightroom\Images`, unchanged 2024-09-29 → 2026-07-17.
>
> **The serial DESIGN.md had recorded — `092023000050` — matches none of the four R5 bodies in
> his archive** (three rentals from 2021, then the owned body from 2024). It was never read off
> his rig. **A wrong serial here mismatches on every run forever**, and would have been
> indistinguishable from the feature working correctly.

**The payoff is decision 23, not the contract-nag it looks like.** A body that does not record
`OffsetTimeOriginal` sends **every frame to `_unfiled`** — discovered today only after the whole
day has streamed through phase 3. One frame at pre-flight turns a 35-minute discovery into a
ten-second one, while the fix is still a decision about tonight.

**The lens MUST NOT be checked**, though the same probe returns it. Terry rents glass constantly,
so a lens check fires on most interesting trips — and the very frame that settled the serial
question carries an `RF24-105mm` he does not own, which would have been its first false positive.

**Nothing is waiting on Terry** — the serial is established above, so this is buildable end to end.

## Test all three USB-C→USB-A adapters — CLOSED 2026-08-07

> **2 good, 1 dud. C→A costs nothing measurable — 275 MB/s through an adapter against 275 native.**
> Unit 1 packaged for Amazon replacement. **A USB-C reader can live on a USB-A port in the travel
> case**, which was not known to be true before today.

**All three get tested, not just the one that failed.** Terry owns three UGREEN C-female→A-male
adapters from one 3-pack; one has already been proven USB 2.0 only. **The question a single
failure cannot answer is whether that unit is defective or the product is built that way**, and
the answer decides whether this is a return, a purchase, or a capability the travel case can have.

| # | PnP chain | Verdict |
|---|---|---|
| **1** | plain `Generic USB Hub`, Intel 3.10 | **USB 2.0 ONLY — proven**, firmly seated, two variables changed |
| **2** | **`Generic SuperSpeed USB Hub`**, Intel 3.20 | **GOOD — 273–276 MB/s, transparent.** Confirmed across a replug |
| **3** | **`Generic SuperSpeed USB Hub`**, Intel 3.20 | **GOOD — 275 MB/s** (273–276), spread **1.1 %** |

**Adapter 3 got the clean full run and is the one to quote: 275 mean, 1.1 % spread, flat.** Adapter
2's run was interrupted by a mid-measurement replug and reads 273–276 across nine clean windows
either side of the gap — good enough to confirm the unit, not a figure to cite.

**275 through an adapter against 275 native is not "close enough", it is indistinguishable.** The
reader measured 275 plugged straight into a USB-C port earlier the same afternoon.

> **Why adapter 3 got a throughput run at all, when the chain had already answered.** It is going
> into the travel case, and this project's standing gate is that **gear entering the case is
> measured before it goes in** — the same rule the card acceptance test enforces. **A working chain
> proves a link negotiates; it does not prove it sustains.**

**Two good, one dud. The 10 Gbps claim is honest and unit 1 is a bad item** — which is the
cleanest return there is: not an advertising dispute, one defective adapter out of a three-pack
with two working siblings as the control. **Adapter 1 was packaged for Amazon replacement
2026-08-07.**

**The capability is the real gain.** C→A works with no measurable penalty, so a USB-C reader can
live on a USB-A port in the travel case. **That was not known to be true before today**, and the
first adapter tested would have taught the opposite.

> **⚠ RIG PROTOCOL, added 2026-08-07 after Claude broke it.** A swap request and a running
> measurement were issued in the same message, and the rig was — correctly — swapped mid-run.
> **Two states, and Claude MUST name the current one whenever it changes:**
>
> | Signal | Means |
> |---|---|
> | **RIG FREE** | Nothing is reading. Swap, pull, replug anything |
> | **HANDS OFF** | A measurement is running. Do not touch the rig until Claude says otherwise |
>
> **A request to swap hardware MUST NOT appear in the same message as a running measurement.**
> The operator cannot be expected to reconcile the contradiction, and the cost is a lost run.

**Adapter 2 changes the conclusion, and it is the more useful outcome.** A working unit from the
same pack means **the design does carry SuperSpeed and unit 1 is defective** — so the 10 Gbps
claim is true of the product, C→A is a real option for the travel case, and the return is for a
bad item rather than a misdescribed one.

> **This is why the spares had to be tested rather than assumed.** Stopping at adapter 1 would
> have recorded "UGREEN adapters are USB 2.0 junk" — a conclusion about a *product line* drawn
> from a *single unit*, which is the same error this project has now made twice about card readers
> (the SDDR-409's "247 ceiling" and the Lexar reader's "222"). **One sample never characterizes a
> population**, and the fix each time is the cheapest possible second sample.

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

### Started 2026-08-07 — the instrument, and the outcomes pre-registered

**Instrument: the UGreen reader with the SanDisk 512 card**, moved as a unit into the front-right
outermost USB-A port through the **original** adapter. That pair measured **275 MB/s** minutes
earlier, so it is a known-good combination and the adapter is the only unknown.

**The PnP parent chain settles this on its own and no throughput run is required** —
`Generic SuperSpeed USB Hub` means the adapter carries SuperSpeed, plain `Generic USB Hub` means
it negotiated USB 2.0. A number is only worth taking afterwards, for the record.

**Three outcomes, written down before the result so none of them can be fitted afterwards:**

| Reading | Verdict |
|---|---|
| **~275** | The adapter is sound and was **mis-seated** the first time. The spares are then irrelevant |
| **~40** | The adapter genuinely does not carry SuperSpeed. *Now* the two spares are worth trying |
| **~90–104** | ⚠ **Not the adapter — the CARD came loose in the move.** Remove and reinsert the card, re-read |

> **The third row is the one that would otherwise be misattributed.** Handling the reader puts the
> card in play too, so a UHS-I reading on this test has a cause that has nothing to do with the
> adapter. **Two candidate failures share this path**, and naming both before the run is what stops
> the first plausible one from collecting the blame.

#### Adapter 1 of 3 — FAILS, firmly seated: `Generic USB Hub`, Intel 3.10

**USB 2.0 again**, with Terry reporting the adapter pushed hard into the hub and the cable hard
into the adapter: *"if it doesn't do SuperSpeed now it never will."*

**Conclusive because two variables moved and the answer did not** — a **different reader** (UGreen,
not the SDDR-409) and **deliberate firm seating**. Same port, same companion hub, same controller.
*When two runs agree, change the other variable*, and the only one left is the adapter itself.

**No throughput run was taken and none was needed.** The chain is decisive, and a USB 2.0 link caps
near 40 MB/s whatever else is true — which also **retires the trap row above**: USB 2.0 masks a
UHS-I problem completely, so no reading on this path could have been misattributed to the card.

> **↺ A prediction recorded and refuted, kept because the error is the interesting part.** ~275 was
> pre-registered on a seating hypothesis. **The original read — "the adapter is bad" — was right,
> and it was abandoned an hour later because the card-seating discovery was fresh and vivid.**
>
> **A new lesson gets over-applied to the next case that looks like it.** The card and the adapter
> shared a *symptom shape* — flat at a lower spec's ceiling on a path proven good — and nothing
> else. **Symptom shape is not a mechanism**, and a finding that explains one case is not evidence
> about another that merely resembles it.

---

## Closed 2026-08-07

- **USB-C→USB-A adapters — 2 of 3 good, 1 defective.** Full record above. **C→A is a real
  capability for the travel case at zero measurable cost**, and the dud went back to Amazon. The
  session's most reusable lesson came from the near-miss: stopping at the first failure would have
  condemned a product line on one unit
- **Reader characterization — all three cleared at 275–280 MB/s.** Full record is above rather
  than here, because the protocol is worth re-reading before the next matrix and moving 120 lines
  would bury it. **What it produced beyond the numbers:** a seating failure that reads as a
  hardware spec limit, and [`CONOPS.md`](CONOPS.md)'s *a card reading slow is a link problem* —
  a field table keyed on the number rather than on which reader is in the bag

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

