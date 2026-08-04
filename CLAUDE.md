# PhotoTravelWorkflow — working notes for Claude

`photoday`: one command, run from a hotel room at the end of a shooting day, that lands
four verified copies of the day's photographs. A Rust workspace on Windows, for one
photographer and one rig.

**The design is settled and written down.** Do not re-derive it, and do not quietly
diverge from it — if a decision looks wrong, say so explicitly and cite what changed.

## Read before you write

| Before you… | Read | Its standing order |
|---|---|---|
| propose or write anything | [`docs/DESIGN.md`](docs/DESIGN.md) | 29 numbered decisions, each with its reasoning, plus what was considered and rejected |
| change what the operator does | [`docs/CONOPS.md`](docs/CONOPS.md) | the nightly ritual and the shooting-day contract the guarantees rest on |
| put anything on `main` | [`docs/REVIEWING.md`](docs/REVIEWING.md) | a branch can be as ugly as it needs to be; `main` has no broken windows |
| write a document or a comment | [`docs/WRITING.md`](docs/WRITING.md) | every document leads with what *its* reader came for |
| touch a dependency | [`docs/UPDATING.md`](docs/UPDATING.md) | once per trip, before you leave — never on the road |

**"Deep dive review" always means all four:** code, tests, code comments (in the code
*and* in the tests), and docs. `REVIEWING.md` has the table of what stales what, and why
"the tests pass" is not a review of the tests.

## The two optimization metrics

In strict priority order. Nearly every decision in `DESIGN.md` traces back to one:

1. **Wall clock from launch to LANDED** — all four copies written and read-back verified.
   That moment is the product.
2. **Wall clock to run complete** — corroboration, geotags, report. Worth shrinking,
   **never at any cost to the first.**

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

## This machine is the rig

The laptop these sessions run on is the i7-13700H the tool targets. So **measure rather
than estimate** — decision 17's hashing table was an estimate for months and moved when
someone finally ran it. `cargo run --release --example hash-rate` re-runs that one.

Estimates presented as measurements are the specific failure to avoid; if you did not run
it, say the number is an estimate.

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

## RawGeotag is the predecessor, not a library — yet

[`../RawGeotag`](../RawGeotag) is the same author's geotagging tool. Its CR3, GPX and XMP
engine is validated against thousands of real files and is destined for `crates/geotag`
here (decision 17); until that lift happens, `crates/geotag` does not exist and its
dependencies sit declared-but-unused in the workspace manifest, deliberately.

Its `CLAUDE.md` and `docs/` carry findings this project inherited rather than re-derived —
including the measured concurrency behavior and the Lightroom XMP verification record.
Read them before re-litigating anything about EXIF, GPX or XMP.
