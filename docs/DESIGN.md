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

## Architecture: five phases

| Phase | Mandatory? | Work | Ends with |
|---|---|---|---|
| **1 · Pre-flight: camera card contents** | **Always** — enumeration is where **N** comes from; a second card adds the match, not the walk (decisions 27, 7) | Find the cards, speed-test to pick the ingest source, walk each CR3 listing — and with two cards, assert they agree name for name and size for size | One file set, and **N** |
| **2 · Pre-flight: destinations and GPX** | Always — `--without` and `--no-gpx` narrow what it asserts, never skip it (decisions 25, 26, 16) | Resolve destinations by serial, assert four distinct, writable, capacity ≥ N; parse GPX; inhibit sleep; sweep orphaned temps; print the ETA | You can walk away |
| **3 · Ingest & verify** | **Always — this phase is the product** | Read CFexpress once → SHA256 + EXIF → fan out to 4 → unbuffered read-back verify per destination | **LANDED** |
| **4 · Corroborate** | Two cards only — a single-source run has nothing to compare (decision 7) | Read SDXC fully, compare hashes, delete + tombstone mismatches | Card health report |
| **5 · Geotag** | Only with tracks (decision 26), and only frames a track brackets within limits (decision 16) | Correlate stashed capture times to GPX, write sidecars to all 4 | Ready to edit from |

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

**Measured on the rig, 2026-08-03**, against the real 2022-09-27 shoot — 3,883 frames,
201.3 GB, offloaded to all four destinations and read back:

| Link | Measured | Time at N = 201 GB |
|---|---|---|
| CFexpress read (1N) | 675–757 MB/s | ~5 min |
| SDXC read — phase 4 only (1N) | **62–67 MB/s**, re-measured quiet | **~52 min** |
| OWC, Thunderbolt (2N) | write ~292, read 2,540 | ~13 min |
| SanDisk / WD, 10 Gbps USB (2N) | write ~292, read ~900 | **~15 min** ← binds |
| Laptop NVMe (2N) | write ~292, read 3,044 | ~12 min |

**The whole run took 20 min 27 s**, and every one of the 15,532 `(file, destination)`
pairs verified. **A later run with the CFexpress moved to a Thunderbolt reader took
18 min 06 s**, and the shape of that gain matters more than its size — see below.

> **This table replaces an estimate that was optimistic, and the reason it was wrong is
> worth more than the correction.** It read *"Each SSD, write + verify — 400–800 MB/s
> sustained — 8–16 min ← binds"*, and treated the three archive drives as
> interchangeable. **They are not: the fleet is one Thunderbolt enclosure and two 10 Gbps
> USB drives**, and the USB pair sets the pace for the entire run. No amount of speed on
> the OWC or the laptop's internal NVMe can recover it, which is exactly why decision 14
> prints a per-destination line and calls naming the slowest device the most useful
> number in the report — it identified the heterogeneity without anyone having to
> remember which enclosure was which.
>
> **The SDXC row was measured badly, re-measured properly, and came out the same.** The
> original sweep ran *while a full offload was in flight*, which is not a measurement —
> and the defence offered for it was invalid, since a bandwidth-starved device reads flat
> at every request size just as a genuinely slow one does. Re-measured on an idle bus on
> 2026-08-04: **62–67 MB/s, unchanged**. The methodology was wrong and the number was
> right; those are separate claims and only the first needed retracting.
>
> **The apparent read/write anomaly, however, was entirely an artefact and is now
> retired.** This section previously noted that the card "reads at roughly half what it
> writes, which is backwards for flash". It does not: measured like for like on an idle
> bus, read is 62–67 MB/s and **write is 50 MB/s** — read faster than write, as flash
> should be. The 117 MB/s write figure it had been compared against came from a robocopy
> average taken hours earlier under different load, fill and thermal conditions. Two
> numbers from different worlds are not a comparison, and the "anomaly" was manufactured
> by treating them as one.
>
> **The destinations, by contrast, were measured with nothing else running and do stand.**
> They read far faster than assumed once asked properly — see `winio::VERIFY_CHUNK`, where
> 16 MiB requests beat 1 MiB by a third to a half on every device.
>
> **The general rule this cost us: a throughput number is only a measurement if nothing
> else was using the bus.** Everything in this table was taken on a quiet system except
> that one row, which is why that one row is the only one carrying a warning.
>
> **A 50 GB day scales down cleanly**: roughly 5–6 minutes, bound by the same pair.

> **Measured 2026-08-04: the four USB devices share one ~930 MB/s controller, and that is
> what sets the write pass.** Each device read alone, then all five at once:
>
> | Device | Link | Alone | Together |
> |---|---|---|---|
> | SD reader | USB | 64 MB/s | 61 (96%) |
> | CFexpress reader | USB | 504 MB/s | 291 (58%) |
> | SanDisk | USB | 772 MB/s | 290 (38%) |
> | WD | USB | 735 MB/s | 289 (39%) |
> | **OWC** | **Thunderbolt** | 1,684 MB/s | **2,109 (125%)** |
>
> The three fast USB devices converge on ~290 MB/s each — 870 MB/s between them, 931
> with the SD reader — which is a 10 Gbps USB controller saturating. **The Thunderbolt
> device is untouched by any of it**, because a TB4 hub tunnels PCIe for native
> Thunderbolt devices and puts everything else behind one internal USB host controller.
> The hub here is a CalDigit Element Hub; the OWC enclosure enumerates as its own
> `USB4 Router` and its disk as NVMe, while both card readers and the other two archive
> drives enumerate as USB.
>
> **This explains the write pass exactly.** Phase 3 runs three USB streams — the card
> read plus two USB destination writes — and `930 / 3 ≈ 310`, against 292 MB/s observed.
> It also predicts the fix: **moving the CFexpress to a Thunderbolt reader takes the
> source off the shared controller**, leaving `930 / 2 ≈ 465 MB/s` for the two USB
> destinations, or roughly a 60% faster write pass. Not yet tried.
>
> The lesson generalises past this rig: **"every stream is well under 10 Gbps" does not
> mean the streams are independent.** What matters is which of them share a controller,
> and on a Thunderbolt hub that is not visible from the port labels.
>
> **Confirmed 2026-08-04 by moving the CFexpress to a Thunderbolt reader**, which is the
> experiment the paragraph above predicted. A ProGrade CFexpress reader enumerates the
> card as **NVMe** rather than USB — PCIe tunnelled, not bridged — and taking that one
> stream off the shared controller did exactly what the arithmetic said it would:
>
> | Device | Link | CF on USB | CF on Thunderbolt |
> |---|---|---|---|
> | SD reader | USB | 61 MB/s | 65 |
> | CFexpress | USB → **TB** | 291 | **1,112** |
> | SanDisk | USB | 290 | **417** |
> | WD | USB | 289 | **417** |
> | OWC | TB | 2,109 | 1,333 |
>
> The USB controller still tops out in the same place — 899 MB/s across three devices
> against 931 across four — so each archive SSD gained 44%, from 290 to 417 MB/s. The
> prediction was 465; subtracting the SD reader's 65 that the model had ignored gives
> 432, about 4% out.
>
> **Two findings worth carrying past this rig.** Contention *moved* rather than vanished:
> the OWC halved because it now shares the Thunderbolt fabric with the card reader — but
> that is contention at 1,333 MB/s rather than 290, which is a different class of
> problem. And **the reader was capping the card, not the card the reader**: the same
> card read 507–581 MB/s through a USB bridge and 1,289 through Thunderbolt, against a
> 1,700 MB/s rating.
>
> The card also reports a **real hardware serial** through the Thunderbolt reader
> (`K03ABCXA9TC0627`) where the USB bridge invented `0123456789ABCDEF`. Decision 7's
> "cheap readers report generic serials" is therefore a fact about *readers*, not about
> cards — it does not change the decision, since a card's volume serial still changes at
> every format, but the reasoning should not be read as universal.
>
> **And the shared thing is not a controller at all — it is a tunnel.** From the device
> tree: this machine has exactly two xHCI controllers and both are the laptop's; the hub
> enumerates as a `USB4 Router`, not as a USB controller, with a `Generic SuperSpeed USB
> Hub` behind it. The dock's USB ports are reached by **USB tunnelling over USB4**, and
> USB4 v1 / TB4 tunnels a single USB 3.2 Gen 2x1 connection — **10 Gbps by
> specification** — shared by every USB device on the dock however many ports it has. No
> hub of this generation avoids that; USB4 v2 / TB5 raises the tunnel to 20 Gbps.
>
> The natural objection is that a 16-port gigabit switch has a non-blocking backplane, so
> dividing 10 Gbps among 10 Gbps ports looks like a design failure. It is not: USB has
> **no peer-to-peer path**, every transfer is host↔device, so downstream ports inherently
> share the upstream — and here that upstream is a spec-capped tunnel rather than a
> backplane.
>
> **The full run came in at 18 min 06 s, and the verify pass did not improve — correctly.**
> The card is not read during verification, so the USB pair was *already* only two streams
> sharing the tunnel; moving the CFexpress off USB cannot help a pass it never
> participated in. Only the write pass, running three USB streams, had anything to gain.
> A prediction of 14–15 minutes applied the gain to both halves and was wrong for exactly
> that reason.
>
> **So the two passes have different bottlenecks and must be tuned separately.** Removing
> a stream from the USB tunnel helps only the write pass. Verification needs a wider
> tunnel, or fewer USB destinations.
>
> ⚠ **These contention figures are under suspicion, and the instrument is the suspect.**
> Two Thunderbolt devices measured 2,445 MB/s together while the OWC alone measured
> 2,582 — two devices sharing a fabric summing to *less* than one of them, which cannot
> be right if they are independent and the link has headroom. `examples/contention.rs`
> reads each device with a **single thread** issuing `FILE_FLAG_NO_BUFFERING` requests,
> and that flag disables read-ahead, so nothing is in flight behind each request and one
> thread is latency-bound. The ceiling being measured may be the probe's.
>
> **That is the request-size finding one level up**: there, an apparent device limit was
> really how the request was made, and a bigger request lifted it by a third. Here the
> suspicion is that an apparent *fabric* limit is really how many requests are made at
> once. Until a multi-threaded read shows whether the total climbs, treat the numbers
> above — including "the USB controller saturates at ~900 MB/s" — as lower bounds rather
> than ceilings. The relative comparisons they support are unaffected, since every row
> was taken with the same instrument.

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
  --force-xmp[=<DEST>]       overwrite existing XMP on every destination, or on
                             just the one named
  --no-eject                 leave the archive SSDs mounted when the run ends

photoday verify <DEST>       standalone re-verify; works years later, without config
photoday sync <DEST>         backfill a disk that missed an offload
```

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

**It lives at `%APPDATA%\photoday\config.json`** — settled 2026-08-03, having been shown
here without ever being located. The Windows convention, and the one a Windows developer
looks in first; it survives rebuilding the binary, which a config sitting beside
`target\release\photoday.exe` does not, and that matters because `UPDATING.md`'s pre-trip
ritual is *rebuild, then dry-run against the real rig*. Reading one environment variable
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
- **`sync`'s copy source is the laptop because it is the copy that is always attached**,
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
     1611Z_2087.CR3 → _runs\2026-08-03T18-22-04\quarantine\

  I/O          bytes        rate
    read      336.6 GB    1,113 MB/s
    written   224.4 GB      890 MB/s
    total     561.0 GB     1.65 GB/s        10.0× amplification

  Per destination        moved     sustained
    laptop  C:\        112.2 GB   1,842 MB/s
    SSD-A   Samsung    112.2 GB     731 MB/s
    SSD-B   Samsung    112.2 GB     724 MB/s
    SSD-C   SanDisk    112.2 GB     445 MB/s   ← set the pace

  Phase                                  wall     I/O
    1  pre-flight: camera card contents  0:02        —
    2  pre-flight: destinations and GPX  0:02        —
    3  ingest & verify                   4:12   504.9 GB
    4  corroborate                       3:31    56.1 GB   ⎫ overlapped 3's verify
    5  geotag                            0:12     0.0 GB   ⎭ pass, and each other
    total                                5:41   561.0 GB

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

**XXH3 is the row that settles it, and it is not a candidate** — a non-cryptographic
checksum, the fastest thing anyone could reasonably put here. It buys three minutes on a
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

### 20. `verify` and `sync` are standalone; only `verify` is config-free

Both take a destination *path* rather than a config label. For `verify` the reason is
absolute: an archive pulled from the safe has to be checkable on a machine that has
never seen this tool's configuration, and it is — `verify` reads nothing but the
destination itself, its marker and its manifests. `sync` does not share that property
and should not pretend to: it needs a copy source — any surviving copy — and GPX tracks
for regeneration, and it takes both from the config — fine, because backfilling a disk
that missed an offload happens on the machine that ran the offload.

**`photoday verify <DEST>`** reads the destination marker to name what it is checking —
at whatever schema that disk was written with, which every later build still understands
(decision 28) —
then walks every date folder, `_unfiled` included (decision 21): manifest checksum first,
so a rotted manifest is reported as a rotted manifest rather than as damaged
photographs; then every raw re-hashed unbuffered
against it. Tombstones are honoured, so a file deliberately deleted in phase 4 reports clean
rather than missing.

**Sidecar drift should not exist on any destination**, and after decision 11's correction
`verify` says so about all four rather than excusing one. Nothing edits these copies, so
a sidecar that differs between them means either a phase 5 re-run that did not reach every
copy — backfillable, and worth knowing about — or corruption. Neither is something to
pass over silently.

**`photoday sync <DEST>`** backfills a destination that missed an offload — the SSD that
sat in a drawer while a `--without` run went on without it (decision 25). It copies from
another destination, since the cards are long since reformatted, and verifies what it
writes exactly as phase 3 does. **It never deletes**, so it cannot be used to make a
destination match by removing files from it.

**Any present copy can be the source, and the laptop is merely the convenient default**
— it is the one destination that is always attached. No copy is authoritative over the
others (decision 11), which is what makes this safe: if the laptop's own copy of a file
fails its manifest, sync sources that file from a destination whose copy passes rather
than having nowhere to turn.

**Sync leaves behind the same manifests a run would have.** Every date folder it touches
gets its manifest written or updated by the same atomic mechanism (decision 12): sync
records itself in the `runs` array, stamps its own `verified_utc` as each read-back
completes, and carries the photo-facts — hash, capture time, source card, corroboration
verdict — unchanged from the laptop's manifest, tombstones included, so a deliberately
deleted file stays explained even on a disk that never held it. This is not optional
bookkeeping: `verify` reads nothing but the disk, so a disk sync built must be as
self-describing as one the nightly run built — and carrying the photo-facts unchanged is
what keeps the four copies' manifests cross-checking after one of them is rebuilt.
The source's manifest also supplies the canonical hash sync verifies against, which cuts
both ways: a source file whose in-flight hash no longer matches its own manifest is
rot in the source copy, and sync refuses to propagate it — the file is named, skipped, and
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

> **Both halves were settled at design review to block two data-loss paths that decision
> 11's correction has since closed on their own.** The reasoning was: *regenerate rather
> than copy*, because at home the laptop's sidecars are Lightroom's and carry develop
> settings that must not leak onto an archive; and *write only where none exists*,
> because a regenerate-all sync pointed at the laptop would overwrite those settings.
> **No copy this tool writes is ever edited**, so there are no develop settings anywhere
> to leak or overwrite.
>
> **Both halves stay anyway, and now rest on a plainer reason: a sidecar is derived
> data, and the tool that can derive it should.** Copying a sidecar propagates whatever
> it happens to be; regenerating it from the manifest's capture time and the tracks
> produces the same answer everywhere, which is what keeps four copies matching. And
> writing only where none exists keeps decision 16's invariant intact — one door through
> it, on the nightly command. The conclusion does not move; only the argument for it
> got simpler and truer.

Regeneration also completes phase 5 recovery: sidecars missing on any destination, for
any reason — a crash, a `--no-gpx` night (decision 26) — are rebuilt by sync with no
dedicated machinery. Pointed at a destination for that purpose, sync's copy step simply
finds nothing to do and regeneration is all that runs.

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
archives — each destination resolved by disk serial is ejected: flush the volume, lock it
(`FSCTL_LOCK_VOLUME`, retried with backoff for ~30 s, since Defender or the indexer may
be holding freshly written files), dismount, then `CM_Request_Device_Eject` so Windows
powers the device down exactly as the tray icon would. The destination that is a path on
this machine's own disk has nothing to eject and is never touched — that is the whole of
the distinction, and it is physical rather than a role (decision 11). The card readers need nothing: the tool never writes to a
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

The destination on this machine's own disk cannot be excluded: it is not a device that
can be left behind, so it cannot be missing —
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
to M, use a newer `photoday` — and never reports the photos as damaged. Same reasoning
as decision 12's self-checksum: for a verification tool, a false alarm on irreplaceable
data is its own kind of failure, and *I cannot read this* must never wear the costume of
*your archive is rotting*.

**The number bumps only when an old reader would be wrong, not when it would be
incomplete.** Adding a field that an old `verify` ignores while still checking every hash
correctly is not a bump. Redefining an existing field, removing one, or making a new one
load-bearing for verification is. That is the same compatible-versus-breaking line
`UPDATING.md` already draws around semver, applied to this project's own artifact.

**The photo facts are a stable core that no bump may redefine.** Decision 12 has the
four copies cross-checking each other's manifests, and decision 20 lets `sync`
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
| `indicatif` | 0.18 | `MultiProgress` — one bar per destination |
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
[`UPDATING.md`](UPDATING.md) has the standing order and now names the pre-1.0 entries
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
crates/photoday/      the binary: CLI, five phases, the Windows storage layer
```

A member's own manifest lists only what its code imports today, so a manifest never
claims a dependency nothing uses.

**The lift moved the engine unrewritten, which was the point.** Decision 17 accepts
these three sub-problems as solved on the strength of their validation, so a lift that
took the opportunity to tidy them would have spent exactly what it was trying to save;
the 67 unit tests came across with the code and the two changes made were the minimum
`photoday` could not do without. `raw::capture_time_in_memory` is one — decision 10 has
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
[`UPDATING.md`](UPDATING.md) sends the reader to RawGeotag's `docs/LIGHTROOM-XMP.md`
after a Lightroom major release, because that is where the XMP engine's verification
lives. Moving the engine here without moving that document would leave the pointer
aiming at the verification record of code that no longer lives beside it — a
one-canonical-place violation ([`WRITING.md`](WRITING.md) rule 2) that would surface at
the worst moment, which is the pre-trip check.

> **Corrected while doing the lift (2026-08-03), in both directions.**
>
> **The `thiserror` count above is one, not two.** The capture-time site named in the
> second bullet does not exist: `raw::Capture` is *already* an enum of outcomes —
> `Resolved`, `NeedsOffset`, `NoCaptureTime` — so the distinction decision 21 needs was
> solved in RawGeotag before this design asked for it. The in-memory entry point closes
> the gap the rest of the way: with the bytes already in RAM there is no I/O failure
> left to tell apart from a defective file, so *every* non-`Resolved` outcome routes to
> `_unfiled` and nothing has to branch on an error type. `thiserror` is therefore
> earned by the manifest schema reader (decision 28) alone, and it stays declared but
> unused until that lands. The general lesson is worth more than the correction: a
> design that reasons about code it has not read will invent error types the code
> already made unnecessary.
>
> **And the document cannot move ahead of the binary.** `LIGHTROOM-XMP.md` was copied
> here and then removed, because its procedure is not prose about the engine — it
> *drives* `rawgeotag.exe`, staging real frames through it and diffing the sidecars
> against Lightroom's. `photoday` cannot write a sidecar yet, so the document's first
> instruction would be unrunnable in the repository holding it, which is precisely the
> failure [`WRITING.md`](WRITING.md) opens with. The pointer in `UPDATING.md` stays
> aimed at RawGeotag, and the move happens when phase 5 can execute the procedure.
>
> **So the lift leaves a deliberate duplication, and it is not resolvable from here.**
> RawGeotag was not modified: it keeps its own copy of these four modules and still
> builds and runs exactly as before. Until it takes a path dependency on `geotag` or is
> retired, the engine exists twice and a fix applied to one copy does not reach the
> other. That is a change in *another repository*, and it is the maintainer's call
> rather than something this decision may assume. **It was made the same day:
> decision 30 retires RawGeotag rather than making it a consumer, and cannot start
> until phase 5 works.**

### 30. RawGeotag retires into `photoday`

The lift of decision 29 left the engine in two repositories: `crates/geotag` here, and
the original four modules in RawGeotag, which was deliberately not modified and still
builds and runs. A fix applied to one does not reach the other, and that window stays
open for as long as both exist.

**The resolution is retirement, not a path dependency.** RawGeotag's CLI is a strict
subset of what `photoday` will do once phase 5 lands — correlate capture times against a
GPX index and write XMP sidecars is *the whole of* phase 5 — so the tool becomes a
subcommand, `photoday geotag`, and its repository is archived. A path dependency was the
alternative and is the weaker end state: it would keep one canonical engine but leave a
second binary, a second CLI, a second set of docs and a second CI to maintain, all for a
capability the primary tool will have anyway.

**Nothing happens until phase 5 exists.** RawGeotag works today and geotags real trips;
it keeps working until its replacement is real and has been run against the fixture
corpus. Retiring it before then would trade a working tool for a promise.

What comes across at that point, beyond the CLI surface itself:

- `docs/LIGHTROOM-XMP.md`, which decision 29 could not move because its procedure drives
  `rawgeotag.exe` — once `photoday geotag` is that binary, the procedure moves with it
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
| `assert_cmd` and `predicates` for the process-level test | RawGeotag's precedent: `env!("CARGO_BIN_EXE_photoday")` and `std::process::Command` are enough, and decision 18 asks for four tests in total |
| `criterion` for decision 17's hash measurement | It is a single sustained throughput figure over 2 GiB, not a microbenchmark needing statistical machinery to see. `examples/hash-rate.rs` reproduces the table in thirty lines and needs no harness |

## Non-goals

- **Touching the cards.** The tool reads them and nothing else — never writes, never
  deletes, never formats. Reformatting stays a deliberate in-camera step at the start of
  each shooting session, which is also what guarantees a card equals a session.
- Modifying raw files. All derived data goes to sidecars and manifests.
- Managing the Lightroom catalog, including renaming historical files.
- Cloud or offsite replication.

## Open questions

None outstanding — the design is complete enough to build from.

What remains is implementation. The cargo workspace and the dependency set landed with
decision 29, along with the CLI surface of decision 8 and `examples/hash-rate.rs`; the
`Cargo.toml` guard is out of `.github/workflows/ci.yml`, so the three checks now run for
real. The engine is lifted: `crates/geotag` holds capture time, GPX indexing and XMP
rendering, with the tests that validate them. Phase 3 is built and tested over ordinary
directories — reader, per-destination fan-out with backpressure, write-through, the
unbuffered verify pass, and the append-only run log — along with decision 5's naming and
decision 21's `_unfiled` routing. Still to build:

- phases 1 and 2 on top of the storage layer: the card speed test, the capacity and
  distinctness assertions, the config loader, the Defender check, and the destination
  marker of decision 6
- eject (decision 22) — `FSCTL_LOCK_VOLUME` with its backoff, dismount, and
  `CM_Request_Device_Eject`; deliberately not built alongside the read-only queries,
  since it is the one part of this layer that dismounts a live volume
- wiring phase 3 to the CLI, which needs the config loader and therefore the above; the
  binary still parses your command and exits 1
- phases 4 and 5, the manifest of decision 12, and the report of decision 14
- retiring RawGeotag into `photoday geotag` and archiving that repository (decision 30),
  which cannot start until phase 5 can do its job

**Two gaps in phase 3 are recorded rather than closed.** Neither is a defect today and
both have an owner. *No test can prove the two file flags are still set*: removing
`FILE_FLAG_NO_BUFFERING` changes where bytes come from, not what they are, so every
assertion still passes — the constants are load-bearing on inspection only, and the
comment in `winio.rs` says so where someone might delete one. And *a file that rots
after a clean run* is not phase 3's to notice; it is exactly what `photoday verify`
exists for (decision 20), whose own test against a committed schema-1 manifest is
decision 28's fourth.
