# Concept of operations

*How this **workflow** is intended to be run — the tool, the operator's routines, and
Claude's part in them. [`DESIGN.md`](DESIGN.md) records how `offload` works and why; this
file records the operating rhythm it was built to serve, which is larger than the program.
If the two ever disagree, that is a defect in one of them — fix it, per
[`WRITING.md`](WRITING.md)'s one-canonical-place rule.*

The operator is one photographer, on travel, running this at the end of a shooting day —
tired, in a hotel room or cruise cabin, with dinner waiting. Every design decision that
matters traces back to that person.

## The project is bigger than the Rust app

**Stated by the operator on 2026-08-05, as a top-level intention.** `offload` is one
component. The project is **the whole workflow**, and Claude is deliberately part of it:

> *"I, like most humans, am horrifically bad at diligence tasks. Part of this project is to
> include Claude in my pre-trip and during-trip photo workflow to watch all these diligence
> tasks and walk me through them as a checklist. It's training my brain to think of Claude
> as my trip assistant and watch all the diligence gotchas, as there are a ton and on travel
> I'm highly distracted. Claude becomes my safety guardrails."*

**The evidence for this already sits in this document, one section down, and it was measured
rather than assumed.** Two trips a year, in bursts of roughly eight consecutive nights, six
months apart. **The operator is never practiced** — night one is performed by someone who
last did it half a year ago, at the end of a day that started before sunrise. Add
unfamiliar rooms, fatigue, and a mind on photographs rather than on card slots, and the
diligence steps are exactly where this workflow is thinnest. `offload` already removes
the ones a program can own: no drive letters, no wrong destination, no trusting the tray
icon. **The remainder are the ones only a human can perform — and they are the ones a
distracted human performs badly.**

### The rule that makes this work rather than merely sound good

> **The checklist is the document. Claude is the reader that walks you through it.**

Every routine here — [trip hygiene](TRIP-HYGIENE.md), daily hygiene, the nightly ritual,
[`FULL-RUN.md`](FULL-RUN.md)'s procedure — lives in `docs/` as prose a human can audit and
correct. **Nothing important lives only in Claude's answer.** That is not deference; it is
the difference between an item that can be *wrong* and an item that can be *fixed*: a
mistaken step in a document is a bug with a commit that repairs it forever, while a mistaken
step in a conversation is a fresh hallucination every trip and nobody can diff it.

So the division of labor is fixed: **the repository owns what the steps are; Claude owns
noticing which ones you have not done, in what order, and what changed since last time.**

### The failure mode to design against, because this project has already hit it once

**A guardrail you have to remember to invoke fails exactly the way the diligence it replaces
fails.** If the checklist runs only when the operator thinks to ask for it, it inherits the
problem it was built to solve.

That is not hypothetical — [`FULL-RUN.md`](FULL-RUN.md) exists because a constraint once
lived only in a conversation, and a reboot destroys conversations while leaving every drive
exactly where it was. The answer was `RUN-STATE.json`: **state on disk, at a path
[`CLAUDE.md`](../CLAUDE.md) forces every cold session to read before its first tool call.**

**That pattern is the template for everything this intention adds.** A routine that spans a
trip needs the same treatment — the trip's state written down where a session that has lost
its context, or was never given any, still finds it. Any future checklist that keeps its
progress in chat is a broken window regardless of how well it reads.

## How often this actually happens, measured

**Counted from the Lightroom catalog on 2026-08-04 rather than estimated**, over **2015–2025**.
The era boundary is the operator's, not the data's: travel got better in 2015 and **he intends
to keep it there**, so those years are both the record and the plan. Before 2015 is a different
photographic life averaging 6.4 shooting days a year; 2012–2014 is missing from the tree
entirely.

| | |
|---|---|
| Trips per year | **2.0** |
| Travel days per year | **15.0** mean, **40** in the busiest year (2022) |
| Mean trip | **7.5 days** |
| Longest trip | **16 days** |

*Trips are runs of consecutive shooting days tolerating a one-day gap, of three days or
more. Single days are excluded as local shooting rather than travel — and there are 48 of
those, so including them would have doubled the count and meant something else entirely.*

**The shape matters more than the count, and it is what makes this document necessary.**
The nightly ritual runs in bursts of roughly eight consecutive nights and then not again for
six months. **The operator is never practiced.** Night one of every trip is performed by
someone who last did it half a year ago, at the end of a day that started before sunrise,
with dinner waiting. That is the argument for decision 8's bare command with no arguments,
for refusing rather than assuming, and for a verdict that is the last line and appears
nowhere else — none of which is taste.

**How much a trip actually costs, measured the same way:**

| 13 trips, R5 era (2022–2026) | |
|---|---|
| Median | **446 GB** |
| **Largest** | **860 GB** — 2024-09-29, 7 days, 15,123 frames |

*R5 era only, because volume is the one figure the equipment changes: bigger storage in the
D3300 from 2017, RAW from 2019, a rented R5 from 2021 and an owned one from 2024. Mean file
size tells the story — 3 MB through 2016, 7–10 MB to 2020, 16 MB in the mixed 2021, then a
flat 26–27 MB from 2022, which is a CR3 and its sidecar. Trip **frequency and length** are
counted from 2015 above, since those did not change with the camera; **gigabytes** are not.*

**So capacity is not a constraint, and the alarming version of this paragraph was wrong.**
It first read *"sixteen days at ~200 GB is 3.2 TB against 4 TB drives — 80 % full; one trip
per drive is the real design point"* — arrived at by multiplying the longest trip by the
biggest day. **Not every day of a trip is a big day**, which is precisely what multiplying
two extremes together assumes. The largest trip on record is **22 % of a 4 TB drive**, and
one drive holds about four of those or eight typical ones. Even the 415 GB record day sits
*inside* that largest trip.

**Kept as a correction rather than quietly fixed**, because it is the error this project
exists to avoid, made two hours after writing down that estimates get replaced by
measurements: two worst cases multiplied together produce a number that has never happened
and never will.

*Re-run the count when it matters: it is a directory walk of the year folders in the
Lightroom tree, clustering `YYYY-MM-DD` names into consecutive runs.*

## The nightly ritual

1. Load both camera cards into the readers on the Thunderbolt hub. Plug the three archive
   SSDs in — **and where each one goes matters** (see below).
2. Run **`offload`**, in a **full-screen console**. Terry, 2026-08-05: *"I'll likely run this
   full screen CMD."* Worth recording because it is a licence rather than a preference —
   **the display may assume width.** Phase headings indent four levels deep, the bars carry a
   28-character gauge beside a count and a percentage, and the pre-flight block groups cards,
   destinations and tracks under their own headings. None of that has to survive an 80-column
   window, and designing for one would cost the grouping that makes the screen readable at a
   glance. If that assumption ever stops holding, the layout is the thing to revisit.
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
| `SAFE TO STORE — UNPLUG <X>` | Everything is done. That drive is flushed and detached; Windows just would not power it down. **Pull it out and store it — there is nothing to do in the tray.** |
| `SAFE TO STORE — EJECT <X> BY HAND` | Everything is done; one volume is still mounted and would not release. Eject it from the tray and store. |
| `NOT SAFE — ...` | Something did not finish. Eject nothing; run `offload` again and it continues where it stopped. |

The physical state carries the meaning: **an SSD this tool has ejected is a claim that
every file from both cards is accounted for, verified, on that disk.** A still-mounted
SSD means work remains, and the report names it.

### The cards are released too, and the USB reader goes with them

**All five removable devices are put to bed, not three.** Both cards are released the same
way the SSDs are — locked, dismounted, powered down — so nothing is left in the tray at the
end of a run.

**The eject report arrives in two parts, and the first is the one you are waiting for.** The
SSD line prints the moment all three are down, usually within seconds. The card line follows
whenever the cards resolve, and on this rig that has taken **as long as eleven minutes**:

```text
Travel SSDs — all 3 put to bed in 13s. Safe to store.

    Cards
        Primary    ejected; remove card from reader
        Secondary  ejected; remove card from reader

Cards — all 2 put to bed in 11m 17s.
```

**Once the SSD line says `Safe to store`, those three drives are done** — pull them, put them
in the safe, and let the cards finish on their own. A card cannot change the verdict or the
exit code, because the tool never wrote to one: a card was safe to pull before the run started
and is safe to pull whatever that second line ends up saying. **The run's own verdict is still
the last line printed**, so read it before closing the window.

**Eleven minutes is normal and is not the tool struggling.** Windows refuses to release a
volume for reasons that clear on their own, and the retry is what eventually wins — on
2026-08-06 both cards came back on the tool's own attempt with nobody touching the tray
([`DESIGN.md`](DESIGN.md) decision 22).

**One consequence you will meet, and it is expected rather than a fault: releasing a card
ejects its device, and for the USB SD reader that device *is* the reader.** It powers down
with the card and will not wake when the next card goes in — **it needs its cable replugged.**
The Thunderbolt CFexpress reader is untouched, because the card sits behind a PCIe port
rather than being the reader itself.

| | What happens at the end of a run | Before the next offload |
|---|---|---|
| CFexpress reader | stays present | nothing to do |
| **USB SD reader** | **powers down with the card** | **replug its cable** |

**This costs nothing on a normal night** — the whole rig is unpacked and rewired every
evening anyway. It is only visible when two offloads happen in one day, which is normal
enough to be worth knowing about: a lunchtime run leaves the SD reader down, and the evening
run needs it back.

**And forgetting is cheap by construction.** Pre-flight refuses in the first ten seconds with
`ONLY ONE CARD FOUND` rather than quietly running on one card, so the worst case is ten
seconds at the desk while the fix is a reach to a cable ([`DESIGN.md`](DESIGN.md) decisions 7
and 22). That property is why this arrangement was chosen over leaving the SD in the tray.

**If you walk in and it is still on the eject stage, it is working, not stuck.** Windows
will sometimes refuse to power a freshly written drive down for many minutes, so the tool
keeps asking — with a running clock, and until **90 minutes** after launch. Everything that
matters was settled long before this point: the raws landed and verified at LANDED, and
corroboration and geotagging finished after it. What is left is the drive parking itself.
**Waiting costs you nothing that matters** — every guarantee was banked at LANDED, and if
Windows never relents the verdict tells you to unplug the drive, which is equally safe
([`DESIGN.md`](DESIGN.md) decision 22).

> **You may now genuinely return to a run still ejecting, and that is by your own choice
> rather than a regression.** The budget was an hour until 2026-08-06, sized so the program
> would always have exited first. You raised it: *"if I do get back before it's done ejecting,
> I will happily wait."* **A few minutes at the desk beats a drive left in the tray**, and the
> longer window matters most on the biggest nights, which is exactly when the retry is
> shortest and a veto is likeliest.
>
> **The 60–90 minutes above was never a measurement.** Your words: *"the hour runtime has slop
> in it, that's a very fuzzy number."* Treating its lower bound as a hard ceiling was false
> precision applied to an estimate — and it cost the one stage that had the least margin to
> give.

Everything above the verdict is detail — mismatches, geotag counts, the throughput
numbers. Read it with the other eye, or in the morning.

### Where each drive plugs in, and why it is not arbitrary

| Device | Goes into | Why this one |
|---|---|---|
| **SanDisk Extreme Pro** | **an Element 5 TB5 port** | the only drive that is Gen 2x2 — **1,486 MB/s there against ~980 on a laptop port** |
| **OWC enclosure** | **a laptop port on the LEFT** | the left ports are Thunderbolt 4; **the right one is USB-only and cannot carry its PCIe tunnel** |
| **WD My Passport** | **the laptop's right-side port** | Gen 2x1, so no port makes it *faster* than ~950 — but a port can make it much slower. On its own controller there it reads **934**; on the hub's shared USB tunnel it measured **360**. It ends the verify pass, so this is the drive whose placement moves LANDED |
| **CFexpress reader** | an Element 5 TB5 port | measured not to contend with the SanDisk, which matters because the reader is the phase 3 *source* while a destination writes |
| **SD card reader** | **any 10 Gbps USB port on the Element 5** — two USB-C and two USB-A on the front, one USB-A at the rear | it reads at ~222 MB/s, which is 1.8 Gbps against a 10 Gbps port. **Five-fold headroom: all five ports are equally golden** |
| Monitor, if you want one | an Element 5 TB5 port | measured to cost the offload **0.7 %** — inside the noise |

**Three of those rows are the ones to get right; the rest are forgiving.** The OWC must be on a
*left* laptop port, the SanDisk wants a TB5 port, and the WD wants the laptop's right one. The
readers and the monitor have enough headroom that the choice does not matter — which is
deliberate, because a wiring rule you have to think about at 11pm is a wiring rule you will get
wrong. **All three are asserted by `scripts\full-run-check.ps1`**, so the way to be sure is to
run it rather than to remember this table.

> **The WD row was the last to be got right, and it was wrong in an instructive way.** It used
> to read *"anywhere convenient — Gen 2x1, so it reads ~950 on every port there is. No placement
> helps it."* Every clause of that is true and the conclusion does not follow: no port makes the
> drive *faster*, but the hub's shared USB tunnel makes it **much slower** — 360 against 934,
> measured together-against-together. **"No placement helps it" got read as "no placement hurts
> it."** On 2026-08-05 that row talked a session into writing hub USB into `FULL-RUN.md`'s
> example topology, for the one drive whose rate ends the verify pass; Terry caught it on
> sight. A row that names the *ceiling* when the risk is the *floor* is the shape to watch for.

> **This replaced an earlier rule that said both USB SSDs go in the laptop's ports and *not* the
> hub.** That was correct when written and measured honestly: with both SSDs on the hub's
> **USB-A** ports they shared one 10 Gbps tunnel at 360 MB/s each. **Nobody had tried a TB5
> port**, where a Gen 2x2 drive gets its own link. See [`DESIGN.md`](DESIGN.md) for the
> measurements and for the honest caveat below.

> **And it is worth being honest that this arrangement does not make the run faster.** The
> verify pass ends when the *slowest* destination finishes, and that is the WD at ~592 MB/s —
> which no port can change, because it is Gen 2x1. Moving the SanDisk to TB5 sped up a drive
> that was merely *tied* for last. **The wiring is right, the wall clock is unmoved**, and the
> only levers that would move it are replacing the WD with a Gen 2x2 drive or overlapping the
> verify read with its hash. **The second was built later the same day** and took the WD from
> 691 to 828 MB/s — 20 % off the binding constraint, and the wall clock *did* move
> ([`DESIGN.md`](DESIGN.md), decision 17). The port rewiring's own verdict stands unchanged:
> it is correct, and it is not what made the run faster.

**The paragraph below is the earlier arrangement's rationale, kept because it explains why the
laptop ports mattered at all.** It is worth about four and a half minutes a night against
putting both SSDs on the hub's USB-A ports, and it costs nothing. A dock reaches
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

  **One body is deliberate and expected to stay that way.** It became true in 2024, when a
  robbery at the start of a trip took the D3300 — no photographs were lost — and the rented
  R5 was replaced with an owned one. The plan is to *replace* rather than add: a future
  Canon R body would be bought and the R5 sold, so the fleet stays at one.

  **Lenses are the opposite, and the contrast is the point.** The body is one and its
  replacement is a design event; **glass changes constantly and that is normal**. The operator
  owns an RF 24-240 and rents specialized lenses eagerly and often — an ultra-wide for
  Monument Valley in 2024 — so a *new lens is a no-op* as far as this tool is concerned and
  must never be reported as anything. Nothing about the guarantees depends on which lens shot
  a frame ([`DESIGN.md`](DESIGN.md) decision 34 records why the body is checked and the lens
  explicitly is not).

  **Two card slots is a purchase criterion, not merely something to verify afterwards.**
  This is the part worth knowing before money is spent: "another Canon R body" does not
  imply compatibility. The R5 and R3 carry CFexpress + SD, the R6 line carries dual SD —
  and the R8 and RP have **a single slot**. A one-slot body would not fail the pre-trip
  check so much as invalidate the design: decisions 4, 7 and 27 all rest on two
  authoritative copies of every frame, corroboration would have nothing to compare, and
  every night would run under `--allow-single-source` with the verdict permanently scarred.
  **Check the slot count before the shortlist, not after the purchase.**
- **Only CR3 raw stills are ever shot.** The camera can produce JPG, HEIF and video;
  none of it is used, and this project's scope is exactly what is shot — the raw
  stills. A non-CR3 file on a card is a contract violation: the tool does not back it
  up, and the report names it so the decision about it happens before the next
  in-camera format, not after ([`DESIGN.md`](DESIGN.md) decision 24).
- **Cards are formatted in the camera body. Only ever, by anything.** Not by Windows,
  not by a disk utility, not by a repair tool, and not by this tool — which never writes
  to a card at all (`DESIGN.md` non-goals). The camera writes the exact filesystem
  geometry it expects, and the widely-held view among photographers is that this is the
  single best defense against card corruption. Whether or not every part of that folklore
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
- **Every session is shot on one 512 GB CFexpress and one 512 GB SDXC, and the matching
  capacity is the point.** Stated by the operator 2026-08-06. The complement is **three of
  each**, rotated:

  | Slot | The three |
  |---|---|
  | CFexpress Type B, 512 GB | Sabrent Rocket CFX · Lexar CFexpress · Angelbird AV Pro CFexpress SE |
  | SDXC UHS-II, 512 GB | Angelbird AV Pro SD · Lexar Silver Pro · SanDisk *(arriving 2026-08-07)* |

  **Equal capacity is not tidiness, it falls out of the row above.** Both slots receive
  every frame, so **the pair holds only as much as its *smaller* card** — a 512 paired with
  a 256 is a 256 GB session, and the moment the small one fills, the two cards stop holding
  the same files. That is decision 27's gate firing at the desk on an equipment problem the
  operator created by pairing unequal cards. **512 against 512 makes the constraint
  symmetric**, so both fill together or neither does.

  **The margin, measured from the catalog 2026-08-06:** the largest *day* on record is
  **2024-10-02 — 7,395 frames, 386.6 GiB, 415.1 GB decimal, which is 81 % of a 512 GB card.**
  Pure CR3 and sidecars at 56.1 MB a frame; no video, no JPEG. It is day 4 of the 2024-09-29
  trip and **48 % of that whole seven-day trip by itself**, with 32–82 GiB days either side.

  **But the card constraint is per session, not per day**, because both cards are formatted at
  the start of each one and a day may hold several. So the worst load any single card has
  actually carried is **at most 415 GB and possibly far less — nobody has counted.** The
  catalog groups by date folder and cannot see the session split. **Count it before a trip
  expecting a day bigger than any on record**, because at 81 % there is not a second one of
  those days' worth of headroom.

  **A day over 200 GB happens about once a year** — 2021, 2022, 2023 and 2024 each have
  exactly one ([`DESIGN.md`](DESIGN.md), *Where the wall clock goes*). The extreme recurs; it
  is not a freak.

  **Anything smaller than 512 GB is a spare or a test card, not a shooting card.** The
  256 GB Lexar Silver Pro measures healthy and stays in the bag on that basis — it spent
  2026-08-04/06 in the rig for testing, which is exactly how a session could mistake it for
  the operational corroborator. It is not one.
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
- **When there is internet, the offload is run through Claude.** An operator commitment, and
  the field half of *The project is bigger than the Rust app* above: the reason to have a
  diligence assistant is the night you are least able to be diligent, which is every night of
  a trip. Hotel connectivity cannot be promised, so this is *whenever possible* rather than
  always.

  **Which sets a hard constraint on the tool in the other direction: nothing may depend on
  Claude being there.** The report has to be complete and actionable for a tired human alone,
  because some nights it will be read by one. Claude adds a layer — noticing, asking, offering
  to fix — and never supplies a guarantee. **A check that is only useful when Claude is in the
  room is a defect in the check**, and the fix belongs in the report rather than in
  [`CLAUDE.md`](../CLAUDE.md).

  What Claude is expected to act on rather than merely relay is listed in `CLAUDE.md` under
  *Report lines you must act on* — today that is decision 34's `Body` line, where the standing
  action is to ask what changed and offer to update the config.
- **The camera runs on UTC, and its clock is right** — checked at trip start and after
  every zone crossing. The two halves fail differently ([`DESIGN.md`](DESIGN.md)
  decision 23): a wrong *timezone* self-corrects — the recorded offset puts every frame
  in its true UTC date, and the report flags the deviation — but a wrong *clock* shifts
  every date and geotag uniformly and looks perfectly normal doing it. The report's
  systematic-miss heuristic may catch it after the fact.

  **The real defense is a frame, not the menu — and that is a correction.** This used to
  read *"thirty seconds with the camera menu is the real defense"*, which is wrong in the
  direction that matters: the menu shows what was *set*, and every timezone bug here has
  lived in the gap between that and what the camera actually *writes*. Note the true UTC,
  take one frame, read its EXIF. [`TRIP-HYGIENE.md`](TRIP-HYGIENE.md) has the procedure and
  both properties to check. **The R5 has no UTC timezone, so the setting is London with
  DST off** — London with DST left on is BST, `+01:00`, all summer, and it looks correct
  in the menu.

## Daily hygiene — before each shooting session

**"Daily hygiene" is the term**, and it is the field counterpart to
[trip hygiene](TRIP-HYGIENE.md): that one runs once at home before departure, this one runs
as you pick the camera up. The shooting-day contract above says what has to be *true*; this
is the thirty seconds in which you confirm it is still true today.

**It is entirely a *camera config* check** — the clock and the settings in the body. Nothing
here is about the laptop, the drives or the tool; those were settled by trip hygiene and are
re-checked by pre-flight at offload.

*RFC 2119 keywords, and they are load-bearing: **MUST** means the day is damaged if you skip
it, **SHOULD** means you will wish you had. Two of these five are MUST because nothing
downstream can catch them — the table below shows which.*

1. **You MUST sync the camera clock to true UTC.** Not "check it looks right" — set it
   against a clock you trust. It drifts, and a body flash or a battery pull can reset it.
2. **You MUST confirm uncompressed RAW, and nothing else** — no compressed raw, no JPEG,
   no HEIF, no video.
3. **You SHOULD confirm both slots are recording every frame** — the same image to both
   cards, not an overflow/relay arrangement where the second card only starts once the
   first fills.
4. **You SHOULD format both cards in the body**, per the contract, at the start of the
   session.
5. **You SHOULD confirm the GPS logger is actually running**, if you want tracks for this
   session.

**The two MUSTs are 1 and 2, and the split is not about which matters most** — a relay-mode
slot loses frames, which is as bad as anything here. It is about **which failures have a
safety net.** 3, 4 and 5 fail loudly downstream: decision 27's gate refuses at the desk,
decision 24 names strays in the report, a missing track is declared. 1 and 2 fail **silently
and permanently**, and the only place in the entire system where they can be caught is
standing in front of the camera before the day starts.

### Which of these the tool backstops, and which it cannot see at all

**This is the part worth internalizing, because two of them have no safety net anywhere
downstream.** Daily hygiene exists for those two; the others merely fail earlier and more
cheaply when you check.

| If this is wrong | What happens | Does the tool catch it? |
|---|---|---|
| Slots in overflow/relay instead of both | Some frames exist on one card only | **Yes** — decision 27's gate refuses at offload. But the day is already shot |
| JPEG, HEIF or video left on | Strays on the card, not backed up | **Yes** — decision 24 names them in the report, exit 2 |
| **Compressed raw instead of uncompressed** | Compressed frames enter the archive, permanently | **No. Never.** Both are `.CR3` — the extension, the container and the pipeline are identical, so nothing at any stage can tell them apart |
| **The clock is wrong as an absolute instant** | Every date folder and geotag shifts together | **No.** Decision 23: honest arithmetic on lying metadata, no error anywhere |

**The two "No" rows are the whole argument for this being a routine rather than a habit.**
Everything else here degrades loudly — a refusal at the desk, a line in the report, an exit
code. Those two degrade silently and permanently, and the only place they can be caught is
in front of the camera before the day starts.

**The clock row is also why daily hygiene says *sync* rather than *check*.** Reading the
menu tells you what was set; it cannot tell you the body has drifted or been reset. Trip
hygiene verifies the clock by taking a frame and reading its EXIF, which is the stronger
form of the same test — do that one whenever a session matters enough to be worth two
minutes.

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

Run the same bare `offload` at lunch, again in the evening, as often as anxiety
suggests — the natural rhythm is one offload per shooting session, each followed by the
next session's in-camera format once the SSDs eject. Every run is a convergence pass:
work already done is recognized and skipped, new files are ingested, and nothing is
ever duplicated — a photo already in the archive is matched by content hash, not
filename, so re-offloading a card is always safe.

## When something goes wrong

One rule covers nearly everything: **plug in whatever is missing and run `offload`
again.** Runs converge — each one finishes whatever the last one could not, and the
SSDs eject the moment nothing remains.

| What happened | What to do |
|---|---|
| Run crashed mid-copy | `offload` again. It resumes at the first unfinished file. |
| Forgot to plug in a card | Pre-flight refuses in the first ten seconds, before anything is written. Plug it in, `offload` again. |
| CFexpress filled or failed mid-day — some frames exist only on the SDXC | Pre-flight refuses: the cards no longer hold the same files, and the refusal says which holds what. Remove the card that stopped and re-run `offload --allow-single-source` — the complete card lands the whole day, never corroborated. |
| Laptop slept / power died | Same as a crash. The archives cannot be left half-written — a partial file never carries a real name. |
| An SSD is missing at offload — dead, lost, still in the safe | Pre-flight refuses. `offload --without <label>` runs the night on the destinations that remain; `offload sync <that disk>` brings it current when it returns, from the laptop copy — no cards needed. |
| No GPX tracks on the laptop — forgot the copy, or the logger died | Pre-flight refuses. Copy the tracks in (or point `--gpx` at them). If none exist tonight, `offload --no-gpx` lands the raws untagged; when tracks turn up, re-run before the next format, or `offload sync` each copy after it. |
| Cards already reformatted before corroboration finished | Nothing to recover — the run closes out on its own at the next offload and the report says which files stayed uncorroborated. They were still verified on all four copies. |

Fatal errors are deliberate ([`DESIGN.md`](DESIGN.md) decision 18): the tool stops and
says why rather than improvising. The recovery is always the same re-run.

### When a card is truly gone

Lost, dead, left in a hotel five towns back — the run can still happen, but only by
saying so:

```
offload --allow-single-source
```

The surviving card becomes the sole source of truth, and which card survived makes no
difference — CFexpress or SDXC, the situation is equally bad. **Phase 4 never runs**,
because corroboration is a comparison and there is no second source to compare
against. The day is recorded as never corroborated, the verdict says so in words, and
the SSDs still eject once every file from the surviving card is verified on all four
copies ([`DESIGN.md`](DESIGN.md) decision 7).

Two boundaries keep the flag honest:

- **It is for a card that is *gone*, not one that is elsewhere.** If the second card
  exists, plug it in instead — and if a "gone" card turns up later, run `offload`
  again with both cards in; corroboration completes after the fact.
- **A resume of a night that had two sources is not a single-source run** and needs no
  flag — whichever card is inserted, the tool continues what that card can answer for
  and tells a remainder from a lone source on its own ([`DESIGN.md`](DESIGN.md)
  decisions 7 and 13).

## Trip hygiene — before a trip, at home

**"Trip hygiene" is this project's name for the whole pre-departure routine**, and the one
term to use for it. The full checklist lives in [`TRIP-HYGIENE.md`](TRIP-HYGIENE.md); the
shape of it:

- **Device firmware — hub, enclosure, card readers *and the camera body* — at T-30 days or
  earlier, and never inside thirty days of departure.** A bright line, not a preference.
  The freeze is sized by how long a *replacement* takes to order and arrive, because a bad
  flash is not a rollback — it is new hardware. A bricked enclosure at T-40 is an errand;
  the same failure at T-5 is a trip that leaves with three copies instead of four. The body
  is on the list because the fleet is exactly one camera and there is no spare to shoot with.
- **Then set the camera to UTC and *verify it by taking a frame*** — after any body flash,
  never before, since a flash can reset the clock. Not frozen at T-30: it is reversible in
  ten seconds, so do it late and do it again at trip start.
- Update dependencies and toolchain **before leaving, never on the road** — travel with
  a verified binary rather than a current one.
- **`offload --dry-run` against the real rig** — both readers, all three SSDs. This is
  the rehearsal that catches a reformatted drive, a changed reader, or a stale config
  entry while the fix is a walk to a drawer rather than a ruined evening.
- If Lightroom Classic had a major release since the last trip, run the XMP checks noted
  there.

## After a trip

**Copy one of the four to the NAS, then edit on the desktop from there.** Which copy is
whichever is easiest to reach — an SSD out of the safe, or the laptop's. They are
interchangeable by construction, and `offload verify <path>` will say so about any of
them before you trust one.

**Import into Lightroom with *Add*, never *Copy*.** This is the step the whole directory
layout exists to serve, and getting it wrong is slow rather than wrong-looking, so it is
easy to do by habit and never notice:

| Import mode | What Lightroom does | Cost |
|---|---|---|
| **Add** — *leave files where they are* | catalogs them in place | **the fast path — use this** |
| Copy | moves them into `YYYY\YYYY-MM-DD` itself, on the NAS | **10× slower, at least** |

The files are already in `YYYY\YYYY-MM-DD` when they reach the NAS, because `offload`
wrote them that way — that is the entire reason for the layout
([`DESIGN.md`](DESIGN.md) decision 31). Asking Lightroom to *Copy* makes it redo an
arrangement that is already correct, at ten times the price.

Nothing edits a copy this tool wrote. The archives go back to the safe byte-stable, and
the laptop's copy stays byte-stable too — so a `verify` years from now is meaningful on
all four rather than on three. Lightroom only ever sees the NAS copy.

## Years later

Any archive SSD can prove itself on any machine, with no config and no memory of this
setup: **`offload verify <path>`** re-hashes every raw against the manifests the disk
carries. Deliberately deleted files are tombstoned, so a clean disk reports *clean* —
not a mystery gap. Every bit is checked, every time; there is no sampled mode.

## What this tool will never do

The full list is [`DESIGN.md`](DESIGN.md)'s non-goals; the two that shape daily use are
binding constraints, stated in [`../CLAUDE.md`](../CLAUDE.md) as absolutes and restated here
in the same words:

- **The tool MUST NOT write to a camera card.** Not a byte, under any flag, on any code
  path. Formatting stays an in-camera act by you.
- **The tool MUST NOT modify a raw file.** All derived data goes to sidecars and manifests.

**Those are the two you can rely on without reading anything else.** Whatever else a run
does or fails to do, the originals on the cards and the raws in the archive are untouched —
which is what makes a bad night recoverable rather than expensive.
