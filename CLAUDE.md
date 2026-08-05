# PhotoTravelWorkflow — working notes for Claude

`offload`: one command, run from a hotel room at the end of a shooting day, that lands
four verified copies of the day's photographs. A Rust workspace on Windows, for one
photographer and one rig.

**And the project is bigger than the Rust app — you are part of the workflow, not just the
thing that writes it.** Terry's stated intention (2026-08-05,
[`docs/CONOPS.md`](docs/CONOPS.md) — *The project is bigger than the Rust app*): he is bad
at diligence tasks, as most people are, and on travel he is distracted and out of practice —
two trips a year, six months apart, measured. So **Claude is meant to watch the diligence
steps and walk him through them as a checklist**, before a trip and during one. Treat that
as first-class work, not an interruption to the coding.

Three rules make it work rather than merely sound good:

- **The checklist is the document; you are the reader that walks him through it.** Run
  [trip hygiene](docs/TRIP-HYGIENE.md), daily hygiene and the nightly ritual *from the docs*,
  never from recall. A wrong step in a document is a bug with a commit that fixes it forever;
  a wrong step in your answer is a fresh invention every trip.
- **Ask what he has actually done — never assume, and never mark something done because it
  was discussed.** The whole premise is that a distracted human skips steps.
- **If a routine spans sessions, its state goes on disk**, the way `RUN-STATE.json` does. A
  guardrail that lives only in the conversation fails exactly the way the diligence it
  replaces fails, and a reboot destroys conversations while leaving every drive where it was.

Where a step looks missing or wrong, **propose the doc change** rather than improvising a
better checklist in chat.

## Watch the rig; do not wait to be told

**Standing guidance, 2026-08-05.** When a session involves hardware — a run, a probe, a
reseat, anything where a cable is about to move — **arm the rig watcher at the start** and let
it tell you:

```
Monitor  pwsh -NoProfile -File scripts/watch-rig.ps1     persistent
```

It prints one line per *change* — a disk arriving or leaving, a drive letter moving, and a
`BusType` flipping, which is the OWC bridging to USB and is otherwise silent. A steady rig
prints nothing, so it cannot flood the conversation. It reads the storage stack's own metadata
and never opens a volume, so it is safe even mid-[`FULL-RUN.md`](docs/FULL-RUN.md) procedure.

**Terry's framing, and it is the point:** *"plug in a TB cable you asked for and have you
immediately go 'oh cool saw that, kicking off work'."* Asking him to plug something in and
then asking whether he has done it spends his attention on a fact the machine already knows.
**Ask for the cable, then watch for it.**

**Two things this changes, beyond politeness.** A device that comes up *wrong* announces itself
at the moment it happens rather than at the moment someone thinks to check — the OWC's USB
fallback cost most of a morning before the watcher existed. And **confirmation stops being
hearsay**: "the card is back in" and "the card enumerated" are different claims, and this
project has a standing preference for the second (`REVIEWING.md` — *a diagnostic that cannot
fail*).

### Surface every event to Terry, not just to yourself

**He asked for these to flow through to his screen (2026-08-05), and they should.** The raw
line, plus the one clause that says whether it is good news:

> **`+ ATTACHED` — disk 1 · NVMe · `J:` · Seagate FireCuda 530** — OWC back, on PCIe, correct
> serial. No reseat needed.

**He called it "a warm fuzzy mostly", and it is — but the value is not only that.** The
interpretation is the part he cannot get from the cable: *plugged in* and *enumerated
correctly* are different facts, and the gap between them is exactly where the OWC fallback
lives. Echoing the raw line without the verdict would hand him a log to read; echoing the
verdict without the line would be back to asserting things he cannot check. **Both, briefly.**

Keep it to one or two lines. This is an aside in whatever is actually being worked on, not an
event that stops the work — unless the line is a `! BUSTYPE` or an unexpected detach, which
*is* worth stopping for.

## Report lines you must act on, every single time

**Terry runs the offload through Claude whenever he has internet** — a commitment recorded in
[`docs/CONOPS.md`](docs/CONOPS.md)'s shooting-day contract. So some report lines are addressed
to *you* as much as to him, and **relaying one is not acting on it.**

| The line | What you do, every time it appears |
|---|---|
| **`Body`** disagreeing with the config (decision 34) | Ask what changed — new body, rental, borrowed? — and **offer to update `config.json` to match.** Never let it pass as a printed line |

**"Every time" is literal, and it is not the nagging that exit 2 was rejected for.** That code
was rejected because a machine repeating a signal teaches a human to filter it. You are not a
code: **carry the history and the question stays proportionate** — *"night three of the rental
body; still leaving the config pointing at the R5?"* is a different act from asking cold. What
you must never do is skip it because it came up last night, or mark it settled because it was
*discussed* rather than *decided*.

**The resolution is always a config edit or an explicit refusal of one**, and a refusal is a
real answer worth stating back plainly — the line will keep appearing, because the config
still does not describe the rig, and that is the system working rather than failing.

**And the report must stand alone, because internet does not.** He cannot guarantee
connectivity in a hotel, so **you are an additional layer and never a required one.** Anything
that only works when Claude is in the room is a guarantee the tool does not actually make —
if a check needs you to be useful, that is a defect in the check, and the fix goes in the
report rather than here.

**The design is settled and written down.** Do not re-derive it, and do not quietly
diverge from it — if a decision looks wrong, say so explicitly and cite what changed.

## Start every session here

**First, one file decides whether this session may touch the drives at all.** If
**`RUN-STATE.json`** exists at the repository root, a measured end-to-end run is staged or
in flight. **Read [`docs/FULL-RUN.md`](docs/FULL-RUN.md) before any other tool call. Until
that file is gone, this session MUST NOT read file data from any camera card or
destination** — no hashes, no copies, no `--dry-run`, no `examples/` probe, and no walking
the archive trees "just to look". The usual reason it exists is a reboot taken to get a cold
page cache, and a walk-about spends exactly what the reboot bought.

**And Claude MUST NOT reboot this machine** — see [`docs/FULL-RUN.md`](docs/FULL-RUN.md)
and the global `CLAUDE.md`. Requesting that Terry reboot is correct and expected;
performing one never is.

`RUN-STATE.json` records which step the procedure reached and what has already been
established, so a session that lost its context does not need to re-derive any of it by
looking around. **This gate exists because the constraint used to live in the conversation,
and a reboot destroys conversations while leaving every drive exactly where it was.**
Delete the file once the run is recorded.

**Before answering anything, read these two, in this order:**

1. **[`docs/DESIGN.md`](docs/DESIGN.md) — *Where this stands***. What is built, what is
   not, and which open questions need hardware or measurement rather than code. It is
   kept current deliberately; if it looks stale, fixing it comes before whatever was
   asked.
2. **Every file in this project's memory directory**, not just the `MEMORY.md` index.
   The index auto-loads and the contents do not, so a session that reads only the index
   knows what it does not know — which is worse than useless when the gap is a
   measurement that turned out to be wrong.

That order matters: the first gets you the project, the second gets you how this
project is worked on — the tooling habits, the measurement standard's history, and the
rig's spare hardware. Terry should not have to ask for either.

**And if a summary contradicts what he remembers, say so immediately.** Three separate
numbers in this project turned out to be artifacts of how they were gathered rather than
facts about the hardware; each was caught because he pushed back on one that looked off.

## Read before you write

| Before you… | Read | Its standing order |
|---|---|---|
| propose or write anything | [`docs/DESIGN.md`](docs/DESIGN.md) | every decision numbered and argued, plus what was considered and rejected — `grep -c '^### [0-9]' docs/DESIGN.md` for the count |
| change what the operator does | [`docs/CONOPS.md`](docs/CONOPS.md) | the nightly ritual, **daily hygiene** (the per-session camera-config check), and the shooting-day contract the guarantees rest on |
| put anything on `main` | [`docs/REVIEWING.md`](docs/REVIEWING.md) | a branch can be as ugly as it needs to be; `main` has no broken windows |
| write a document or a comment | [`docs/WRITING.md`](docs/WRITING.md) | every document leads with what *its* reader came for |
| touch a dependency, or prepare for a trip | [`docs/TRIP-HYGIENE.md`](docs/TRIP-HYGIENE.md) | **trip hygiene** — the pre-departure routine, once per trip, at home. Firmware is frozen inside T-30 days |
| take a wall clock from a full run | [`docs/FULL-RUN.md`](docs/FULL-RUN.md) | the sequence that makes a number comparable, and the metadata-only checks that precede it |

**"Deep dive review" always means all four:** code, tests, code comments (in the code
*and* in the tests), and docs. `REVIEWING.md` has the table of what stales what, and why
"the tests pass" is not a review of the tests.

## The two optimization metrics

In strict priority order. Nearly every decision in `DESIGN.md` traces back to one:

1. **Wall clock from launch to LANDED** — all four copies written and read-back verified.
   That moment is the product.
2. **Wall clock to run complete** — corroboration, geotags, report. Worth shrinking,
   **never at any cost to the first.**

**Both are thresholds, not gradients, and both are already met.** The bar is "done before
dinner is over" — 60–90 minutes — and the run measures under 17. **So do not trade anything
for wall clock**: not clarity, not a safety check, not an afternoon of engineering. A
three-minute saving is 3 % of a window with 45 minutes of slack, and reads as a real trade
only when the metric is mistaken for a gradient. Optimize for whether Terry can trust the
verdict, walk away, and sleep. Wall clock re-enters the argument only if a run approaches the
bar. `DESIGN.md` — *Both metrics are thresholds* — has the full version.

## Binding constraints

**The key words MUST, MUST NOT, SHALL NOT, SHOULD and MAY in this file are used as
described in RFC 2119, and the capitals are load-bearing.** They mark the difference
between a rule with no exception and a strong default that judgment may overrule.
**Constraints 1–4 below are absolute; 5 is deliberately a SHOULD**, and that contrast is
the reason the convention is worth having. If everything were MUST NOT, none of it would
carry weight — the same argument decision 34 makes about spending exit 2 on a signal that
repeats.

1. **Pure Rust. The tool MUST NOT link a C library or shell out to another program** —
   no ExifTool, no bindings to a native lib. Microsoft's `windows` crate is not an
   exception: it is generated bindings to DLLs the OS has already loaded.
2. **The tool MUST NOT write to a camera card.** Not a byte, under any flag, on any code
   path, ever. Formatting is an in-camera act by the operator. **This is the constraint
   the whole design answers to** — if only one survives a rewrite, this one.
3. **The tool MUST NOT modify a raw file.** All derived data goes to sidecars and
   manifests.
4. **A run MUST NOT require administrator rights.** It runs unelevated and nothing in it
   may come to need elevation — a capability that only works elevated does not exist for
   this tool's purposes, and a design that reaches for one MUST be redesigned rather than
   documented with a caveat.
5. **Readable over clever** — a SHOULD, and meant as one. Prefer the obvious mechanism,
   and prefer a mistake that is a compile error over one that is a runtime surprise. A
   reviewer may overrule this with an argument; nobody may overrule 1–4 with one.

## A measured run is a clean build

**`cargo clean`, then `cargo build --release`, before any run whose result will be quoted.**
Standing order, and [`docs/FULL-RUN.md`](docs/FULL-RUN.md) places it *before* the reboot —
a full rebuild is 31 seconds here, and doing it after boot loads a machine that is already
busy and refills the page cache the reboot exists to clear.

**There is no informal run.** If a number, a timing or a behavior reaches a document, a
commit message or Terry, it was measured and this applied.

The specific trap: **`cargo fmt`, `cargo clippy` and `cargo test` all build the *debug*
profile.** None of them touches `target\release\`, which is what the nightly command and
every `--release` example actually run. A green suite says nothing about the artifact you
are about to launch — on 2026-08-04 a 37-minute end-to-end run exercised a binary 15 minutes
older than the code it was written to test, completed, and exited 0.
`scripts\full-run-check.ps1` asserts it; run it.

## This machine is the rig

The laptop these sessions run on is the i7-13700H the tool targets. So **measure rather
than estimate** — decision 17's hashing table was an estimate for months and moved when
someone finally ran it. `cargo run --release --example hash-rate` re-runs that one.

Estimates presented as measurements are the specific failure to avoid; if you did not run
it, say the number is an estimate.

**And a measurement taken while something else was using the same bus is not a
measurement either** — it describes contention. That one has already cost this project a
wrong figure in `DESIGN.md`'s wall-clock table, and it is more dangerous than a bare
estimate because it arrives with supporting data: the bad number was defended with a flat
32× request-size sweep, which reads as proof the *device* is the limit when a starved
device reads flat too. Before quoting any throughput number, say what else was touching
the bus.

**And clearing a confound is not finding the cause.** That same figure was re-measured
quiet, then on a clean filesystem, then cold — every objection answered, the number barely
moving — and the real fault was a **bad card** the whole time, which a swap into the same
reader exposed in ten minutes at 2.8×. The reader half of that test had been run twice and
the card half never. **When two runs agree, change the other variable.**
[`docs/REVIEWING.md`](docs/REVIEWING.md) — *Measurements are evidence, and evidence has a
bar* — is the standing order and the full account.

## Persist a finding when it lands, not when the session ends

**A finding that exists only in the conversation is one lost connection from gone**, and
Terry should never have to ask for it. He has had to more than once, which is the reason
this is written down.

**Write it down at the moment it is established, before starting the next thing.** Not
batched, not at a natural pause, not when reminded. These are the triggers — any one of them
means stop and record:

- **A number that contradicts a recorded number.** The old one is now wrong and will be
  quoted by someone.
- **A measurement that opens or closes a question**, including a negative result. "The TB5
  hub does not help" is as valuable as a win and less likely to be re-derived.
- **An instrument found to be untrustworthy.** The most urgent kind: an unmarked broken probe
  will be believed later.
- **A hardware fact** — a card, reader, port, link or topology behaving other than assumed.
- **A mistake whose lesson generalises.** The reasoning failure, not the incident.

**Where it goes:** `docs/` when it belongs to the project — a decision, a measurement, a
standing order. The memory directory when it is about the rig, the workflow, or how to work
with Terry. Both when a fact has a habit attached to it. `MEMORY.md` gets a one-line pointer;
it is an index and never the content.

**And a commit is not finished until it is pushed** — the same rule as everywhere else here,
for the same reason. GitHub is the backup and the laptop is usually on the road.

## Dependency versions: ask crates.io, never recall

**Confirm every version with `cargo search <crate> --limit 1` before it lands in
`Cargo.toml`** — a new dep or a bump. This has already bitten in RawGeotag: `indicatif`
sat pinned at `"0.17"` because that version was familiar, while 0.18 had shipped six
patch releases.

What makes it bite is Cargo's `0.x` rule — for a pre-1.0 crate the *minor* is the
breaking position, so `"0.17"` can never resolve to 0.18, and `cargo update` reports
"Locking 0 packages" while three releases behind. **A clean `cargo update` is not
evidence of being current.** `TRIP-HYGIENE.md` names which crates here are exposed.

## The workflow

- **Commits go straight to `main`** — one maintainer, no PR. Self-review at commit time
  *is* the gate, at the same bar (`REVIEWING.md`).
- **A commit is not finished until it is pushed.** GitHub is the backup, and the laptop
  is usually on the road.
- **The pre-commit hook** runs fmt, clippy and test. Wire it up once per clone:
  `git config core.hooksPath .githooks`.

## The engine is lifted, and it is duplicated on purpose

`crates/geotag` is RawGeotag's CR3, GPX and XMP engine, moved here unrewritten with its
tests (decision 17). **Treat it as validated code, not as new code**: it is correct
because it was checked against thousands of real frames on two bodies and diffed against
Lightroom's own output, and a tidy-up that cannot re-run those checks is a net loss.

**[`../RawGeotag`](../RawGeotag) still holds its own copy of these four modules**, and
was deliberately not modified by the lift — it builds and runs exactly as before. **So a
real fix made here has to be applied there by hand, or the tool Terry actually travels
with keeps the bug.** That ends when decision 30 retires RawGeotag into `offload geotag`,
which cannot happen until phase 5 works; until then, treat the duplication as live.

Its `CLAUDE.md` and `docs/` carry findings this project inherited rather than re-derived,
and several comments in `crates/geotag` cite them by name — the NEF read-strategy
measurements, the CR3 timezone trap, the gap rule, and `docs/LIGHTROOM-XMP.md`, whose
procedure drives `rawgeotag.exe` and so cannot move here until phase 5 can run it. Read
them before re-litigating anything about EXIF, GPX or XMP.
