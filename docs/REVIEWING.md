# Code quality: no broken windows on main

*Carried over from RawGeotag, this project's predecessor. Where a table cites an instance,
that instance was found and fixed there — they are kept because concrete shapes are worth
more than generic advice, not because they happened here.*

## The standing order

> **A branch can be as ugly as it needs to be. `main` has no broken windows.**

The asymmetry is the whole policy, and both halves are load-bearing.

**On a branch, be as messy as the problem requires.** Spike it, hard-code it, copy and
paste it, leave the ugly `if` chain, skip the tests while you are still finding out what
the thing even is. Exploration that has to look presentable while it is happening is
exploration that gets abandoned early. Nobody is reviewing your branch.

**Then clean it before it goes near `main`.** Not "mostly clean", not "clean apart from
that one bit". The bar is that a reasonable reviewer reading the diff cold finds nothing
to wince at — no cringe, no debt, no "I'll fix that later", no commented-out experiment,
no leftover `dbg!`. If you would apologize for a line in the PR description, that line is
not ready.

## Why "broken windows"

From *The Pragmatic Programmer* (Hunt & Thomas), borrowing the criminology observation: a
building with one unrepaired broken window is soon a building with none intact. One is a
signal that nobody cares, and once that signal is up, the decline is fast and nobody feels
responsible for it.

Code does the same. One tolerated shortcut on `main` is a license for the next one, and
the third arrives without anyone deciding. **The cost is not the shortcut; it is the
precedent.** Repairing a window is cheap. Re-establishing that windows get repaired, after
a year of not, is not.

The book's other half is worth keeping too: **if you genuinely cannot fix it now, board it
up.** Damage that is visibly contained does not send the signal. A named `TODO` with a
reason and a bound is a boarded window; the same code with no comment is a broken one.

## The gate, honestly

There is one maintainer, and the workflow is commit straight to `main` — no branch, no PR.
**So the gate is self-review at commit time, and it is the same bar.** "Would this survive
a reviewer" is not softened by there being no reviewer; it is the only thing standing in
for one.

### What GitHub enforces

Two rulesets on `main`, added 2026-08-03. They are deliberately separate, because bypass
is per *ruleset*, not per rule — one combined ruleset with the maintainer on its bypass
list would have exempted him from the force-push block too.

| Ruleset | Rules | Bypass |
|---|---|---|
| `main: require pull request` | `pull_request` — 1 approval, **code-owner review required**, **squash the only permitted merge**, stale reviews dismissed on push, last push must be approved; plus `required_status_checks` on the CI job, strict | repository admin, always |
| `main: no force-push or deletion` | `non_fast_forward`, `deletion` | **none — binds the admin as well** |

**"One approval" is not the same as "the maintainer's approval"**, and the difference only
shows up once there are two collaborators who could rubber-stamp each other. So
`.github/CODEOWNERS` assigns every path to the maintainer and the ruleset requires a
code-owner review, which makes his approval specifically mandatory. Two related switches
are on for the same reason: `dismiss_stale_reviews_on_push` drops approvals when new
commits land, and `require_last_push_approval` stops someone approving a branch and then
pushing to it.

A code owner cannot approve their own pull request — which never bites here, because the
maintainer bypasses this ruleset and commits straight to `main`. A code owner also needs
write access or the rule silently matches nobody; check that before adding anyone to
`CODEOWNERS`.

**The first ruleset is a no-op today and that is the point.** The repository is public with
one collaborator, so a non-collaborator already cannot push at all — GitHub refuses it and
their only route is fork-and-PR. The ruleset exists so that stays true the moment anyone is
granted write access, rather than depending on nobody having been.

**The second is the one with teeth**, and it protects against the only account that can
actually damage `main`: the maintainer's. Rewriting or deleting published history is now
refused by the server rather than by remembering not to — the same preference for making a
mistake impossible that runs through the rest of this project.

Note that "Claude" is not a separate actor to allow-list. Commits are authored and pushed
as the maintainer, with Claude recorded in a `Co-Authored-By` trailer, so GitHub sees one
identity and any rule permitting the human permits the assistant.

### Every merge is a squash

**One pull request becomes exactly one commit on `main`.** Set in two places on purpose,
because they fail differently: the ruleset restricts `allowed_merge_methods` to `squash`
for `main` specifically, and the repository settings switch off merge commits and rebase
merges outright so the other buttons are not even offered. The ruleset is the enforcement;
the repo setting is what stops someone reaching for a button that would then be refused.

The squash commit is configured to take its **title from the pull request title** and its
**body from the pull request body**, rather than concatenating every "wip" and "fix typo"
message from the branch. That is deliberate given how much of this project's reasoning
lives in commit messages — a squashed history is only an improvement if the surviving
message is the considered one.

None of this touches the maintainer's workflow, which bypasses pull requests entirely. It
governs what arrives from anyone else.

### A merged branch is deleted immediately

**Standing order: the moment a branch is merged, it is gone.** A merged branch that
lingers is a broken window of the housekeeping kind — after a few of them nobody can tell
at a glance which branches are live, and the signal that goes up is that nothing here is
tended.

`delete_branch_on_merge` is **on**, so GitHub deletes the head branch automatically the
instant a pull request merges. That is the forced part, and it needs no discipline from
anyone.

**Two cases it cannot reach, which are therefore standing orders rather than rules:**

- **Branches on a fork.** GitHub deletes branches in *this* repository; a contributor
  working from their own fork owns that branch and only they can remove it. Delete it
  after your PR merges.
- **Branches that are never merged.** An abandoned spike, or a PR closed without merging,
  leaves the branch behind and no setting fires. Delete it when you abandon it — the point
  at which you know it is dead is the point at which you are the only person who knows.

Locally, `git branch -d <name>` refuses anything not merged, which is the safe form;
`git fetch --prune` clears remote-tracking refs for branches GitHub has already removed.

**Deliberately not automated:** a scheduled job that reaps stale branches. It is standing
infrastructure with a permanent carrying cost, aimed at a repository that has had exactly
one branch its whole life. If branch clutter ever becomes real, revisit it then.

## What counts as a broken window here

Derived from a real review pass over RawGeotag, not from a style guide. Every row is
something that was actually found and fixed, which is why these and not the usual generic
advice:

| Shape | The instance |
|---|---|
| Reimplementing what a dependency already gives you | a hand-rolled scratch-directory type, duplicated in two modules, while `tempfile` was already a dependency **and cited in `Cargo.toml` for exactly those properties** |
| A function long enough to hide its own control flow | `run()` at 159 lines with a dozen mutable accumulators, one of them incremented in two loops 55 lines apart |
| Two types modeling the same thing differently | one enum repeated `path` in every variant; its neighbour did not |
| Passing an owned value by reference, then cloning out of it | a `PathBuf` cloned per photo because the function took `&Photo` |
| Rebuilding a constant inside the loop | limits reconstructed per photo, because `&Args` was threaded in instead of resolved settings |
| `pub` that buys nothing | a type nothing outside the module could construct or receive |
| The same normalization written twice, with a loose primitive | `trim_start_matches('.')` where `strip_prefix('.')` was meant, in two modules |
| A data table whose shape the code does not honor | an `extensions()` returning a slice, while the directory walk matched only the string the user typed |
| A module reaching up into the binary root | a leaf module calling `crate::format_utc` |
| A runtime assertion where the type system would do | `unreachable!` guarding a state that consuming the value made unrepresentable |
| Dead conditions | `clamp(-1.0, 1.0)` on a value that cannot go negative |
| Over-permissive parsing | an offset parser that accepted `+0:0:00` and `+::0700` |
| Comments describing constraints that do not exist | a note about borrow ordering on a type that is `Copy` |

**The recurring test, if you want one line:** would an experienced reviewer, reading this
cold, have to ask why? If yes, either change the code or write down the answer.

## What is *not* a broken window

The policy is not license to relitigate. Specifically:

- **A settled decision you would have made differently.** [`DESIGN.md`](DESIGN.md) records
  its decisions with their reasoning. Disagreeing is fine; say so explicitly rather than
  quietly diverging. Reopening one needs new evidence, not fresh taste.
- **Deliberate simplicity.** A flat `Vec` and a linear scan are not debt when the input is
  bounded by what a human types on a command line. Speculative generality is the defect,
  not its absence.
- **A recorded gap.** A known gap with a written reason is a boarded window and stays
  boarded until the reason stops holding.
- **Verbosity that buys clarity.** This project takes the obvious mechanism over the clever
  one on purpose. Longer and duller is not a window.

## Tests: four of them, and each has to be able to fail

[`DESIGN.md`](DESIGN.md) decision 18 sets the scope deliberately narrow — the phase 4
deletion path, the naming function, one end-to-end happy path, and `verify` against a
committed schema-1 manifest fixture. Everything else is untested on purpose.

**That makes the bar on each surviving test higher, not lower:**

> **Write the test, then break the thing it guards and confirm it fails — ideally that it,
> and only it, fails. Revert immediately.**

A green test proves the code passes today. It does not prove the test would notice if the
code stopped being right. With only four tests there is no redundancy to cover for one
that turns out to be decorative.

RawGeotag produced a worked example worth knowing. A test asserted that `collect_paths`
returned sorted results — on filenames the filesystem already yielded in order. It passed,
it looked like coverage, and deleting the sort it existed to guard changed nothing. The
fix was to build it on two names differing only in case: NTFS enumerates case-insensitively
while `PathBuf`'s `Ord` is byte-wise, so only an explicit sort produces the asserted order.
**A test whose subject is an ordering has to be built on inputs the underlying source does
not already order for you**, or it measures the filesystem rather than the code.

**This project produced its own on 2026-08-05, and it is a different enough shape to be
worth keeping beside it.** Four operator-facing strings had been reflowed so that fourteen
to eighteen literal spaces sat mid-sentence — `the logger stopped··················and
restarted` — and printed that way, one of them in `verify`'s verdict line. Two of the four
had tests, and both passed: they asserted `note.contains("logger stopped")`, a fragment
that ends exactly where the hole begins. **A test that asserts a fragment on one side of a
string's line break cannot see a defect at the seam** — so assert across the join, or the
continuation is unguarded.

Two things make that worth a rule rather than a shrug. **Nothing else in the toolchain
looks at string contents**: `cargo fmt` does not reformat them, clippy has no lint for
them, and a green suite is therefore silent about every literal the program prints. And it
arrived in two separate commits on one day, so it is an editing artifact that recurs rather
than a typo — the scan is a regex for a run of three or more spaces between two word
characters inside a `"…"`, and the only legitimate hits are the report's own column
alignment.

**If a mutation produces no failure, the test is decorative. Fix it then, while you still
know what it was meant to catch.**

## Measurements are evidence, and evidence has a bar

**This project decides things by measuring.** Decision 17 picked the hash on a measured
rate and says to re-run it when the crates or the laptop change; decision 15 sizes
`--jobs` on measured behavior; the wall-clock table in [`DESIGN.md`](DESIGN.md) is
measured, and replaced an estimate that was wrong. `examples/` carries the harnesses that
re-take those numbers. So a bad measurement is not a private mistake — **it gets written
into a decision and cited afterwards by people who cannot see how it was taken.**

> **A throughput number is a measurement only if nothing else was using the same bus.**
> Otherwise it describes contention and is worth less than no number at all.

This is not hypothetical. On 2026-08-03 a card's read speed was taken *while a full
188 GB offload was in flight*, reported as fact, and pushed into the wall-clock table.
The correction is still visible there.

**What made it survive scrutiny is the part worth internalising.** The figure was not
merely asserted — it was defended with a request-size sweep, flat at ~60 MB/s across
1–32 MiB plus a buffered comparison. Flat across a 32× sweep reads as decisive proof that
the *device* is the limit. It is not: **a device starved of bus bandwidth also reads flat
at every request size**, because each size gets the same small share of a saturated link.
The evidence fit both explanations and the one already believed was chosen.

**The sequel is worse, and it ran for months.** That same card was re-measured on a quiet
bus and came back **unchanged**, which looked like the end of it: the methodology had been
wrong and the number right. Two further contaminants were then found and removed — a
corrupted filesystem, and heat — and the number still barely moved. Every objection had
been answered. **The card was faulty, and a different card in the same reader read 2.8×
faster.**

> **Clearing a confound is not the same as finding the cause.** A valid objection that
> points away from the real fault is more dangerous than no objection, because answering
> it feels like progress and buys the wrong theory more credibility each time.

**What should have caught it years earlier is embarrassingly simple.** Card-versus-reader
has exactly two tests — same card in a different reader, and a different card in the same
reader. The first was run **twice**, both times agreeing, and each agreement was read as
confirmation. The second was never run at all. **When two runs agree, you have learned
what that variable does; the information is now entirely in the one you have not changed.**

So, before a number goes in a document or a commit message:

- **State what else was touching the bus.** If a run was going, say so or re-take it.
- **A flat response to a swept parameter is not proof of a device limit.** Ask what else
  produces flatness — saturation does.
- **Name the component you are blaming, then ask what would exonerate it.** If the answer
  is a swap you have not made, the number is not evidence about that component yet.
- **Change the variable you have not changed.** Re-running the test that already agreed
  adds no information, however clean the conditions get.
- **Prefer the shape that distinguishes.** Solo, then in company;
  `examples/contention.rs` exists for exactly that and reports what each device lost.
- **Read the second pass.** Storage measured straight after a bulk write reports its
  garbage collection, not its speed — one card here climbed 176 → 213 MB/s through a first
  pass and opened its second at 207. `examples/sustained.rs` shows the curve instead of
  hiding it in a mean.
- **Say which device and which link.** On this rig the four destinations differ by 3× and
  the two card readers by 10×, so "the SSD" and "the card" are not units.

None of this is specific to one rig. The numbers are; the standard is not.

## A review is always all four

**"Do a deep dive review" means all four of these, every time, without being asked for them
separately:**

1. **Code**
2. **Tests** — held to the bar above, not merely "they pass"
3. **Code comments** — in the code *and* in the tests
4. **Docs** — everything in `docs/`

They are one request, not four, because they fail together. Removing a CLI argument in
RawGeotag left stale invocations in the README, a comment naming a function that no longer
existed, and a sample output that no longer matched. Reviewing one dimension and not the
others just moves the broken window somewhere less visible.

The same applies in the other direction. **Changing any one of the four is reason to look
at the other three**, because a change rarely stays in its lane:

| A change to… | …routinely stales |
|---|---|
| code | tests pinning the old shape; comments naming what moved; every doc showing a command or a sample output |
| tests | the comments inside them, which are what explain why a case exists at all |
| comments | nothing else, but they drift from the code faster than anything else |
| docs | little, though a fact corrected in one usually belongs in a code comment too |

## Before you push to main

Two standards apply to the same diff:

- **this file** — the code and its tests
- [`WRITING.md`](WRITING.md) — every document leads with what its reader came for, and
  comments explain *why*, not *what*

Mechanically, that is:

```
cargo clippy --all-targets -- -D warnings
cargo test
```

A green suite is the floor, not the bar: **clippy has no opinion about any row in the
broken-window table above.**

### What runs automatically

Two layers, and the second is not redundant with the first:

| | Where | Runs | Skippable |
|---|---|---|---|
| `.githooks/pre-commit` | your machine, before the commit exists | fmt, clippy, test | `--no-verify`, and only present if the clone was wired up |
| `.github/workflows/ci.yml` | GitHub, on push to `main` and on every PR | the same three | no |

CI is also a **required status check** on the pull-request ruleset, in strict mode — so a
PR cannot merge while the checks are red, and cannot merge on a stale base either: strict
means the branch must be up to date with `main` first. That is what turns CI from a
notification into a gate. It does not apply to the maintainer's direct pushes, which
bypass the ruleset; there CI reports after the fact.

The hook is the layer that saves you time, because it catches a problem before it is in the
history. CI is the layer that cannot be talked out of it, and the only one that sees a pull
request from a fork.

**Wire the hook up once per clone** — git does not track `.git/hooks`, so a hook living
only there protects nothing on a fresh checkout:

```
git config core.hooksPath .githooks
```

It skips the Rust checks entirely on a docs-only commit, which is most of them so far.
