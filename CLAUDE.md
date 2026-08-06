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

**Only once that gate is clear: [`docs/BACKLOG.md`](docs/BACKLOG.md) is what is in flight and
what state it is in.** A session's task list dies with the session, and on 2026-08-06 that list
was briefly the only record of four open items. **Claude MUST update that file when an item
opens, closes or materially changes — in the same turn, not at the end of a session**, because
end of session is exactly when a session does not get to finish.

### ⚠ THE CHECKLIST AND `BACKLOG.md` MUST NEVER DRIFT ⚠

**Standing order, Terry, 2026-08-06, twice in one evening. The second time, after it had already
drifted:** *"should we promote that standing order to CLAUDE? I am relying on those never ever
ever drifting. Make that real loud."*

> **THE RULE, AND IT IS MECHANICAL RATHER THAN ASPIRATIONAL:**
>
> **Claude MUST NOT call `TaskCreate` or `TaskUpdate` without editing
> [`docs/BACKLOG.md`](docs/BACKLOG.md) in the same turn. Claude MUST NOT edit `BACKLOG.md`'s item
> list without a matching task call in the same turn.** Not the next turn, not before the commit,
> not at the end of the session — **the same turn.** If one of them is not worth updating, then
> neither was worth updating.

**This is written as a coupling between two tool calls on purpose**, because the aspirational
version — *"keep them in sync"* — was already in this file and **failed on its second day.** Three
items changed state in the CLI and none of it reached the file. A rule that depends on remembering
to look is a rule that fails exactly when a session is moving fast, which is when the list changes
most.

**The two lists hold different things, and that is not a drift.** Standing order, 2026-08-06:
*"BACKLOG is permanent memory, UI checklist is only stuff that's both a) eligible to be worked,
and b) not complete."*

| | In the CLI checklist | In `BACKLOG.md` |
|---|---|---|
| Eligible to be worked, not complete | **yes** | yes |
| **Blocked** — on Terry, on hardware, on a shoot | **no, remove it** | yes, marked `BLOCKED` with what unblocks it |
| Complete | **no, remove it** | yes, in the closed list |

**So "in sync" means the *working set* matches, not that the lists are identical.** An item
leaving the checklist MUST be explained in `BACKLOG.md` in the same turn — `BLOCKED ON TERRY`,
or moved below — never simply deleted from both.

**A short checklist is the intended state and an empty one is a real answer**: it means everything
left needs him rather than Claude. **A list padded with things nobody can act on is a list you
stop reading**, which is the same argument decisions 9 and 12 make about warnings that fire when
you cannot act.

**Each `BACKLOG.md` item carries its status in its own heading** — `OPEN`, `IN PROGRESS`,
`BLOCKED`, or moved to the closed list — so drift is *visible* rather than inferred, and so what
is *missing* from the checklist is explained rather than merely absent.

**And "in sync" includes the STATUS, not just the words.** This failed within the hour: two items
landed, both had their text rewritten in `BACKLOG.md` *and* their task descriptions updated in the
same turn — and neither had its **status** moved, so the checklist still read `pending` for a fix
that had already shipped. Terry, spotting it: *"what checklist/BACKLOG changes are needed if two
tasks just landed?"*

> **When work lands, the status field is the whole point.** He reads a one-line summary and a
> state; a beautifully updated description under a stale `pending` tells him nothing has happened.
> **Ask on every landing: does this item change state?** Completed, newly blocked, newly started —
> and if it does, move it in *both* places before the commit.

**Why he relies on this absolutely, and why the two failure directions are not symmetric:**

| Drift | Consequence |
|---|---|
| In `BACKLOG.md`, not on his list | **He never sees it.** The CLI shows at most five items and is the only view he has |
| On his list, not in `BACKLOG.md` | **It dies with the session.** A task list does not survive; the file does |

His words when this first came up: *"by the way is our checklist persisted? That's my memory
right now and that's dangerous."* **He is treating the checklist as his memory, and that is the
arrangement — so a drift is not an untidiness, it is a fact quietly disappearing from the only
place he looks.**

**The precedence is absolute and the order is not cosmetic:**

| | |
|---|---|
| **1. `RUN-STATE.json` exists** | [`FULL-RUN.md`](docs/FULL-RUN.md) governs, **before any other tool call.** A measured run is staged or in flight and the backlog is irrelevant until it is finished |
| **2. No `RUN-STATE.json`** | `BACKLOG.md` is the starting point — it says what was being worked on and how far it got |

**Why this way round.** A staged run is a *perishable* state: a cold page cache bought with a
reboot, a wiped set of destinations, a settled machine. **Reading the backlog first and picking
up an interesting task is exactly how that gets spent** — a probe, a walk of the archive trees,
an `examples/` run, and the reboot was for nothing. The backlog will still be there afterwards;
the cold cache will not.

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

| Before you... | Read | Its standing order |
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

## A config item that is never used MUST NOT exist

**Standing order, Terry, 2026-08-06, verbatim: *"a config item that is never used should not
exist — that's a dangerously unused code path waiting to bite us."*** RFC 2119 sense.

**The sharp version of the argument is not "clutter", it is *when* the path runs.** An option
nobody selects is code that executes for the first time on the night someone is desperate enough
to start trying flags — which is the worst possible moment for its first execution, and the least
likely moment for anyone to notice it behaving oddly. **Unused configuration is not inert; it is
deferred, and it defers to the worst hour.**

**Three questions before any flag, and a "no" to the first MUST mean it is not written:**

1. **Would Terry ever pass a value other than the default?** If the honest answer is no, it is not
   configuration — it is a constant with extra steps and an untested branch.
2. **Does a wrong value do harm?** `--eject-prepare every-attempt` was *known* to hang unwinnably,
   and `never` dropped decision 2's flush guarantee. **Shipping a selectable known-bad mode is
   worse than shipping no option at all.**
3. **Is a diagnostic the real motive?** Then it belongs in `examples/`, not on the product's
   command line. `examples/eject-one.rs` drives every arm of `eject::Prepare` directly, which is
   why removing the flag cost the experiment nothing.

**A flag added to compare two candidates MUST be removed when the comparison settles.** That is
the shape this came from: `--eject-prepare` was right for one evening of A/B work and became a
liability the moment a winner existed. **Deleting it is finishing the experiment**, not
discarding a capability — and the losing arm stays in the library with its tests, where it is
exercised rather than merely available.

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

## The build chain is checked live, once a day, on the first build

**Standing order, Terry, 2026-08-06, in his words: *"it's important to me to keep my
buildtool chains within 24 hours of current."*** He calls it an obsession; treat it as a
requirement.

**A hook does it, so neither of us has to remember.**
`~/.claude/hooks/rust-toolchain-check.py` fires on the first
`cargo build|test|clippy|run|bench` and is scoped by `~/.claude/toolchain-projects.json`,
which names this repository. It **MUST** ask the network every time. It **MUST NOT** answer
from recall, from a cached version, or from anything written in this file — a freshness check
that trusts a memory is not a freshness check.

**Three outcomes, three volumes, and Terry is shown all three:**

| What it established | How it appears | What you do |
|---|---|---|
| **Confirmed current** | one quiet line per toolchain, naming the latest stable it just read and that the installed one matches | nothing. **Cite those version numbers** rather than re-running `rustup check` |
| **Could not confirm** — offline, missing tool | a short informational note | nothing. **Offline is not stale**, and you MUST NOT report it as though it were |
| **Confirmed behind** | an unmissable banner | **raise it with Terry before further work**, and offer to run the update |

**The middle row is the one to get right.** *I could not check* is a real answer and MUST NOT
be spelled like *current* — the same rule [`docs/REVIEWING.md`](docs/REVIEWING.md) applies to
`offload verify`, and the reason a flight with no wi-fi MUST NOT manufacture an alarm.

**And it is deliberately *quiet*, which is the part a future session will try to "improve".**
Volume tracks **actionability, not importance.** Terry, 2026-08-06: *"we need to never train
my brain to treat warnings as something to ignore. A warning that fires when I don't care —
e.g. on a plane and I can't fix the versions — is a warning that loses its teeth. It needs to
be a confirmed positive that I have learned to care about and be highly motivated to act
upon."*

**The banner is a conditioned response and every firing spends a little of it.** Fire it where
he cannot act and it buys nothing while costing some of the reflex; spend that often enough
and it becomes scenery, and then the evening it means *your linker is a release behind and
this measured run is worthless* he reads straight past it. **The loud shape is therefore
reserved for a confirmed positive** — proven by the network, fixed by one command, now.
*Could not confirm* may be the more worrying state and stays quiet anyway, because it is not
one he can act on.

**This project already makes that argument three times** — decision 12 (a verification tool
whose warnings you learn to ignore is worse than one that checks less and means it), decision
9 (a warning that fires regardless of the truth is the one you learn to read past), and
decision 34 (a repeating fact is INFO however much it matters). **Claude MUST NOT make the
unreachable case louder.**

**The cadence is deliberately two rules.** A clean result is suppressed for 24 hours; a
**behind** result re-fires once per session until it is fixed. A stale chain is actionable and
one command from repaired, so repeating it is correct — the opposite of decision 34's rented
body, which repeats *unfixably* and is INFO for exactly that reason.

**After any toolchain or linker change, `cargo clean` before anything you intend to trust.**
Cargo fingerprints `rustc` and not the linker, so an incremental build re-checks bytes the old
toolset produced and passes. The banner says so;
[`docs/TRIP-HYGIENE.md`](docs/TRIP-HYGIENE.md) carries the reasoning.

## The workflow

- **Commits go straight to `main`** — one maintainer, no PR. Self-review at commit time
  *is* the gate, at the same bar (`REVIEWING.md`).
- **A commit is not finished until it is pushed.** GitHub is the backup, and the laptop
  is usually on the road.
- **Every run gets a commit and a push first.** Standing order, Terry, 2026-08-05:
  *"Every time I do a run, you should commit and push — it gives us rollback spots."*

  **The reason is the rollback, not the backup**, and that is what makes it a rule rather
  than a nicety. A run is the only thing that tells you whether a change was good; a commit
  immediately before it is a known point to return to when the answer is no. Six rounds of
  display changes in one evening is exactly the shape that needs them — without commits, "put
  back the version from two runs ago" is a request nobody can satisfy.

  **It also makes every run reproducible for free.** The binary maps to a commit, so a screen
  that looked wrong can be diffed rather than remembered. `FULL-RUN.md` already demands a
  clean tree for *measured* runs (`binary is HEAD's`); **this extends it to every run**,
  including the casual ones — which is where the fast iteration actually happens.
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
