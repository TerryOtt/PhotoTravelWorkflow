# PhotoTravelWorkflow — working notes for Claude

`photoday`: one command, run from a hotel room at the end of a shooting day, that lands
four verified copies of the day's photographs. A Rust workspace on Windows, for one
photographer and one rig.

**The design is settled and written down.** Do not re-derive it, and do not quietly
diverge from it — if a decision looks wrong, say so explicitly and cite what changed.

## Start every session here

**First, one file decides whether this session may touch the drives at all.** If
**`RUN-STATE.json`** exists at the repository root, a measured end-to-end run is staged or
in flight. Read [`docs/FULL-RUN.md`](docs/FULL-RUN.md) before any other tool call, and until
that file is gone, **read no file data from any camera card or destination** — no hashes, no
copies, no `--dry-run`, no `examples/` probe, and no walking the archive trees "just to
look". The usual reason it exists is a reboot taken to get a cold page cache, and a
walk-about spends exactly what the reboot bought.

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
| propose or write anything | [`docs/DESIGN.md`](docs/DESIGN.md) | 32 numbered decisions, each with its reasoning, plus what was considered and rejected |
| change what the operator does | [`docs/CONOPS.md`](docs/CONOPS.md) | the nightly ritual and the shooting-day contract the guarantees rest on |
| put anything on `main` | [`docs/REVIEWING.md`](docs/REVIEWING.md) | a branch can be as ugly as it needs to be; `main` has no broken windows |
| write a document or a comment | [`docs/WRITING.md`](docs/WRITING.md) | every document leads with what *its* reader came for |
| touch a dependency | [`docs/UPDATING.md`](docs/UPDATING.md) | once per trip, before you leave — never on the road |
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

1. **Pure Rust.** No ExifTool, no C-library bindings. Microsoft's `windows` crate is not
   an exception to this — it is generated bindings to DLLs the OS has already loaded.
2. **The tool never writes to a camera card.** Not a byte, under any flag. Formatting is
   an in-camera act by the operator.
3. **Raw files are never modified.** All derived data goes to sidecars and manifests.
4. **It runs unelevated**, and nothing in a run may come to need administrator rights.
5. **Readable over clever**, per the same rule RawGeotag runs under: prefer the obvious
   mechanism, and prefer a mistake that is a compile error over one that is a runtime
   surprise.

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
evidence of being current.** `UPDATING.md` names which crates here are exposed.

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
with keeps the bug.** That ends when decision 30 retires RawGeotag into `photoday geotag`,
which cannot happen until phase 5 works; until then, treat the duplication as live.

Its `CLAUDE.md` and `docs/` carry findings this project inherited rather than re-derived,
and several comments in `crates/geotag` cite them by name — the NEF read-strategy
measurements, the CR3 timezone trap, the gap rule, and `docs/LIGHTROOM-XMP.md`, whose
procedure drives `rawgeotag.exe` and so cannot move here until phase 5 can run it. Read
them before re-litigating anything about EXIF, GPX or XMP.
