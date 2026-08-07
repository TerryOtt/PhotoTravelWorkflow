# End-of-day photo offload — design

Status: **design complete, not yet implemented.** Decisions are numbered, each recorded
with its reasoning, plus what was considered and rejected; what remains to build is
listed at the end.

## The goal, in one sentence

Get back to the hotel, plug two card readers and three external SSDs into a Thunderbolt
hub, run one command, go to dinner, and come back to four verified copies of the day's
photos — so an SSD can go in the safe without anxiety.

> **Said better by the operator, 2026-08-05, and worth keeping in his words:** *"four SSDs
> have byte identical copies, don't need to pray your SD or CF cards stay good."*
>
> **That names the anxiety more precisely than the sentence above does.** The worry is not
> about the SSD in the safe; it is that until the run finishes, the day exists **only on two
> cards** — small, removable, repeatedly reformatted, carried around all day, and with a
> failure mode that gives no warning. LANDED is the moment the day stops depending on them.
>
> Several decisions read differently once that is the stated goal rather than an implication.
> **The tool never writes to a card** (binding constraint 2) because the cards are the only
> copy for the whole shooting day and must not be risked by the program meant to rescue them.
> **Both cards are read and compared** (decisions 4, 27) because a card that is quietly going
> bad is the failure this exists to survive. **Decision 32 watches a card against its own
> history** for the same reason. And the verify pass is unbuffered (decision 2) because
> "copied" is not the claim being made — *byte identical, read back off the media* is.

## The two optimization metrics

Standing, project-level, and in strict priority order. Nearly every engineering decision
below traces back to one of these:

1. **Primary — wall clock from launch to LANDED**: all four copies of the date-divided
   raw files written and read-back verified. That moment is the product; it is when the
   data-loss anxiety ends and an SSD can go in the safe.
2. **Secondary — wall clock from launch to run complete**: corroboration, geotags,
   report. Worth shrinking, but **never at any cost to the primary** — an improvement to
   this metric that moves LANDED later is rejected outright.

Everything after LANDED — GPS sidecars, second-card corroboration — is gravy, and is
explicitly allowed to take longer as long as it does not delay that milestone.

### Both metrics are thresholds, not gradients — and both are already met

**The requirement is "finished before dinner is over," and dinner is 60–90 minutes.** The run
measured 16 m 55 s on 2026-08-04. **There is nothing left to win here**, and every further
minute is worth approximately zero.

**Which cuts both ways, and the operator said so outright on 2026-08-04: as long as the
program exits inside the hour, taking longer is perfectly acceptable.** A threshold has slack
on both sides of it. Wall clock the run does not need is not a saving to be banked — it is
budget available for anything that makes the *result* better, and spending it costs nothing
because nobody is in the room. Decision 22's eject retry is the worked example: it keeps
asking Windows to power a drive down until the hour is up, because a drive that parks itself
at minute 40 beats one that gave up at minute 36 and left a chore. **Do not shorten a wait
that no human is waiting through** — and do not read the "never trade for wall clock" rule
above as permission to spend it either. Both directions have the same test: does this make
the verdict more trustworthy, or the morning easier?

That has a consequence worth stating plainly, because the metrics above invite the opposite
reading: **do not trade anything for wall clock while the run is this far under the bar.** Not
clarity, not a safety check, not the hash's readability in 2031, not an afternoon of
engineering time. Decision 17 is the worked example — SHA-256 costs at most three minutes
against the fastest possible alternative, which is 3 % of a window that has 45 minutes of
slack in it, in exchange for an archive a stranger can verify with `sha256sum`. Framed as a
gradient that looks like a real trade. Framed against the threshold it is not a trade at all.

**The failure mode this is written to prevent is optimizing past the point of value**, which
is easy to do because throughput is measurable and confidence is not. On 2026-08-04 an
afternoon went into hubs, ports, cards and tunnels; it found one genuinely faulty card and one
free 2.7× improvement — and none of it mattered as much as wiring phase 4, because the tool
was already comfortably inside the window before any of it started.

**What to optimize instead, once the threshold is met:** whether the operator can trust the
verdict, walk away, and sleep. Wall clock only re-enters the argument if a run approaches the
bar — a much larger shoot, or hardware degrading toward it.

### Total runtime is not the figure of merit, and comparing it would miss the point

**The exposure window is.** It opens the moment the shutter is first pressed and closes when
four validated, byte-identical copies exist. Everything before that instant is time during
which a single card failure loses a day's work, and *that* is the quantity this design
attacks. The Go predecessor this tool replaces took, by the operator's recollection, **20–30
minutes** for this same 3,883-frame day — a figure he offers as memory rather than
measurement, so treat it as an order of magnitude.

`offload` reaches **LANDED at 16 m 35 s** and runs about **33 minutes** in total. Two honest
comparisons follow, and they are different claims:

- **Against the predecessor**, the exposure window closes several minutes earlier — and the
  predecessor's whole run had to finish, because it had no earlier milestone to offer.
- **Against a naïve reading of this tool**, waiting for the run to *complete* would mean
  breathing out at 33 minutes instead of 16. **The restructuring is worth ~15 minutes of
  anxiety on its own**, and costs nothing, because the deferred work was never what the
  operator was waiting for.

**So a reviewer comparing total runtimes would conclude this project bought nothing, and
would be wrong.** Total runtime went sideways *on purpose*: corroboration and geotagging were
moved to *after* the guarantee rather than before it, which lengthens the run while shortening
the only interval that carries risk. Decision 1 is the same trade in miniature — the SDXC read
is deferred rather than skipped, preserving the guarantee while keeping ~20 minutes off
LANDED.

**This is the answer to "the new tool takes as long as the old one, so what was the point?"**
The point is that the operator can stop worrying at the halfway mark, and everything after it
is gravy that happens while he eats.

**And two more returns that never show up in a stopwatch**, both recorded from the operator
on 2026-08-04 because they are the sort of thing that is obvious while it hurts and invisible
once fixed:

- **No drive letters are typed, ever** (decision 6). The predecessor took source and
  destination letters as arguments, which never once went wrong — and *that is not the
  measure*. The cost was the care it demanded: standing in front of File Explorer
  cross-checking letter assignments, at the end of a day that started at 3:30 am for sunrise
  and ended at 9 pm after sunset, tired and hungry. **A footgun that has not gone off is
  still being carefully avoided**, and the avoidance is the tax. Identifying destinations by
  hardware serial removes the decision from the ritual rather than making it safer.
- **The operator never has to trust the system tray again** (decisions 2 and 22). The fear
  was concrete and correct: tens of gigabytes buffered in RAM, a drive that *looks* finished,
  and a right-click on "safely remove" as the only thing standing between that and a corrupt
  archive. `FILE_FLAG_WRITE_THROUGH` answers the first half **today** — LANDED means bytes
  are on media, not in a cache. Decision 22's eject answers the second half and **is not yet
  built**; until it is, the report says so in as many words rather than implying a safety it
  cannot deliver.

Both are the same shape as the exposure window: the win is a decision the operator no longer
has to make correctly while exhausted.

This is not a "correctness at any cost" design. Where certainty and wall clock conflict,
the tie is broken in favor of wall clock *provided the guarantee is preserved, only
deferred* — see [Phase 3 reads one card](#1-phase-3-reads-the-cfexpress-card-only).

## Inputs

- **Camera:** Canon EOS R5 — the only body — shooting uncompressed RAW (CR3) stills
  and nothing else (decision 24), ~40–50 MB per frame, clock intended to run on UTC
  (decision 23).
- **Cards:** one CFexpress Type B (512 GB), one SDXC UHS-II (512 GB). Every frame is
  written to **both** slots by the camera. Both cards are formatted in-camera at the
  start of every **shooting session** — there may be several in a day, and a session
  never spans more than one local-time day.
- **GPX tracks** covering the day, from an external logger.
- **Multiple offloads per day are normal** — a lunchtime offload and an evening one,
  one per session. The cards are typically reformatted between them (the next
  session's format), which resets the camera's file counter and causes filename
  collisions in an already-populated date folder.

## Outputs

Four independent copies, each containing `YYYY\YYYY-MM-DD\` directories holding the raw
files and their geotagged XMP sidecars:

| Copy | How it is found | Notes |
|---|---|---|
| `C:\` on the laptop | a path on this machine's own disk | Present whenever the tool runs; nothing to eject |
| External SSD × 3 | by disk serial, on the hub | Ejected when the run completes, then into the safe |

Plus a JSON manifest per destination making each copy self-describing.

**All four copies are backups, and they are interchangeable.** None of them is a working
copy, none is edited, and all four are expected to stay byte-identical forever — the
laptop's included. Editing happens at home on a different machine entirely: one of the
four is copied to a NAS, and the desktop edits from there. The fourth copy exists for the
same reason as the other three.

**Lightroom never opens one of these four — but it does read a copy of one.** The
distinction matters in both directions: nothing edits a destination, so none of them may
drift (decision 11); and the sidecars this tool writes are eventually parsed by Lightroom
from the NAS, which is why they have to be exactly what Lightroom expects rather than
merely XMP-conformant. That is what the engine's validation against Lightroom Classic's
own output is for (decision 17), and it is why the directory layout is Lightroom's too
(decision 31).

The only asymmetry is physical, and it is about how a destination is *found* rather than
what it is for: three are removable devices identified by disk serial (decision 6) and
are ejected at the end of a run (decision 22); one is a path on the disk this program is
running from, and there is nothing to eject.

**Date folders are derived from the UTC capture time.** Deliberate: no timezone logic
anywhere, monotonic across a trip, and unambiguous when crossing the date line. The
accepted consequence is that a shot taken early in the morning east of UTC files under
the previous day.

## Architecture: five phases, then eject

| Phase | Mandatory? | Work | Ends with |
|---|---|---|---|
| **1 · Pre-flight: camera card contents** | **Always** — enumeration is where **N** comes from; a second card adds the match, not the walk (decisions 27, 7) | Find the cards, speed-test to pick the ingest source, walk each CR3 listing — and with two cards, assert they agree name for name and size for size | One file set, and **N** |
| **2 · Pre-flight: destinations and GPX** | Always — `--without` and `--no-gpx` narrow what it asserts, never skip it (decisions 25, 26, 16) | Resolve destinations by serial, assert four distinct, writable, capacity ≥ N; parse GPX; inhibit sleep; sweep orphaned temps; print the ETA | You can walk away |
| **3 · Ingest & verify** | **Always — this phase is the product** | Read CFexpress once → SHA256 + EXIF → fan out to 4 → unbuffered read-back verify per destination | **LANDED** |
| **4 · Corroborate** | Two cards only — a single-source run has nothing to compare (decision 7) | Read SDXC fully, compare hashes, delete + tombstone mismatches | Card health report |
| **5 · Geotag** | Only with tracks (decision 26), and only frames a track brackets within limits (decision 16) | Correlate stashed capture times to GPX, write sidecars to all 4 | Ready to edit from |
| **6 · Eject** | Unless `--no-eject`, and only once nothing remains for the current cards (decision 22) | Lock, dismount and power down each destination resolved by serial — concurrently, retried with backoff until 90 minutes after launch | The SSDs can go in the safe |

**Eject is a stage rather than a footnote, and it is timed like one** — promoted 2026-08-04
at the operator's request. It is different in kind from the five phases: it moves no data and
cannot turn SAFE into NOT SAFE. But it can now run for twenty minutes while it argues with
Windows, and **an unlabeled twenty-minute silence reads as a hang while a timed one reads as
persistence.** Printing its wall clock is the whole difference. It is also the project's
number one technical risk in the operator's own assessment (decision 22), which is reason
enough for it to appear in the accounting rather than after it.

**Phases 1 and 2 are both pre-flight**, and together they are the ten seconds that decide
whether you can leave. They are split because they answer different questions and fail
differently: phase 1 establishes *what tonight is* — walking the camera cards, which is
where the file set and **N** come from, and with two cards proving both hold that one set
— while phase 2 checks whether the rig can take it. Both always run; what a second card
adds to phase 1 is the match, not the walk. The order is forced rather than chosen: **N**
is phase 1's output and phase 2's input, since a capacity assertion needs a number to
compare against. It also puts the fatal that means *equipment failure* ahead of the ones
that merely mean *go fetch something*.

> ### ✗ Phases 4 and 5 *do* wait for LANDED — the paragraph below describes an intent, not the code
>
> **Found 2026-08-05.** `pipeline::run` joins every destination thread — write pass *and*
> verify pass — before it returns, and `main` only then calls `corroboration_phase`. So the
> overlap this section has described since the first draft has never existed: phases 3, 4 and 5
> run strictly in sequence.
>
> **The arithmetic was already in this document and nobody read it.** The 2026-08-04 run
> reports `LANDED · phase 3 took 13m 04s`, corroboration at *~19.5 minutes*, and a total wall
> clock of *32.78 min*. Those add up — 13.1 + 19.5 + 0.3 ≈ 32.9 — which they could not do if
> phase 4 had run underneath the verify pass. **A number that contradicted the design was
> printed, quoted approvingly, and read past twice.**
>
> **It costs the secondary metric and nothing else**, which is why it survived: LANDED is
> unaffected, both metrics are thresholds, and both are met. It is recorded rather than fixed
> for that reason — but the *description* had to change, because a design document that
> describes unbuilt behavior in the present tense is how the next reader plans against a rig
> that does not exist.
>
> **And it accidentally made a wrong inference safe**, which is the part worth keeping. The
> operator watched the SD reader start blinking and concluded the run was past LANDED. **True
> as built, and false as designed** — under the intended overlap the SD read begins at the end
> of the *write* feed, well before the verify pass finishes. He was reasoning about the
> document; the document was wrong; the code happened to agree with his conclusion anyway.

**Phases 4 and 5 do not wait for LANDED.** The moment the CFexpress reader goes idle —
the end of phase 3's write feed, since backpressure keeps the reader busy until roughly
the last write drains — both have what they need: the SDXC read begins, and with every
capture time already stashed and the GPX parsed since pre-flight, sidecar writing does
too. Both overlap phase 3's verify pass, and they do not contend with each other either —
one reads the SD card, the other writes a few thousand 3 KB files. All of it serves the
secondary metric; decision 2 explains why the two-pass verify is what makes the early
start possible.

### Where the wall clock goes

Let **N** = one copy of the day's raws.

> **Measured from the Lightroom catalog on 2026-08-04, and the estimate it was checked
> against turned out to be well calibrated.** Across the 83 shooting days of the R5 era —
> 2022 onward, the only years whose files are the ~54 MB uncompressed CR3s this tool
> actually carries:
>
> | | GB per day |
> |---|---|
> | Median | **43** |
> | Mean | 68 |
> | p75 | 90 |
> | p90 | 181 |
> | p95 | 193 |
> | **Max** | **415** — 2024-10-02, ~7,350 frames |
>
> **`N ≈ 50 GB` for a normal day sits between the median and p75, and `N ≈ 188 GB` for a big
> day lands on p90–p95.** Both stand. **What the estimate lacked was a ceiling**, and that is
> the correction: the largest day on record is **415 GB**, more than twice the "big day," and
> a day over 200 GB happens about once a year — 2021, 2022, 2023 and 2024 each have exactly
> one, so the extreme recurs rather than being freak.
>
> **A first pass at this got it wrong by ignoring the operator's own history**, and the
> mistake is worth keeping because it is a data-quality failure rather than an arithmetic
> one. Measured from 2015 the median day is 20 GB and the estimate looks badly off — but
> bigger storage arrived in a D3300 in 2017 and the switch to RAW came in 2019, so those
> early years are 3 MB JPEGs and a 2016 day is 0.3 GB. **Averaging across an equipment
> change measures the equipment, not the photographer.** The transitions are plainly visible
> in the mean file size: 3 MB through 2016, 7–10 MB to 2020, 16 MB in 2021, and 26–27 MB from
> 2022 — which is a raw and its sidecar, and is how the R5 era identifies itself.
>
> ⚠ **That 26–27 MB is per *file*, not per frame, and it reads as the frame size.** Every raw
> is paired with a ~3 KB sidecar, so the mean is almost exactly half the truth. **An R5
> uncompressed CR3 is ~52–56 MB** — measured directly 2026-08-06: 51.8 MB across 2022-09-27's
> 3,883 frames and 56.1 MB across 2024-10-02's 7,395. Use **~55 MB** for anything that asks
> how many frames fit somewhere; the 26 is only ever an era fingerprint. A session that read
> it the other way halved every card-capacity answer it gave.
>
> *Method: whole date folders from the historical archive, so file counts include sidecars.
> The arithmetic checks out on the known day — 2022-09-27 reports 7,767 files, exactly 3,883
> raws + 3,883 sidecars + 1 manifest.*

Phase 3 moves **9N**: 1N read from CFexpress, 4N written, 4N read back to verify. Only
**7N crosses the Thunderbolt hub**, because the laptop's copy is internal.

**Measured on the rig, 2026-08-03**, against the real 2022-09-27 shoot — 3,883 frames,
201.3 GB, offloaded to all four destinations and read back:

| Link | Measured | Time at N = 201 GB |
|---|---|---|
| CFexpress read (1N) | 675–757 MB/s | ~5 min |
| SDXC read — phase 4 only (1N) | **205 MB/s** on a sound card | **~16 min** |
| OWC, Thunderbolt (2N) | write ~292, read 2,540 | ~13 min |
| SanDisk / WD, 10 Gbps USB (2N) | write ~292, read ~900 | **~15 min** ← binds |
| Laptop NVMe (2N) | write ~292, read 3,044 | ~12 min |

**The whole run took 20 min 27 s**, and every one of the 15,532 `(file, destination)`
pairs verified. **A later run with the CFexpress moved to a Thunderbolt reader took
18 min 06 s**, and the shape of that gain matters more than its size — the narratives are in
[`RUNS.md`](RUNS.md).

### Phase 3 in detail

The mechanism, stated explicitly since it is what everything else is measured against:

1. A **reader** pulls one file from the CFexpress card into a buffer taken from a bounded
   pool. Each source file is read **exactly once** — never once per destination.
2. From that in-memory buffer: the **SHA-256**, which becomes this photo's canonical hash,
   and the **EXIF capture time**, which decides its output directory and filename.
3. The buffer is handed to **four independent writer queues**, one per destination. They
   are independent so a slow SSD stalls only itself; the pool size bounds how far it can
   lag and applies backpressure to the reader when it falls too far behind.
4. Each writer writes through (`FILE_FLAG_WRITE_THROUGH`) to a temporary name, then
   **renames** — so a partial file never carries the real name, and nothing renamed is
   still sitting in a cache.
5. When a destination's write pass completes, its **verify pass** begins: every file
   re-read sequentially with `FILE_FLAG_NO_BUFFERING` and compared against the canonical
   hash. One pass per destination, each starting as soon as its own writes finish — the
   laptop's NVMe is verifying while the slowest SSD is still writing.
6. A record per `(file, destination)` is appended to the run log as each verify read
   completes — never before.

**Writes are write-through; verify reads are unbuffered.** The two flags are
deliberately different, and neither substitutes for the other — decision 2 carries the
why for each side.

## Decisions

### 1. Phase 3 reads the CFexpress card only

Optimistic and greedy. In the normal run the SDXC card contributes no bytes to the
output — it is a corroborating hash — and reading it costs ~11.6 minutes at UHS-II
speeds on a big day. Keeping it off the critical path is the single largest available
win against the metric.

The guarantee is **preserved but deferred**: any disagreement is still detected in
phase 4, just after the milestone rather than before it.

### 2. Verification must defeat both caches, and runs as a second pass

Writing a file and immediately reading it back proves nothing — Windows serves it from
the page cache, and you have compared a buffer to itself. So every verify read uses
`FILE_FLAG_NO_BUFFERING`, and verification runs as a **sequential second pass per
destination**: write everything, then read everything back.

The two-pass shape was settled at design review, replacing an interleaved scheme where a
verifier trailed the write front by a ~4 GB byte window. Three things decided it:

- **The primary metric cannot tell the difference.** The slowest SSD must absorb N of
  writes and N of reads under any schedule, so time-to-LANDED is `N/w + N/r` either way
  — and clean sequential passes sidestep whatever a controller loses to a mixed
  read/write stream.
- **The secondary metric can.** Under two-pass the CFexpress reader goes idle at the end
  of the write feed, which is what lets phase 4's SDXC read start during phase 3's
  verify pass. The interleaved scheme holds the reader busy to the end of everything —
  writes finish later because they contend with verify reads throughout — so the early
  start does not exist there.
- **Less machinery.** No lag-window coordination between writer and verifier, and resume
  simplifies — see decision 13.

> ⚠ **The second bullet describes something the code does not do, found 2026-08-05.** Phase 4
> does not start during phase 3's verify pass: `main.rs` runs `pipeline::run` to completion,
> prints LANDED, and *then* calls `phase4::run`. The overlap is argued for here as a reason to
> prefer two-pass, and it was never built.
>
> **Two-pass is still the right shape** — the first and third bullets stand on their own, and
> the primary metric genuinely cannot tell the difference. What is wrong is the *claimed
> benefit*, which matters because it was being used as evidence.
>
> **And the size of it is worth knowing before anyone decides it is fine.** Corroboration is
> ~16 minutes of the ~27-minute run, spent reading the SD card while every archive SSD sits
> idle. Overlapping it with the verify pass is the largest remaining lever on the *secondary*
> metric by a wide margin — and per *Both metrics are thresholds*, that is a reason to record
> it rather than to go and do it: the run finishes in 27 minutes against a 60–90 minute bar,
> so nothing here is worth trading clarity or safety for.
>
> **The estimate now adds corroboration rather than overlapping it**, because that is what the
> program does. Estimating the design instead of the build would understate every run by a
> quarter of an hour.

On a big day the pass structure alone defeats the page cache: hundreds of gigabytes flow
through between a file's write and its verify read. On a small day (a few GB) it does
not, which is why `FILE_FLAG_NO_BUFFERING` stays mandatory — it defeats the OS cache at
any size. What it cannot defeat on a small day is the **SSD's own onboard cache**, which
may still hold just-written data. That residual is accepted and recorded here rather
than engineered away.

The phase 4 overlap carries one measurable risk to the primary metric: three SSD verify
streams plus the SDXC read can brush the hub's usable bandwidth on the biggest days,
slipping LANDED by single-digit percent. Accepted rather than throttled — Windows offers
no clean way to deprioritize one USB stream, and the report's per-destination sustained
rates will show whether the contention is ever real.

The write side is `FILE_FLAG_WRITE_THROUGH` rather than unbuffered — settled at design
review, replacing an "unbuffered in both directions" claim that a raw file's partial
final sector makes unimplementable as stated: `FILE_FLAG_NO_BUFFERING` demands
sector-multiple writes, and the pad-and-truncate dance that would satisfy it buys no
guarantee write-through does not already provide. What LANDED needs is *durably on
media before it is claimed*, not a cache bypass — and a plain buffered write would let
the program exit with gigabytes still in RAM, which makes the metric unmeasurable. The
verify side keeps `FILE_FLAG_NO_BUFFERING`, where the alignment constraint never bites:
short reads at EOF are legal. Write-through also softens the small-day residual above —
even a verify read served from the SSD's onboard cache then describes data the device
has already committed to media.

### 3. Source mismatch: warn by default, delete after the fact

Mismatches between the two cards are real but exceedingly rare — 1–2 files in several
thousand, in years of shooting.

- **Default:** warn, naming the file, and continue.
- **`--fail-on-source-mismatch`:** abort the run. This flag necessarily puts the SDXC
  read in lockstep on the critical path, since "stop before more work happens" is
  meaningless once everything is already written. You pay for strictness only when you
  ask for it.
- On a confirmed mismatch, the file is **deleted from all four destinations.** With
  30 frames of each scene, losing one is not measurable data loss, and it keeps the
  archives clean.

Two safeguards on the deletion:

- **Re-read both copies before acting.** A mismatch may be a transient read error in a
  card reader rather than media corruption. At 1–2 incidents per run this costs ~90 MB
  and almost never happens.
- **Quarantine rather than unlink.** Both variants go to
  `C:\Travel\Images\_runs\<timestamp>\quarantine\`, outside the `YYYY\` tree so Lightroom
  never sees them and the archives stay pristine. ~90 MB per incident, deletable
  whenever, and it makes the irreversible step reversible for the window where it
  matters.

### 4. Corroboration has two outcomes; decision 27 settled the rest

Decision 27 settles *membership* before any byte moves — both cards hold the same
names at the same sizes — so phase 4 is left with the one question a listing cannot
answer, and it has two outcomes per file:

| SDXC hash | Meaning | Action |
|---|---|---|
| matches the canonical hash | corroborated | keep, mark `matched` |
| differs | genuine mismatch | delete from all four, tombstone, quarantine (decision 3) |

This decision originally carried **four outcomes, not two** — *absent* and *present
but never ingested* joined the table — because a pair could diverge mid-day and
phase 4 was where the divergence finally surfaced: a slot fills or errors, the camera
carries on writing the other, and a naive "delete anything uncorroborated" rule would
have deleted every frame shot after the failure. Both branches retired when decision
27 moved the divergence check to pre-flight: a fresh run cannot start diverged, a
lone-card remainder resume must match the listing its run recorded (decision 13), and
a two-card resume is gated again like any fresh run. A file the gate saw that cannot
be read back in phase 4 is not a corroboration outcome at all — a card that changes
under a run is environmental, and environmental is fatal (decision 18).

**Pairing is by card-relative path** — `DCIM\100EOS5R\_50A0001.CR3` — which phase 3
records in the run log for every file it ingests. The camera writes the same tree to
both slots: file numbering is a single camera-level counter and both cards start each
session freshly formatted, so the paths mirror. The bare basename is deliberately not
the key: the counter is four digits with a folder rollover past 9,999, and at ~45 MB a
frame a 512 GB card holds 11,000+ — more frames than the counter spans, so one session
can legitimately contain two `_50A0001.CR3` in different DCIM folders. And the hash is
never the key, because matching by content would read a genuine mismatch as two
unrelated files rather than one disputed one, losing decision 3's quarantine. Run
identity is separate evidence — the cards' volume serials (decision 13) — so pairing
never has to double as a swap detector. And if the two trees ever failed to mirror,
that is a listing difference now: the gate refuses it before phase 3 starts
(decision 27).

The gate also narrowed the residual. Reformatting cards before their run finished now
costs at most what that run had not finished: files already landed lose only their
second opinion — close-out marks them `forfeited` (decision 13) — and files not yet
landed were never claimed safe, which is what the held eject exists to say
(decision 22). What no longer exists is the silent case, frames the tool never knew
about: decision 27 proved at launch that the source card held everything either card
held.

A run reporting far more mismatches than the 1–2 baseline is a dying card, and the
summary should say so in those words rather than making the number speak for itself.

### 5. Filenames are a pure function of the photo

Output filenames prefix the UTC time of capture to the camera's sequence number. The date
is already carried by the directory, so only the time of day is needed:

```
2026\2026-08-03\1422Z_0001.CR3
2026\2026-08-03\1422Z_0001.xmp
```

**Only the sequence number is kept** — `_50A0001.CR3` becomes `1422Z_0001.CR3`. The rest
of the camera's stem is the body prefix, and with the fleet fixed at one R5
(`CONOPS.md`) it is the same three characters on every frame ever shot: it distinguishes
nothing and costs four characters in every filename in the archive, forever. The
separator is this tool's rather than borrowed from the camera's leading underscore, so
the name reads the same way for either shape Canon writes — `IMG_1234.CR3` becomes
`1422Z_1234.CR3` and not `1422ZIMG_1234.CR3`. All trailing digits are taken rather than a
fixed four, so a longer counter cannot silently collide; a stem with no trailing digits
keeps the whole stem, which no camera should produce and which still beats an empty
sequence.

> **Settled 2026-08-03, when phase 3 made the rule executable.** As written, this
> decision showed `1422Z_50A0001.CR3` and said only "prefix the UTC time of capture to
> the camera's own basename" — two different rules that agree on that one example,
> because the R5's leading underscore is Canon's Adobe-RGB marker and `1422Z` +
> `_50A0001.CR3` reads as though a separator had been inserted when none had. The
> ambiguity surfaced the moment code had to choose, and it was settled toward the
> shorter name. *Considered and rejected* carries the reversal it implies.

The camera's uppercase `.CR3` extension is preserved rather than normalized, so the
archive stays consistent with everything already in it and with anything that ever lands
on a case-sensitive filesystem.

The property worth stating explicitly: **the name derives entirely from the photo
itself** — capture time from EXIF, basename from the camera — and not at all from what is
already present in the destination. So it is deterministic, idempotent, and identical
across all four destinations with no coordination between them. The same photo offloaded
twice, or a crashed run re-offloaded, produces the same name every time.

It also sorts in shooting order, which the camera's bare filename does not: after a
mid-day format resets the counter, afternoon `_50A0001` was shot after morning
`_50A3999`. Minute resolution suffices for that ordering, because within any single
minute the camera's counter is monotonic, so ties break correctly on the sequence number.

**Collision handling reduces to one content check.** Two different photos taken in
different minutes now get different names, so the mid-day-format collision that motivated
a rename scheme largely disappears. What remains is the re-offload case, where the same
photo is ingested twice and yields an identical name:

- **Same SHA256** — already ingested. **Skip.**
- **Different SHA256** — two distinct photos sharing a basename within one minute.
  Pathological but not impossible; append `_001`. This branch should effectively never
  fire.

Deciding on filename alone would get the first case wrong in the silent direction — a
genuinely different photo skipped because its name matched — so the hash is what decides.
Phase 3 computes it before writing anyway, so the check is free.

**Historical files keep their existing names.** Renaming already-imported raws on disk
breaks the Lightroom catalog's links to them; a migration, if ever wanted, has to be
driven from inside Lightroom rather than by this tool. New offloads use the new scheme
and the archive is mixed by design.

### 6. Destinations are identified by hardware, not drive letter

Windows reassigns drive letters to external SSDs freely. Passing three letters that
happen to be right today is how two "copies" silently land on one physical disk.

**This is not a theoretical hazard here, and the operating context is why.** The rig is
unpacked and wired from nothing every night, in a different hotel room, at the end of a
shooting day — so enumeration order is genuinely fresh each time rather than settled once
and inherited. The letters observed on 2026-08-04 alone moved from `G/I/J` to `F/I/J` across
a hub change and two replugs, with nothing reformatted and no configuration touched. A
design that pinned letters would not fail on some unlucky future night; it would fail most
nights.

> **Disk numbers renumber across a reboot even when letters do not.** 2026-08-05: before the
> reboot `full-run-check.ps1` reported the OWC as disk 1 and the SanDisk as disk 4; after it,
> they had swapped to 4 and 1 while `J:`, `F:` and `I:` all stayed put. Port classes were
> byte-identical either side — OWC `laptop-tb4` at 2 PCIe hops, SanDisk `hub-tb5` on xHCI
> 3.20, WD `laptop-usb` on xHCI 3.10 — so nothing physical moved. A modest refinement to the
> row above: the two unstable identifiers are unstable *independently*, and a design pinning
> either would break on a night the other survived.
>
> ⚠ **This was first written up here as a mystery, and the error is worth more than the
> observation.** The claim was that the swap happened *within one session with nothing
> unplugged* — presented with the watcher's silence as corroboration and a flourish about it
> needing "nothing at all." **A reboot had happened in between.** The session had resumed
> across it via `claude --continue`, which preserves the conversation and hides the boot, and
> the operator had to say so. `scripts\watch-rig.ps1` logged nothing because the pre-reboot
> watcher died *with* the machine and its replacement started fresh afterwards — so the
> silence that read as evidence was an artifact of the very event being missed.
>
> **The lesson is this project's oldest one, in a new costume: check the mundane cause before
> narrating a novel one.** Decision 6 already said letters move across a reboot; the boot time
> was one `LastBootUpTime` away; and *the resuming session had been told in its own state file
> that a reboot was the next step*. **A plausible mechanism is not a measurement** — and a
> silent instrument is not a negative result until you have checked the instrument was alive.

| Identifier | Survives letter change | Survives reformat | Portable to another PC |
|---|---|---|---|
| Drive letter | ✗ | ✗ | ✗ |
| Volume GUID | ✓ | ✗ | ✗ (per-machine registry) |
| Disk serial | ✓ | ✓ | ✓ |

Store **both**: the volume GUID as a fast local index, the disk serial as the true
identity of the physical SSD. Match on either — if the GUID misses but the serial
matches, pre-flight self-heals the config and says so; if the serial matches but the
GUID changed, that disk was reformatted and you want to hear about it loudly.

Serials also make the distinct-device assertion exact, where four volume GUIDs could be
four partitions on two disks.

> **Measured against the rig on 2026-08-03, and it holds.** All three archive SSDs report
> real, distinct serials — `6479_A751_AF00_3CFF.` (OWC), `2138FB400347` (SanDisk),
> `2143EC400323` (WD) — on three distinct physical disks. So the identity scheme works on
> the hardware it was designed for, which was not a given: a USB bridge that declined to
> report a serial would have left this decision resting on the volume GUID alone.
>
> Two things the same run taught, both now enforced in code. **Serials are stored
> verbatim** — the OWC's really does end in a period, and tidying that away is how two
> devices become one string. And **the distinctness check is real work, not ceremony**:
> the laptop alone presents four volumes on one physical disk, which is exactly the
> shape this assertion exists to reject.

Each destination also carries a `.photoday-destination.json` marker at its root, itself
schema-versioned and readable by every later build (decision 28). An archive pulled from
the safe in 2031 can then prove what it is and verify itself on a machine that has never
seen the config.

### 7. Cards are identified by measurement, and there are always two

An in-camera format at the start of every shooting session assigns a new volume serial,
so a card's volume GUID changes at least daily; and cheap readers report generic or
empty hardware serials, so the reader is not reliably identifiable either.

So pre-flight finds **volumes containing `DCIM`** that are not configured destinations,
reads ~64 MB from each, and uses the faster one as the phase 3 source.

> **Corrected 2026-08-03 against the real rig, and this one would have stopped the tool
> dead.** The rule here said *removable* volumes containing `DCIM`. Both cards were put
> in their readers and enumerated, and they disagree:
>
> | Card volume | `GetDriveTypeW` | Disk serial reported |
> |---|---|---|
> | `D:` `EOS_DIGITAL` | **`DRIVE_REMOVABLE`** | `000000000009` |
> | `F:` `EOS_DIGITAL` | **`DRIVE_FIXED`** | `0123456789ABCDEF` |
>
> Same camera, same hub, two reader bridges that answer differently — and all three
> archive SSDs report `DRIVE_FIXED` as well. A rule filtering on removability finds one
> card, and decision 7's own refusal then fires: *ONLY ONE CARD FOUND*, on a night when
> both are sitting in their readers. Removability describes the enclosure's firmware, not
> the medium, and it is not usable evidence about anything.
>
> **`DCIM` is the discriminator, and the configured destinations are the exclusion.** A
> volume the config already names is a destination and can never be a card, which closes
> the only way `DCIM` alone could go wrong.
>
> The same run confirmed this decision's other premise by measurement rather than by
> assumption: **both readers report fake serials.** `0123456789ABCDEF` is the more
> dangerous of the two precisely because it looks plausible — two readers could easily
> report it and collide. Cards are identified by measurement because they cannot be
> identified any other way. CFexpress lands near 65 ms against UHS-II's 240 ms
— unambiguous. Costs two seconds, needs no configuration, survives buying a new reader,
and is correct by construction: phase 3 always runs off the fast card regardless of which
reader is in which port. A config override exists for the day that surprises us.

> ⚠ **A dual-CFexpress body would remove the discriminator this decision runs on, and that is
> worth knowing before it happens rather than after.** The operator, 2026-08-05, on rumours of
> an R5 Mark III: *"that single feature would legit separate me from basically whatever Canon
> decided to charge."* Two CFexpress cards measure within noise of each other, so *"the faster
> card is the source"* stops picking anything — it becomes a coin toss between two equals.
>
> **That is a smaller problem than it looks, and the reason is decision 12.** When the cards
> are equals, an arbitrary choice is a *correct* choice: either is a fine source, and
> `source_volume_serial` already records which one was actually read rather than which role it
> was assigned. The record stays honest without the tie-break being meaningful. What would need
> checking is only that the selection is **stable within a run** and does not oscillate between
> two near-identical timings.
>
> **What it would buy is most of the wall clock.** Corroboration is SD-bound and takes ~18 of
> the ~27 minutes; off a second CFexpress it is ~3. **LANDED does not move** — phase 3 already
> reads the fast card — so this is entirely a secondary-metric win, from ~27 minutes to ~12.
> Per *Both metrics are thresholds*, that is a reason to want the body for other reasons and
> enjoy the side effect, not a reason to buy one.
>
> A new body is a design event either way (`CONOPS.md`), and decision 34's config check would
> report it on the first run.

**A single card at offload is an equipment failure, not a mode.** The camera has two
slots for a reason and every frame is shot to both (`CONOPS.md`, the shooting-day
contract), so this tool is always run with two authoritative sources. If pre-flight
finds one card, something upstream is wrong — the other card still in the camera, a
reader gone dead, a card gone bad — and the default is to **refuse to run**, in the
first ten seconds, while the fix is still a reach into the camera bag:

```
╔════════════════════════════════════════════════════════════╗
║  ONLY ONE CARD FOUND — SDXC is missing                     ║
║                                                            ║
║  Every frame is shot to both cards. If this offload has    ║
║  only one, a card, a reader, or the camera has failed.     ║
║  Check the rig.                                            ║
║                                                            ║
║  Refusing to run. If one card is truly all there is        ║
║  tonight, re-run with --allow-single-source.               ║
╚════════════════════════════════════════════════════════════╝
```

Proceeding requires saying so: **`--allow-single-source`**. Under the flag, whichever
card is present becomes the sole source of truth — CFexpress or SDXC makes no
difference, the two cases are equally bad and are treated identically. Phase 3 runs on
that card and **phase 4 does not run at all** — not deferred, eliminated: corroboration
is a comparison, and no second source exists to compare against. Every file is
recorded `waived` rather than corroborated (decision 12), and the eject gate treats
waived as settled (decision 22) — the SSDs still eject once the card's contents are
verified on all four destinations and phase 5 is complete, because holding them for a
card the operator has declared absent would hold the night hostage to nothing.
The verdict carries the scar (decision 14), the exit code is 2 (decision 18), and if
the missing card ever does turn up, a re-run converges: corroboration finishes and
waived upgrades to matched (decision 13).

One case is exempt because it is not a single-source run at all: a resume of a night
that had two sources (decision 13). That night's pre-flight recorded both cards' volume
serials, so a lone card whose serial matches the incomplete run is not a lone source —
it is the remainder of a known pair, and the run continues with whatever that card can
answer for. A lone SDXC finishes corroboration, and the SSDs eject at that moment
(decision 22). A lone CFexpress finishes landing and verifying the raws while
corroboration stays pending: the SSDs stay mounted and the verdict says to insert the
SDXC and re-run (decision 14). The recorded serials are what tell a remainder from a
lone source, and no flag is required.

Two simpler postures were rejected. Refusing outright, with no escape, leaves a
one-card night with zero backups — the exact inversion of the goal. Proceeding
automatically behind a warning — this design's first answer, replaced the same day —
makes routine what must never be routine. The flag is the narrow gate between them:
the run is always possible, and it can never happen by accident. Presence is also only
half of what pre-flight asserts about a pair — decision 27 holds the two cards present
to a single listing before phase 3 moves a byte.

### 8. One command, almost no arguments

"One intuitive CLI command" and "six paths that shuffle between sessions" are in tension,
and typing destination paths at 11pm after a day of shooting is how a destination ends up
pointed at the wrong disk. So the rig is described once in config, and the nightly
command is bare:

```
offload
```

```
offload                            the nightly command

  --dry-run                  plan the entire run and write nothing — names every
                             output file exactly, in seconds
  --jobs <N>                 CPU pool for hashing/EXIF/XMP [default: logical CPUs]
  --fail-on-source-mismatch  abort rather than warn when the two cards disagree
  --allow-single-source      proceed when only one card is present — it becomes
                             the sole source of truth; corroboration is waived
  --without <LABEL>          run without a named archive destination — pre-flight
                             otherwise refuses when one is missing; repeatable;
                             re-run the night when it returns
  --gpx <PATH>               override when tracks aren't in the usual place
  --no-gpx                   proceed with no tracks at all — raws land as normal,
                             no sidecars are written; pre-flight otherwise refuses
                             when the GPX directory holds none
  --max-gap-seconds <S>      refuse to interpolate across a longer hole [default: 60]
  --max-gap-meters <M>       refuse to interpolate across a wider hole [default: 100]
  --force-xmp[=<DEST>]       overwrite existing XMP on every destination, or on
                             just the one named
  --no-eject                 leave the archive SSDs mounted when the run ends
  --eject-gap-seconds <S>    ask this often during eject, instead of the
                             2s-doubling-to-60s backoff — a diagnostic

offload verify <DEST>       standalone re-verify; works years later, without config
```

> **`--eject-gap-seconds` is the last survivor of the eject investigation**, and it should be
> looked at against `CLAUDE.md`'s *a config item that is never used MUST NOT exist*. Its sibling
> `--eject-prepare` was deleted on 2026-08-06 once the comparison it existed for had settled.
> This one is a cadence override rather than a mode selector, so it cannot select known-bad
> behavior — but nothing routine passes it, and `examples/eject-one.rs` can do the same job.
> **Left in place pending a decision rather than removed quietly.**

Config is JSON:

```json
{
  "destinations": [
    { "label": "laptop",
      "path": "C:\\Travel\\Images" },
    { "label": "SSD-A",
      "disk_serial": "S5H9NS0R123456",
      "volume_guid": "{a1b2c3d4-...}",
      "subpath": "Images" }
  ],
  "gpx_dir": "C:\\Travel\\GPX"
}
```

**There is no `role` field, as of decision 11's correction.** It carried `working` and
`archive`, and those named a distinction that does not exist — all four copies are
backups. What is left is how a destination is *found*, and the config already says it: a
`path` is a location on this machine's own disk, a `disk_serial` is a removable device on
the hub. Everything that used to consult the role now follows from that one fact —
notably eject, which applies to exactly the destinations resolved by serial (decision
22). A separate field restating it would be a second place to get it wrong.

**It lives at `%APPDATA%\offload\config.json`** — settled 2026-08-03, having been shown
here without ever being located. The Windows convention, and the one a Windows developer
looks in first; it survives rebuilding the binary, which a config sitting beside
`target\release\offload.exe` does not, and that matters because `TRIP-HYGIENE.md`'s trip
hygiene ends in *rebuild, then dry-run against the real rig*. Reading one environment variable
is the whole implementation, which is why decision 29 declined the `directories` crate.

A missing config is a pre-flight fatal that names the path it looked in, not a prompt and
not a generated skeleton — a tool that invents a config would be inventing a rig. There
is deliberately no `--config` flag: typing a path at 11pm is the failure this decision
exists to prevent, and the one command that genuinely must run without any of this is
`verify`, which reads nothing but the disk (decision 20).

The risk is config drift — an entry wrong in a way you don't notice until it matters.
Pre-flight validates every entry against connected hardware and fails in the first ten
seconds, so drift surfaces while you are still standing at the desk.

### 9. Pre-flight must be able to fail, and only there

The worst outcome for a walk-away tool is returning from dinner to a run that died two
minutes in. Before anything is written, pre-flight asserts — in the two phases whose
order the data forces rather than taste:

**Phase 1, the camera card contents.** Walking them is what produces the file set and
**N**, so it happens at any card count. The assertions layered on that walk: both cards
present — `--allow-single-source` (decision 7) and a remainder resume (decision 13) are
the deliberate exceptions — the faster one readable and serving as the phase 3 source,
and wherever a run does have two, both listings identical name for name and size for
size (decision 27).

**Phase 2, the rig.** All four destinations present — `--without` is the declared
exception (decision 25) — distinct physical devices, writable, and with capacity ≥ N
plus margin, which is precisely why this cannot come first; orphaned temps swept
(decision 13); sleep inhibited (`SetThreadExecutionState`); GPX tracks present and
parsed — `--no-gpx` is the declared exception (decision 26).

**It also checks Windows Defender exclusions.** Real-time scanning of several hundred
gigabytes of freshly written files across four volumes is a large and invisible tax on
exactly this workload. The archive roots should be excluded; pre-flight checks, and
**warns rather than fails**, since this is a throughput problem and not a correctness
one.

The check has three outcomes, not two, because Windows hides the exclusion list from
unelevated processes — and this tool runs unelevated by design: nothing else in the run
needs administrator rights, and demanding them for a throughput check would be
backwards. Readable with the roots excluded is silent; readable without them is the
warning, naming the roots; unreadable says exactly that — the list could not be read,
check Windows Security by hand — and never masquerades as the not-excluded warning.
Conflating the two would fire a false warning on every run on precisely the machines
that hide the list, and a warning that fires regardless of the truth is the warning you
learn to read past (decision 12). Where the list stays unreadable, the real check is
one the report already prints: the per-destination sustained rates (decision 14), where
a Defender tax shows as every destination running far below its known ability.

> **Confirmed on the rig, 2026-08-04: the unreadable outcome is not hypothetical.**
> `Get-MpPreference` from an unelevated session returns
> *"N/A: Must be an administrator to view exclusions"* for all three exclusion lists, on a
> machine whose owner is a full administrator. Real-time protection was on and `WSearch`
> was running. The three-outcome design is therefore load-bearing rather than defensive.

**When the exclusions are set, they should be by extension and process — not by path.**
Recorded 2026-08-04 as future work; the operator's decision on the trade is below.

| Exclusion | Why it suits this rig |
|---|---|
| **Extension** — `CR3`, `xmp` | Covers essentially every byte the archive holds, on any drive, at any letter, forever. Nothing to maintain |
| **Process** — `offload.exe` | Letter-independent, and scoped to the one binary that writes these files |
| ~~Path~~ | The archive roots move: decision 6 exists precisely because Windows reassigns letters to these drives, and it observed `G/I/J` become `F/I/J` in one evening |

**Pinning drive letters to make path exclusions viable is rejected.** It would reintroduce
the exact dependency decision 6 removed, and it buys nothing — the tool resolves
destinations by serial and never needs a stable letter. If stable paths are ever genuinely
wanted, NTFS **mount points** into fixed folders are the supported mechanism, not
`HKLM\SYSTEM\MountedDevices`.

**The operator has settled the security trade explicitly: he is not worried about malware on
travel photo SSDs.** So a broad extension exclusion is acceptable here in a way it would not
be on a general-purpose machine, and this design need not hedge it. Note the split that keeps
binding constraint 4 intact: **setting exclusions is a one-time administrative act by the
operator; pre-flight only ever checks and warns.** Nothing in a run comes to need elevation.

Because the cards are formatted at the start of every session, enumeration is exact and
cheap — the card *is* the session — so pre-flight can print a real estimate:

```
1,247 files on both cards · 56.1 GB · 4 destinations verified distinct · est. 6-8 min
```

That number is what actually lets you leave. *On both cards* is the gate's receipt
(decision 27) — the assertion that just passed, and what makes the estimate cover the
whole day rather than the source card's share of it, since files only the other card
held could once have surfaced and been ingested long after the estimate was printed. A
declared single-source run prints `1,247 files · single source (SDXC)` instead: nothing
agreed, and nothing claims to have.

### 10. Phase 3 collects EXIF for free, so phase 5 re-reads nothing

Phase 3 already holds each file in RAM to hash it, so extracting capture time costs no
I/O. Stashing it in the run manifest means phase 5 never re-reads a raw file — it
correlates timestamps against the GPX index and writes a few thousand 3 KB sidecars.

This eliminates a full re-read of the day that a standalone geotagging pass would cost,
without putting anything on phase 3's critical path. Sidecar generation failure is never
fatal and is always backfillable; the one fatal near geotagging is pre-flight's, before
anything runs — an empty GPX directory is refused unless declared away (decision 26).

### 11. All four copies are backups; none of them is a working copy

**Reversed 2026-08-03, on the operator's correction.** This decision previously read
*"the laptop copy is a working copy — after the trip"*, and had Lightroom importing from
the laptop, rewriting its sidecars, and that copy diverging from the archives by design.
That is not the workflow and never was. What actually happens at home: **an archive SSD
is copied to a NAS, and editing happens on the desktop, from the NAS.** The laptop's copy
is a fourth backup, kept because a fourth backup is worth having, and nothing edits it.

The correction is worth recording rather than quietly deleting, because the wrong version
had reached six other decisions and each was reasoning from it:

- **Nothing this tool writes is ever edited**, so no copy is *expected* to drift. All
  four stay byte-identical, forever, and `verify` may hold every one of them to that
  standard (decision 20). The tolerance for sidecar drift on one destination is gone,
  and it was the most dangerous consequence of the error — a verification tool with a
  blind spot it cannot justify.
- **`--force-xmp` had no honest reason to scope itself by role** (decision 16).
- **Eject is about removable versus internal, not archive versus working** (decision 22).
- ~~**`sync`'s copy source is the laptop**~~ — withdrawn 2026-08-06; kept only because the point below still holds,
  not because it is authoritative (decision 20).
- **`_runs` lives on the laptop for the same reason** (decision 14).
- **The manifest's raws-only rule keeps one of its two supports** (decision 12).

The general lesson, since it will recur: **a fact about the operator's habits was
inferred from the shape of the rig rather than asked about.** Four copies with one on the
machine that has Lightroom installed *looks* like a working copy and three archives, and
the design reasoned from that resemblance for its entire life. `CONOPS.md` exists to
carry exactly these facts; when a decision rests on one, it belongs there first.

### 12. The manifest covers raw files only

Raws and sidecars have opposite natures. A raw file is immutable — hash it once, and any
later deviation is corruption. A sidecar is *supposed* to change: **a re-run of phase 5
against a better track rewrites it**, which is a normal and desirable thing to do.

If `verify` hashed both the same way, re-tagging a day from a corrected GPX track would
make every copy report as damaged, and its output would become something to ignore —
which quietly destroys the only thing the tool exists to provide. **A verification tool
whose warnings you learn to ignore is worse than one that checks less and means it.** So
sidecars are treated as regenerable derived data and are not covered.

> **One of this rule's two supports was removed on 2026-08-03** and it stands on the
> other. The stronger-sounding half used to be "Lightroom rewrites a sidecar the moment
> develop settings are touched" — but decision 11's correction establishes that Lightroom
> never opens anything this tool writes, so that half was never true here. Re-tagging is
> the real case, it is entirely sufficient, and the conclusion does not move.

Two artifacts, split by their durability requirements:

- **The run log is append-only** — one record per file as it lands. A crash mid-phase-3
  therefore leaves a valid partial record that resume can read, rather than a truncated
  array. JSON Lines.
- **The per-date-folder manifest is the durable artifact** — written atomically via
  temp-then-rename at the end of a run. JSON.

Per *date folder* rather than per destination root, so a day is self-contained: a
`2026\2026-08-03\` directory can be copied anywhere and still verify itself, and no run
has to rewrite a manifest spanning years.

```json
{
  "schema": 1,
  "date_utc": "2026-08-03",
  "destination": "SSD-A",
  "runs": [
    { "run_id": "2026-08-03T18:22:04Z", "files_added": 1247, "bytes_added": 60236492800 }
  ],
  "files": [
    {
      "name": "1422Z_0001.CR3",
      "status": "present",
      "sha256": "9f2b...",
      "bytes": 47185920,
      "captured_utc": "2026-08-03T14:22:37Z",
      "source_card": "cfexpress",
      "run_id": "2026-08-03T18:22:04Z",
      "verified_utc": "2026-08-03T18:23:31Z",
      "corroborated": "matched"
    }
  ]
}
```

**The manifest carries its own checksum** — the same SHA-256 as everything else
(decision 17). It holds every hash in the archive, so if those
few hundred kilobytes rot in the safe over five years, `verify` would otherwise report
damage on photos that are perfectly intact — and a false alarm on irreplaceable data is
its own kind of failure. A self-hash lets verify distinguish *your photos are damaged*
from *this manifest is damaged, your photos are probably fine*. All four copies each
carry their own manifest of the same day, so they also cross-check against each other —
which is why decision 28 pins the fields that cross-check rests on, and why `schema`
sits at the top of the record rather than being implied by what a reader happens to find.

**`source_card` is the durable name for what the report calls `primary`.** Settled
2026-08-05: operator-facing output says **primary** and **secondary**, because that is what a
tired human at a desk reads and because the tool cannot honestly say *CFexpress* — decision 7
identifies cards by measurement, and a CFexpress in a bridge reader enumerates as USB. The
manifest keeps `source_card`, because a field on disk is read by `verify` years later and
renaming it would be a schema change under decision 28 for a synonym. Two audiences, one
concept, and the mapping is stated here so it is stated once.

**And `source_card` records the role while a new `source_volume_serial` records the
evidence.** Settled with the operator 2026-08-05, replacing
`if plan.cards.agreed { "cfexpress" } else { "single" }` — a line that could never emit the
`"sdxc"` this decision's own example shows, and whose `"cfexpress"` was an assumption nothing
checked. **The volume serial is the closest honest answer to "which card fed this run":** it
is an observation rather than a label, and decision 13 already captures both cards' serials
in the run log, so nothing new is measured. An in-camera format assigns a new one, so years
later it identifies the card *generation* rather than the physical card — which is exactly
what a run is a property of.

**Two fields rather than one packed string**, deliberately. `"primary:A4E2-91CC"` would have
to be split on a colon by every reader, which is the stringly-typed shape this project's
JSON-by-default rule exists to avoid. Adding a field an old `verify` ignores is explicitly
not a schema bump (decision 28); redefining `source_card`'s type would have been one.

> **Adding it broke every existing archive on the first attempt, and the schema-1 fixture
> caught it immediately.** `#[serde(default)]` alone makes the field optional on *read* while
> still *writing* it — so a schema-1 manifest, re-read and re-canonicalized, gains
> `"source_volume_serial": ""`, its `body` serializes to different bytes, and **its own
> checksum stops validating.** Every disk in the safe would have begun reporting as damaged:
> precisely the false alarm on irreplaceable data that decision 12's self-checksum exists to
> prevent, produced by the mechanism meant to prevent it.
>
> The fix is `skip_serializing_if = "String::is_empty"` alongside the default, so an absent
> value round-trips to absent. **The general rule for this manifest: a new field must be
> invisible in the serialized form when it has no value**, because the checksum covers the
> bytes and not the meaning.
>
> This is the fourth test of decision 18's four, doing the exact job it was written for — a
> defect that would otherwise have surfaced years later, on a disk pulled from a safe, with
> nothing left to fix it with.

`corroborated` carries phase 4's verdict per file — `matched`, `waived`, `forfeited`,
or `null` while still pending. `waived` is the single-source run's mark (decision 7):
no second card existed to consult — and `source_card` records which card fed the run,
so a single-source night run off the SDXC appears as `"source_card": "sdxc"` with
`"corroborated": "waived"`. `forfeited` is close-out's mark (decision 13): the card
generation that could have answered was reformatted before phase 4 examined it. The
two non-matched verdicts are deliberately distinct — `waived` is by declaration,
`forfeited` by loss — because conflating them is how a record stops meaning anything
years later. (A third, `absent` — by examination — existed while a pair could still
diverge mid-run; decision 27's gate retired it.) A file deleted for a genuine mismatch
stays in the list as a **tombstone**
(`"status": "deleted"` with both competing hashes, the reason and a timestamp) so a
`verify` years later reports *clean* rather than flagging a missing file nobody remembers
deleting.

The `runs` array is what makes several offloads a day legible after the fact — each file
records which offload brought it in.

### 13. Resume is automatic and scoped by card generation

A file counts as done only for a **specific destination** — the run log records
`(file, destination) → verified`, not `file → done` — so a crash partway through fan-out
means redoing that file on one disk, not redoing the run.

The **log is trusted up to its last intact line.** A verified record is appended only
after that file's verify read completed, so nothing in the log ever describes work still
in flight — the only artifact a crash can leave is a torn final line, which is discarded.
(The interleaved verify this design originally had needed a rule distrusting a ~4 GB tail
of the log on resume; the two-pass verify of decision 2 removed it.)

Interrupted writes leave no ambiguity, because writes are temp-then-rename: a partial file
never carries the real name. Pre-flight sweeps orphaned temps.

**Resume is scoped by card generation, and a generation's identity is its volume
serial.** Every in-camera format assigns a new serial (decision 7), so the format that
starts a session also stamps it — and the stamp is free: pre-flight already opens both
card volumes to measure their speed, and records both serials in the run log alongside
the gated listing (decision 27). A card whose serial matches the incomplete run is the
same generation; a serial the log has never seen is a new generation, and the stale
run is closed out below.

Growth within a generation is legitimate — a session continues after a crashed midday
offload, and both slots grow identically. A **two-card** resume therefore needs no
special rule: the gate compares the pair to itself again, and convergence ingests the
growth. A **lone** card claiming to be a remainder (decision 7) is held to the listing
its run recorded: equal, names and sizes, means the exact remainder — proceed. Grown
means the session continued, and growth needs the pair — the refusal says to insert
both cards, and the same convergence then finishes the remainder and ingests the
growth.

Settled at design review, replacing a file-set comparison — same set meant the same
offload, a different set meant a new one. The set is the wrong evidence in both
directions. A format resets the camera's file counter (decision 5), so a reformatted,
reshot card with the same frame count presents the *identical* path set: resume would
trust a log describing photos it never ingested, and phase 4 would then read every one
of them as a confirmed mismatch — quarantining a verified morning while the evening
never lands at all. And a continued session presents a *superset* — a "different" set —
that would close out a run whose corroboration bytes still sit in the reader. The serial
answers the question actually being asked — did a format happen since this log was
written? — instead of inferring it from what the format leaves behind.

Resume is **automatic** — no flag — and announced:

```
Found incomplete run 2026-08-03T18:22:04Z - 847 of 1,247 files already verified.
Resuming. Est. 3 min.
```

The scenario is discovering at 11pm that the run died. The failure mode of automatic
resume is bounded to redundant work, it cannot produce a wrong archive, and the serial
check already prevents resuming across a card swap. Requiring a flag to get the obviously
correct behavior is friction at precisely the wrong moment.

**Resume covers every phase — extended at design review from phase 3 to the whole run.**
A run is a convergence pass: it does whatever the log says remains — copies what is
missing, verifies what is unverified, corroborates what is uncorroborated, tags what is
untagged — idempotent on finished work, monotonic on the rest. Combined with decision
7's remainder exemption, two recoveries fall out for free, no flag or subcommand needed:
re-inserting just the SDXC days later finishes corroboration alone, and the archive SSDs
eject at that moment, per decision 22's gate; re-inserting just the CFexpress finishes
landing and verifying the raws, and the SSDs stay mounted for the corroboration only the
SDXC can supply — the verdict says exactly that (decision 14).

A generation that never comes back — reformatted and reshot before phase 4 examined it —
is the one thing that can strand corroboration, because the SDXC bytes the old run still
needed no longer exist anywhere. That is detected by evidence, never by timeout: the
moment a new serial appears where the old one was still owed work, the stale run is
closed out and its unexamined files are marked **`forfeited`** in the manifest
(decision 12) — permanently uncorroborated, reported once at close-out with the closing
run exiting 2 (decision 18), informational in `verify` forever after, and never gating
anything again. The mark is its own rather than a reuse of `waived` — by loss, not by
declaration (decision 12). Gating eject on bytes that provably no longer exist would
wedge the tool for good.

### 14. The report separates "your raws are safe" from "everything went well"

> **The four per-destination lines are the payload, not the verdict.** 2026-08-05, watching a
> run finish: *"That's the biggest warm fuzzy of the whole run. Genuine blood pressure drop at
> those four lines."* The verdict is a summary, and a summary is believed rather than checked;
> the relief comes from each destination accounting for itself — `3,883 written · 0 skipped ·
> 3,883 verified`, four times. So those lines get the visual weight, and their badge is a
> signal rather than decoration: it reports a comparison made moments earlier against every
> file. Contrast pre-flight's capacity tick, a receipt for a check that already refused the run
> and therefore can only ever be green.

Phase 3 is the product and the rest is gravy, so **only phase 3 may change the verdict.**
A geotag miss or a track that didn't cover the evening walk is a count in the body, never
a downgrade at the top — otherwise you learn to read past the verdict line, which is the
same failure the raws-only manifest exists to avoid.

**LANDED is announced when it happens**, not only in the final summary, because phases 4
and 5 run on afterward and someone walking in during them should see the thing they care
about already settled.

```
═══ 2026-08-03 · LANDED 18:26:16 · phase 3 took 4m 12s ═══

  1,247 files · 57 GB · read from CFexpress

  laptop  C:\Travel\Images        1,247 written · 1,247 verified   OK
  SSD-A   Samsung T9   S5H9NS...    1,247 written · 1,247 verified   OK
  SSD-B   Samsung T9   S5H9NT...    1,247 written · 1,247 verified   OK
  SSD-C   SanDisk E61  2312A9...    1,247 written · 1,247 verified   OK

  Corroboration   1,246 matched · 1 mismatch
  Body            Canon EOS R5 · 012345000123 — as configured
  Timezone        1,247 files +00:00 — camera on UTC as intended
  Geotag          1,198 tagged · 49 outside track
  Eject           SSD-A ✓ · SSD-B ✓ · SSD-C ✓

  !  1 file deleted from all four copies — source mismatch
     1611Z_2087.CR3 → _runs\2026-08-03T18-22-04\quarantine\

  I/O          bytes        rate
    read        337 GB    1,113 MB/s
    written     225 GB      890 MB/s
    total       561 GB      1.7 GB/s        10.0× amplification

  Per destination        moved     sustained
    laptop  C:\          113 GB   1,842 MB/s
    SSD-A   Samsung      113 GB     731 MB/s
    SSD-B   Samsung      113 GB     724 MB/s
    SSD-C   SanDisk      113 GB     445 MB/s   ← set the pace

  Phase                                  wall     I/O
    1  pre-flight: camera card contents  0:02        —
    2  pre-flight: destinations and GPX  0:02        —
    3  ingest & verify                   4:12     505 GB
    4  corroborate                       3:31      57 GB   ⎫ overlapped 3's verify
    5  geotag                            0:12       0 GB   ⎭ pass, and each other
    total                                5:41     561 GB

  ►  [ SAFE TO STORE ]
```

> **The sample above is a layout sketch and does not render colour.** Every heading that is a
> *step* carries a badge in the real output — `Writing`, `Verifying`, `Corroborating`,
> `Geotagging`, `Travel SSDs`, `Cards`, `Safe to Unhook` — all pinned to one absolute column, and
> the verdict's headline is itself a badge. **Read this block for structure, never for the exact
> bytes**; the sections below the sample are what is kept current.

**Sizes are whole GiB and rates carry one decimal**, added 2026-08-06 at the operator's
request: *"at no point will I care about fractional GB"*, and for rates *"single digit after
the decimal with basic rounding. 1.65 GB/s should become 1.7."* Sizes round **up** and free
space rounds **down**, so a printed pair always straddles the truth outward — `human::gib_up`
and `human::gib_down` carry the argument, and `preflight`'s `NOT ENOUGH ROOM` is the line that
would otherwise print two equal numbers while refusing the run.

**Below 10 GiB the decimal comes back, and the threshold is a fact about the operator rather
than a compromise.** He shoots ~30 frames of each scene and comes home with 300–500 even
messing about locally ([`CONOPS.md`](CONOPS.md), shooting-day contract), so **a sub-10 GiB
payload is never a shooting day — it is a staged test slice.** On a real night he sees whole
gibibytes as asked; on the 50-frame development corpus a plain ceiling turned 2.6 into `3`, a
15 % overstatement that cannot be checked against the source by eye. The threshold gives each
case the rendering it needs without either borrowing the other's.

**This is a rendering rule and nothing more. Every stored, compared, asserted and serialized
figure stays at full precision** — manifests, checksums, the capacity check and the run log are
untouched, and a test asserting a rounded value MUST NOT be written against them.

> **A consequence to know before someone "fixes" it: rounded components need not sum to the
> rounded total.** Each figure is rounded independently from exact bytes, so a column can be
> off by a GiB or two against its own total. That is arithmetic, not a defect — reconciling
> them would mean rendering one figure from another rather than from the measurement, which is
> how a display bug becomes a wrong number.

### The opposite of green is never red

**Standing order, Terry, 2026-08-06. RFC 2119 sense: a status badge MUST NOT be red.** Clean is
green; anything else is yellow, meaning *this needs your attention*.

**The two badges, exactly:**

| | Glyph | Foreground | Background | Bold |
|---|---|---|---|---|
| Clean | `✓` | white | `SGR 42` green | **yes** |
| Needs attention | `!!!` | black | `#FFFF00` true colour | **no** |

Both are five cells wide — `!!!` is two characters wider than the tick, so the tick carries one
extra space each side.

**Black on yellow rather than white on yellow**, which is the higher-contrast pairing and the
reason road signs and hazard tape use it. The green badge keeps white, because white on green is
the strong pairing there.

**Each of those choices came out of looking at real output, and two of them contradicted the
reasoning that preceded them.**

**It MUST NOT be bold, and that is the non-obvious one.** `black().bold()` emits `ESC[1;30m`, and
this console renders bold black as *intense* black — **grey**. Terry: *"it's almost a gray and
getting washed out on the yellow."* **The attribute added to make it louder was the thing
silencing it.** The green badge keeps `bold` because bold white is *bright* white — same
attribute, opposite effect, entirely because of the base colour.

> **Why, since it looks like a bug:** bold once meant more beam current, so palettes map bold +
> colour *N* onto *N+8*, the bright half. Index 8 is "bright black", the palette's name for grey
> — so black is the one colour where brighter moves *toward* a light ground.

**The palette's yellow was the other half of the washout.** `SGR 43` is `#C19C00` in Windows
Terminal's default scheme — a dark mustard gold. **Two causes were dulling the same badge and
each was hiding the other**, which is why the first fix improved things without fixing them.

**Pure yellow beat the road-sign amber it was expected to lose to.** `#FFD500` was the reasoned
choice and lost a side-by-side instantly: *"for my eyes, pure yellow is the clear winner."*
Signage amber is chosen for how it holds up under sun and retroreflective sheeting, none of which
applies to an emissive panel two feet from a face. **And it is a true colour rather than a
palette index**, so it survives a theme change — a signal whose entire job is instant recognition
must not have to be re-learned.

**The clean badge stays on `SGR 42`, reviewed against three candidates on 2026-08-06 and
deliberately not changed.** **So the pair is half true-colour and half palette**, accepted rather
than overlooked: pinning the green would harden it against a theme change that has never
happened, at the cost of freezing a colour chosen by eye under one scheme.

> **The scheme is Campbell, verified rather than assumed.** Windows Terminal's `settings.json`
> sets no `colorScheme` anywhere, and leaves `intenseTextStyle` unset and therefore `bright` —
> the promotion that greyed the badge. That is what makes `SGR 43` `#C19C00`. **Re-check both on
> another machine**; every colour statement above is downstream of them.

**His reasoning, and it is about the moment the badge is read rather than about the fact it
reports:** *"let's be gentle with 11pm Terry and just flag it as 'hey this needs your attention,
don't freak out, we're gonna be fine, you shoot dual card for a reason, no data is lost, just
need some help'."*

**Red is not softer information, it is a different instruction.** Red says *something is
broken*; yellow says *come and look*. Almost everything this report can flag is the second: a
card that would not release, a frame outside the track, a destination that needs a second look.
**The data is on four verified copies before any of it prints** — decision 2 — so a red badge
would be reporting a crisis the run has already made impossible.

**And the cost of getting it wrong is asymmetric**, the same argument decision 12 makes for
warnings: a red badge at 11pm on night three buys adrenaline rather than a faster fix, and spent
often enough it teaches the operator to stop reading badges.

**The glyph is `!!!`, and `⚠` (U+26A0) was tried and rejected on 2026-08-06.** It renders here
with *emoji presentation* — an orange-filled triangle that supplies its own colours and ignores
the foreground set for it — so on yellow it came out orange-on-yellow and muddy. **Width was the
worry going in and was never the problem; legibility was**, and only a side-by-side on the real
terminal showed that. A screenshot from Claude Code's own renderer suggested double-width and was
wrong about that too.

**The ban is total, and the last exception was closed on 2026-08-06.** The per-destination badge
in the `LANDED` block — ` N UNVERIFIED ` — was red, and had an argument for it: it reports a
comparison made moments ago against every file on the destination, and a failure there is the
difference between `LANDED` and `NOT SAFE`. Terry's ruling, the same day: *"red is hard banned,
downgrade failed to verify as yellow."*

**That argument is worth recording because it is the one to expect again.** It says *this case is
serious enough to earn red* — and seriousness is not what the colour encodes. **The colour
encodes the action.** Red sends a tired operator hunting for damage; yellow sends him to read the
line underneath, which is the true instruction in every case this report can produce. The frames
are still on the cards, which were never written to (constraint 2), and what failed is the run's
own convergence rather than anything about the data.

**So there is no badge anywhere in this tool that renders red**, and a future session proposing
one for a sufficiently severe case is re-running an argument that has already been heard and
refused. Grep enforces it: `\.red\(\)|\.on_red\(\)` MUST return nothing.

### The badge column is a go/no-go on unplugging things

**Standing order, Terry, 2026-08-06, and it is the reason the badges exist at all.** In his
words:

> *"I want to train my brain that all green = unhook and put in the safe, any yellow anywhere =
> slow your roll, take a breath, don't touch anything, carefully read everything on screen to
> understand why it's not 100% green. To be clear I mean green on ALL sections."*

**So the badges are read as a single column and answer a single question**, and the whole set is
the unit — not any one badge. That has three consequences, all of them RFC 2119 MUSTs:

1. **Every section MUST carry a badge.** A section without one is indistinguishable from a
   section that is fine, and it breaks the scan — the operator's rule is *all* green, which he
   cannot evaluate against a gap. This is why `Eject` gained one on 2026-08-06.
2. **Badges MUST line up in one absolute column** whatever their heading's indent, because a
   ragged column cannot be scanned in one glance from across the room. `Geotagging` sits at
   phase level and the rest at subsection level; the hierarchy is carried by the *heading's*
   indent, and the badge is a separate signal that does not follow it.
3. **A badge MUST NOT be green unless its section is wholly clean.** A badge that could only
   come out green is the check that cannot fail — `REVIEWING.md`'s standing objection.

**Yellow is not a severity. It is a stop signal on a physical act.** This is the part that gets
mis-implemented, because "warning" reads as "something is wrong." Terry's case that settles it:
*"It could be as easy as I pointed it at the wrong GPX. And that's great. It does mean stop and
don't yank drives."* **A benign cause still gets yellow, and that is correct rather than a false
alarm** — the badge does not grade the news, it gates whether the next thing he does is unplug
five devices in a dark hotel room. Grading belongs in the words underneath, which he reads
*because* the badge stopped him.

**That is what makes `--no-eject` a deliberate yellow rather than an exemption.** The flag is used
constantly in development, and a run that used it has drives still mounted — so the one output
that would be actively dangerous is a green column. Terry: *"it stops my muscle memory from
yanking SSDs that are still mounted. You say NTFS can survive that. I do not want to TEST that
personally with those drives."*

**That rules out a "nothing to report" grey and an omitted badge for a skipped stage.** Both read
as *not yellow*, which under his rule reads as *go*. **A stage that did not run MUST be yellow**,
because the state of the rig after it is exactly what yellow exists to stop him acting on.

### The report's layout rules, settled 2026-08-06

**RFC 2119 keywords, and the capitals are load-bearing.** These came out of an evening of reading
real output on a real terminal, which is the only place several of them were visible at all —
**and they are written down because they were re-derived three times in one session.**

**Indentation carries the hierarchy, and nothing else does:**

| Level | Column | Example |
|---|---|---|
| Phase | 0 | `Pre-Flight Checks`, `Offloading`, `Geotagging`, `Eject` |
| Subsection | 4 | `Camera Cards`, `Travel SSDs`, `Corroborating`, the `LANDED` banner |
| Row | 8 | a destination line, an eject attempt |
| Detail about the row above | +4 | a veto reason, the gap explanation |

**A phase is either a *container* or a *step*, and only steps carry badges.** Settled 2026-08-06
after a version that hung the `Eject` section's badge on a closing line and left the operator
asking what it was for.

| | Carries a badge | Why |
|---|---|---|
| **Container** — `Pre-Flight Checks`, `Offloading`, `Eject` | no | it has steps under it, and each answers for itself |
| **Step** — `Writing`, `Verifying`, `Corroborating`, `Travel SSDs`, `Cards`, `Safe to Unhook` | yes | it is one thing that either went cleanly or did not |
| **Both at once** — `Geotagging` | yes | a phase with no subsections *is* its own step |

**`Eject`'s steps are `Progress Log`, `Travel SSDs`, `Cards` and `Safe to Unhook`.** The live
per-attempt lines are the `Progress Log`'s rows at column 8 — **not** loose content under the
phase heading, where they read as an unattached preamble.

**`Safe to Unhook` is last because it is the roll-up and the decision**, and green only when
every SSD *and* every card released.

> **`Eject` cannot carry the badge itself, and the reason is timing rather than taste.** Its
> heading prints before the stage runs, because `watch_attempt` starts writing the moment the
> first device is asked and a header arriving after its own rows would read backwards. Nor could
> it roll up the cards without waiting for a retry that can run for minutes, where reporting the
> SSDs the moment they are down was a deliberate fix. **Both constraints point the same way,
> which is usually the sign of a real boundary.**

**Blank lines above a heading MUST be:** two for a phase, one for a subsection, **none for a
status line.** A status line is content, not a heading, so `Corroborating` is followed
immediately by `50 matched`. `Pre-Flight Checks` is the **one deliberate exception** and its
print site says so — it has three subsections under it where the others have a line of result,
and a heading with sections under it wants air.

**`LANDED` and `Corroborating` nest under `Offloading`; `Geotagging` does not.** The operator's
model, and it is a better one than four peers in a row: those two are *what offloading
produced*, where geotagging is — his words — *"value add and not part of offloading"*. The
`LANDED` block closes with a rule of the banner's own width, so it reads as bounded rather than
as a heading trailing off into the next phase.

**Every line a human reads MUST start with a capital**, with three carve-outs: a line opening
with a count keeps its digit (`49 tagged`), an identifier or flag or path keeps its case
(`--no-eject`), and a status following a label is a table cell rather than a sentence
(`SanDisk    ejected; ready to disconnect`). A wrapped sentence continuation also stays
lowercase — capitalizing mid-sentence is worse than the inconsistency.

**Padding is for columns, never for prose.** `duration_aligned` pads to two characters each way
for the eject attempt block, where durations stack; `duration` does not, because a padded value
mid-sentence reads as a double space rather than as alignment. The same call the operator made
about the `LANDED` banner: *"leave landed alone, it looks better as is."*

**Durations are one shape everywhere** — `5m 0s`, `15m 12s`. Never zero-padded, because `00s`
reads as a clock and this is a measurement, and never dropping the minutes below sixty seconds,
because the same quantity in two formats is what the consistency is for.

Serials on every destination line so a glance confirms four genuinely distinct disks. The verdict
is the last line and that phrase appears nowhere else, so it cannot be confused with anything
above it. **The headline is a badge**, in the same two colours as the rest of the report and green
in exactly one case. Every row below was checked against `verdict()` on 2026-08-06.

> **This table is the authority; the run records further down are not.** Those quote what the
> tool printed on the night they were written — `EJECTED — SAFE TO STORE` and similar — and are
> deliberately left alone, because editing a record of what happened to match what happens now
> destroys the record. **A grep for a verdict phrase will therefore find older wording in dated
> narratives.** Trust this table.

| Condition | Headline | Colour | Rest of the line |
|---|---|---|---|
| Phase 3 verified everywhere, every device released | ` SAFE TO STORE ` | green | the claim |
| A volume dismounted but would not power down | ` UNPLUG FIRST ` | yellow | `UNPLUG SSD-B.` + the claim |
| An eject refused with the volume still mounted | ` STILL MOUNTED ` | yellow | `EJECT SSD-B BY HAND.` + the claim |
| Both, on different devices | ` STILL MOUNTED ` | yellow | `EJECT SSD-B BY HAND AND UNPLUG SSD-C.` + the claim |
| Run under `--no-eject`, everything else complete | ` STILL MOUNTED ` | yellow | `Nothing was ejected;` + the claim |
| Anything unverified anywhere | ` NOT SAFE ` | yellow | `See the unverified counts above.` |

**The claim** is one of two sentences, and which one appears is decision 22's rule that an eject
must not imply more than it proved: *every file from both cards is accounted for*, or *every file
from the one card present is accounted for — corroboration was waived*.

**Eject can modulate the safe verdict's wording; it can never turn SAFE into NOT SAFE.**

> **`SAFE TO STORE` in yellow is a real combination, and it is not the contradiction it looks
> like.** The **words** are decided by the archive SSDs alone — decision 14's *only phase 3 may
> change the verdict*, and decision 22 keeping the cards out of it. The **colour** follows
> `everything_released`, which counts the cards, because a badge is a *come and look* signal
> rather than a verdict. A night where all four archives released and a camera card would not
> therefore prints a yellow ` SAFE TO STORE `: the archives are safe to store, and something
> above wants a glance. **Do not "fix" this by letting a card rewrite the headline** — that is
> the boundary decision 22 exists to hold.

### Specified here and never built

**These four rows sat in the table above as though they were behavior, and none of them has ever
been printed.** Re-checked 2026-08-07: all four strings return nothing, while `SAFE TO STORE` and
`--allow-single-source` are found — so the grep works and the absence is real. They are kept
because they may still be wanted, and `Still to build` lists the same four.
[`WRITING.md`](WRITING.md) rule 5 carries the general lesson.

| Condition | Intended verdict |
|---|---|
| Phase 3 verified everywhere, corroboration incomplete | `SAFE, NOT EJECTED — ENSURE SDXC IS INSERTED AND RE-RUN` |
| Phase 3 clean, mismatches far above baseline | append `— BUT CHECK YOUR SDXC CARD (47 mismatches)` |
| Run under `--allow-single-source`, phase 3 verified everywhere | append `— SINGLE SOURCE, NEVER CORROBORATED` |
| Run under `--without`, phase 3 verified on the rest | append `— SSD-C EXCLUDED, SYNC IT ON RETURN` |

**The single-source case is partly covered already**, by the second form of the claim rather than
by a suffix — so building it means deciding whether the suffix adds anything the claim does not,
rather than starting from nothing.

Because writes are write-through and verify reads unbuffered, the rates are real device
throughput rather than page-cache artifacts — which is what makes the per-destination
line a usable diagnostic. The slowest destination sets the pace of the whole run, so
naming it is the single most useful number for spotting a disk going bad. With a
`_runs\` directory accumulating across a trip, a later comparison against a
destination's own rolling average is the natural extension.

`_runs\<timestamp>\report.json` carries the full forensic record: per-file outcomes,
per-phase timings, per-destination throughput, and the resolved hardware identities.

**`_runs` lives on the laptop's copy alone** — settled 2026-08-03, having been
written as a bare relative path in two places without ever saying under which root.
Decision 20 is what decides it: `verify` reads nothing but a destination's marker and its
manifests, so `_runs` is by construction not part of what makes an archive
self-describing, and putting it on the archives would add non-photograph directories to
disks whose whole promise is that they hold photographs and their proof. The laptop is
also the one destination that is never absent — `--without` names archives (decision 25),
never the machine's own disk — so resume has exactly one place to look for the run log
rather
than a quorum to reconcile. The quarantine of decision 3 goes to the same root: one copy
of the evidence, on the machine that will still be there when someone asks about it.

### 15. `--jobs` sizes the CPU pool, not the I/O fan-out

`--jobs N`, defaulting to logical CPU count, following RawGeotag's finding that this
problem class parallelizes well into double-digit thread counts.

**It governs the CPU-bound work**, where that finding applies directly. Phase 3 hashes 5N
— one source read plus four verify reads, 280 GB on a 56 GB day. At the measured
2,380 MB/s per core with SHA-NI (decision 17) that is ~118 s single-threaded against a
252 s phase, close enough to bind the run on faster storage. Spread across cores it
disappears. EXIF extraction and XMP generation ride the same pool.

**It does not govern I/O concurrency**, which is structural:

- one reader per card
- **one writer and one verifier per destination**, so the four devices stream alongside
  each other while each stays sequential within itself

RawGeotag's 12× came from a *latency*-bound workload — SMB round trips and container
seeks, where threads hide waiting. Phase 3 is bandwidth-bound sequential streaming, and
threads do not create bandwidth: a destination sustaining 445 MB/s still sustains
445 MB/s under thirty-two writers, minus whatever sequential locality they break.

**NTFS's single-directory serialization is a smaller factor here than it first appears.**
It applies to *metadata* operations — create, rename, delete — not to data writes; once a
handle is open, pushing 45 MB through it never touches the directory index. Temp-then-
rename costs two metadata operations per file, so ~2,500 serialized operations for a
1,247-file day, which is a couple of seconds against a 252-second phase. Under 1%.

RawGeotag measured that effect on 3 KB sidecars, where metadata *is* the work. Which means
it bites in **phase 5**, not phase 3 — thousands of tiny sidecars into one directory per
destination is precisely the case that will not scale with threads. Same structural answer
for both phases, for opposite reasons.

One buffer-fed blocking writer per device is therefore enough. The only idle gap is file
open/close between 45 MB writes — roughly 1 ms against ~100 ms of transfer — so overlapped
I/O stays unbuilt until measurement shows a device actually going idle.

This is settled by measurement rather than argument: the run report prints per-destination
sustained MB/s, so comparing `--jobs 4` against `--jobs 32` on the real hub is a two-run
experiment with the answer at the bottom of the output.

### 16. Flags carried over from RawGeotag

Four earned their keep there and come across, two of them changed by the new context.

**`--max-gap-seconds` (default 60) and `--max-gap-meters` (default 100).** A photo is
tagged only if the two track points bracketing its capture time are within *both* limits
and come from the same `<trkseg>`. Both are needed: endpoint separation does not bound
error, so a 140-second hole with 8 m between its ends is still untrustworthy and only the
time limit rejects it; a short hole with wide separation is genuine fast movement and only
the distance limit rejects that.

The parallel naming is deliberate — RawGeotag's `--max-gap` / `--max-distance` reads like
two unrelated knobs rather than two limits on one concept.

The 60-second default is deliberately harsh: against a 60-second logger it tagged 748 of
1,377 frames where `--max-gap 200` tagged 1,317. That is the intended behavior, not a
regression — no tag beats a wrong tag.

**`--dry-run` is stronger here than it was there.** Because output filenames derive from
EXIF capture time, and EXIF is cheap to read, a dry run can name every output file exactly
— target paths, collisions, what would be skipped as already present, the ETA — without
reading a file body or writing a byte. One honest limit: skip-versus-`_001` is decided
by hash (decision 5), which a dry run never computes — its already-present calls are by
name, and the real run re-decides them by content. A complete answer in seconds, which
makes it the natural thing to run before walking away.

**`--force` becomes `--force-xmp`.** In RawGeotag the bare name is unambiguous
because sidecars are all it writes. This tool writes raws *and* sidecars, so `--force`
reads most naturally as "overwrite my archive" — and that reading is the dangerous one. A
destructive flag has to be honest about what it destroys, which includes being
unambiguous about *what*. Semantics are otherwise unchanged, including composing with
`--dry-run` as a rehearsal that reports what would be overwritten while writing nothing.

The rule the flag is the exception to deserves stating outright: **an existing sidecar
is never rewritten without `--force-xmp` — by any code path.** Phase 5 tags what is
untagged and skips the rest (decision 13's convergence); a re-run writes a sidecar only
where none exists (decision 20). One invariant, one door through it.

Two consequences of scoping it that way:

- **No flag overwrites a raw file.** There is no case where it is correct — identical
  content is skipped by hash, different content takes a `_001` suffix. Better as a
  structural impossibility than as a flag nobody should reach for.
- **No flag bypasses pre-flight.** Running against fewer destinations is an explicit
  selection (`--without`, decision 25), not an override — it narrows what pre-flight
  asserts, never skips it. Pre-flight exists to fail while you are still at the desk.

Sidecars on the archives are only ever written by this tool, so forcing them is harmless.
**`--force-xmp` covers every destination, and the `=<DEST>` form narrows it to one.**

> **Reversed 2026-08-03 with decision 11.** This read: *"Sidecars on the laptop are
> written by Lightroom and hold develop settings that exist nowhere else. So
> `--force-xmp` covers archive destinations, and touching the working copy requires
> naming it."* Nothing writes develop settings to any copy this tool produces, so the
> asymmetry was protecting something that does not exist — and it protected it in the
> wrong direction, since re-tagging a day *should* reach all four copies or they stop
> matching. Defaulting to three of four would have quietly manufactured the drift
> decision 20 now reports.
>
> The `=<DEST>` form keeps its purpose, which was never about roles: overwriting one
> copy's sidecars when you want to check a re-tag before committing it everywhere.

### 17. Rust, with RawGeotag's engine as a workspace library

Two independent reasons, and they point the same way.

**The hashing is real compute.** Decision 15 sizes it: 5N through SHA-256, which on a
188 GB day is 940 GB — ~7 minutes single-threaded at the measured rate below, about
half of an 8–16 minute phase on its own. It has to spread across cores, which rules out
anything with a global interpreter lock and rewards a language with real threads and no
runtime overhead.

**The crate is `sha2`, and the acceleration is automatic.** The machine is not
hypothetical — this tool runs on the travel laptop's i7-13700H, whose P-cores and
E-cores all carry the SHA extensions — and RustCrypto's ubiquitous `sha2` selects its
SHA-NI backend at runtime through `cpufeatures`: no build flags, no `target-cpu`
pinning, no per-machine binary to get wrong before a trip. And it is one algorithm
everywhere — until the day it is not, which decision 28 routes additively rather than as
a break: the manifest's self-checksum (decision 12) is the same SHA-256, so a second
hash function never needs choosing, validating, or explaining.

**Measured on the rig, not assumed**, since that pairing is the whole argument — 2 GiB
streamed through each in 8 MiB chunks, single-threaded, release build, on the
i7-13700H. Median of four runs, against `sha2` 0.11.0, `sha3` 0.12.0 and `blake3` 1.8.5:

| Crate | Algorithm | Per core | Spread over four runs |
|---|---|---|---|
| `sha2` | SHA-256 (SHA-NI) | **2,380 MB/s** | 2,356–2,396 |
| `sha3` | SHA3-256 | 532 MB/s | 521–540 |
| `blake3` | BLAKE3 | 5,185 MB/s | 5,061–5,245 |

That is the number decision 15's arithmetic uses, and it is what settled the two
alternatives in *Considered and rejected*. Re-run it if the crates or the laptop ever
change; nothing here should be taken on the strength of having once been true. The
re-run is `cargo run --release --example hash-rate` — decision 29 added it, and
`--release` is not optional, since a debug build measures the optimizer rather than the
CPU.

**What this choice actually costs, measured end to end on 2026-08-04.** The same 3,883
frames offloaded to all four destinations three times, changing only the hash:

| Hash | Phase 3 | vs SHA-256 |
|---|---|---|
| SHA-256 | 20m 57s | — |
| BLAKE3 | 19m 05s | saves 1m 52s |
| XXH3 | 17m 58s | saves 2m 59s |

**XXH3 is the row that settles it, and it is not a contender** — *not* because a checksum
cannot catch a flipped bit, which it can: detecting corruption needs error detection, not
collision resistance against an adversary, and XXH3 would find a bad byte in a 45 MB raw as
reliably as SHA-256 does. It is not a contender because its digest is a number only this
tool's algorithm choice can interpret, where a SHA-256 is a lingua franca every operating
system ships a verifier for. **It appears here as the speed *bound*, not as an option** — the
fastest thing anyone could reasonably put in this slot, and therefore the ceiling on what any
hash choice could ever be worth. It buys three minutes on a
twenty-one minute run, so **no hash choice whatsoever can save more than ~14%**, and
BLAKE3's share of that is under two minutes. Decision 17 trades those two minutes for an
archive a stranger can verify in 2031 with `sha256sum` and no copy of this tool. That is
not a close call, and it is now a priced one rather than an asserted one.

**Settled by the operator on 2026-08-04, on seeing the price: two minutes is not worth
it. SHA-256 stays, and the question is closed** — reopening it needs a change to the
numbers above, not a preference. The `hash-experiments` feature and
`examples/verify-rate.rs` stay in the tree for the same reason decision 17 keeps
`hash-rate.rs`: so a future rig can re-price the trade rather than re-argue it.

Note how much smaller this is than the in-memory table above implies. BLAKE3 is 2.2× the
raw rate and 1.10× the run, because phase 3 spends most of its time on I/O that no hash
touches. **Beware quoting the memory-bandwidth figures as though they were run times.**

> ✔ **Built and measured 2026-08-05, and the prediction held.** `winio::unbuffered_sha256`
> now runs the read and the hash on separate threads, two 16 MiB buffers ping-ponging through
> a `sync_channel(1)`. Same SHA-256, same requests, same devices — **the difference is
> scheduling alone**:
>
> | Drive | Read ceiling | SHA-256 serialized | BLAKE3 serialized | **SHA-256 interleaved** |
> |---|---|---|---|---|
> | SanDisk | 1,455 | 910 | 1,127 | **1,134 — 1.25×** |
> | **WD** *(sets the verify pass)* | 957 | 691 | 798 | **828 — 1.20×** |
> | OWC | 3,284 | 1,366 | 1,933 | **1,670 — 1.22×** |
>
> **On both USB drives, interleaved SHA-256 beats *serialized BLAKE3*** — 1,134 against 1,127,
> and 828 against 798. So the lever recovers more than the hash swap would have, **while
> keeping the property decision 17 spent two minutes a run to buy.** That is the claim the
> paragraph below made when it was a prediction, now measured.
>
> **The floor moved, which is the only thing that matters.** The verify pass ends when the
> slowest destination finishes, and that is the WD: 691 → 828 MB/s, a **20 % gain on the
> binding constraint**. Unlike 2026-08-05's port rewiring, this one shortens LANDED.
>
> **And roughly a quarter is still on the table, with a named cause.** Perfect overlap would
> reach `min(read, hash)` — 2,380 MB/s on the OWC against the 1,670 observed. The gap is
> **per-file pipeline drain**: a ~52 MB raw is only about 3.3 chunks, so every file starts with
> an unoverlapped read and ends with an unoverlapped hash, and the pipeline never gets deep.
> Fixing it means pipelining *across* files rather than within one, which is a larger change
> and is not attempted here.
>
> *The original note, kept because its reasoning is what produced this:*
>
> **The measurement found a better lever than the hash, and it is in `winio.rs` rather
> than here.** `unbuffered_sha256` reads a chunk, hashes it, then reads the next —
> nothing is in flight during the hash — so a destination's verify rate is
> `1/(1/read + 1/hash)` rather than `min(read, hash)`. That is why SHA-256 costs 27–54%
> of each device's read ceiling. **Overlapping the read and the hash would recover more
> than switching to BLAKE3 does, while keeping SHA-256 entirely**: on the WD, 490 MB/s
> against a 747 ceiling, where BLAKE3 reaches only 582. Decision 15's claim that hashing
> "disappears" across cores is true *between* destinations and false *within* one
> destination's pass. Not yet built.

> **Re-measured 2026-08-03, when decision 29 pinned the crate versions.** The table
> previously read 2,252 / 529 / 5,023 MB/s and recorded no versions, which is the gap
> that made a re-run necessary rather than merely diligent. `sha3` and `blake3` reproduce
> within run-to-run noise; `sha2` is consistently ~6% higher, outside the spread, and the
> most likely cause is the version it now names. **Nothing downstream moves:** SHA3-256
> is still ~4.5× slower and BLAKE3 still ~2.2× faster, so both rejections stand on the
> same margins, and the 188 GB single-threaded figure above is ~7 minutes either way.

**Three of the hard sub-problems are already solved and validated.** Not "code exists" —
validated:

- CR3 EXIF extraction by seeking the container rather than reading 45 MB (~0.3 s for
  3,883 files), which is exactly what phase 3 needs
- GPX indexing and interpolation, with the gap/distance refusal logic
- XMP packets diffed against Lightroom Classic 15.4.1's own output, agreeing to
  0.02–0.12 m on CR3

Re-implementing that last one elsewhere means re-earning validation across thousands of
real files on two bodies. The test is not "was time already spent on it" but "does it
currently solve a hard problem correctly," and it does.

Genuinely new: the ingest and verification pipeline, and a Windows storage-identity layer
(volume GUID and disk serial enumeration, `FILE_FLAG_NO_BUFFERING` with its alignment
requirements, `SetThreadExecutionState`). Neither exists in any language today.

**The honest cost:** the phase 3 fan-out is the one part Go would make easier — a reader
feeding a channel with one writer goroutine per destination is a page of obvious code,
where Rust needs a bounded pool of buffers whose lifetimes span four concurrent consumers.
That is real work, and it is the trade being accepted.

### 18. Fatal out, and test lightly

**Almost every error is fatal.** Print why, exit non-zero, stop. There is no recovery
machinery for unlikely hardware events — a destination unplugged mid-run, a disk filling
unexpectedly — because handling them gracefully would cost more complexity than the
scenarios are worth. The one exception is a defect in an input file itself, where fatal
would not fail safe — see decision 21, whose table collects every per-file path.

**That is affordable only because the robustness is structural rather than in error
handling.** Four decisions already made carry it:

- temp-then-rename means a killed process cannot leave a partial file under a real name
- the append-only run log means the record survives whatever killed the process
- automatic resume (decision 13) means recovery is "run it again"
- pre-flight (decision 9) means predictable failures happen while you are still at the
  desk, not two minutes after you leave

So an SSD yanked mid-run needs no handler: the process dies, the archive stays consistent,
and a re-run finishes the job. What must hold on *any* fatal exit is only what the
structure already guarantees — no partial file under a real name, a readable log, and a
stated reason.

Exit codes, kept deliberately coarse:

| Code | Meaning |
|---|---|
| 0 | Phase 3 verified everywhere, no source mismatches |
| 1 | Fatal — the run did not complete; reason printed |
| 2 | Completed, but something wants your attention; the report names it — a mismatch, a deletion, a stray or unfiled file, a refused eject, an eject held for unfinished corroboration (decision 22), a run missing a source (decision 7), a destination (decision 25), or its tracks (decision 26), or one that closed out a predecessor's corroboration as forfeited (decision 13) |

**Testing is four things**, and stops there:

1. **The phase 4 deletion path.** The only code path in the tool that destroys data, so a
   bug there deletes photographs. Everything else fails safe — worst case is a re-run.
   Cheap to exercise: flip one byte in a fixture and confirm the file is tombstoned and
   quarantined rather than silently dropped or wrongly kept.
2. **The naming function.** Pure, trivially testable, and it decides where irreplaceable
   files land: UTC date foldering, the `HHMMZ` prefix, and skip-on-identical-hash.
3. **One end-to-end happy path** against real CR3 fixtures — two synthetic cards, four
   temporary destinations, four identical trees out.
4. **`verify` against a committed schema-1 manifest fixture.** The one test aimed at a
   defect that surfaces years from now: a reader that quietly stops understanding an old
   archive (decision 28). Same criterion as the first two — the damage is irreversible,
   because the disk in the safe cannot be regenerated.

Everything else is deliberately untested. This is a personal tool with one user, one rig
and a recoverable failure mode; the RawGeotag testing standard would be a poor trade here.

### 19. There is no sampled verification anywhere

Every bit is checked, on every run and on every `verify`. These are the most emotionally
valuable files this archive holds, and the run happens unattended — the entire point is
that the result needs no interpretation. A full re-verify of a multi-terabyte archive takes
on the order of an hour, an acceptable price for a check performed occasionally and trusted
completely.

Transitivity is what makes the guarantee whole. Phase 3 proves every destination equals
the source card's hash; phase 4 proves the other card's copy equals that same hash. Both
holding means every destination is proven identical to **both** cards — and decision 27
asserted at launch that the two cards hold the same files, so in a gated run that is
every file. A file proven against one card alone is exactly what its manifest verdict
records: `waived` (decision 7) or `forfeited` (decision 13). Note the timing: the full
two-source property completes at the end of phase 4, not at LANDED, where all four are
proven equal to the source card alone.

### 20. `verify` is standalone and config-free

Both take a destination *path* rather than a config label. For `verify` the reason is
absolute: an archive pulled from the safe has to be checkable on a machine that has
never seen this tool's configuration, and it is — `verify` reads nothing but the
destination itself, its marker and its manifests.

**`offload verify <DEST>`** reads the destination marker to name what it is checking —
at whatever schema that disk was written with, which every later build still understands
(decision 28) —
then walks every date folder, `_unfiled` included (decision 21): manifest checksum first,
so a rotted manifest is reported as a rotted manifest rather than as damaged
photographs; then every raw re-hashed unbuffered
against it. Tombstones are honored, so a file deliberately deleted in phase 4 reports clean
rather than missing.

> ⚠ **DEFECT, found 2026-08-06: `verify` reports `CLEAN` on a disk that holds nothing.**
> Run against `I:\Travel\Images` after the drive was knocked off a desk, it printed:
>
> ```
> 0 files verified across 0 folders
> ►  CLEAN — every recorded file is present and matches
> ```
>
> The archive tree had been cleared after an earlier run; only the destination marker
> remained. `Report::clean()` is `damaged == 0 && missing == 0 && unreadable_manifests
> .is_empty()`, and **all three are vacuously true when there are no manifests at all.**
>
> **This is the shape [`REVIEWING.md`](REVIEWING.md) collects under *A diagnostic that cannot
> fail*, and it is the worst instance yet because it is in the product rather than in a
> script.** The five recorded there are probes written in a hurry; this is the command whose
> entire purpose is to answer *is this disk still good*, and it answers **the reassuring thing**
> when it cannot answer at all. The scenario decision 20 exists for is a disk pulled from a safe
> years later — where "CLEAN" on a silently empty disk is precisely the wrong answer, and there
> is no second chance to notice.
>
> **The fix is a fourth outcome, not a tweak to `clean()`.** Today the verdict has three:
> `CLEAN`, `NOT CLEAN`, and `CANNOT FULLY VERIFY` for an unreadable manifest. **A disk with no
> manifests is a fourth state** — *nothing here claims to be an archive* — and it must not be
> spelled the same way as a verified one. `REVIEWING.md`'s own rule names this exactly: **an
> empty result must never be spelled the same way as a negative result.**
>
> Also worth carrying: **the check that prompted this proved nothing.** It was run to see
> whether a dropped drive had lost data, on a disk that turned out to hold none — the tool was
> asked a question it had no material to answer, and said `CLEAN`. *Confirm the check has
> something to check before quoting its verdict.*
>
> ### ✔ Fixed 2026-08-06 in `8118a7b` — and the type is what does the work
>
> **`Report::clean() -> bool` is gone**, replaced by `Report::verdict() -> Verdict` with the
> four states this note called for:
>
> | Verdict | What the last line says | Exit |
> |---|---|---|
> | `Clean` | `CLEAN — every recorded file is present and matches` | 0 |
> | **`NothingToVerify`** | **`NOTHING TO VERIFY — no manifest found under <root>. Either this is not an archive root, or this disk has been cleared. Nothing was checked, and nothing here says the photographs are fine.`** | **2** |
> | `Incomplete` | `CANNOT FULLY VERIFY — a manifest could not be read...` | 2 |
> | `Damaged` | `NOT CLEAN — N damaged, N missing` | 2 |
>
> **An enum rather than a fourth `if`, and that is the point rather than the styling.** A `bool`
> leaves every caller free to keep asking the old question; a non-exhaustive `match` is a
> compile error. That is binding constraint 5 — prefer a mistake the compiler catches over one
> that surprises at runtime — spent on the one command whose wrong answer has no backstop
> anywhere.
>
> **The wording names both causes**, because the operator cannot distinguish them from where he
> is standing: a disk cleared since its last run and a path that was never an archive root
> produce the identical empty walk.
>
> **`NothingToVerify` exits 2, not 0.** The command ran exactly as designed, so this is not a
> failure — and it is emphatically not a pass, so a script keying on the status must not be able
> to read it as one (decision 18).
>
> **Four tests came with it, and `folders.is_empty()` rather than `checked() == 0` is the
> discriminator they exist to pin.** `a_folder_of_tombstones_is_clean_rather_than_empty` is the
> one that keeps the fix honest: phase 4 deleting every frame of a day leaves manifests that
> verify perfectly and check **zero** files, and that disk is *clean* rather than absent
> (decision 12's tombstones). An implementation keyed on files-checked would pass the empty-disk
> test and quietly break that one. **Mutation-checked** per
> [`REVIEWING.md`](REVIEWING.md): restoring the old three-`== 0` body returns `Clean` on the
> empty root and fails that test by name, and **nothing else in the suite notices** — which is
> precisely the measure of how invisible this defect was.

**Sidecar drift should not exist on any destination**, and after decision 11's correction
`verify` says so about all four rather than excusing one. Nothing edits these copies, so
a sidecar that differs between them means either a phase 5 re-run that did not reach every
copy — backfillable, and worth knowing about — or corruption. Neither is something to
pass over silently.

> **`offload sync <DEST>` was designed here and deleted on 2026-08-06 without being built.**
> It would have backfilled a destination that missed a night, sourcing from the laptop copy
> because the cards are formatted by the time the disk returns.
>
> **Terry's actual procedure makes the problem it solved impossible:** *"if a drive failed mid
> trip, I'd remove it from APPDATA config and finish out. It's why I bring four, because the
> mantra in photography is 'if you don't have three copies, you have none.' This way we are at
> N+1 and can still have that three with a failure."* **Four minus one is three, and three is the
> number that counts as backed up** — so there is no hole to backfill, and nothing to reconcile
> on the flight home.
>
> **The rig's specification absorbed the failure mode the feature existed for**, which is the
> better answer: no flag, no per-night decision, no machinery. [`CONOPS.md`](CONOPS.md) carries
> the procedure. **The subcommand was removed from the CLI the same day** — it had been
> advertised in `--help` for weeks while being a stub that printed an error and exited.

### 21. A file whose EXIF cannot be read lands in `_unfiled`, not on the floor

The one narrow exception to decision 18's fatal-out rule, added at design review, because
this is the case where fatal does not fail safe. A corrupt EXIF header on one file — a
camera write glitch, one-in-ten-thousand territory — would otherwise kill the run two
minutes after you left for dinner, costing the night's backup of every other photo. And
pre-flight cannot warn: enumeration is metadata-only, so EXIF is not read until the file
streams through phase 3.

Such a file is still hashed, still written to all four destinations, still verified —
everything phase 3 does — but under `_unfiled\<run-id>\<original-name>` instead of a date
folder. Outside the `YYYY\` tree, so Lightroom never sees it; the per-run subfolder makes
name collisions impossible without collision logic. `_unfiled` carries a manifest like
any date folder, so `verify` covers it (decision 20). The report
calls it out loudly and the exit code is 2, but the verdict stays SAFE — every bit on
the card is verified in four places, which is what SAFE means. Phase 4 corroborates
these files like any other, and a mismatch there follows decision 3.

The contrast with the mismatch path is deliberate: a two-card hash mismatch means the
bits themselves are untrustworthy and no copy can be proven right, so the photo is
called a loss. An unreadable EXIF means the bits are fine — both cards agree — and only
the *placement* is unknowable, so the photo is kept. **Discard what cannot be trusted;
keep what merely cannot be named.**

The complete per-file defect set, in one place:

| Defect | Run | The file |
|---|---|---|
| two-card hash mismatch | continues | deleted everywhere, tombstoned, quarantined in `_runs` (decision 3) |
| EXIF unreadable | continues | written and verified to `_unfiled`, reported (this decision) |
| not a CR3 — a contract violation | continues | left on the card, named in the report, exit 2 (decision 24) |
| a diverged pair | **fatal, at pre-flight** | untouched — refused before any byte moves (decision 27) |
| anything environmental | **fatal** | — (decision 18) |

**No per-file defect stops a run the gate admitted; only the environment can.** The
99.9% case always finishes.

### 22. The run ends by safely ejecting the archive SSDs

Added at design review, from the field: the nightly ritual used to end with three trips
to the tray icon, ejecting each SSD by hand — usually twice per device, out of fear that
bits were still sitting in a cache. By the time this tool ejects, that fear is
structurally dead: writes were write-through and every byte was read back off the media
(decision 2), so ejection *confirms* persistence rather than providing it.

When the full run completes — not at LANDED, since phases 4 and 5 still write to the
archives — each destination resolved by disk serial is ejected: flush the volume, lock it
(`FSCTL_LOCK_VOLUME`, retried with backoff for ~30 s, since Defender or the indexer may
be holding freshly written files), dismount, then `CM_Request_Device_Eject` so Windows
powers the device down exactly as the tray icon would. The destination that is a path on
this machine's own disk has nothing to eject and is never touched — that is the whole of
the distinction, and it is physical rather than a role (decision 11).

**The two camera cards are dismounted as well**, added 2026-08-04 at the operator's request,
so the ritual ends with all five removable devices settled rather than three. The reason is
not safety — the tool never wrote to a card, so pulling one was always safe at any time after
the run — it is that **an asymmetry the operator has to remember is a cost paid at the end of
a long day**, and this design spends real effort elsewhere to remove exactly that kind of
decision (decisions 6 and 8).

**All five are released concurrently and reported in two parts, added 2026-08-06 at the
operator's direction:** *"SSD are like two orders of magnitude more important. Let's start
shutting them all down at the same time but print how long until SSD were fully put to bed as
soon as all three are down, then follow up with either how long cards took to be put to bed OR
declare failure at 90 minutes."*

**Concurrent for correctness rather than speed** — nobody is waiting on eject, but the devices
share one deadline, so run in sequence a drive that retried to the end of the budget would leave
the others a single attempt each. **Two-part because the answer that matters exists long before
the run ends**: the SSDs are typically down in seconds while a card can take eleven minutes, and
one shared closing line withheld the important number for the duration of the unimportant one.
It also reported `Released 5 devices in 22m 16s` on a night when four were released — the same
shape as the card-dismount overstatement this decision already fixed once.

**The card half MUST NOT reach the verdict or the exit code**, and `exit_code` is not given the
card results at all, so it cannot begin to. **Both failure branches are covered by unit tests**
rather than waiting for a device to refuse: `report_ssd_release` and `report_card_release` write
to an `impl Write`, so the stuck-SSD line and the 90-minute give-up are asserted directly. The
clean path is the only one a real run has ever printed.

**Cards take the same three steps as an archive SSD — lock, dismount, power down.** A correction
made 2026-08-05, replacing a rule that gave them lock and dismount only on the reasoning that a
card is pulled from a reader which stays put. **That rule's every clause turned out false or
irrelevant, and a dismount releases nothing** — both cards sat in the tray after every run that
claimed to have settled them.

> **The refutation, the trace and why `IOCTL_STORAGE_EJECT_MEDIA` was predicted to work and does
> not are in [`EJECT-SERIES.md`](EJECT-SERIES.md).** The decision is here; the working out is
> there.

**A card that will not dismount changes nothing**, and the report says so in those words
rather than reporting a failure. It was safe to pull before the attempt and it is safe to
pull after. **Neither the verdict nor the exit code considers the result** — letting it
downgrade either would claim this bought a guarantee it did not.

**One nuance against the never-write-to-a-card non-goal, recorded rather than glossed.** A
dismount writes no photograph and touches no file, but it lets the filesystem driver complete
a clean unmount, which can update volume metadata such as the dirty bit. Against a non-goal
that says "not a byte, under any flag," that deserves stating outright. It is accepted
because it improves on the status quo rather than eroding it: today the card is yanked while
mounted and left dirty, formatting remains an in-camera act, and no photograph is touched.
**The operator was told before it was written and accepted the trade.**

A refused eject — something else holds the volume — is named per device and downgrades
nothing, because the data guarantees were settled before eject was attempted. See
decision 14 for how the verdict phrases it.

**This is the operator's stated number one technical risk on the project**, recorded
2026-08-04 in his terms: eject failing to *just work* needs as much attention as it takes,
as fast as possible. The reason it outweighs its size is where it sits — last. A run can
land 201 GB, verify 15,532 pairs and corroborate every frame against a second card, and if
the final step leaves a chore then that is the night's closing impression. *"So close but so
far away"* is how he put it, *"a fresh wound every time."* The feature stays only while it
keeps improving; the alternative he has named is removing it outright, which would cost the
flush and dismount along with the cosmetic power-down.

**The whole sequence is retried with exponential backoff, not just the lock.** Originally
only `FSCTL_LOCK_VOLUME` was retried and `CM_Request_Device_Eject` got a single attempt —
and on 2026-08-04 that single attempt was vetoed on two of three archive SSDs, with
`PNP_VETO_TYPE(6)` naming **the volume itself** rather than any application. The mechanism
is a race the design creates: the lock lives on the handle, the handle must be closed before
the eject or the process is itself the outstanding open, and closing it lets Windows remount
the volume. Retrying only the final call would then ask the same question of a volume that
has since remounted, so the lock and dismount are redone with it. The operator had already
found this empirically — the pre-tool ritual was pressing the tray icon *twice*.

**The retry runs until ninety minutes after launch, and that is deliberate rather than
generous.** See *Both metrics are thresholds* — the budget is the dinner window, and
**whatever of it the run does not need is time nobody is waiting through.** Spending it asking
Windows again costs exactly nothing. One attempt always happens even if the budget is already
spent, since refusing to try at all would turn a slow night into a manual one for no gain.

> **It was sixty minutes until 2026-08-06, and the operator raised it.** `CONOPS.md` puts
> dinner at 60–90 and the constant took the *bottom* of that range, so that the program would
> always have exited before he returned. His words retiring that: *"this app is run when I'm
> away for dinner. Let's push the eject timeframe to 90 mins. If I do get back before it's
> done ejecting, I will happily wait."*
>
> **The premise the sixty rested on was one only he could supply, and it was wrong.** Coming
> back to a run still arguing with Windows was treated as the thing to avoid; the thing to
> avoid is **a drive left in the tray**. Waiting a few minutes at the desk is cheaper than a
> chore, and he is the authority on which of those he minds more.
>
> **And the 60–90 was never a measurement in the first place**, which is the more general
> lesson. His words: *"the hour runtime has slop in it, that's a very fuzzy number."* The
> constant took the lower bound of a fuzzy estimate and treated it as a hard ceiling —
> **false precision applied to an approximation**, and the cost landed on the one stage with
> the least margin to give. When a constant is derived from a soft number, the soft number's
> *width* is part of the input; picking its safest edge is a decision that needs stating, not
> a free default.
>
> **This is also the note directly below being overruled by evidence rather than by taste.**
> It said *"widening the budget past an hour is not the answer, since the hour is the actual
> constraint"* — sound, and resting on a constraint that turned out not to exist. It stays
> where it is, uncorrected in place, because the alternative it proposed (starting eject early
> for destinations nothing is waiting on) is still the better lever if ninety ever proves
> tight; it is simply no longer the *only* lever.

> **How much retry that actually leaves is a function of the day, and the biggest days give
> the least.** Corrected 2026-08-04 when the day-size distribution was measured; this
> previously said a run "reaches LANDED in about a quarter of it and finishes in about half,"
> which is true at 201 GB and not at the 95th percentile.
>
> | | 201 GB day | 415 GB day — the largest on record |
> |---|---|---|
> | LANDED | ~17 min | ~30 min |
> | Corroboration ends | ~29 min | **~52 min** |
> | Retry window left | ~25 min | **~5 min** |
>
> **So eject gets a fifth of the budget on precisely the nights the drives are busiest** —
> the most freshly written data, the most scanner activity, the most likely veto. The
> constant is not wrong, but the reasoning behind it only held for an average day, and the
> consequence lands on the one part of the run the operator calls his number one risk.
> Widening the budget past an hour is not the answer, since the hour is the actual
> constraint; **if this ever bites, the lever is starting eject earlier for destinations
> nothing is waiting on, not waiting longer.**
>
> **Re-derived 2026-08-06 against the current baseline, because that table predates the
> interleaved verify and the conclusion moved.** Scaled from the 2026-08-05 run — LANDED
> 10 m 55 s, whole run 27 m 06 s, so ~15.5 min of corroboration — by the real ratio
> 415.1 / 201.3 = 2.06×:
>
> | 415 GB day | corroborating on the **222 MB/s** card | on the **246 MB/s** card |
> |---|---|---|
> | LANDED | ~22 m 30 s | ~22 m 30 s |
> | Corroboration | ~32 min | **~29 min** |
> | Total before eject | ~55 min | **~52 min** |
> | Retry window left, **60 min budget** | ~5 min | ~8 min |
> | **Retry window left, 90 min budget** | **~35 min** | **~38 min** |
>
> **The last row is why the budget moved to ninety the same day this table was written.** At
> sixty, the worst night of the year gave eject a fifth of what an ordinary night gives it —
> and the card-choice lever above, worth 3 minutes, was the largest one available. Ninety
> makes that lever irrelevant, which is the right outcome: **the fastest fix for a squeezed
> retry window was never to squeeze the run.**
>
> **Two things this changes.** The interleave bought LANDED back — 22 m rather than 30 — but
> it bought eject *nothing*, because corroboration grew to fill it: the SD read is now most of
> the run. And **which SD card corroborates is a lever on the eject budget**, worth 3 minutes
> of retry at the 415 GB extreme. That is not a reason to choose a card, but it is a reason to
> know the fastest one is doing that job on the biggest nights.
>
> **All six numbers here are arithmetic, not measurement.** No run at this size has happened.
> They are recorded so the prediction is on the record *before* the run rather than fitted to
> it afterwards — this project has a standing problem with plausible stories arriving after
> the fact.
>
> ### ✗ Measured 2026-08-06, and the table above was wrong in the direction that matters
>
> **The run happened. Every row predicting the clock was optimistic**, and the mechanism this
> table rests on — scale the last run by the ratio of the data — does not hold at this size:
>
> | 415 GB day | Predicted (246 MB/s card) | **Measured** |
> |---|---|---|
> | LANDED | ~22 m 30 s | **35 m 29 s** |
> | Corroboration | ~29 min | ~31.5 min ✔ |
> | Total before eject | ~52 min | **~67.7 min** |
> | Retry window left, 90 min budget | ~38 min | **22 m 16 s, all of it consumed** |
>
> **Corroboration was predicted well and LANDED was not**, which is the useful half. Phase 4 is
> a pure sequential read and scaled linearly; phase 3 scaled **3.25× against 2.06× of data**.
> [`RUNS.md`](RUNS.md)'s *415 GB run* carries the candidates, none of them adopted.
>
> **So the scaling assumption is the thing to retire, not the arithmetic.** Anyone re-deriving
> this table for a future size MUST NOT scale LANDED linearly from a smaller run — the write
> path does not behave that way, and this is now the second time a number in this project has
> been produced by extrapolating a measurement instead of taking one.
>
> **And the retry window did not buy what this section assumed it would.** It was spent
> entirely on a CFexpress that never released, while all three SSDs — the devices the budget
> exists for — succeeded on their first attempt. See [`RUNS.md`](RUNS.md)'s *415 GB run* for the
> withdrawn claim that this run justified the 60 → 90 change.

**The devices are ejected concurrently**, which is not about speed — nothing waits on eject.
It is because they share one deadline: done in sequence, a drive that retried to the end of
the budget would leave the others a single attempt each, and whatever holds one freshly
written volume is usually holding all of them.

**Two failures, two instructions**, and collapsing them is what made a successful run read as
a failure. A volume still *mounted* needs the tray icon. A volume that dismounted but would
not power down is flushed and detached — pulling it out is the whole of what remains, and
decision 14's verdict now says so rather than sending the operator to the tray to repeat work
already done. **Fixing that wording removed a real part of the pain without changing any Win32
behavior at all**, which is worth remembering the next time eject is reported as broken: ask
whether the complaint is about behavior or about description.

**Each device reports what its eject cost** — attempts made and wall clock spent — printed
only when it took more than one attempt. Decision 22 can only be tuned from real numbers, and
a run is the only place they occur. **Those numbers accumulate in
[`EJECT-SERIES.md`](EJECT-SERIES.md)**, along with the run that first proved the retry works.

**Eject is also the certainty gate — deliberate, settled at design review.** It fires
only when nothing remains for the current cards: every file verified on all four
destinations, phase 4 run to completion against the SDXC card with every mismatch
resolved (decisions 4 and 3), phase 5 run to completion — a file outside the track is
a settled outcome, not a hold, and a `--no-gpx` run has no tagging work to hold for
(decision 26). "Complete" is the bar, not "all matched": a mismatch resolved
by deletion-and-tombstone is settled, and so is every non-matched verdict — `waived`,
`forfeited` — whose distinctions decision 12 carries. Only files the current
cards could still answer for hold the gate. If corroboration could not finish — a lone
CFexpress landed the raws without the SDXC ever being seen (decision 7's remainder
resume), or phase 4 was interrupted — the SSDs stay mounted and the report says exactly
what to do: ensure the SDXC card is inserted and re-run, or eject by hand. Re-runs
converge (decision 13), so the normal recovery is plugging in what was missing and
running again; the tool corroborates the remainder and ejects the moment certainty
arrives.

**An SSD this tool has ejected is therefore a physical claim: every file from both cards
is accounted for on that disk** — literal in a two-card run, because pre-flight proved
the pair holds one set of files (decision 27) and phase 4 then verified every one of
them (decision 4). A declared single-source night ejects on the narrower claim its one
card can support, and the verdict says so in words rather than letting the same eject
imply more than it proved (decisions 7 and 14). The tray icon can never say either.

`--no-eject` disables it for the rare night the SSDs should stay mounted; the verdict
names the withheld eject rather than letting silence look like a refusal (decision 14).

### 23. Timezone: derived per photo, intended UTC, deviation flagged

Settled at design review, from three operator facts: the only body is an R5, the R5
records `OffsetTimeOriginal` on every frame, and the standing intent is to run every
camera on UTC.

- **The camera setting may be any timezone; storage is always true UTC.** Capture time
  is derived per photo — EXIF wall time minus the recorded offset — and the
  `YYYY\YYYY-MM-DD` foldering uses nothing else. The recurring real case: the camera
  accidentally set to London time with DST on, so a summer shoot arrives stamped
  `+01:00` BST. The offset is recorded, the arithmetic self-corrects, and every file
  still lands in its true UTC date folder. No timezone *setting* can misfile a photo.
- **Deviation from the UTC intent is flagged, as information.** Storage needed no help —
  the flag is about the camera, not the data: the report counts files by recorded
  offset, and anything not `+00:00` gets its own line, because the menu is not set as
  intended and tonight is the night to fix it.
- **A readable EXIF with no offset goes to `_unfiled`** (decision 21). The R5 always
  writes one, so its absence means a malformed file, not a policy question. There is
  deliberately no `--utc-offset` flag — RawGeotag needed one for a body that recorded
  no zone, and that body is gone.

**What none of this can catch: a wall clock that is wrong as an absolute instant.** A
camera set to UTC whose clock reads two hours off derives a wrong UTC from honest
arithmetic — wrong date folders, shifted geotags, no error anywhere, because the
metadata itself is lying. Two defenses, both partial: the operator habit in
`CONOPS.md`'s shooting-day contract, and one heuristic in the report — when phase 5's
misses are *systematic*, photos falling outside the track by a near-constant offset,
the report says so in words rather than printing a bare count:

```
Geotag   0 tagged · 1,247 outside track — misses look systematic (~+2:00): check the camera clock
```

A scattered miss pattern is a logging gap; a uniform one is a clock. The distinction is
computable, and it is the difference between finding out tonight and finding out on
Lightroom's map three weeks later.

### 24. The tool ingests CR3 raw stills, and nothing else

Scope, settled with the operator: only CR3 raw stills are ever shot (`CONOPS.md`, the
shooting-day contract) — the R5 can produce JPG, HEIF and video, and none of it is
used. So the walk sees every file — that is how a stray gets named — while ingest takes
`*.CR3` only, on both cards, and every guarantee in this design is a claim about raw
stills: LANDED, corroboration, and decision 22's eject claim all read "every file" as
"every CR3."

A non-CR3 file on a card is therefore a contract violation, and the answer is the
report, not the pipeline. The file is named — on the card, not backed up by this tool —
and the exit code is 2; it is not ingested, not `_unfiled`, and does not hold the eject
gate. The tool never writes to cards (non-goals), so the file sits untouched until the
next in-camera format, and what happens to it before then is the operator's call — made
tonight, because the report said it in words, rather than discovered after the format
has already eaten it.

`_unfiled` deliberately does not catch these. It exists for a CR3 whose EXIF cannot be
read (decision 21) — a defective instance of the thing this tool backs up — not as a
junk drawer for formats the operator never shoots. A backup tool that quietly hoovers
up whatever it finds would be building machinery for a contract violation instead of
naming it.

> ⚠ **The check is unbuilt, and when it is built it MUST NOT report the camera's own
> housekeeping.** Found 2026-08-06 on a live card: `DCIM\CANONMSC\M3100.CTG`, 62 KB, written
> by the R5 itself. Canon puts a management catalog there on every card it shoots to — the
> file is not a photograph, was never going to be backed up, and needs no decision from
> anybody.
>
> **A stray check that fires on it fires on every run forever**, which is the failure this
> project keeps naming: a warning that always appears is a warning that stops being read, and
> it would be sitting directly beside the ones that matter. The rule to implement is
> **"an image file that is not a CR3"** — a JPG, HEIF or video the operator did not intend to
> shoot — rather than **"any file that is not a CR3."**
>
> The wording above is already right about the *intent* — it names JPG, HEIF and video — but
> the sentence a future implementer will read is *"a non-CR3 file on a card is a contract
> violation,"* and that is the one that would ship the false alarm. Known camera housekeeping
> is out of scope: `DCIM\CANONMSC\`, and `MISC\` at the volume root, both of which the format
> itself creates.

### 25. A destination missing at offload is declared, not configured around

The destination mirror of decision 7, and it closes a real gap: decision 9 refuses to
run unless all four destinations are present, and nothing said how an offload could
legitimately happen without one. As previously written, an SSD that died mid-trip would
have blocked every remaining night.

> **Two cases, and 2026-08-06 established that they take different answers.** The rejected
> alternative below — editing the config — was rejected for a drive that is *temporarily*
> absent, and it is the right call there: the config describes the rig, so removing a disk that
> is coming back makes it lie and owes a second edit.
>
> **A drive that has *died* is not that case.** The rig genuinely changed, one edit is the whole
> of it, and nothing is owed on return. Terry's procedure: *"if a drive failed mid trip, I'd
> remove it from APPDATA config and finish out. It's why I bring four... this way we are at N+1
> and can still have that three with a failure."*
>
> **So `--without` is for tonight and a config edit is for the rest of the trip**, and neither
> is a workaround for the other.

The default stays refusal, in the first ten seconds, naming the fix:

```
DESTINATION MISSING — SSD-C (SanDisk E61 2312A9...) not connected.
Plug it in, or re-run with --without SSD-C and re-run the night when it returns.
If the drive is dead, remove it from config.json and finish the trip on three.
```

Proceeding requires naming what is missing: **`--without <LABEL>`**, taking a config
label, repeatable, archive destinations only. Each absent disk is declared per run, so
a three-copy night can never happen by accident and the cost of the night is visible in
the command itself. The flag narrows what pre-flight asserts — it never weakens an
assertion on what remains, which is validated exactly as decision 9 demands.

Under the flag, every "all four destinations" in this design reads as "every
destination this run was asked to cover." LANDED and the eject gate are scoped the same
way; the verdict carries the scar (decision 14) and the exit code is 2 (decision 18).

The destination on this machine's own disk cannot be excluded: it is not a device that
can be left behind, so it cannot be missing.

**Recovery is the next run, and only while the cards still hold the session.** Re-run the whole
night once the disk is back and decision 13's resume writes exactly what it missed. **After the
cards are formatted there is no recovery path at all**, and that is accepted rather than
overlooked — `offload sync` was designed for precisely this and withdrawn on 2026-08-06.

> **What that costs, stated plainly: an archive that is permanently asymmetric.** A night run
> under `--without` on a disk that returns after the next format lives on three copies rather
> than four, forever. **No photograph is at risk** — three is the number the operator's own
> standard calls backed up — but the four disks are no longer interchangeable, and `verify` will
> not notice, because a disk with no manifest for that night has nothing to report missing.
>
> **This is the price of not building `sync`, and it is worth re-reading if `--without` ever
> gets used in anger.** Until then it is a cost nobody has paid.

Two alternatives were rejected. Editing the config makes the rig description lie — the
config describes the rig, not tonight's subset — owes a second edit when the disk
returns, and an 11pm JSON edit in a hotel room is the exact failure decision 8 exists
to prevent. Refusing with no gate blocks every remaining night of a trip the moment one
SSD dies: the same inversion of the goal that decision 7 rejects on the card side.

### 26. Tracks missing at offload are declared, not skipped

The third member of a family: a missing card (decision 7), a missing destination
(decision 25), and now missing tracks. Same grammar each time — refusal in the first
ten seconds by default, and proceeding requires saying so on the command line, so the
degraded night is always possible and can never happen by accident.

If pre-flight finds no GPX files — `gpx_dir` empty, or missing outright — it refuses:

```
NO TRACKS FOUND — C:\Travel\GPX holds no GPX files.
Copy tonight's tracks in (or point --gpx at them), or re-run with --no-gpx
to land the raws without sidecars and backfill them when tracks turn up.
```

An empty directory almost always means the tracks were never copied off the logger, or
`gpx_dir` points somewhere stale — both fixed in a minute while still standing at the
desk. Proceeding behind a warning instead would make an untagged night routine, noticed
only when Lightroom's map comes up empty weeks later — the posture decision 7 already
rejects.

**`--no-gpx`** declares the genuine case: the logger is dead or its data is unreachable
tonight. Phase 5 does not run — raws land in their `YYYY\YYYY-MM-DD` folders exactly as
always, and no sidecar is written anywhere. Phases 3 and 4 are untouched, so LANDED
means what it always means, and the eject gate holds for nothing: there is no tagging
work outstanding (decision 22). The verdict is deliberately unmarked, unlike decisions
7 and 25 — those flags scar it because they narrow what it certifies, while sidecars
were never in its subject (decisions 12 and 14). The report body carries the line —
`0 sidecars written — ran with --no-gpx` — and the exit code is 2 (decision 18).

Unlike decision 7's waived corroboration, nothing is permanently lost. Capture times
are already stashed in the manifest (decision 10), so sidecars need no card: tracks
that turn up before the next format are applied by a plain re-run, which converges and
tags what is untagged (decision 13); after the format, nothing regenerates them on every
destination from the manifest and the tracks (decision 20), the laptop included.

The fatal is about *zero* tracks, nothing subtler. Tracks that exist but fail to cover
a frame remain per-file outcomes — outside track, a count in the report body — and no
flag is involved.

### 27. Both cards must present the same listing before any byte moves

Carried over from photoendofdaygo, this pipeline's predecessor, where it proved itself
in the field. **This is phase 1's assertion.** Phase 1 walks the camera card contents at
any card count — that walk is where the file set and N come from — and a second card is
what gives it something to prove. It comes before pre-flight's rig checks because what
it produces is what those checks are measured against. The comparison sorts each listing
by card-relative path — the key phase 4 pairs on (decision 4) — and goes entry by entry:
same names, same sizes. A directory read, not a data read; seconds, inside pre-flight's
ten. The listings matching exactly is phase 3's precondition — a pair that has diverged
is an equipment failure announced while the fix is a reach into the camera bag:

```
CARDS DISAGREE — the two cards do not hold the same files.
  CFexpress: 1,034 CR3s · SDXC: 1,247 CR3s
  first difference: DCIM\100EOS5R\_50A1035.CR3 — on the SDXC only

Every frame is shot to both cards. A diverged pair means a slot, a card, or
the camera failed mid-day — or a reader holds the wrong card. Check the rig.

Refusing to run. If one card holds the complete day, remove the other and
re-run with --allow-single-source: the survivor becomes the sole source of
truth, never corroborated.
```

Names catch a *presence* divergence — the slot that filled or died mid-day, the wrong
card in a reader. Sizes catch a *content* divergence without reading a byte: the same
name at two lengths cannot be the same photo. Hashing stays out, because it would mean
reading both cards end to end before phase 3 starts — the posture decision 1 already
rejected — and content divergence at equal size is exactly what phase 4's hash pass
exists to find (decision 4).

The pattern is older than this project and has a name: **reconcile control totals between
the sources before the load commits.** Worth saying outright, because it moves the
justification off one predecessor's field experience — the gate is not a photoendofdaygo
quirk, it is the standard answer for a pipeline that must never load a partial extract.
That this design arrived at it from an operator's anxiety rather than from the literature
is corroboration, not coincidence.

What the gate buys is subtraction. "What if the SDXC holds half the day once phase 4
finally reads it" — and every case shaped like it — stopped being a state the pipeline
can reach and became one fatal at the desk. Corroboration dropped from four outcomes
to two (decision 4), `absent` retired from the manifest vocabulary (decision 12), the
per-file defect table lost both divergence rows (decision 21), and the eject claim
sharpened: cards reformatted early can cost corroboration, never bytes (decision 13).
The pipeline launches from a proven sanity point instead of defending against what it
might find past one.

There is no flag to waive the check — decision 16's rule that no flag bypasses
pre-flight holds. The escape is physical and explicit: remove the card that stopped
mid-day and declare the survivor with `--allow-single-source` (decision 7). The trade
is recorded honestly: the retired four-outcome machinery would have corroborated the
morning and landed the afternoon from the surviving slot, all in one evening; the gate
gives that up so an equipment failure is heard before dinner rather than read about
after it.

The check runs if and only if the run has two cards. `--allow-single-source` has no
pair to compare, and a lone-card remainder resume is held to the listing its run
recorded instead (decision 13).

### 28. Every manifest this tool has ever written stays readable

Decision 20's promise — an archive pulled from the safe in 2031 proves itself on a
machine that has never seen this configuration — carries a dependency nobody had
stated. The binary doing the proving is whatever is current *then*; the manifest it
reads was written by whatever was current when the photos landed. Those are not the
same program, and the gap only widens.

**So `verify` reads every schema version this tool has ever written, permanently.** No
deprecation window, no migration deadline. Dropping schema 1 would not degrade an old
archive — it would strip it of the one thing that makes it self-describing, and the disk
in the safe cannot be regenerated. A reader's backward compatibility is therefore a
promise with the same lifetime as the photographs.

**The other direction fails loudly instead of guessing.** An older binary meeting a
newer manifest says exactly that — this manifest is schema N, this build understands up
to M, use a newer `offload` — and never reports the photos as damaged. Same reasoning
as decision 12's self-checksum: for a verification tool, a false alarm on irreplaceable
data is its own kind of failure, and *I cannot read this* must never wear the costume of
*your archive is rotting*.

**The number bumps only when an old reader would be wrong, not when it would be
incomplete.** Adding a field that an old `verify` ignores while still checking every hash
correctly is not a bump. Redefining an existing field, removing one, or making a new one
load-bearing for verification is. That is the same compatible-versus-breaking line
`TRIP-HYGIENE.md` already draws around semver, applied to this project's own artifact.

**The photo facts are a stable core that no bump may redefine.** Decision 12 has the
four copies cross-checking each other's manifests, and decision 20 would let a repair
rewrite one disk's manifest years after its siblings were written — so a cross-check
will eventually span two schema versions, and it can only work if the fields it rests on
still mean what they meant: `name`, `status`, `sha256`, `bytes`, `captured_utc`. A bump
may add to that set and never redefine it. Even retiring SHA-256, if it ever comes to
that, goes in additively — keep `sha256`, add the successor alongside it, and let each
reader use the strongest field it recognizes (decision 17's one-algorithm claim holds
until exactly that day).

**The destination marker carries a schema too**, under the same rules. `verify` reads it
before anything else on the disk (decision 20), so it is the first thing that must
survive a decade in a safe.

### The archives still say `photoday`, and that is this decision enforcing itself

**The command was renamed to `offload` on 2026-08-05** — it runs once per shooting *session*
and several sessions happen in a day, so a name asserting a daily cadence contradicted this
design's own *Inputs*. **Three names deliberately did not move:**

| Name | Where it lives | Why it stayed |
|---|---|---|
| `.photoday-manifest.json` | every date folder, on every disk | `verify` finds manifests by walking for this exact filename. Renaming it would make every archive already in the safe unverifiable — not damaged, *invisible*, which is worse |
| `.photoday-destination.json` | each destination root | the first thing `verify` reads (decision 20) |
| `.photoday-tmp-` | in-flight writes | pre-flight's orphan sweep keys on it, so a rename would strand debris from any run written before it |

**This is the promise at the top of this decision, arriving sooner than expected.** *Every
manifest this tool has ever written stays readable, permanently* — and a filename is a
stronger commitment than a schema version, because a reader that cannot parse a manifest at
least reports something, while a reader looking for the wrong *name* reports a clean disk
with no photographs on it. **The silent failure is the unacceptable one.**

**So the rename was scoped by what is on removable media rather than by what is tidy.** What
the operator types, the crate, the config directory and the docs all moved; what is written
into an archive did not. A file format outliving the name of the tool that writes it is
ordinary — `.git` and `.DS_Store` both do it — and the alternative here was a compatibility
shim that would have had to live forever, or an archive nobody could verify.

**What *did* move safely: `%APPDATA%\photoday\` → `%APPDATA%\offload\`.** One file, on one
machine, and a config that is not found is a loud pre-flight fatal naming the path it looked
in (decision 8) rather than a silent miss — which is exactly the property the archive names
lack, and exactly why they were treated differently.

**This adds a fourth test to decision 18's three**, and it qualifies on that decision's
own criterion rather than as an exception to it: those tests exist where a defect is
irreversible. A schema-1 manifest fixture, committed to the repository, that `verify`
must read correctly forever. The mutation that proves it can fail is deleting the
schema-1 branch — and without the fixture nothing fails until someone opens a 2026 disk
in 2031, at which point the archive is unverifiable and there is nothing left to fix it
with.

### 29. The dependency set, and where its rebuttals live

**The rule this set was chosen under: integrate wherever a crate is the obvious answer,
and hand-write only what integration would make worse.** Decision 17 already picked the
language and one crate; this settles the rest, so that the question is answered once
rather than per module at the moment each phase is written.

| Crate | Ver | Role |
|---|---|---|
| `clap` (derive) | 4.6 | the CLI of decision 8, subcommands and all |
| `serde` (derive) | 1.0 | config, run log, manifest, report — four artifacts, one derive |
| `serde_json` | 1.0 | those four; the run log is the same serializer a line at a time |
| `sha2` | 0.11 | SHA-256 with SHA-NI selected at runtime (decision 17) |
| `rayon` | 1.12 | the `--jobs` CPU pool of decision 15 |
| `walkdir` | 2.5 | card walks, destination walks, `verify`'s date-folder sweep |
| `tempfile` | 3.27 | temp-then-rename, and the prefix pre-flight's orphan sweep keys on |
| `indicatif` | 0.18 | `MultiProgress` — one bar per destination, **at a terminal only**; it hides itself off-tty, so `progress.rs` wraps it in a three-mode enum |
| `console` | 0.16 | verdict styling, and whether this is a terminal at all |
| `thiserror` | 2.0 | the two error types a caller must branch on, below |
| `anyhow` | 1.0 | every other error, which decision 18 makes fatal |
| `windows` | 0.62 | volume and disk identity, unbuffered I/O, lock/dismount/eject, sleep inhibit |
| `windows-registry` | 0.6 | the Defender exclusion read of decision 9, including its unreadable outcome |
| `nom-exif` | 3.6 | CR3 capture time — arrives with the engine (decision 17) |
| `gpx` | 0.10 | tracks — arrives with the engine |
| `chrono` | 0.4 | the program's instant and duration types — arrives with the engine |
| `time` | 0.3 | `gpx`'s public type only, converted at one named boundary |

Versions are what crates.io served on 2026-08-03, confirmed rather than recalled;
[`TRIP-HYGIENE.md`](TRIP-HYGIENE.md) has the standing order and now names the pre-1.0 entries
that go stale silently.

**Three choices in that table are genuine judgment calls rather than the only answer,
and a reviewer should know they were made deliberately.**

- **`thiserror` alongside `anyhow`, at two sites only.** Decision 18's fatal-out makes
  `anyhow` right nearly everywhere: print why, exit non-zero. Two places instead need a
  caller to branch on the *kind* of failure. `verify` must tell "this manifest is a
  schema I do not understand" from "this archive is damaged" and must never let the
  first wear the costume of the second (decision 28); capture-time extraction must tell
  unreadable EXIF from EXIF carrying no offset, which both route to `_unfiled` and are
  worded differently (decisions 21, 23). Both are enums the compiler should force you to
  match on. String-matching an `anyhow::Error` to make either decision is the shape that
  rots quietly.
- **`windows` rather than `windows-sys`.** Both are Microsoft's, generated from the same
  Win32 metadata, and both are pure Rust calling DLLs the OS has already loaded — which
  is not the C-library dependency the pure-Rust constraint excludes. The ergonomic one
  costs compile time and returns `windows::core::Error`, which implements
  `std::error::Error` and so composes with `anyhow` through `?`. The call sites here are
  `DeviceIoControl` with union-shaped output structs, SetupAPI device enumeration and
  `CM_Request_Device_Eject` — precisely where raw FFI gets a buffer length wrong and
  reports it at runtime, against a disk bound for the safe.
- **No `crossbeam-channel`**, which is the reflexive answer for phase 3's fan-out and is
  already in the tree under `rayon`. `std::sync::mpsc` has *been* crossbeam internally
  since Rust 1.67, and `sync_channel(k)` gives exactly the bounded backpressure decision
  15 describes: the reader hands the same buffer to four bounded queues in turn and
  blocks on the slowest, so the intended behavior falls out of the types rather than
  being arranged. Nothing here needs `select!` or multiple consumers. A buffer pool whose
  free list has several returners is the evidence that would reopen this.

  > **And the backpressure turned out to be *visible*, which nobody claimed when this was
  > decided.** Terry, 2026-08-05, watching the progress bars: *"In verifying the ETCs pop in at
  > markedly different times due to the drives hitting 10% at different times. Writing they're
  > neck and neck the whole way through (due to queue backpressure I suspect?)"* — correct, and
  > it is the clearest possible demonstration of the argument above.
  >
  > **Writing is in lockstep because three drives are waiting**, not because they are equally
  > fast: one reader hands each frame to four bounded queues in turn and cannot get more than
  > `DEPTH` frames ahead of the slowest. **Verify has no shared producer** — each destination
  > re-reads its own files on its own thread — so the rows separate immediately, by actual
  > hardware speed.
  >
  > **That makes the invariant checkable by eye.** *Lockstep means coupled; spread means
  > independent.* A night where the **writing** rows separate would mean a destination had
  > fallen out of the queue discipline — a failure mode nothing else in this tool watches for,
  > and one the display would show in the first minute.
  >
  > A second-order effect worth knowing, since it costs nothing: **the order the verify ETCs
  > appear in is the speed ranking**, because reaching the 10 % display threshold first means
  > being fastest. The threshold was added purely to suppress noisy early estimates.

**The rebuttals for the crates that were taken live in `Cargo.toml`, not here** — beside
the version string each is about, where they cannot drift away from what they describe.
That is RawGeotag's pattern, and [`REVIEWING.md`](REVIEWING.md)'s broken-window table
records why it works: the hand-rolled scratch directory that got written anyway was
duplicated *while `tempfile` was already a dependency and already cited in `Cargo.toml`
for exactly those properties*. The crates that were **not** taken have no such site, and
those are in *Considered and rejected* below.

**What stays hand-written, and why that is not a contradiction.** Thousands separators,
the report's byte and duration formats, its aligned tables, sector-aligned buffers for
`FILE_FLAG_NO_BUFFERING`, and the config path under `%APPDATA%`. Each is a case where the
available crate is either a dependency for a dozen unit-tested lines or a format the
report would have to fight; the entries in *Considered and rejected* name them
individually. Two more need no crate because the standard library already answers them:
`OpenOptionsExt::custom_flags` sets both the write-through and unbuffered flags, and
`available_parallelism` supplies `--jobs`'s default.

**The workspace is two members.**

```
Cargo.toml            [workspace] — the whole dependency set, declared once
crates/geotag/        the CR3, GPX and XMP engine, lifted from RawGeotag (decision 17)
crates/offload/       the binary: CLI, five phases and eject, the Windows storage layer
```

A member's own manifest lists only what its code imports today, so a manifest never
claims a dependency nothing uses.

**The lift moved the engine unrewritten, which was the point.** Decision 17 accepts
these three sub-problems as solved on the strength of their validation, so a lift that
took the opportunity to tidy them would have spent exactly what it was trying to save;
the 67 unit tests came across with the code and the two changes made were the minimum
`offload` could not do without. `raw::capture_time_in_memory` is one — decision 10 has
every file in RAM already, and re-reading it from the card to find its capture time
would be the waste that decision exists to avoid. `xmp::render` taking the writing
tool's identity is the other: two tools emit these packets now, and a sidecar whose
`x:xmptk` names the wrong one is a small lie in a file whose whole job is provenance.

The in-memory path was checked against real frames rather than argued about, since no
committed test can reach its success path without a fixture: it agrees with the
path-based function on a `+01:00` CR3, a `+00:00` CR3, and a NEF with no offset at all.
That last one matters more than it looks — it is why `capture_time_in_memory` still
consults `read_strategy` instead of assuming that bytes in memory make the distinction
moot. The strategy records which *parser path* a format survives, not how much of the
file to read, and NEF parses through only one of them.

**The lift has one coupling worth writing down before it happens.**
[`TRIP-HYGIENE.md`](TRIP-HYGIENE.md) sends the reader to RawGeotag's `docs/LIGHTROOM-XMP.md`
after a Lightroom major release, because that is where the XMP engine's verification
lives. Moving the engine here without moving that document would leave the pointer
aiming at the verification record of code that no longer lives beside it — a
one-canonical-place violation ([`WRITING.md`](WRITING.md) rule 2) that would surface at
the worst moment, which is trip hygiene.

> **Two corrections from doing the lift (2026-08-03), and one lesson that outlived them.**
> `thiserror` is earned by the manifest schema reader alone — `raw::Capture` was *already* an
> enum of outcomes, so the error type this design asked for had been made unnecessary before it
> was requested. **A design that reasons about code it has not read will invent types the code
> already made redundant.**
>
> **`LIGHTROOM-XMP.md` stays in RawGeotag** because its procedure *drives* `rawgeotag.exe` rather
> than describing it, and `offload` cannot write a sidecar yet. A document whose first instruction
> is unrunnable in the repository holding it is the failure [`WRITING.md`](WRITING.md) opens with.
>
> **The engine therefore exists twice and a fix to one copy does not reach the other**, until
> decision 30 retires RawGeotag — which cannot start until phase 5 works.

### 30. RawGeotag retires into `offload`

The lift of decision 29 left the engine in two repositories: `crates/geotag` here, and
the original four modules in RawGeotag, which was deliberately not modified and still
builds and runs. A fix applied to one does not reach the other, and that window stays
open for as long as both exist.

**The resolution is retirement, not a path dependency.** RawGeotag's CLI is a strict
subset of what `offload` will do once phase 5 lands — correlate capture times against a
GPX index and write XMP sidecars is *the whole of* phase 5 — so the tool becomes a
subcommand, `offload geotag`, and its repository is archived. A path dependency was the
alternative and is the weaker end state: it would keep one canonical engine but leave a
second binary, a second CLI, a second set of docs and a second CI to maintain, all for a
capability the primary tool will have anyway.

**Nothing happens until phase 5 exists.** RawGeotag works today and geotags real trips;
it keeps working until its replacement is real and has been run against the fixture
corpus. Retiring it before then would trade a working tool for a promise.

What comes across at that point, beyond the CLI surface itself:

- `docs/LIGHTROOM-XMP.md`, which decision 29 could not move because its procedure drives
  `rawgeotag.exe` — once `offload geotag` is that binary, the procedure moves with it
- `docs/FIXTURES.md` and `scripts/verify-fixtures.ps1`, the harness that hashes the real
  corpus, which is what actually validates the engine
- `docs/TESTING.md`'s one load-bearing principle, already folded into
  [`REVIEWING.md`](REVIEWING.md) — the rest stays retired, per *Considered and rejected*
- the duplicated `fixture-manifests/`, which stops being duplicated

The one thing that does not come across is RawGeotag's `--utc-offset`, which existed for
a body that recorded no timezone. Decision 23 removed the need for it here, and a
subcommand that reintroduced it would reintroduce the gate it implies.

**Until then the duplication is live and must be treated as such:** a change to
`crates/geotag` that fixes a real defect has to be applied to RawGeotag's copy by hand,
or the trip tool silently keeps the bug. `CLAUDE.md` says so where a session will see it.

### 31. `YYYY\YYYY-MM-DD` is Lightroom's layout, and the reason is import speed

The shape of the output tree has been stated since the first draft and defended nowhere,
which made it look like taste. It is not: **it is the layout the Lightroom catalog is
already configured to use, and writing it directly is what makes the import at home fast.**

Lightroom's import offers two relevant modes. *Copy* has Lightroom move the files into
its configured `YYYY\YYYY-MM-DD` structure on the NAS as part of importing. *Add* leaves
files where they are and simply catalogs them. **Copy is agonizingly slow — measured in
the field at more than 10× the cost of the alternative**, which is to put the files on
the NAS already in that structure and import with *Add*.

So this tool writes the final layout, the NAS copy inherits it unchanged, and the import
is the fast path by construction. The tool is doing the file arrangement that Lightroom
would otherwise do far more slowly, and it is doing it during a phase that is already
writing every byte anyway — the arrangement costs nothing here because the write has to
happen regardless.

**Three consequences worth stating, because they turn a preference into an interface:**

- **The layout is not free to change.** It is pinned to another program's configuration,
  and changing it here would silently return the operator to the slow import. A future
  reader who finds `YYYY\YYYY-MM-DD` arbitrary should read this decision before
  "simplifying" it — that is exactly why it is now written down.
- **`_unfiled` sitting outside the `YYYY\` tree is load-bearing** (decision 21), not
  merely tidy. The import points at the year folders, so a file parked outside them is
  invisible to it — which is the correct treatment for a frame whose capture time could
  not be read, since it has no date folder to belong to.
- **The date must be the one Lightroom would have chosen**, which is why decision 23's
  per-photo UTC derivation matters beyond foldering: a frame in the wrong day folder is
  not merely misfiled here, it is misfiled in the catalog too, and the catalog is the
  thing that is hard to correct later.

Recorded 2026-08-03, from the operator, after the layout had gone undefended through the
entire design.

### 32. A degrading card is caught by comparing it to itself

**A card that dies is loud. A card that is slowly dying is silent, and silence is the
failure this tool exists to remove.** The CFexpress that failed on 2026-08-03 threw
`os error 483` on every data read and pre-flight caught it in under two seconds. Nothing
would have caught the SDXC card that spent an unknown period running at **26% of its rated
speed** — it was found on 2026-08-04 only because an unrelated investigation happened to
measure it. A run with a half-speed card looks exactly like a big day on a tired laptop.

**This is not only an offload cost.** The camera writes every frame to both slots, so the
buffer drains at the slower card's rate. That card was taking frames away in the field,
invisibly, for as long as it had been degrading.

**Pre-flight already measures both cards** (decision 8) — that measurement is the card speed
test, and it is what caught the dying card. Recording it costs a file write. The check
proposed here is therefore nearly free: it adds no I/O to the critical path, only a
comparison against what previous runs saw.

**Compare a card to its own history, never to a fleet median.** Healthy cards on this rig
differ by more than 2×: on one Thunderbolt reader a Lexar CFexpress reads 938 MB/s and a
Sabrent Rocket CFX ~1,135, and both are sound. A cross-card threshold loose enough to accept
that spread cannot detect a card that has halved, and one tight enough to detect it would
condemn a healthy card for being a different model. **Only a card's own past has the
sensitivity to matter.**

**Identity is the hard part, and it splits by card type.** Decision 7 established that cards
cannot be identified by volume serial, because the in-camera format at the start of every
session assigns a new one. But a CFexpress in an NVMe reader exposes a stable *hardware*
serial through `IOCTL_STORAGE_QUERY_PROPERTY`, which the storage layer already queries, while
SD through a USB bridge frequently reports a generic or empty one. So the check is asymmetric,
and that asymmetry is acceptable because it favors the card that matters:

| | Anchor | Why |
|---|---|---|
| **CFexpress** | per-card history, keyed by hardware serial | stable identity, and it is the card on the *LANDED* path — phase 3 reads it |
| **SDXC** | a fixed floor | identity may not survive a reader, and it is only read in phase 4, after the product moment |

**Record before warning.** The first run has no baseline, and a threshold chosen on day one
is a guess dressed as a check. Ship the recording, let history accumulate, and enable the
warning once there are enough samples to set the bound from evidence — which is the same
standard `REVIEWING.md` applies to every other number here.

**Record the machine's uptime with every sample, and only compare like with like.**
Measured 2026-08-04, and it is the largest source of noise found so far: the same healthy
SDXC card, in the same reader, through this same pre-flight measurement, reported
**168 MB/s at 17 minutes of uptime and 218 MB/s at 129 minutes**. A 30 % swing, driven
entirely by how busy Windows still was — larger than the degradation this check exists to
detect. A freshly booted machine is running Defender catch-up, the indexer and prefetch,
and a card measured then looks like a card that is dying.

**Without this, the check would fire on healthy cards on precisely the nights it is most
likely to run.** `FULL-RUN.md`'s procedure opens with a reboot, and the operating pattern
this tool was built for — unpack the rig in a hotel room and offload — often follows one
too. A history that mixes busy-machine and settled-machine samples has more noise in it
than signal.

**The threshold must survive a healthy card that is noisy.** The Sabrent reproducibly swings
**856–1,394 MB/s, ±28%**, across passes that agree with each other — it is not degrading, it
is simply variable, most likely by position on the card. A single pre-flight sample from that
card is itself a noisy number. So the comparison is **median of this run's samples against
the median of history**, not one reading against one reading, and a card is judged on its
floor rather than its mean.

**The warning must name the link, or it will blame the wrong thing.** A card in a USB 2.0
port produces *exactly* the symptom this check looks for. Measured 2026-08-04: one card, one
reader, **222 MB/s on a SuperSpeed port and 38 MB/s on a USB 2.0 one** — a 5.8× shortfall
with no error, no failed mount and every file readable. A check that reported only "this card
is slow" would have the operator suspecting a card that is perfectly sound, on the night
before a trip, which is the precise anxiety this tool exists to remove.

So the warning reports the **link generation alongside the speed**, read from the PnP parent
chain: a SuperSpeed device sits behind `Generic SuperSpeed USB Hub`, a USB 2.0 one behind
plain `Generic USB Hub`. *"Card reads 38 MB/s, connected at USB 2.0"* is an instruction to
move a cable. *"Card reads 38 MB/s, connected at SuperSpeed"* is a dying card. Same number,
opposite actions.

**Do not use the storage protocol as the discriminator.** BOT versus UAS looks like the same
signal and is not: this rig's SSDs enumerate as `USB Attached SCSI (UAS) Mass Storage Device`
while both card readers report plain `USB Mass Storage Device`, yet one of those readers
sustains 222 MB/s — beyond what USB 2.0 can physically carry. The parent chain is evidence;
the protocol name is a coincidence.

**It warns; it does not block.** A slow card is still a *correct* card, and every guarantee
the tool makes is about bytes rather than speed. The run proceeds, all four copies land, and
the report names the card and the shortfall. Refusing to offload a night's shooting over a
speed regression would have the tool manufacturing the emergency it exists to prevent.

The history file is JSON, per this project's default for structured data, keyed by hardware
serial with a bounded list of samples. **Decision 33 is where that file is specified**, and
it covers destinations by the same mechanism — the run log already holds what a destination
history needs, so this check and that one share one artifact rather than two.

Recorded 2026-08-04, from the operator, after a faulty card was found by accident. The
motivating case is stated in his terms: the tool is an anxiety instrument, and **the
terrifying failure is not the one that announces itself.**

### 33. Throughput history covers destinations too, and lives beside the config

Decision 32 catches a card going quietly slow. **Nothing catches a destination doing the
same thing** — and the destinations are where the photographs actually live. An archive SSD
degrading in the safe is the same silent failure as a degrading card, with more at stake.

**The measurement is already free on both sides, which is what makes this cheap.** Pre-flight
measures each card to pick the phase 3 source (decision 8). And the run log already records
`verified_utc` for every `(file, destination)` pair, so a destination's write and verify
rates are *derivable from a run that has already happened* — that is exactly how the
2026-08-04 per-destination figures were obtained, and it needed no probe.

**Which means the history can be backfilled rather than started empty.** The laptop holds
every run log this tool has ever written (decision 14 keeps `_runs` there). Card samples are
the only ones genuinely lost, because pre-flight prints them and discards them.

**One file, one record shape**, so the two subjects cannot drift into two mechanisms:

```json
{
  "schema": 1,
  "devices": {
    "K03ABCXA9TC0627": {
      "kind": "card", "label": "AV PRO CFexpress SE",
      "samples": [
        { "utc": "2026-08-04T23:04:41Z", "run_id": "2026-08-04T23-04-41Z",
          "read_mb_s": 842, "uptime_min": 129, "link": "NVMe" }
      ]
    },
    "2138FB400347": {
      "kind": "destination", "label": "SanDisk",
      "samples": [
        { "utc": "2026-08-04T23:31:12Z", "run_id": "2026-08-04T23-04-41Z",
          "write_mb_s": 428, "verify_mb_s": 597, "uptime_min": 129,
          "link": "USB SuperSpeed" }
      ]
    }
  }
}
```

**It lives at `%APPDATA%\offload\history.json`, beside the config — never on an archive.**
Decision 20 is what decides it: `verify` reads nothing but a destination's marker and its
manifests, so anything else on that disk is by construction not part of what makes it
self-describing. History is machine state about hardware, not archive data, and putting it
on the archives would add non-photograph directories to disks whose whole promise is that
they hold photographs and their proof. Same reasoning that put `_runs` on the laptop.

**Every sample carries `uptime_min`, and this is not bookkeeping.** Measured 2026-08-04: the
same healthy SDXC card, same reader, same pre-flight measurement, read **168 MB/s at 17
minutes of uptime and 218 at 129** — a 30 % swing larger than the degradation these checks
exist to detect. **Compare like with like or the history is noise.** Each sample also carries
the link generation, for decision 32's reason: a card at 38 MB/s on a USB 2.0 port and a
dying card at 38 MB/s are the same number and opposite actions.

**A destination's health signal is its verify rate, not its write rate.** Measured across
three full runs in one evening on 2026-08-04, same 201.3 GB to the same four destinations:

| Pass | Run 1 | Run 2 | Run 3 | Run 4 |
|---|---|---|---|---|
| Write | 7 m 47 s — **431 MB/s** | 10 m 47 s — **311** | 8 m 29 s — **395** | 8 m 10 s — **411** |
| Verify | 5 m 41 s | 5 m 47 s | 5 m 35 s | 5 m 40 s |

**Verify reproduces to ±1.8 % — a 12-second spread across four runs moving 201 GB to four
devices each time**, with the per-destination windows agreeing to a few seconds throughout.
That is what makes it usable as a health signal, and why decision 33 warns on `verify_mb_s`
and merely records `write_mb_s`.

**The write pass has one outlier, not a spread**, and saying so precisely matters. Three runs
sit at **395–431 MB/s (±4 %)** and run 2 sits at 311. An earlier version of this section read
"write swings 28 %", which implied general variability and invited a shrug; one anomalous run
invites a cause.

**Two explanations were tried and both failed.** After two runs this blamed SLC exhaustion
and garbage collection falling behind — refuted by run 3, which followed run 2 by 12 minutes,
had *less* recovery than run 2 had, and came back faster. Cumulative write load was the
fallback, and run 4 refutes that too: it is the fourth consecutive 201 GB write onto the same
drives and among the fastest. **Neither rest nor accumulated writes predicts it.** Machine
uptime does not either — the fastest run was on the busiest machine, 17 minutes after boot.
`examples/write-contention.rs` remains unfit to investigate it (see its own note), so this
stays open and unexplained rather than explained badly.

**So a write-rate history would record phantom degradation** — a healthy drive looks 28 %
slower simply because it was written to recently, which is larger than the decline the check
is hunting. Store `write_mb_s` for the record, warn on `verify_mb_s`. Same lesson as
`uptime_min` one paragraph up: **a throughput sample means nothing without the state the
device was in when it was taken**, and for a destination that state is how much it has
recently absorbed.

**Card samples and destination samples are never compared to each other, and neither is
compared across kinds of measurement.** A card sample is a *burst* — a brief pre-flight
read. A destination sample is *sustained*, derived from a whole run's worth of bytes. The
two are not the same quantity, and the 2026-08-04 SDXC episode is what that mistake looks
like: a burst held against a sustained figure read as a 24 % shortfall on a perfectly
healthy card.

**Identity follows what each device can actually prove**, which is decision 32's asymmetry
extended:

| | Key | Why |
|---|---|---|
| **Destinations** | disk serial | Reliable, and already the thing decision 6 resolves them by |
| **CFexpress** | hardware serial through an NVMe reader | Stable, and it is on the LANDED path |
| **SDXC** | *none* | Reports a generic serial through a USB bridge — `000000000003` on this rig |

So the corroborating card is the one device this can say least about, which is the right
place for the gap: it is read in phase 4, after the product moment.

**Record before warning, and warn rather than block** — both inherited from decision 32 for
the same reasons. A threshold chosen before there is history is a guess dressed as a check,
and a slow device is still a *correct* device.

Recorded 2026-08-04, from the operator, on seeing that the run log already held everything a
destination history needs.

### 34. The body is named in the config, and an unexpected one is reported

**`CONOPS.md`'s shooting-day contract opens with "the fleet is one body — a Canon EOS R5",
and nothing observes it.** Every other clause of that contract has a check behind it: two
cards present (decision 7), the pair holding one listing (decision 27), CR3 only (decision
24), tracks present (decision 26). The body is the one the tool takes entirely on trust.

So the config names it, and pre-flight compares:

```json
"body": { "model": "Canon EOS R5", "serial": "..." }
```

**It reports; it never refuses.** Frames from an unexpected body are perfectly good
photographs that still need four verified copies, and refusing would leave the night with
zero backups over a *process* violation — the inversion decisions 7 and 25 both reject.

**And it is INFO, not a warning — settled by the operator 2026-08-05, correcting a first
draft that made it exit 2.** The body goes in the report body as a plain line beside decision
23's timezone line, which is the same kind of fact about the camera rather than about the
data. **It does not touch the verdict and it does not touch the exit code.**

**The reason is that a mismatch persists, and exit 2 is a signal that must not.** A different
body is not a one-night event: replace the R5, or shoot a trip on a rental, and the mismatch
is true on *every* run until the config is edited. Exit 2 on every night of a trip would
train the operator to read past exit 2 — which also carries unfiled frames, confirmed
mismatches, a refused eject and a run that never corroborated. **Spending a scarce signal on
something that repeats is precisely how it stops meaning anything**, which is decision 12's
argument about the manifest applied to the exit code, and decision 9's about a warning that
fires regardless of the truth.

> **This gives the report an explicit severity vocabulary it had been using implicitly**, and
> naming it is worth more than the one case that prompted it:
>
> | Level | How it appears | May it change the verdict or exit code? |
> |---|---|---|
> | **INFO** | a plain line in the report body | **never** — timezone, geotag counts, per-destination rates, and now the body |
> | **WARNING** | a `!`-prefixed block | exit 2, and the verdict may carry a scar — unfiled frames, a deleted mismatch, a refused eject |
> | **VERDICT** | the last line, phrases appearing nowhere else | it *is* the answer (decision 14) |
>
> The test for which one a new finding takes: **would it still be true tomorrow night, and
> the night after?** A repeating fact is INFO however much it matters, because a warning that
> repeats is a warning that gets filtered. A thing that is true about *tonight* can be a
> warning.
>
> **INFO does not mean nobody acts on it — it means the *program* does not.** This line has two
> readers, and that is the point of putting it at INFO rather than dropping it: a tired human
> sees a fact about their camera beside a fact about its clock, while **Claude has a standing
> instruction to act on it every single time it disagrees with the config** — ask what changed,
> offer the config edit ([`../CLAUDE.md`](../CLAUDE.md), *Report lines you must act on*). The
> operator runs the offload through Claude whenever he has internet, which `CONOPS.md`'s
> shooting-day contract now records as a commitment.
>
> **The severity level and the follow-up are therefore orthogonal, which is worth stating
> because collapsing them is how the first draft went wrong.** *How loud should the program be*
> and *who has to do something* are different questions. Exit 2 was the wrong answer to the
> first; it was never an answer to the second at all.
>
> **And it must stay useful with nobody watching.** Hotel internet is not guaranteed, so this
> line has to be complete and actionable read cold by a tired human — Claude is an additional
> layer, never a required one. A check that only works when Claude is present is a guarantee
> the tool does not actually make.

**The serial matters more than the model, and the operator's own history is why.** He rented
an R5 in 2021 and bought one in 2024 after a robbery took the previous body. A model check
passes cleanly on a rented R5; only a serial tells *an* R5 from *his* R5, and the contract is
a claim about a body rather than about a product line.

**The payoff is bigger than the contract-nag it looks like, and it is decision 23.** That
decision rests on *the only body is an R5, and the R5 records `OffsetTimeOriginal` on every
frame*. A body that does not record an offset sends **every frame to `_unfiled`** (decision
21) — and today that is discovered only after the whole day has streamed through phase 3. One
frame at pre-flight turns a 35-minute discovery into a ten-second one, while the fix is still
a decision about tonight rather than a fact about it.

**What it does not buy, stated so the case is not oversold.** The naming scheme looked like a
second victim — decision 5 drops the body prefix precisely because *the fleet is fixed at one
R5*, so two bodies would collide on `HHMMZ_NNNN`. They do not: decision 5's hash check already
resolves that to a `_001` suffix correctly. Two bodies make that branch less pathological, not
wrong. The dual-slot half of the contract is likewise already caught, by decision 27's gate.
**One genuine early warning, not a correctness hole being plugged.**

**The cost is nothing, which is what makes it worth having.** Phase 3 and `--dry-run` already
parse each frame's EXIF, and `--dry-run` does it for 3,883 files in about 0.3 s by seeking the
container rather than reading it. Reading two more tags from the *first* frame on each card is
microseconds and no new dependency.

> **One dependency is genuinely open and must be measured before this is built.** Canon has
> historically written the body serial into its **MakerNotes** rather than into standard EXIF,
> and the two live in different boxes of a CR3: `CMT2` is ExifIFD, `CMT3` is MakerNotes.
> `nom-exif` collects the standard `CameraSerialNumber` (0xa431) and *locates* CMT3, but
> decoding Canon's MakerNote structure is vendor-specific work it may not do.
>
> **That Lightroom displays the serial proves it is in the file, not which box it is in** —
> Lightroom reads MakerNotes heavily. And binding constraint 1 is what makes the distinction
> expensive rather than academic: there is no ExifTool to fall back on, so a MakerNotes-only
> serial means decoding Canon's structure by hand.
>
> ✔ **Settled 2026-08-05: the R5 writes it into standard EXIF.**
> `crates/geotag/examples/body-identity.rs` against a real frame returns
> `Make Canon · Model Canon EOS R5 · CameraSerialNumber 092023000050`, read with
> `exif.get(ExifTag::CameraSerialNumber)` — the same call shape `raw.rs` already makes for
> capture time. **No MakerNote decoding, no new dependency, no strain on binding constraint
> 1.** The lens fields returned too, which was the control: a lens serial present with the
> body serial absent would have meant Canon's tag layout rather than the parser failing to
> reach the block, and there is no such ambiguity to chase.
>
> The fallback is therefore unneeded, and recorded only because it shaped the design: had the
> serial been unreachable, **`model` alone still carried the decision 23 payoff** — the part
> that actually pays — so the feature was never blocked on this, only sharpened by it.

**The lens is deliberately not checked, and the probe that reads the body serial reads the
lens one too — so this needs saying before someone connects them.** `body-identity.rs` returns
`LensModel` and `LensSerialNumber` alongside the body fields, and treating them the same way
would be the obvious next step and a mistake.

**Bodies and lenses have opposite operating models here** (`CONOPS.md`, the shooting-day
contract). The fleet is *one body*, replacing it is a design event, and an unexpected one is
worth a line in the report. **Lenses change constantly and by design** — the operator owns an
RF 24-240 and rents specialized glass eagerly and often, an ultra-wide for Monument Valley in
2024 being his own example. **A lens check would fire on every rental, which is to say most
interesting trips**, and decision 34 has already rejected exit 2 for a signal that repeats.
An INFO line that is wrong most of the time is no better: it is the warning you learn to read
past, aimed at a fact that was never a problem.

The tell that this is not a close call: **the very frame that settled the serial question
carries `RF24-105mm F4-7.1 IS STM`, which is not a lens he owns.** A lens check would have
produced its first false positive on the first real frame it ever saw.

**One body, not a list**, per this project's preference for the flat thing until a real second
case appears. A rental during a repair is the case that would promote it; until one happens,
a list would be machinery for a fleet that has been size one for its entire history.

**And a replaced body warns on every run until the config is edited, which is correct rather
than annoying.** `CONOPS.md` already calls a new body *a design event, not a config change* —
its EXIF and dual-slot behavior get verified at home before any trip trusts it — so a
deliberate edit is exactly the friction that decision wants. Same posture as decision 6's loud
`REFORMATTED` warning about a disk whose serial matched at a new volume GUID.

Recorded 2026-08-05, from the operator, who asked whether recording the body would catch a
contract violation *by him*.

## Considered and rejected

Recorded so a later reviewer does not spend effort re-proposing them. Reopening one needs
new evidence rather than fresh taste.

| Proposal | Why not |
|---|---|
| A fleet-median card speed threshold | Healthy cards here differ by more than 2× on one reader (938 vs ~1,135 MB/s), so a bound loose enough to accept them cannot catch a card that has halved. Decision 32 compares a card to its own history instead |
| Blocking the run on a slow card | A slow card is still a correct card, and the guarantees are about bytes, not speed. Refusing a night's offload over a speed regression manufactures the emergency the tool exists to prevent. Decision 32 warns in the report |
| Parquet, or any columnar format, for the throughput history | Raised by the operator and rejected on arithmetic before it was proposed. Columnar formats exist for data that will not fit in memory; this is six samples per run — two cards, four destinations — at ~30 travel days a year with two offloads a day. **360 samples a year, roughly 540 KB of JSON per decade**, and decision 33 bounds the list per device so the file stays under ~50 KB regardless. The query engine is a linear scan over a `Vec`. It would also spend the property decision 17 paid two minutes a run to keep: `history.json` opens in a text editor on any machine in 2031, where Parquet needs a specific library that may not still be around. Binary costs `git diff` and hand-repair too. Decision 33 |
| JSON Lines for the throughput history | The global preference sends append-only logs to JSONL so a row can be added without rewriting the file — and it does not apply here, which is worth stating so the rule is not applied mechanically. Decision 33's history is *bounded* per device, so old samples are dropped and the file is rewritten atomically every run: a ring buffer, not an append log. The run log beside it *is* JSONL, because that one is genuinely append-only and has to survive a crash mid-write (decision 12) |
| Checking the **lens** the way decision 34 checks the body | Opposite operating models: the fleet is one body and replacing it is a design event, while lenses change constantly and by design — he owns one and rents specialized glass often. A lens check fires on every rental, i.e. on most interesting trips, and an INFO line that is wrong most of the time is the warning you learn to read past. The frame that settled the serial question already carries a lens he does not own. Decision 34 |
| Refusing a run whose frames came from an unexpected body | The frames are good photographs that still need four copies; refusing over a *process* violation leaves the night with zero backups, which is the inversion decisions 7 and 25 both reject. Decision 34 reports instead |
| Making an unexpected body exit 2 | Decision 34's own first draft, corrected by the operator the same day. A replaced or rented body is true on *every* run until the config is edited, so exit 2 would fire nightly for a whole trip and teach the operator to read past a code that also carries unfiled frames, deleted mismatches and refused ejects. **A repeating fact is INFO however much it matters** |
| A list of accepted camera bodies | Machinery for a fleet that has been size one for its entire history, and `CONOPS.md` records the intent to replace rather than add. Decision 34 takes the flat single body; a rental during a repair is the real second case that would promote it |
| Local-time date folders | UTC is a deliberate conviction, not an oversight. The early-morning-east-of-UTC consequence is understood and accepted |
| A `date_folders` config setting | A knob whose only legal value is its default — UTC foldering is the conviction above, not a preference |
| Ingesting non-CR3 files into `_unfiled` as a catch-all | Machinery for formats the operator never shoots. `_unfiled` exists for a defective CR3, not as a junk drawer; the report names strays instead. Decision 24 |
| Verifying immediately after each write | Reads out of the OS page cache, then out of the SSD's own DRAM cache — proves nothing about what is on the disk. Replaced by the sequential verify pass, decision 2 |
| A verifier trailing the write front by a ~4 GB lag window | The original design here, replaced at design review. Equal on the primary metric at best (`N/w + N/r` under any schedule), pays mixed read/write penalties, and keeps the CFexpress reader busy to the end — forfeiting phase 4's early start. Decision 2 |
| `FILE_FLAG_NO_BUFFERING` on the write side | Also original here, replaced at design review: it demands sector-multiple writes, which a raw file's partial final sector cannot meet without a pad-and-truncate dance — for no guarantee beyond what `FILE_FLAG_WRITE_THROUGH` already provides. Decision 2 |
| Reading both cards before writing anything | Puts the ~11.6-minute SDXC read on the critical path for a guarantee that can be delivered after it without being weakened. Decision 1 |
| A single-card run that either always refuses or always proceeds | Refusing outright leaves a one-card night with zero backups; proceeding behind a warning — this design's first answer — makes routine what must never be routine. `--allow-single-source` is the narrow gate between them. Decision 7 |
| Scoping resume by comparing the card's file set against the incomplete run's | The original design here, replaced at design review. A format resets the file counter, so an equal-count reshoot presents the identical path set — resume would trust a log describing photos it never ingested, then phase 4 would quarantine the verified morning while the evening never lands; and a continued session presents a superset, closing out corroboration its own SDXC could still deliver. The volume serial the format already assigns is the exact evidence. Decision 13 |
| Running a diverged pair through a four-outcome corroboration | The posture here before decision 27 — *absent* and *SDXC-only* branches so a mid-day slot failure could not delete the surviving afternoon. It survived the divergence after dinner; the gate refuses it at the desk instead, and both branches retired with the states that needed them. Decision 27 |
| Hashing both cards in the equivalency gate | Reading every byte of both cards before phase 3 is the posture decision 1 rejected, returned in a new coat. Sizes already convict unequal content without a read; equal-size divergence is what phase 4's hash pass exists to find. Decision 27 |
| Editing the config when a destination is missing | The config describes the rig, not tonight's subset — the entry would need editing back, and an 11pm config edit is the failure decision 8 exists to prevent. `--without` declares the absence per run. Decision 25 |
| Warning and proceeding when the GPX directory is empty | Empty almost always means tracks never copied off the logger, or a stale `gpx_dir` — both fixed in a minute at the desk. A warning makes an untagged night routine, noticed when Lightroom's map comes up empty weeks later. `--no-gpx` declares the genuine case. Decision 26 |
| Skipping a file whose two source copies disagree | Leaves the one file known to have a problem in *zero* backups — the exact inversion of the goal. Decision 3 |
| A `_NNN` suffix assigned per offload batch, coordinated across destinations | Superseded by decision 5. Timestamp-prefixed names are a pure function of the photo, so no coordination is needed and collisions are pathological |
| A timestamp prefix replacing the camera's name *entirely* | Still rejected: the sequence number is what breaks ties inside a minute, so discarding it would cost the shooting order this scheme exists to restore. Decision 5 |
| Keeping the camera's whole basename after the prefix | **Reversed 2026-08-03**, having been decision 5's original reading. It carried the body prefix — three characters identical on every frame a one-body fleet has ever shot — into every filename in the archive, permanently. Only the sequence number survives now. This is a correction rather than taste because `CONOPS.md`'s shooting-day contract is what makes the prefix provably non-distinguishing |
| A content heuristic for `--force-xmp` (refuse when the XMP carries `crs:` properties) | A destructive flag that sometimes declines is worse than one that is honest. `--force-xmp=<DEST>` is explicit targeting, not a guess at intent |
| A flag to overwrite raw files, or to bypass pre-flight | No case where either is correct; better as structural impossibilities. Decision 16 |
| Per-device queue depth and overlapped I/O | One buffer-fed blocking writer idles ~1 ms per 45 MB write. Not worth the machinery on speculation — revisit if measurement shows a device going idle. Decision 15 |
| Scaling `--jobs` to the I/O fan-out | Threads do not create bandwidth. Decision 15 |
| Graceful handling of a destination unplugged mid-run | Crash safety is already structural, so the cost exceeds the benefit. Decision 18 |
| Fatal-out on a file whose EXIF cannot be read | The original decision 18 reading, replaced at design review: fatal does not fail safe there — one corrupt file would cost the whole night's backup while nobody is watching. Decision 21 |
| Go for the pipeline, shelling out to `rawgeotag.exe` for phase 5 | Defensible, and rejected on the validated CR3, GPX and XMP assets. Decision 17 |
| SHA3-256 instead of SHA-256 | Litigated on measurement, not taste: **4.3× slower on the rig** — 529 MB/s against SHA-256's 2,252 — and the gap is structural, since SHA-NI accelerates SHA-1 and SHA-256 while Keccak has no instruction to select. Nothing is bought for it here. SHA-256 is not deprecated and has no practical attack; SHA-3 was standardized as a structural *alternative* to SHA-2, not a replacement for a broken primitive, and its one clear advantage — length-extension immunity — is a MAC property that never arises when hashing files for integrity. **This hash detects bit rot, a dying card and a flaky reader, not a motivated adversary.** Paying 4.3× on every byte, five times per photo, to hedge against a break in Merkle–Damgård is the wrong trade for a data workflow tool. If SHA-2 ever does weaken, decision 28 already routes the replacement additively. Decision 17 |
| BLAKE3 instead of SHA-256 | Measured in the same run and genuinely faster — 5,023 MB/s, 2.2× SHA-NI SHA-256 — and still rejected, on longevity rather than speed. `sha256sum` ships with every operating system, so a person in 2031 holding this disk and no copy of this tool can still recompute a manifest by hand; BLAKE3 needs a specific binary they may not have. **Re-tested end to end on 2026-08-04 and the price is now known: 1m 52s on a 20m 57s run.** Decisions 17, 28 |
| XXH3, or any faster checksum | Never a candidate — non-cryptographic, and it detects accidental corruption only. Measured on 2026-08-04 purely to bound the question, and it does: the fastest plausible hash saves 2m 59s of a 20m 57s run, so **no hash choice can be worth more than ~14%** and the argument closes. Decision 17 |
| Carrying RawGeotag's `TESTING.md` whole | A stricter regime than decision 18 calls for; its one load-bearing principle is folded into `REVIEWING.md` |
| `crossbeam-channel` for phase 3's fan-out | `std::sync::mpsc` has been crossbeam internally since Rust 1.67, and `sync_channel` is the bounded backpressure decision 15 describes. Nothing needs `select!` or multiple consumers. Decision 29 |
| `windows-sys` instead of `windows` | Saves compile time and gives up `windows::core::Error`'s `std::error::Error` impl at exactly the call sites — `DeviceIoControl`, SetupAPI, device eject — where raw FFI fails at runtime against a disk bound for the safe. Decision 29 |
| `sysinfo` for storage enumeration | Cross-platform disk listing; it does not expose the volume GUID and physical-disk serial that decision 6 identifies a destination by, which is the entire question being asked |
| `memmap2` for the card reads | An I/O error on removable media becomes a fault at a memory access rather than a `Result`. Phase 3's premise is that a failing card is *reported*, tonight, not that the process dies without a verdict |
| `thousands` or `num-format` for separators | RawGeotag settled this: a dependency for a dozen unit-tested lines with no locale surface. Its `count()` lifts with the engine |
| `humansize`, `bytesize` or `humantime` for the report | Decision 14's report writes the same quantities four different ways by column — `56.1 GB`, `1,113 MB/s`, `4m 12s`, `0:02`. No crate produces that set, so every column would be fighting one |
| `comfy-table` or `tabled` for the report's tables | The layout is bespoke and its column widths are load-bearing; `format!` width specifiers already do this without a layout engine's opinion |
| `aligned-vec` for the unbuffered read buffers | `FILE_FLAG_NO_BUFFERING` needs a sector-aligned buffer, which is `std::alloc` with a `Layout`, a `Drop` and a `Deref`. The one allocation in the program whose invariant should be visible where it is written |
| `directories` for the config path | Cross-platform standard-location machinery for a tool that only ships on Windows. One read of `%APPDATA%` |
| `assert_cmd` and `predicates` for the process-level test | RawGeotag's precedent: `env!("CARGO_BIN_EXE_offload")` and `std::process::Command` are enough. Argued when decision 18's end-to-end surface was four tests; the crate would still earn nothing now |
| `criterion` for decision 17's hash measurement | It is a single sustained throughput figure over 2 GiB, not a microbenchmark needing statistical machinery to see. `examples/hash-rate.rs` reproduces the table in thirty lines and needs no harness |

## Non-goals

- **Touching the cards.** The tool reads them and nothing else — never writes, never
  deletes, never formats. Reformatting stays a deliberate in-camera step at the start of
  each shooting session, which is also what guarantees a card equals a session.
- Modifying raw files. All derived data goes to sidecars and manifests.
- Managing the Lightroom catalog, including renaming historical files.
- Cloud or offsite replication.

## Where this stands

*Kept current deliberately: this section is what lets someone — or some future session —
pick the work up from the repository alone, without needing to have been here. If it is
stale, fix it before doing anything else.*

**The nightly command works end to end and has been run at full scale.** Verified against the
source on 2026-08-06.

| | |
|---|---|
| `crates/geotag` | the lifted engine — CR3 capture time, GPX indexing, XMP (decision 17) |
| `storage`, `power` | volume and device identity, sleep inhibit (decision 6) |
| `config`, `destinations`, `cards` | the rig, resolved by serial; cards found by `DCIM` and timed (decisions 6, 7, 8) |
| `preflight` | phases 1 and 2, including decision 27's card-equivalency gate |
| `pipeline` | phase 3 — read once, fan out, write through, verify unbuffered (decisions 2, 5, 10) |
| `manifest`, `marker`, `verify` | the durable artifact and `offload verify <DEST>` (decisions 12, 20, 28) |
| `phase4` | corroboration — proven on the rig, 3,883 matched, 0 mismatched (decisions 3, 4) |
| `phase5` | geotag, wired and running (decisions 16, 23, 26) |
| `eject` | all five devices, concurrently, retried with backoff (decision 22) |
| `progress`, `human`, `runlog`, `naming`, `hash`, `winio` | the supporting layer — bars in three modes, formatting, the append-only log, filenames, SHA-256, unbuffered I/O |

**Largest run: 7,395 frames, 386.6 GiB, four destinations** — LANDED **35 m 29 s**, whole run
**89 m 59 s**, exit 0. Four earlier runs at 3,883 frames landed in 13–17 minutes. `verify` has
caught a single flipped bit in 201 GB and named the file. **Five run narratives in
[`RUNS.md`](RUNS.md).**

**Settled 2026-08-06:** the eject veto — cause found and `Prepare::FirstAttemptOnly` shipped as
the only behavior — and the report's badge column, which the operator reads as a go/no-go on
unplugging drives.

## Still to build

**Everything below is designed above and does not exist in the source.** Checked against the code
on 2026-08-06 rather than remembered.

| | State |
|---|---|
| ~~`offload sync <DEST>`~~ | **Withdrawn 2026-08-06, not deferred.** The rig is specified at N+1 so a dead drive is a config edit, which leaves nothing to backfill — decision 20 and [`CONOPS.md`](CONOPS.md) carry the reasoning |
| **Stray reporting** (decision 24) | The walk does not carry non-CR3 files out of pre-flight, so `exit_code` has nothing to consult. Named in `main.rs`'s own comment |
| **The Defender check** (decision 9) | `windows-registry` is declared in the workspace and imported by nothing |
| **Throughput history** (decision 33) | No history is written or read; `runlog` and `config` have no such field |
| **`offload geotag`** (decision 30) | No subcommand. RawGeotag stays the tool Terry travels with until this lands |
| **Four verdict suffixes** | `SAFE, NOT EJECTED`, `— BUT CHECK YOUR SDXC CARD`, `— SINGLE SOURCE, NEVER CORROBORATED`, `— SSD-C EXCLUDED` — see *Specified here and never built* under decision 14 |

> **`sync` was the one that could mislead a person rather than a session**, and it was withdrawn
> the day this list was written. The CLI offered it with a description that read like a working
> feature and the only way to find out otherwise was to run it. **A subcommand that fails on use
> is worse than one that is absent** — `CLAUDE.md`'s rule about configuration that is never used
> applies to subcommands too.
>
> **Everything still listed above fails honestly**: it is absent, and nothing claims otherwise.
