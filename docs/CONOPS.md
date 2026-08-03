# Concept of operations

*How this tool is **intended** to be used. [`DESIGN.md`](DESIGN.md) records how it works
and why; this file records the operating rhythm it was built to serve. If the two ever
disagree, that is a defect in one of them — fix it, per
[`WRITING.md`](WRITING.md)'s one-canonical-place rule.*

The operator is one photographer, on travel, running this at the end of a shooting day —
tired, in a hotel room or cruise cabin, with dinner waiting. Every design decision that
matters traces back to that person.

## The nightly ritual

1. Load both camera cards into the readers on the Thunderbolt hub. Confirm the three
   archive SSDs are plugged into it.
2. Run **`photoday`**.
3. Read the pre-flight summary — file count, gigabytes, four destinations confirmed
   distinct, estimated time. That one line is what earns walking away.
4. Go to dinner.
5. Back at the desk, read the **last line of the report first**. It is the verdict, and
   it is the only place its phrases ever appear:

| The last line says | You do |
|---|---|
| `EJECTED — SAFE TO STORE` | Pull the SSDs — they are already ejected — put them in the safe, go to bed. |
| `SAFE, NOT EJECTED — ENSURE SDXC IS INSERTED AND RE-RUN` | Raws are safe on all four copies; certainty work remains. Do what it says. |
| `SAFE TO STORE — EJECT <X> BY HAND` | Everything is done; one volume would not release. Eject it from the tray and store. |
| `NOT SAFE — …` | Something did not finish. Eject nothing; run `photoday` again and it continues where it stopped. |

The physical state carries the meaning: **an SSD this tool has ejected is a claim that
every file from both cards is accounted for, verified, on that disk.** A still-mounted
SSD means work remains, and the report names it.

Everything above the verdict is detail — mismatches, geotag counts, the throughput
numbers. Read it with the other eye, or in the morning.

## The shooting-day contract

The tool's guarantees rest on habits it cannot enforce. They are the operator's half of
the deal:

- **The fleet is one body — a Canon EOS R5.** The tool leans on that: CR3 is the only
  format, every frame records its timezone offset, and both slots receive every shot. A
  new or replacement body is a **design event**, not a config change — its EXIF and
  dual-slot behavior get verified at home before any trip trusts it
  ([`DESIGN.md`](DESIGN.md) decision 23).
- **Both cards are formatted in-camera at the start of each shooting day.** This is what
  makes a card equal a day, which is what makes pre-flight's estimate exact and the
  file-set resume check trustworthy.
- **The camera writes every frame to both slots** (CFexpress + SDXC), uncompressed —
  and both cards come to the readers at every offload. Two authoritative sources is
  the standing assumption, and the camera has two slots for exactly this reason. A
  run that finds only one card refuses to start: that is an equipment failure, not a
  mode ([`DESIGN.md`](DESIGN.md) decision 7). The deliberate exception is below,
  under *When a card is truly gone*.
- **The GPS logger runs all day**, and its tracks land in the configured GPX directory
  before the evening run (or are pointed at with `--gpx`).
- **The camera runs on UTC, and its clock is right** — checked at trip start and after
  every zone crossing. The two halves fail differently. A camera left on some other
  *timezone* is harmless — the classic being London time with DST on, producing BST
  frames: the recorded offset lets every date self-correct into true UTC, and the
  report flags the deviation. A *clock* that is wrong as an instant cannot be detected
  from metadata at all — it shifts every date folder and every geotag uniformly while
  looking perfectly normal. The report's systematic-miss heuristic may catch it after
  the fact; thirty seconds with the camera menu is the real defense.

## One application at a time

The trip rhythm keeps Lightroom and this tool from ever touching the same files in the
same season:

| When | What runs |
|---|---|
| Before a trip, at home | Lightroom only — plus the pre-trip checklist below |
| During a trip | **this tool only** — Lightroom is never run on travel |
| Home again | Lightroom — import from the laptop copy, then edit |

Trips are content generation; editing happens at home. The consequence worth stating
plainly: **during a trip, every XMP on every copy is tool-written**, so all four copies
of a day are interchangeable while traveling. The laptop copy's divergence
([`DESIGN.md`](DESIGN.md) decision 11) begins only when Lightroom starts editing at
home — nothing this tool does mid-trip can step on an edit, because no edits exist yet.

## Offloading more than once a day

Run the same bare `photoday` at lunch, again in the evening, as often as anxiety
suggests. Every run is a convergence pass: work already done is recognized and skipped,
new files are ingested, and nothing is ever duplicated — a photo already in the archive
is matched by content hash, not filename, so re-offloading a card is always safe.

## When something goes wrong

One rule covers nearly everything: **plug in whatever is missing and run `photoday`
again.** Runs converge — each one finishes whatever the last one could not, and the
SSDs eject the moment nothing remains.

| What happened | What to do |
|---|---|
| Run crashed mid-copy | `photoday` again. It resumes at the first unfinished file. |
| Forgot to plug in a card | Pre-flight refuses in the first ten seconds, before anything is written. Plug it in, `photoday` again. |
| CFexpress filled or failed mid-day — some frames exist only on the SDXC | Nothing special — phase 2 finds them, ingests them with full verification, and the report names the card that missed them. Nothing ejects until they are on all four copies. |
| Laptop slept / power died | Same as a crash. The archives cannot be left half-written — a partial file never carries a real name. |
| An SSD stayed in the safe last night | `photoday sync <that disk>` backfills it from the laptop copy. |
| Cards already reformatted before corroboration finished | Nothing to recover — the run closes out on its own at the next offload and the report says which files stayed uncorroborated. They were still verified on all four copies. |

Fatal errors are deliberate ([`DESIGN.md`](DESIGN.md) decision 18): the tool stops and
says why rather than improvising. The recovery is always the same re-run.

### When a card is truly gone

Lost, dead, left in a hotel five towns back — the run can still happen, but only by
saying so:

```
photoday --allow-single-source
```

The surviving card becomes the sole source of truth, and which card survived makes no
difference — CFexpress or SDXC, the situation is equally bad. **Phase 2 never runs**,
because corroboration is a comparison and there is no second source to compare
against. The day is recorded as never corroborated, the verdict says so in words, and
the SSDs still eject once every file from the surviving card is verified on all four
copies ([`DESIGN.md`](DESIGN.md) decision 7).

Two boundaries keep the flag honest:

- **It is for a card that is *gone*, not one that is elsewhere.** If the second card
  exists, plug it in instead — and if a "gone" card turns up later, run `photoday`
  again with both cards in; corroboration completes after the fact.
- **A resume that only needs to finish corroboration is not a single-source run** and
  needs no flag — that night had two sources, and the tool tells the two situations
  apart on its own ([`DESIGN.md`](DESIGN.md) decisions 7 and 13).

## Before a trip, at home

The full checklist lives in [`UPDATING.md`](UPDATING.md); the shape of it:

- Update dependencies and toolchain **before leaving, never on the road** — travel with
  a verified binary rather than a current one.
- **`photoday --dry-run` against the real rig** — both readers, all three SSDs. This is
  the rehearsal that catches a reformatted drive, a changed reader, or a stale config
  entry while the fix is a walk to a drawer rather than a ruined evening.
- If Lightroom Classic had a major release since the last trip, run the XMP checks noted
  there.

## After a trip

Lightroom ingests from the **laptop copy** (`C:\Travel\Images`) — never from an archive
SSD. The laptop copy is the working copy; Lightroom will rewrite its XMP sidecars as
editing happens, and that divergence is expected and harmless. The three archive SSDs
stay in the safe, byte-stable, untouched by the catalog.

## Years later

Any archive SSD can prove itself on any machine, with no config and no memory of this
setup: **`photoday verify <path>`** re-hashes every raw against the manifests the disk
carries. Deliberately deleted files are tombstoned, so a clean disk reports *clean* —
not a mystery gap. Every bit is checked, every time; there is no sampled mode.

## What this tool will never do

The full list is [`DESIGN.md`](DESIGN.md)'s non-goals; the two that shape daily use:
it **never writes to a camera card** — formatting stays an in-camera act — and it
**never modifies a raw file**, anywhere, under any flag.
