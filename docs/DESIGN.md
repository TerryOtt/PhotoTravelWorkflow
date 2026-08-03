# End-of-day photo offload — design

Status: **design complete, not yet implemented.** Twenty decisions, each with the reasoning
behind it, plus what was considered and rejected; what remains to build is listed at the
end.

## The goal, in one sentence

Get back to the hotel, plug two card readers and three external SSDs into a Thunderbolt
hub, run one command, go to dinner, and come back to four verified copies of the day's
photos — so an SSD can go in the safe without anxiety.

## The metric

**Wall clock from pressing Enter to all four copies of the date-divided raw files being
written and read-back verified.**

That moment is the product. Everything after it — GPS sidecars, second-card
corroboration — is gravy, and is explicitly allowed to take longer as long as it does
not delay that milestone.

This is not a "correctness at any cost" design. Where certainty and wall clock conflict,
the tie is broken in favor of wall clock *provided the guarantee is preserved, only
deferred* — see [Phase 1 reads one card](#1-phase-1-reads-the-cfexpress-card-only).

## Inputs

- **Camera:** Canon EOS R5, uncompressed RAW (CR3), ~40–50 MB per frame.
- **Cards:** one CFexpress Type B (512 GB), one SDXC UHS-II (512 GB). Every frame is
  written to **both** slots by the camera. Both cards are formatted in-camera at the
  start of each shooting day.
- **GPX tracks** covering the day, from an external logger.
- **Multiple offloads per day are normal** — a lunchtime offload and an evening one.
  The card may be formatted between them, which resets the camera's file counter and
  causes filename collisions in an already-populated date folder.

## Outputs

Four independent copies, each containing `YYYY\YYYY-MM-DD\` directories holding the raw
files and their geotagged XMP sidecars:

| Copy | Role | Notes |
|---|---|---|
| `C:\` on the laptop | working | Lightroom edits here; its sidecars will diverge |
| External SSD × 3 | archive | Goes in the safe; expected to stay byte-stable |

Plus a JSON manifest per destination making each copy self-describing.

**Date folders are derived from the UTC capture time.** Deliberate: no timezone logic
anywhere, monotonic across a trip, and unambiguous when crossing the date line. The
accepted consequence is that a shot taken early in the morning east of UTC files under
the previous day.

## Architecture: four phases

| Phase | Work | Ends with |
|---|---|---|
| **0 · Pre-flight** (~10 s) | Enumerate the fast card, validate destinations, parse GPX, inhibit sleep, print ETA | You can walk away |
| **1 · Ingest & verify** | Read CFexpress once → SHA256 + EXIF → fan out to 4 → lagged unbuffered read-back verify | **LANDED** |
| **2 · Corroborate** | Read SDXC, compare hashes, delete + tombstone mismatches | Card health report |
| **3 · Geotag** | Correlate stashed capture times to GPX, write sidecars to all 4 | Lightroom-ready |

Phases 2 and 3 do not contend for the same hardware — one reads the SD card, the other
writes a few thousand 3 KB files — so they **run concurrently** to compress the tail.

### Where the wall clock goes

Let **N** = one copy of the day's raws. A big day is N ≈ 188 GB; a normal one N ≈ 50 GB.

Phase 1 moves **9N**: 1N read from CFexpress, 4N written, 4N read back to verify. Only
**7N crosses the Thunderbolt hub**, because the laptop's copy is internal.

| Link | Realistic sustained | Time at N = 188 GB |
|---|---|---|
| CFexpress read (1N) | 1000–1700 MB/s | 2–3 min |
| TB4 hub aggregate (7N) | ~3 GB/s usable | ~7 min |
| Each SSD, write + verify (2N) | 400–800 MB/s sustained | **8–16 min** ← binds |
| Laptop NVMe, write + verify (2N) | 2000+ MB/s | ~3 min |

These run concurrently, so the floor is the maximum, not the sum: **~2–4 min for a
50 GB day, ~8–16 min for a 188 GB day**, bound by the slowest SSD's *sustained* write
once its SLC cache is exhausted — which a continuous 188 GB write will certainly
exhaust.

### Phase 1 in detail

The mechanism, stated explicitly since it is what everything else is measured against:

1. A **reader** pulls one file from the CFexpress card into a buffer taken from a bounded
   pool. Each source file is read **exactly once** — never once per destination.
2. From that in-memory buffer: the **SHA-256**, which becomes this photo's canonical hash,
   and the **EXIF capture time**, which decides its output directory and filename.
3. The buffer is handed to **four independent writer queues**, one per destination. They
   are independent so a slow SSD stalls only itself; the pool size bounds how far it can
   lag and applies backpressure to the reader when it falls too far behind.
4. Each writer writes to a temporary name, flushes, and **renames**, so a partial file
   never carries the real name.
5. A **verifier** per destination trails the write front by a fixed byte window (~4 GB),
   re-reading with `FILE_FLAG_NO_BUFFERING` and comparing against the canonical hash.
6. A record per `(file, destination)` is appended to the run log as it is verified.

**All I/O is unbuffered in both directions.** Writes, because a buffered write leaves data
in RAM after the program believes it landed, which makes the milestone unmeasurable; reads,
because a cached read compares a buffer to itself.

## Decisions

### 1. Phase 1 reads the CFexpress card only

Optimistic and greedy. The SDXC card contributes no bytes to the output — it is a
corroborating hash — and reading it costs ~11.6 minutes at UHS-II speeds on a big day.
Keeping it off the critical path is the single largest available win against the metric.

The guarantee is **preserved but deferred**: any disagreement is still detected in phase
2, just after the milestone rather than before it.

### 2. Verification must defeat both caches

Writing a file and immediately reading it back proves nothing — Windows serves it from
the page cache, and you have compared a buffer to itself. `FILE_FLAG_NO_BUFFERING`
handles that, but a verify running immediately behind the write front then reads out of
the **SSD's own DRAM cache** (512 MB–1 GB), which is subtler and harder to notice.

So the verify **lags the write front by a fixed byte window (~4 GB)**. That flushes the
device cache so the read genuinely hits NAND, while keeping nearly all of the overlap —
reads on these devices run ~3× sustained write speed, so verification hides behind the
write almost entirely.

Unbuffered writes matter for a second reason: buffered writes make the metric
unmeasurable, because the program can exit with gigabytes still sitting in RAM.

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

### 4. Corroboration has three outcomes, not two

| SDXC state | Meaning | Action |
|---|---|---|
| present, hash matches | corroborated | keep, mark verified |
| present, hash differs | genuine mismatch | delete from all four, tombstone |
| **absent** | **uncorroborated** | **keep**, report separately |

Conflating *absent* with *mismatched* is the one way this design could lose a real day's
shooting — if a card errors mid-day and the camera carries on with the other slot, a
naive "delete anything uncorroborated" rule deletes everything after that point. Matched
512 GB cards make this unlikely, but the branch costs almost nothing.

A run reporting far more mismatches than the 1–2 baseline is a dying card, and the
summary should say so in those words rather than making the number speak for itself.

### 5. Filenames are a pure function of the photo

Output filenames prefix the UTC time of capture to the camera's own basename. The date is
already carried by the directory, so only the time of day is needed:

```
2026\2026-08-03\1422Z_50A0001.CR3
2026\2026-08-03\1422Z_50A0001.xmp
```

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
minute the camera's counter is monotonic, so ties break correctly on the basename.

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
Phase 1 computes it before writing anyway, so the check is free.

**Historical files keep their existing names.** Renaming already-imported raws on disk
breaks the Lightroom catalog's links to them; a migration, if ever wanted, has to be
driven from inside Lightroom rather than by this tool. New offloads use the new scheme
and the archive is mixed by design.

### 6. Destinations are identified by hardware, not drive letter

Windows reassigns drive letters to external SSDs freely. Passing three letters that
happen to be right today is how two "copies" silently land on one physical disk.

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

Each destination also carries a `.photoday-destination.json` marker at its root. An
archive pulled from the safe in 2031 can then prove what it is and verify itself on a
machine that has never seen the config.

### 7. Cards are identified by measurement, not configuration

An in-camera format each morning assigns a new volume serial, so a card's volume GUID
changes daily; and cheap readers report generic or empty hardware serials, so the reader
is not reliably identifiable either.

So pre-flight finds removable volumes containing `DCIM`, reads ~64 MB from each, and uses
the faster one as the phase 1 source. CFexpress lands near 65 ms against UHS-II's 240 ms
— unambiguous. Costs two seconds, needs no configuration, survives buying a new reader,
and is correct by construction: phase 1 always runs off the fast card regardless of which
reader is in which port. A config override exists for the day that surprises us.

If only one card is present, phase 1 runs on it and phase 2 reports the day as
uncorroborated rather than refusing to work.

### 8. One command, almost no arguments

"One intuitive CLI command" and "six paths that shuffle between sessions" are in tension,
and typing destination paths at 11pm after a day of shooting is how a destination ends up
pointed at the wrong disk. So the rig is described once in config, and the nightly
command is bare:

```
photoday
```

```
photoday                            the nightly command

  --dry-run                  plan the entire run and write nothing — names every
                             output file exactly, in seconds
  --jobs <N>                 CPU pool for hashing/EXIF/XMP [default: logical CPUs]
  --fail-on-source-mismatch  abort rather than warn when the two cards disagree
  --gpx <PATH>               override when tracks aren't in the usual place
  --max-gap-seconds <S>      refuse to interpolate across a longer hole [default: 60]
  --max-gap-meters <M>       refuse to interpolate across a wider hole [default: 100]
  --force-xmp[=<DEST>]       overwrite existing XMP; archives only unless a
                             destination is named

photoday verify <DEST>       standalone re-verify; works years later, without config
photoday sync <DEST>         backfill a disk that missed an offload
```

Config is JSON:

```json
{
  "destinations": [
    { "label": "laptop", "role": "working",
      "path": "C:\\Travel\\Images" },
    { "label": "SSD-A", "role": "archive",
      "disk_serial": "S5H9NS0R123456",
      "volume_guid": "{a1b2c3d4-...}",
      "subpath": "Images" }
  ],
  "gpx_dir": "C:\\Travel\\GPX",
  "date_folders": "utc"
}
```

The risk is config drift — an entry wrong in a way you don't notice until it matters.
Pre-flight validates every entry against connected hardware and fails in the first ten
seconds, so drift surfaces while you are still standing at the desk.

### 9. Pre-flight must be able to fail, and only there

The worst outcome for a walk-away tool is returning from dinner to a run that died two
minutes in. Before anything is written, pre-flight asserts: all four destinations
present, distinct physical devices, writable, and with capacity ≥ N plus margin; the fast
card readable and enumerated; sleep inhibited (`SetThreadExecutionState`); GPX parsed.

**It also checks Windows Defender exclusions.** Real-time scanning of several hundred
gigabytes of freshly written files across four volumes is a large and invisible tax on
exactly this workload. The archive roots should be excluded; pre-flight reads the
exclusion list and **warns rather than fails**, since this is a throughput problem and not
a correctness one.

Because the cards are formatted daily, enumeration is exact and cheap — the card *is*
the day — so pre-flight can print a real estimate:

```
1,247 files · 56.1 GB · 4 destinations verified distinct · est. 6-8 min
```

That number is what actually lets you leave.

### 10. Phase 1 collects EXIF for free, so phase 3 re-reads nothing

Phase 1 already holds each file in RAM to hash it, so extracting capture time costs no
I/O. Stashing it in the run manifest means phase 3 never re-reads a raw file — it
correlates timestamps against the GPX index and writes a few thousand 3 KB sidecars.

This eliminates a full re-read of the day that a standalone geotagging pass would cost,
without putting anything on phase 1's critical path. Sidecar generation failure is never
fatal and is always backfillable.

### 11. The laptop copy is a working copy

Lightroom rewrites the laptop's sidecars as soon as editing begins, so its manifest is
*expected* to diverge from the archives'. `verify` must treat sidecar drift as normal on
a `working` destination and as damage on an `archive` one.

### 12. The manifest covers raw files only

Raws and sidecars have opposite natures. A raw file is immutable — hash it once, and any
later deviation is corruption. A sidecar is *supposed* to change: Lightroom rewrites it
the moment develop settings are touched, and a re-run of phase 3 against a better track
would rewrite it too.

If `verify` hashed both the same way, it would report the archives as damaged the first
time a photo was edited, and its output would become something to ignore — which
quietly destroys the only thing the tool exists to provide. **A verification tool whose
warnings you learn to ignore is worse than one that checks less and means it.** So
sidecars are treated as regenerable derived data and are not covered.

Two artifacts, split by their durability requirements:

- **The run log is append-only** — one record per file as it lands. A crash mid-phase-1
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
      "name": "1422Z_50A0001.CR3",
      "status": "present",
      "sha256": "9f2b...",
      "bytes": 47185920,
      "captured_utc": "2026-08-03T14:22:37Z",
      "source_card": "cfexpress",
      "run_id": "2026-08-03T18:22:04Z",
      "verified_utc": "2026-08-03T18:31:12Z",
      "corroborated": "matched"
    }
  ]
}
```

**The manifest carries its own checksum.** It holds every hash in the archive, so if those
few hundred kilobytes rot in the safe over five years, `verify` would otherwise report
damage on photos that are perfectly intact — and a false alarm on irreplaceable data is
its own kind of failure. A self-hash lets verify distinguish *your photos are damaged*
from *this manifest is damaged, your photos are probably fine*. The three archives each
carry their own manifest of the same day, so they also cross-check against each other.

`corroborated` carries phase 2's three outcomes — `matched`, `absent`, or `null` while
still pending. A file deleted for a genuine mismatch stays in the list as a **tombstone**
(`"status": "deleted"` with both competing hashes, the reason and a timestamp) so a
`verify` years later reports *clean* rather than flagging a missing file nobody remembers
deleting.

The `runs` array is what makes several offloads a day legible after the fact — each file
records which offload brought it in.

### 13. Resume is automatic and scoped by the source file set

A file counts as done only for a **specific destination** — the run log records
`(file, destination) → verified`, not `file → done` — so a crash partway through fan-out
means redoing that file on one disk, not redoing the run.

The **tail of the log is not trusted.** Records inside the verify lag window (~4 GB)
describe files that may have been in flight when the crash happened, so those are
re-verified rather than believed. Everything earlier is trusted. This is what lets the log
be merely flushed-per-record rather than perfectly durable to the last byte.

Interrupted writes leave no ambiguity, because writes are temp-then-rename: a partial file
never carries the real name. Pre-flight sweeps orphaned temps.

**Resume is scoped by comparing the incomplete run's file list against what is on the card
now.** Same set means the same offload, so resume it; a different set means a new offload,
so the old run stays recorded as incomplete and a fresh one begins. That check is what
makes it safe to resume without asking whether the cards were swapped.

Resume is **automatic** — no flag — and announced:

```
Found incomplete run 2026-08-03T18:22:04Z - 847 of 1,247 files already verified.
Resuming. Est. 3 min.
```

The scenario is discovering at 11pm that the run died. The failure mode of automatic
resume is bounded to redundant work, it cannot produce a wrong archive, and the file-set
check already prevents resuming across a card swap. Requiring a flag to get the obviously
correct behavior is friction at precisely the wrong moment.

### 14. The report separates "your raws are safe" from "everything went well"

Phase 1 is the product and the rest is gravy, so **only phase 1 may change the verdict.**
A geotag miss or a track that didn't cover the evening walk is a count in the body, never
a downgrade at the top — otherwise you learn to read past the verdict line, which is the
same failure the raws-only manifest exists to avoid.

**LANDED is announced when it happens**, not only in the final summary, because phases 2
and 3 run on afterward and someone walking in during them should see the thing they care
about already settled.

```
═══ 2026-08-03 · LANDED 18:26:16 · phase 1 took 4m 12s ═══

  1,247 files · 56.1 GB · read from CFexpress

  laptop  C:\Travel\Images        1,247 written · 1,247 verified   OK
  SSD-A   Samsung T9   S5H9NS…    1,247 written · 1,247 verified   OK
  SSD-B   Samsung T9   S5H9NT…    1,247 written · 1,247 verified   OK
  SSD-C   SanDisk E61  2312A9…    1,247 written · 1,247 verified   OK

  Corroboration   1,246 matched · 1 mismatch · 0 uncorroborated
  Geotag          1,198 tagged · 49 outside track

  !  1 file deleted from all four copies — source mismatch
     1611Z_50A2087.CR3 → _runs\2026-08-03T18-22-04\quarantine\

  I/O          bytes        rate
    read      336.6 GB    1,113 MB/s
    written   224.4 GB      890 MB/s
    total     561.0 GB     1.21 GB/s        10.0× amplification

  Per destination        moved     sustained
    laptop  C:\        112.2 GB   1,842 MB/s
    SSD-A   Samsung    112.2 GB     731 MB/s
    SSD-B   Samsung    112.2 GB     724 MB/s
    SSD-C   SanDisk    112.2 GB     445 MB/s   ← set the pace

  Phase              wall     I/O
    pre-flight       0:04        —
    1  ingest        4:12   504.9 GB
    2  corroborate   3:31    56.1 GB   ⎫ concurrent
    3  geotag        0:12     0.1 GB   ⎭
    total            7:43   561.0 GB

  ►  SAFE TO EJECT AND STORE
```

Serials on every destination line so a glance confirms four genuinely distinct disks. The
verdict is the last line and that phrase appears nowhere else, so it cannot be confused
with anything above it. It takes three forms:

| Condition | Verdict |
|---|---|
| Phase 1 verified everywhere | `SAFE TO EJECT AND STORE` |
| Anything unverified anywhere | `NOT SAFE — 12 files unverified on SSD-C` |
| Phase 1 clean, mismatches far above baseline | `SAFE TO EJECT — BUT CHECK YOUR SDXC CARD (47 mismatches)` |

Because all I/O is unbuffered, the rates are real device throughput rather than page-cache
artifacts — which is what makes the per-destination line a usable diagnostic. The slowest
destination sets the pace of the whole run, so naming it is the single most useful number
for spotting a disk going bad. With a `_runs\` directory accumulating across a trip, a
later comparison against a destination's own rolling average is the natural extension.

`_runs\<timestamp>\report.json` carries the full forensic record: per-file outcomes,
per-phase timings, per-destination throughput, and the resolved hardware identities.

### 15. `--jobs` sizes the CPU pool, not the I/O fan-out

`--jobs N`, defaulting to logical CPU count, following RawGeotag's finding that this
problem class parallelizes well into double-digit thread counts.

**It governs the CPU-bound work**, where that finding applies directly. Phase 1 hashes 5N
— one source read plus four verify reads, 280 GB on a 56 GB day. At roughly 2 GB/s per
core with SHA-NI that is ~140 s single-threaded against a 252 s phase, close enough to
bind the run on faster storage. Spread across cores it disappears. EXIF extraction and XMP
generation ride the same pool.

**It does not govern I/O concurrency**, which is structural:

- one reader per card
- **one writer and one verifier per destination**, so the four devices stream alongside
  each other while each stays sequential within itself

RawGeotag's 12× came from a *latency*-bound workload — SMB round trips and container
seeks, where threads hide waiting. Phase 1 is bandwidth-bound sequential streaming, and
threads do not create bandwidth: a destination sustaining 445 MB/s still sustains
445 MB/s under thirty-two writers, minus whatever sequential locality they break.

**NTFS's single-directory serialization is a smaller factor here than it first appears.**
It applies to *metadata* operations — create, rename, delete — not to data writes; once a
handle is open, pushing 45 MB through it never touches the directory index. Temp-then-
rename costs two metadata operations per file, so ~2,500 serialized operations for a
1,247-file day, which is a couple of seconds against a 252-second phase. Under 1%.

RawGeotag measured that effect on 3 KB sidecars, where metadata *is* the work. Which means
it bites in **phase 3**, not phase 1 — thousands of tiny sidecars into one directory per
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
reading a file body or writing a byte. A complete answer in seconds, which makes it the
natural thing to run before walking away.

**`--force` becomes `--force-xmp`.** In RawGeotag the bare name is unambiguous
because sidecars are all it writes. This tool writes raws *and* sidecars, so `--force`
reads most naturally as "overwrite my archive" — and that reading is the dangerous one. A
destructive flag has to be honest about what it destroys, which includes being
unambiguous about *what*. Semantics are otherwise unchanged, including composing with
`--dry-run` as a rehearsal that reports what would be overwritten while writing nothing.

Two consequences of scoping it that way:

- **No flag overwrites a raw file.** There is no case where it is correct — identical
  content is skipped by hash, different content takes a `_001` suffix. Better as a
  structural impossibility than as a flag nobody should reach for.
- **No flag bypasses pre-flight.** Running against fewer destinations is an explicit
  selection, not an override. Pre-flight exists to fail while you are still at the desk.

Sidecars on the archives are only ever written by this tool, so forcing them is harmless.
Sidecars on the laptop are written by Lightroom and hold develop settings that exist
nowhere else. So `--force-xmp` covers archive destinations, and touching the working
copy requires naming it: `--force-xmp=laptop`.

### 17. Rust, with RawGeotag's engine as a workspace library

Two independent reasons, and they point the same way.

**The hashing is real compute.** Phase 1 hashes 5N — one source read plus four verify
reads — which is 940 GB on a 188 GB day. At roughly 2 GB/s per core with SHA-NI that is
~7.8 minutes single-threaded against a phase that should take 8-16, so it would be about
half the run on its own. It has to spread across cores, which rules out anything with a
global interpreter lock and rewards a language with real threads and no runtime overhead.

**Three of the hard sub-problems are already solved and validated.** Not "code exists" —
validated:

- CR3 EXIF extraction by seeking the container rather than reading 45 MB (~0.3 s for
  3,883 files), which is exactly what phase 1 needs
- GPX indexing and interpolation, with the gap/distance refusal logic
- XMP packets diffed against Lightroom Classic 15.4.1's own output, agreeing to
  0.02-0.12 m on CR3

Re-implementing that last one elsewhere means re-earning validation across thousands of
real files on two bodies. The test is not "was time already spent on it" but "does it
currently solve a hard problem correctly," and it does.

Genuinely new: the ingest and verification pipeline, and a Windows storage-identity layer
(volume GUID and disk serial enumeration, `FILE_FLAG_NO_BUFFERING` with its alignment
requirements, `SetThreadExecutionState`). Neither exists in any language today.

**The honest cost:** the phase 1 fan-out is the one part Go would make easier — a reader
feeding a channel with one writer goroutine per destination is a page of obvious code,
where Rust needs a bounded pool of buffers whose lifetimes span four concurrent consumers.
That is real work, and it is the trade being accepted.

### 18. Fatal out, and test lightly

**Almost every error is fatal.** Print why, exit non-zero, stop. There is no recovery
machinery for unlikely hardware events — a destination unplugged mid-run, a disk filling
unexpectedly — because handling them gracefully would cost more complexity than the
scenarios are worth.

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
| 0 | Phase 1 verified everywhere, no source mismatches |
| 1 | Fatal — the run did not complete; reason printed |
| 2 | Completed, but something wants your attention (mismatches, deletions) |

**Testing is three things**, and stops there:

1. **The phase 2 deletion path.** The only code path in the tool that destroys data, so a
   bug there deletes photographs. Everything else fails safe — worst case is a re-run.
   Cheap to exercise: flip one byte in a fixture and confirm the file is tombstoned and
   quarantined rather than silently dropped or wrongly kept.
2. **The naming function.** Pure, trivially testable, and it decides where irreplaceable
   files land: UTC date foldering, the `HHMMZ` prefix, and skip-on-identical-hash.
3. **One end-to-end happy path** against real CR3 fixtures — two synthetic cards, four
   temporary destinations, four identical trees out.

Everything else is deliberately untested. This is a personal tool with one user, one rig
and a recoverable failure mode; the RawGeotag testing standard would be a poor trade here.

### 19. There is no sampled verification anywhere

Every bit is checked, on every run and on every `verify`. These are the most emotionally
valuable files this archive holds, and the run happens unattended — the entire point is
that the result needs no interpretation. A full re-verify of a multi-terabyte archive takes
on the order of an hour, an acceptable price for a check performed occasionally and trusted
completely.

Transitivity is what makes the guarantee whole. Phase 1 proves every destination equals the
CFexpress hash; phase 2 proves the SDXC copy equals that same hash. Both holding means every
destination is proven identical to **both** cards. Note the timing: that full two-source
property completes at the end of phase 2, not at LANDED, where all four are proven equal to
the CFexpress copy alone.

### 20. `verify` and `sync` are standalone and config-free

Both take a destination *path* rather than a config label, because an archive pulled from
the safe has to be checkable on a machine that has never seen this tool's configuration.

**`photoday verify <DEST>`** reads the destination marker to name what it is checking, then
walks every date folder: manifest checksum first, so a rotted manifest is reported as a
rotted manifest rather than as damaged photographs; then every raw re-hashed unbuffered
against it. Tombstones are honoured, so a file deliberately deleted in phase 2 reports clean
rather than missing. XMP drift is ignored on a `working` destination and should not exist on
an `archive` one.

**`photoday sync <DEST>`** backfills a destination that missed an offload — the SSD that was
in a drawer during the lunchtime ingest. It copies from the laptop's working copy, since the
cards are long since reformatted, and verifies what it writes exactly as phase 1 does. **It
never deletes**, so it cannot be used to make a destination match by removing files from it.

## Considered and rejected

Recorded so a later reviewer does not spend effort re-proposing them. Reopening one needs
new evidence rather than fresh taste.

| Proposal | Why not |
|---|---|
| Local-time date folders | UTC is a deliberate conviction, not an oversight. The early-morning-east-of-UTC consequence is understood and accepted |
| Verifying immediately after each write | Reads out of the OS page cache, then out of the SSD's own DRAM cache — proves nothing about what is on the disk. Replaced by the lagged verify, decision 2 |
| Reading both cards before writing anything | Puts the ~11.6-minute SDXC read on the critical path for a guarantee that can be delivered after it without being weakened. Decision 1 |
| Skipping a file whose two source copies disagree | Leaves the one file known to have a problem in *zero* backups — the exact inversion of the goal. Decision 3 |
| A `_NNN` suffix assigned per offload batch, coordinated across destinations | Superseded by decision 5. Timestamp-prefixed names are a pure function of the photo, so no coordination is needed and collisions are pathological |
| A timestamp prefix replacing the camera's filename | Unnecessary — prefixing rather than replacing keeps both the original name and shooting order. Decision 5 |
| A content heuristic for `--force-xmp` (refuse when the XMP carries `crs:` properties) | A destructive flag that sometimes declines is worse than one that is honest. Decision 16's role scoping is explicit targeting, not a guess at intent |
| A flag to overwrite raw files, or to bypass pre-flight | No case where either is correct; better as structural impossibilities. Decision 16 |
| Per-device queue depth and overlapped I/O | One buffer-fed blocking writer idles ~1 ms per 45 MB write. Not worth the machinery on speculation — revisit if measurement shows a device going idle. Decision 15 |
| Scaling `--jobs` to the I/O fan-out | Threads do not create bandwidth. Decision 15 |
| Graceful handling of a destination unplugged mid-run | Crash safety is already structural, so the cost exceeds the benefit. Decision 18 |
| Go for the pipeline, shelling out to `rawgeotag.exe` for phase 3 | Defensible, and rejected on the validated CR3, GPX and XMP assets. Decision 17 |
| Carrying RawGeotag's `TESTING.md` whole | A stricter regime than decision 18 calls for; its one load-bearing principle is folded into `REVIEWING.md` |

## Non-goals

- **Touching the cards.** The tool reads them and nothing else — never writes, never
  deletes, never formats. Reformatting stays a deliberate in-camera step at the start of
  each shooting day, which is also what guarantees a card equals a day.
- Modifying raw files. All derived data goes to sidecars and manifests.
- Managing the Lightroom catalog, including renaming historical files.
- Cloud or offsite replication.

## Open questions

None outstanding — the design is complete enough to build from.

What remains is implementation:

- the cargo workspace, with RawGeotag's GPX and XMP engine lifted into a library crate
- the phase 1 pipeline and the Windows storage-identity layer
- deleting the temporary `Cargo.toml` guard from `.github/workflows/ci.yml`
