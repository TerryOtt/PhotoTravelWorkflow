# Concept of operations

*How this tool is **intended** to be used. [`DESIGN.md`](DESIGN.md) records how it works
and why; this file records the operating rhythm it was built to serve. If the two ever
disagree, that is a defect in one of them — fix it, per
[`WRITING.md`](WRITING.md)'s one-canonical-place rule.*

The operator is one photographer, on travel, running this at the end of a shooting day —
tired, in a hotel room or cruise cabin, with dinner waiting. Every design decision that
matters traces back to that person.

## The nightly ritual

1. Load both camera cards into the readers on the Thunderbolt hub. Plug the three archive
   SSDs in — **and where each one goes matters** (see below).
2. Run **`photoday`**.
3. Read the pre-flight summary — file count identical on both cards, gigabytes, four
   destinations confirmed distinct, estimated time. That one line is what earns walking
   away.
4. Go to dinner.
5. Back at the desk, read the **last line of the report first**. It is the verdict, and
   it is the only place its phrases ever appear ([`DESIGN.md`](DESIGN.md) decision 14):

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

### Where each drive plugs in, and why it is not arbitrary

| Device | Goes into |
|---|---|
| **The two USB SSDs** | **the laptop's own USB-C ports**, one each — *not* the hub |
| The OWC (Thunderbolt) | the hub |
| Both card readers | the hub |

**This is worth about four and a half minutes a night, and it costs nothing.** A dock reaches
its USB ports by tunnelling them over one shared 10 Gbps USB4 connection — a spec limit, not
a property of any particular hub — so every USB device on it divides a single pipe. With both
archive SSDs there, each managed 360 MB/s during the verify pass. Given a port of their own on
the laptop, each holds its full rate: **720 MB/s between them becomes 1,914, and the verify
pass falls from roughly eight minutes to under four.**

**It is free because the drives are already in your hands, and nothing was wired to begin
with.** All three SSDs come out of the safe every night and go back in afterwards. More than
that: on most trips the hotel is different every night, so the whole rig is unpacked and
wired from nothing as part of the ritual anyway. **There is no already-connected hub to
compare against** — plugging two drives into the laptop rather than the dock is the same
count of connectors, seated in a different order, during a setup that was happening
regardless.

The one circumstance that would make it a trade again: a stay long enough that the rig lives
wired on a desk between sessions. Then the marginal cost stops being zero and the arrangement
is worth re-examining.

Measured 2026-08-04. [`DESIGN.md`](DESIGN.md) has the numbers and the two hardware upgrades
this beat.

## The shooting-day contract

The tool's guarantees rest on habits it cannot enforce. They are the operator's half of
the deal:

- **The fleet is one body — a Canon EOS R5.** The tool leans on that: every frame
  records its timezone offset, and both slots receive every shot. A new or replacement
  body is a **design event**, not a config change — its EXIF and dual-slot behavior get
  verified at home before any trip trusts it ([`DESIGN.md`](DESIGN.md) decision 23).
- **Only CR3 raw stills are ever shot.** The camera can produce JPG, HEIF and video;
  none of it is used, and this project's scope is exactly what is shot — the raw
  stills. A non-CR3 file on a card is a contract violation: the tool does not back it
  up, and the report names it so the decision about it happens before the next
  in-camera format, not after ([`DESIGN.md`](DESIGN.md) decision 24).
- **Cards are formatted in the camera body. Only ever, by anything.** Not by Windows,
  not by a disk utility, not by a repair tool, and not by this tool — which never writes
  to a card at all (`DESIGN.md` non-goals). The camera writes the exact filesystem
  geometry it expects, and the widely-held view among photographers is that this is the
  single best defence against card corruption. Whether or not every part of that folklore
  survives scrutiny, **it is the operator's standing rule and it binds everything here**:
  a card that misbehaves goes back in the body and gets formatted, rather than being
  handed to `chkdsk /f` or a Windows format.

  **This binds diagnostics too, and that was learned the hard way.** On 2026-08-04 a
  throughput probe wrote a gigabyte to a live card to measure it, failed partway with
  `os error 1392`, and left the volume dirty with corruption in its own directory. The
  product's non-goal would have prevented it; a tool written to investigate the product
  ignored the rule the product obeys. **If a measurement needs to write, it writes
  somewhere that is not a camera card.**

  **The rhythm:** both cards are formatted at the start of every shooting session. There
  may be several sessions in a day — a midday return to the hotel often means offload,
  reformat, and back out for the evening — and a session never spans more than one
  local-time day. The format is what makes a card equal a session — and it stamps each
  card with a fresh volume serial, the session identity the tool trusts when it resumes
  ([`DESIGN.md`](DESIGN.md) decision 13) — which is what makes pre-flight's estimate
  exact. **Format only after the previous offload's SSDs have ejected**: the eject is
  the tool's claim that every file from both cards is accounted for, which is exactly
  what makes the format safe ([`DESIGN.md`](DESIGN.md) decision 22).
- **The camera writes every frame to both slots** (CFexpress + SDXC), uncompressed —
  and both cards come to the readers at every offload. Two authoritative sources is
  the standing assumption, and the camera has two slots for exactly this reason. A
  run that finds only one card refuses to start, and so does a run whose two cards no
  longer hold the same files: either is an equipment failure, not a mode
  ([`DESIGN.md`](DESIGN.md) decisions 7 and 27). The deliberate exception is below,
  under *When a card is truly gone*.
- **GPS logging is an intention, not a promise — and the tool is built for the
  difference.** The ideal is a track behind every frame. The reality is an operator who
  is human, at altitude, chasing light, and who will sometimes forget to start the
  logger. Tracks land in the configured GPX directory before the evening run (or are
  pointed at with `--gpx`), but *whether a logger was running* varies within a single
  day. All three of these are normal, and none is an error:

  | | What the tool does |
  |---|---|
  | A track recorded on a day with **no photographs** | Loaded, matches nothing, costs nothing — a track without frames is not a problem |
  | Shooting **with** logging on | The ordinary case: frames tag wherever the track genuinely brackets them |
  | Shooting **with** logging off | Those frames go untagged, counted as *outside track*, and the report names the boundary when the misses sit on one side of it — which is what makes "ah, the sunset shoot" click |

  **Forgetting the logger is a known risk of this workflow, not a failure, so the tool
  flags it and moves on**: the verdict is untouched, the exit code is unchanged, and the
  raws land and verify regardless ([`DESIGN.md`](DESIGN.md) decision 14). A geotag is a
  nice-to-have earned from evidence; a photograph is not.

  **`--no-gpx` is not the flag for a partly-logged day.** It declares a night with *no
  tracks at all*, which pre-flight otherwise refuses because an empty GPX directory
  almost always means the tracks were never copied off the logger (decision 26). Partial
  coverage needs no flag and no thought — it is simply Tuesday.

  The one thing that **is** refused: two tracks covering the same instant. A photo in the
  overlap could resolve to either recording, and a geotag decided by which file was listed
  first is exactly the authoritative-looking wrong answer this project exists to avoid.
  Recording one window twice — two apps, or a re-export — means pruning one before the
  run.
- **The camera runs on UTC, and its clock is right** — checked at trip start and after
  every zone crossing. The two halves fail differently ([`DESIGN.md`](DESIGN.md)
  decision 23): a wrong *timezone* self-corrects — the recorded offset puts every frame
  in its true UTC date, and the report flags the deviation — but a wrong *clock* shifts
  every date and geotag uniformly and looks perfectly normal doing it. The report's
  systematic-miss heuristic may catch it after the fact; thirty seconds with the camera
  menu is the real defense.

## Four backups, and nothing edits them

**All four copies are the same thing: a backup.** None is a working copy, none is
special, and all four are expected to stay byte-identical forever — including the one on
the laptop, which exists because a fourth backup is worth having.

Editing never touches any of them. The path home is:

| When | What happens |
|---|---|
| During a trip | **this tool only** — four copies, verified, the SSDs ejected into the safe each night |
| Home again | **one of the four** is copied to the NAS — any of them will do |
| Editing | on the desktop, from the NAS — never from a copy this tool wrote |

That middle row is what makes the four interchangeable rather than merely similar: the
NAS source is whichever copy is convenient, so no copy can be allowed to drift from the
others. `verify` holds all four to that standard ([`DESIGN.md`](DESIGN.md) decisions 11
and 20).

**Lightroom never opens one of the four — but it does read the NAS copy of one**, which
is why the sidecars have to be exactly what Lightroom expects and why the folder layout
is Lightroom's own (decision 31). The four are protected from editing; they are not
irrelevant to it.

## Offloading more than once a day

Run the same bare `photoday` at lunch, again in the evening, as often as anxiety
suggests — the natural rhythm is one offload per shooting session, each followed by the
next session's in-camera format once the SSDs eject. Every run is a convergence pass:
work already done is recognized and skipped, new files are ingested, and nothing is
ever duplicated — a photo already in the archive is matched by content hash, not
filename, so re-offloading a card is always safe.

## When something goes wrong

One rule covers nearly everything: **plug in whatever is missing and run `photoday`
again.** Runs converge — each one finishes whatever the last one could not, and the
SSDs eject the moment nothing remains.

| What happened | What to do |
|---|---|
| Run crashed mid-copy | `photoday` again. It resumes at the first unfinished file. |
| Forgot to plug in a card | Pre-flight refuses in the first ten seconds, before anything is written. Plug it in, `photoday` again. |
| CFexpress filled or failed mid-day — some frames exist only on the SDXC | Pre-flight refuses: the cards no longer hold the same files, and the refusal says which holds what. Remove the card that stopped and re-run `photoday --allow-single-source` — the complete card lands the whole day, never corroborated. |
| Laptop slept / power died | Same as a crash. The archives cannot be left half-written — a partial file never carries a real name. |
| An SSD is missing at offload — dead, lost, still in the safe | Pre-flight refuses. `photoday --without <label>` runs the night on the destinations that remain; `photoday sync <that disk>` brings it current when it returns, from the laptop copy — no cards needed. |
| No GPX tracks on the laptop — forgot the copy, or the logger died | Pre-flight refuses. Copy the tracks in (or point `--gpx` at them). If none exist tonight, `photoday --no-gpx` lands the raws untagged; when tracks turn up, re-run before the next format, or `photoday sync` each copy after it. |
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
difference — CFexpress or SDXC, the situation is equally bad. **Phase 4 never runs**,
because corroboration is a comparison and there is no second source to compare
against. The day is recorded as never corroborated, the verdict says so in words, and
the SSDs still eject once every file from the surviving card is verified on all four
copies ([`DESIGN.md`](DESIGN.md) decision 7).

Two boundaries keep the flag honest:

- **It is for a card that is *gone*, not one that is elsewhere.** If the second card
  exists, plug it in instead — and if a "gone" card turns up later, run `photoday`
  again with both cards in; corroboration completes after the fact.
- **A resume of a night that had two sources is not a single-source run** and needs no
  flag — whichever card is inserted, the tool continues what that card can answer for
  and tells a remainder from a lone source on its own ([`DESIGN.md`](DESIGN.md)
  decisions 7 and 13).

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

**Copy one of the four to the NAS, then edit on the desktop from there.** Which copy is
whichever is easiest to reach — an SSD out of the safe, or the laptop's. They are
interchangeable by construction, and `photoday verify <path>` will say so about any of
them before you trust one.

**Import into Lightroom with *Add*, never *Copy*.** This is the step the whole directory
layout exists to serve, and getting it wrong is slow rather than wrong-looking, so it is
easy to do by habit and never notice:

| Import mode | What Lightroom does | Cost |
|---|---|---|
| **Add** — *leave files where they are* | catalogs them in place | **the fast path — use this** |
| Copy | moves them into `YYYY\YYYY-MM-DD` itself, on the NAS | **10× slower, at least** |

The files are already in `YYYY\YYYY-MM-DD` when they reach the NAS, because `photoday`
wrote them that way — that is the entire reason for the layout
([`DESIGN.md`](DESIGN.md) decision 31). Asking Lightroom to *Copy* makes it redo an
arrangement that is already correct, at ten times the price.

Nothing edits a copy this tool wrote. The archives go back to the safe byte-stable, and
the laptop's copy stays byte-stable too — so a `verify` years from now is meaningful on
all four rather than on three. Lightroom only ever sees the NAS copy.

## Years later

Any archive SSD can prove itself on any machine, with no config and no memory of this
setup: **`photoday verify <path>`** re-hashes every raw against the manifests the disk
carries. Deliberately deleted files are tombstoned, so a clean disk reports *clean* —
not a mystery gap. Every bit is checked, every time; there is no sampled mode.

## What this tool will never do

The full list is [`DESIGN.md`](DESIGN.md)'s non-goals; the two that shape daily use:
it **never writes to a camera card** — formatting stays an in-camera act — and it
**never modifies a raw file**, anywhere, under any flag.
