# End-of-day photo offload — design

Status: **design complete, not yet implemented.** Decisions are numbered, each recorded
with its reasoning, plus what was considered and rejected; what remains to build is
listed at the end.

## The goal, in one sentence

Get back to the hotel, plug two card readers and three external SSDs into a Thunderbolt
hub, run one command, go to dinner, and come back to four verified copies of the day's
photos — so an SSD can go in the safe without anxiety.

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

| Copy | Role | Notes |
|---|---|---|
| `C:\` on the laptop | working | Lightroom edits here; its sidecars will diverge |
| External SSD × 3 | archive | Goes in the safe; expected to stay byte-stable |

Plus a JSON manifest per destination making each copy self-describing.

**Date folders are derived from the UTC capture time.** Deliberate: no timezone logic
anywhere, monotonic across a trip, and unambiguous when crossing the date line. The
accepted consequence is that a shot taken early in the morning east of UTC files under
the previous day.

## Architecture: five phases

| Phase | Mandatory? | Work | Ends with |
|---|---|---|---|
| **1 · Pre-flight: camera card contents** | **Always** — enumeration is where **N** comes from; a second card adds the match, not the walk (decisions 27, 7) | Find the cards, speed-test to pick the ingest source, walk each CR3 listing — and with two cards, assert they agree name for name and size for size | One file set, and **N** |
| **2 · Pre-flight: destinations and GPX** | Always — `--without` and `--no-gpx` narrow what it asserts, never skip it (decisions 25, 26, 16) | Resolve destinations by serial, assert four distinct, writable, capacity ≥ N; parse GPX; inhibit sleep; sweep orphaned temps; print the ETA | You can walk away |
| **3 · Ingest & verify** | **Always — this phase is the product** | Read CFexpress once → SHA256 + EXIF → fan out to 4 → unbuffered read-back verify per destination | **LANDED** |
| **4 · Corroborate** | Two cards only — a single-source run has nothing to compare (decision 7) | Read SDXC fully, compare hashes, delete + tombstone mismatches | Card health report |
| **5 · Geotag** | Only with tracks (decision 26), and only frames a track brackets within limits (decision 16) | Correlate stashed capture times to GPX, write sidecars to all 4 | Lightroom-ready |

**Phases 1 and 2 are both pre-flight**, and together they are the ten seconds that decide
whether you can leave. They are split because they answer different questions and fail
differently: phase 1 establishes *what tonight is* — walking the camera cards, which is
where the file set and **N** come from, and with two cards proving both hold that one set
— while phase 2 checks whether the rig can take it. Both always run; what a second card
adds to phase 1 is the match, not the walk. The order is forced rather than chosen: **N**
is phase 1's output and phase 2's input, since a capacity assertion needs a number to
compare against. It also puts the fatal that means *equipment failure* ahead of the ones
that merely mean *go fetch something*.

**Phases 4 and 5 do not wait for LANDED.** The moment the CFexpress reader goes idle —
the end of phase 3's write feed, since backpressure keeps the reader busy until roughly
the last write drains — both have what they need: the SDXC read begins, and with every
capture time already stashed and the GPX parsed since pre-flight, sidecar writing does
too. Both overlap phase 3's verify pass, and they do not contend with each other either —
one reads the SD card, the other writes a few thousand 3 KB files. All of it serves the
secondary metric; decision 2 explains why the two-pass verify is what makes the early
start possible.

### Where the wall clock goes

Let **N** = one copy of the day's raws. A big day is N ≈ 188 GB; a normal one N ≈ 50 GB.

Phase 3 moves **9N**: 1N read from CFexpress, 4N written, 4N read back to verify. Only
**7N crosses the Thunderbolt hub**, because the laptop's copy is internal.

| Link | Realistic sustained | Time at N = 188 GB |
|---|---|---|
| CFexpress read (1N) | 1,000–1,700 MB/s | 2–3 min |
| TB4 hub aggregate (7N) | ~3 GB/s usable | ~7 min |
| Each SSD, write + verify (2N) | 400–800 MB/s sustained | **8–16 min** ← binds |
| Laptop NVMe, write + verify (2N) | 2,000+ MB/s | ~3 min |

These run concurrently, so the floor is the maximum, not the sum: **~2–4 min for a
50 GB day, ~8–16 min for a 188 GB day**, bound by the slowest SSD's *sustained* write
once its SLC cache is exhausted — which a continuous 188 GB write will certainly
exhaust.

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
Phase 3 computes it before writing anyway, so the check is free.

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

### 7. Cards are identified by measurement, and there are always two

An in-camera format at the start of every shooting session assigns a new volume serial,
so a card's volume GUID changes at least daily; and cheap readers report generic or
empty hardware serials, so the reader is not reliably identifiable either.

So pre-flight finds removable volumes containing `DCIM`, reads ~64 MB from each, and uses
the faster one as the phase 3 source. CFexpress lands near 65 ms against UHS-II's 240 ms
— unambiguous. Costs two seconds, needs no configuration, survives buying a new reader,
and is correct by construction: phase 3 always runs off the fast card regardless of which
reader is in which port. A config override exists for the day that surprises us.

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
photoday
```

```
photoday                            the nightly command

  --dry-run                  plan the entire run and write nothing — names every
                             output file exactly, in seconds
  --jobs <N>                 CPU pool for hashing/EXIF/XMP [default: logical CPUs]
  --fail-on-source-mismatch  abort rather than warn when the two cards disagree
  --allow-single-source      proceed when only one card is present — it becomes
                             the sole source of truth; corroboration is waived
  --without <LABEL>          run without a named archive destination — pre-flight
                             otherwise refuses when one is missing; repeatable;
                             sync the disk when it returns
  --gpx <PATH>               override when tracks aren't in the usual place
  --no-gpx                   proceed with no tracks at all — raws land as normal,
                             no sidecars are written; pre-flight otherwise refuses
                             when the GPX directory holds none
  --max-gap-seconds <S>      refuse to interpolate across a longer hole [default: 60]
  --max-gap-meters <M>       refuse to interpolate across a wider hole [default: 100]
  --force-xmp[=<DEST>]       overwrite existing XMP; archives only unless a
                             destination is named
  --no-eject                 leave the archive SSDs mounted when the run ends

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
  "gpx_dir": "C:\\Travel\\GPX"
}
```

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

### 11. The laptop copy is a working copy — after the trip

Lightroom is never run on travel: trips are content generation, editing happens at home
(see `CONOPS.md`, *One application at a time*). During a trip, every XMP on every copy is
tool-written and all four copies of a day are interchangeable.

The divergence begins at home, when Lightroom imports from the laptop copy and editing
starts: from then on its sidecars are Lightroom's, its state is *expected* to drift from
the archives', and `verify` treats sidecar drift as normal on a `working` destination
while it should not exist at all on an `archive` one.

### 12. The manifest covers raw files only

Raws and sidecars have opposite natures. A raw file is immutable — hash it once, and any
later deviation is corruption. A sidecar is *supposed* to change: Lightroom rewrites it
the moment develop settings are touched, and a re-run of phase 5 against a better track
would rewrite it too.

If `verify` hashed both the same way, it would report the archives as damaged the first
time a photo was edited, and its output would become something to ignore — which
quietly destroys the only thing the tool exists to provide. **A verification tool whose
warnings you learn to ignore is worse than one that checks less and means it.** So
sidecars are treated as regenerable derived data and are not covered.

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
      "name": "1422Z_50A0001.CR3",
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
from *this manifest is damaged, your photos are probably fine*. The three archives each
carry their own manifest of the same day, so they also cross-check against each other.

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

Phase 3 is the product and the rest is gravy, so **only phase 3 may change the verdict.**
A geotag miss or a track that didn't cover the evening walk is a count in the body, never
a downgrade at the top — otherwise you learn to read past the verdict line, which is the
same failure the raws-only manifest exists to avoid.

**LANDED is announced when it happens**, not only in the final summary, because phases 4
and 5 run on afterward and someone walking in during them should see the thing they care
about already settled.

```
═══ 2026-08-03 · LANDED 18:26:16 · phase 3 took 4m 12s ═══

  1,247 files · 56.1 GB · read from CFexpress

  laptop  C:\Travel\Images        1,247 written · 1,247 verified   OK
  SSD-A   Samsung T9   S5H9NS…    1,247 written · 1,247 verified   OK
  SSD-B   Samsung T9   S5H9NT…    1,247 written · 1,247 verified   OK
  SSD-C   SanDisk E61  2312A9…    1,247 written · 1,247 verified   OK

  Corroboration   1,246 matched · 1 mismatch
  Timezone        1,247 files +00:00 — camera on UTC as intended
  Geotag          1,198 tagged · 49 outside track
  Eject           SSD-A ✓ · SSD-B ✓ · SSD-C ✓

  !  1 file deleted from all four copies — source mismatch
     1611Z_50A2087.CR3 → _runs\2026-08-03T18-22-04\quarantine\

  I/O          bytes        rate
    read      336.6 GB    1,113 MB/s
    written   224.4 GB      890 MB/s
    total     561.0 GB     1.65 GB/s        10.0× amplification

  Per destination        moved     sustained
    laptop  C:\        112.2 GB   1,842 MB/s
    SSD-A   Samsung    112.2 GB     731 MB/s
    SSD-B   Samsung    112.2 GB     724 MB/s
    SSD-C   SanDisk    112.2 GB     445 MB/s   ← set the pace

  Phase              wall     I/O
    1  cards         0:02        —
    2  destinations  0:02        —
    3  ingest        4:12   504.9 GB
    4  corroborate   3:31    56.1 GB   ⎫ overlapped 3's verify
    5  geotag        0:12     0.0 GB   ⎭ pass, and each other
    total            5:41   561.0 GB

  ►  EJECTED — SAFE TO STORE
```

Serials on every destination line so a glance confirms four genuinely distinct disks. The
verdict is the last line and that phrase appears nowhere else, so it cannot be confused
with anything above it. Its forms:

| Condition | Verdict |
|---|---|
| Phase 3 verified everywhere, all ejects clean | `EJECTED — SAFE TO STORE` |
| Phase 3 verified everywhere, an eject refused | `SAFE TO STORE — EJECT SSD-B BY HAND (volume in use)` |
| Run under `--no-eject`, everything else complete | `SAFE TO STORE — STILL MOUNTED (--no-eject)` |
| Phase 3 verified everywhere, corroboration incomplete | `SAFE, NOT EJECTED — ENSURE SDXC IS INSERTED AND RE-RUN` |
| Anything unverified anywhere | `NOT SAFE — 12 files unverified on SSD-C` |
| Phase 3 clean, mismatches far above baseline | append `— BUT CHECK YOUR SDXC CARD (47 mismatches)` |
| Run under `--allow-single-source`, phase 3 verified everywhere | append `— SINGLE SOURCE, NEVER CORROBORATED` |
| Run under `--without`, phase 3 verified on the rest | append `— SSD-C EXCLUDED, SYNC IT ON RETURN` |

Eject can modulate the safe verdict's wording; it can never turn SAFE into NOT SAFE.

Because writes are write-through and verify reads unbuffered, the rates are real device
throughput rather than page-cache artifacts — which is what makes the per-destination
line a usable diagnostic. The slowest destination sets the pace of the whole run, so
naming it is the single most useful number for spotting a disk going bad. With a
`_runs\` directory accumulating across a trip, a later comparison against a
destination's own rolling average is the natural extension.

`_runs\<timestamp>\report.json` carries the full forensic record: per-file outcomes,
per-phase timings, per-destination throughput, and the resolved hardware identities.

### 15. `--jobs` sizes the CPU pool, not the I/O fan-out

`--jobs N`, defaulting to logical CPU count, following RawGeotag's finding that this
problem class parallelizes well into double-digit thread counts.

**It governs the CPU-bound work**, where that finding applies directly. Phase 3 hashes 5N
— one source read plus four verify reads, 280 GB on a 56 GB day. At roughly 2 GB/s per
core with SHA-NI that is ~140 s single-threaded against a 252 s phase, close enough to
bind the run on faster storage. Spread across cores it disappears. EXIF extraction and XMP
generation ride the same pool.

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
untagged and skips the rest (decision 13's convergence); `sync` writes a sidecar only
where none exists (decision 20). One invariant, one door through it.

Two consequences of scoping it that way:

- **No flag overwrites a raw file.** There is no case where it is correct — identical
  content is skipped by hash, different content takes a `_001` suffix. Better as a
  structural impossibility than as a flag nobody should reach for.
- **No flag bypasses pre-flight.** Running against fewer destinations is an explicit
  selection (`--without`, decision 25), not an override — it narrows what pre-flight
  asserts, never skips it. Pre-flight exists to fail while you are still at the desk.

Sidecars on the archives are only ever written by this tool, so forcing them is harmless.
Sidecars on the laptop are written by Lightroom and hold develop settings that exist
nowhere else. So `--force-xmp` covers archive destinations, and touching the working
copy requires naming it: `--force-xmp=laptop`.

### 17. Rust, with RawGeotag's engine as a workspace library

Two independent reasons, and they point the same way.

**The hashing is real compute.** Decision 15 sizes it: 5N through SHA-256, which on a
188 GB day is 940 GB — ~7.8 minutes single-threaded at the same ~2 GB/s per core, about
half of an 8–16 minute phase on its own. It has to spread across cores, which rules out
anything with a global interpreter lock and rewards a language with real threads and no
runtime overhead.

**The crate is `sha2`, and the acceleration is automatic.** The machine is not
hypothetical — this tool runs on the travel laptop's i7-13700H, whose P-cores and
E-cores all carry the SHA extensions — and RustCrypto's ubiquitous `sha2` selects its
SHA-NI backend at runtime through `cpufeatures`: no build flags, no `target-cpu`
pinning, no per-machine binary to get wrong before a trip. That pairing is what grounds
decision 15's ~2 GB/s per core. And it is one algorithm everywhere: the manifest's
self-checksum (decision 12) is the same SHA-256, so a second hash function never needs
choosing, validating, or explaining.

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

**Testing is three things**, and stops there:

1. **The phase 4 deletion path.** The only code path in the tool that destroys data, so a
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

Transitivity is what makes the guarantee whole. Phase 3 proves every destination equals
the source card's hash; phase 4 proves the other card's copy equals that same hash. Both
holding means every destination is proven identical to **both** cards — and decision 27
asserted at launch that the two cards hold the same files, so in a gated run that is
every file. A file proven against one card alone is exactly what its manifest verdict
records: `waived` (decision 7) or `forfeited` (decision 13). Note the timing: the full
two-source property completes at the end of phase 4, not at LANDED, where all four are
proven equal to the source card alone.

### 20. `verify` and `sync` are standalone; only `verify` is config-free

Both take a destination *path* rather than a config label. For `verify` the reason is
absolute: an archive pulled from the safe has to be checkable on a machine that has
never seen this tool's configuration, and it is — `verify` reads nothing but the
destination itself, its marker and its manifests. `sync` does not share that property
and should not pretend to: it needs a copy source (the laptop working copy) and GPX
tracks for regeneration, and it takes both from the config — fine, because backfilling
a disk that missed an offload happens on the machine that ran the offload.

**`photoday verify <DEST>`** reads the destination marker to name what it is checking,
then walks every date folder, `_unfiled` included (decision 21): manifest checksum first,
so a rotted manifest is reported as a rotted manifest rather than as damaged
photographs; then every raw re-hashed unbuffered
against it. Tombstones are honoured, so a file deliberately deleted in phase 4 reports clean
rather than missing. XMP drift is ignored on a `working` destination and should not exist on
an `archive` one.

**`photoday sync <DEST>`** backfills a destination that missed an offload — the SSD that
sat in a drawer while a `--without` run went on without it (decision 25). It copies from
the laptop's working copy, since the cards are long since reformatted, and verifies what
it writes exactly as phase 3 does. **It never deletes**, so it cannot be used to make a
destination match by removing files from it.

**Sync leaves behind the same manifests a run would have.** Every date folder it touches
gets its manifest written or updated by the same atomic mechanism (decision 12): sync
records itself in the `runs` array, stamps its own `verified_utc` as each read-back
completes, and carries the photo-facts — hash, capture time, source card, corroboration
verdict — unchanged from the laptop's manifest, tombstones included, so a deliberately
deleted file stays explained even on a disk that never held it. This is not optional
bookkeeping: `verify` reads nothing but the disk, so a disk sync built must be as
self-describing as one the nightly run built — and carrying the photo-facts unchanged is
what keeps the three archives' manifests cross-checking after one of them is rebuilt.
The laptop's manifest also supplies the canonical hash sync verifies against, which cuts
both ways: a laptop file whose in-flight hash no longer matches its own manifest is
working-copy rot, and sync refuses to propagate it — the file is named, skipped, and
left for recovery from an archive rather than written over a good copy's future.

**`_unfiled` is inside sync's walk like any date folder** (decision 21): its raws are
copied and read-back verified, its manifest carried, and no sidecar question arises — a
file is in `_unfiled` precisely because it has no capture time to correlate. Skipping it
would be the quiet failure: a disk that missed an offload would come back "current"
while permanently missing a raw, and `verify` could never notice — it reads nothing but
the disk, and a folder that was never written checks clean.

**Sync copies raws and regenerates XMP sidecars** from the manifest's capture times and
the GPX tracks, exactly as phase 5 does — it never copies a sidecar, and it writes one
only where none exists. Sync does not accept `--force-xmp`: decision 16's invariant has
one door, and it is on the nightly command.

Both halves of that rule were settled at design review, and each blocks its own
data-loss path. *Regenerate rather than copy*: during a trip the laptop's sidecars are
all tool-written and copying them would be harmless (decision 11), but at home they are
Lightroom's, carrying develop settings that must not leak onto an archive. *Write only
where none exists*: pointed at the laptop copy at home, a regenerate-all sync would
overwrite those same settings — the one data loss this tool could cause outside the
deletion path. Together they are correct in both regimes without anyone having to
remember which one they are in.

Regeneration also completes phase 5 recovery: sidecars missing on any destination, for
any reason — a crash, a `--no-gpx` night (decision 26) — are rebuilt by sync with no
dedicated machinery. Pointed at the laptop copy for that purpose, sync's copy step
simply finds nothing to do and regeneration is all that runs.

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
any date folder, so `verify` covers it and `sync` carries it (decision 20). The report
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
archives — each `archive` destination is ejected: flush the volume, lock it
(`FSCTL_LOCK_VOLUME`, retried with backoff for ~30 s, since Defender or the indexer may
be holding freshly written files), dismount, then `CM_Request_Device_Eject` so Windows
powers the device down exactly as the tray icon would. The `working` destination is
internal and never touched. The card readers need nothing: the tool never writes to a
card, so pulling one is safe at any time after the run.

A refused eject — something else holds the volume — is named per device and downgrades
nothing, because the data guarantees were settled before eject was attempted. See
decision 14 for how the verdict phrases it.

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

### 25. A destination missing at offload is declared, not configured around

The destination mirror of decision 7, and it closes a real gap: decision 9 refuses to
run unless all four destinations are present, `sync` exists to backfill a disk that
missed an offload — and nothing said how an offload could legitimately happen without
one. As previously written, an SSD that died mid-trip would have blocked every
remaining night.

The default stays refusal, in the first ten seconds, naming the fix:

```
DESTINATION MISSING — SSD-C (SanDisk E61 2312A9…) not connected.
Plug it in, or re-run with --without SSD-C and sync the disk when it returns.
```

Proceeding requires naming what is missing: **`--without <LABEL>`**, taking a config
label, repeatable, archive destinations only. Each absent disk is declared per run, so
a three-copy night can never happen by accident and the cost of the night is visible in
the command itself. The flag narrows what pre-flight asserts — it never weakens an
assertion on what remains, which is validated exactly as decision 9 demands.

Under the flag, every "all four destinations" in this design reads as "every
destination this run was asked to cover." LANDED and the eject gate are scoped the same
way; the verdict carries the scar (decision 14) and the exit code is 2 (decision 18).

The `working` destination cannot be excluded: it is internal, so it cannot be missing —
and it is `sync`'s copy source, the thing that makes exclusion recoverable at all.

Recovery is `sync`, never the next run. The next offload ingests from cards that by
then hold a different session, so the missed session survives only on the laptop copy,
and reading that is `sync`'s charter (decision 20), not the card pipeline's.
Format-after-eject stays safe for the same reason: the ejected disks and the laptop
copy hold everything, and `sync` needs no cards.

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
tags what is untagged (decision 13); after the format, `sync` regenerates them on every
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

## Considered and rejected

Recorded so a later reviewer does not spend effort re-proposing them. Reopening one needs
new evidence rather than fresh taste.

| Proposal | Why not |
|---|---|
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
| A timestamp prefix replacing the camera's filename | Unnecessary — prefixing rather than replacing keeps both the original name and shooting order. Decision 5 |
| A content heuristic for `--force-xmp` (refuse when the XMP carries `crs:` properties) | A destructive flag that sometimes declines is worse than one that is honest. Decision 16's role scoping is explicit targeting, not a guess at intent |
| A flag to overwrite raw files, or to bypass pre-flight | No case where either is correct; better as structural impossibilities. Decision 16 |
| Per-device queue depth and overlapped I/O | One buffer-fed blocking writer idles ~1 ms per 45 MB write. Not worth the machinery on speculation — revisit if measurement shows a device going idle. Decision 15 |
| Scaling `--jobs` to the I/O fan-out | Threads do not create bandwidth. Decision 15 |
| Graceful handling of a destination unplugged mid-run | Crash safety is already structural, so the cost exceeds the benefit. Decision 18 |
| Fatal-out on a file whose EXIF cannot be read | The original decision 18 reading, replaced at design review: fatal does not fail safe there — one corrupt file would cost the whole night's backup while nobody is watching. Decision 21 |
| Go for the pipeline, shelling out to `rawgeotag.exe` for phase 5 | Defensible, and rejected on the validated CR3, GPX and XMP assets. Decision 17 |
| Carrying RawGeotag's `TESTING.md` whole | A stricter regime than decision 18 calls for; its one load-bearing principle is folded into `REVIEWING.md` |

## Non-goals

- **Touching the cards.** The tool reads them and nothing else — never writes, never
  deletes, never formats. Reformatting stays a deliberate in-camera step at the start of
  each shooting session, which is also what guarantees a card equals a session.
- Modifying raw files. All derived data goes to sidecars and manifests.
- Managing the Lightroom catalog, including renaming historical files.
- Cloud or offsite replication.

## Open questions

None outstanding — the design is complete enough to build from.

What remains is implementation:

- the cargo workspace, with RawGeotag's GPX and XMP engine lifted into a library crate
- the phase 3 pipeline and the Windows storage-identity layer
- deleting the temporary `Cargo.toml` guard from `.github/workflows/ci.yml`
