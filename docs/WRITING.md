# Documentation standard

*Two standing orders share one diff: [`REVIEWING.md`](REVIEWING.md) for the code and its
tests, this file for the prose.*

*Carried over from RawGeotag, this project's predecessor, where these rules were derived
from real mistakes rather than from a style guide. Where a rule cites an incident, that
incident happened there.*

## The standing order

> **Every document leads with what its reader came for. Everything else comes after.**

For the README that reader is the 98% case — someone deciding whether this tool is for
them and then trying to run it. They get *what it does*, then *how to run it*, and
nothing stands between those two. Philosophy, benchmarks and verification evidence all
matter, and all of them come later.

**The rule generalises past the README, but the reader does not.** Nobody reads
`TRIP-HYGIENE.md` casually. Applying "write for the 98% user" to a maintainer document would
be as wrong as burying the README's first command — the point is not that every document
is for beginners, it is that every document opens with the thing *its own* reader arrived
wanting.

So the first question for any document is **who opens this, and what were they after?**

| Document | Its reader | What they came for |
|---|---|---|
| `README.md` | someone evaluating or running the tool | what it does, then a command that works |
| `CONOPS.md` | the operator, mid-trip | the ritual, and what to do when it goes sideways |
| `DESIGN.md` | someone changing the design | the settled design and why |
| `REVIEWING.md` | someone about to put a change on `main` | the bar it has to clear |
| `TRIP-HYGIENE.md` | someone preparing to leave on a trip | the routine, in the order it has to happen |
| `FULL-RUN.md` | someone about to record what a full run cost | the sequence that makes the number comparable |
| `CLAUDE.md` | a Claude session, cold | which of these to read before touching anything |
| this file | someone writing or reviewing a document | the standing order |

Get that wrong and the document reads as though it were written for its author. Both
corrections made in RawGeotag on 2026-08-02 were exactly that mistake: the README opened
with 122 lines of reasoning before a runnable command, and its design doc opened with two
sections marked **COMPLETE** before any design.

## Rules

1. **Lead with the reader's goal.** Rationale, history and evidence go after it — or into
   an appendix if they are finished business.
2. **One canonical place per fact.** Where a summary must repeat something, it names its
   source and the two are corrected together. Carry the *caveat* across, not just the
   number; a stale caveat is what has actually drifted, twice.
3. **No hand-maintained counts.** Test totals, file tallies, "done three times so far" —
   all of them go stale, and a test count did it three separate times. Name the command
   that answers the question instead.
4. **Record decisions and their reasons, not restatements.** "Prints the summary" above
   `print_summary` is noise. Why the column is seven characters wide is not.
5. **Correct by appending, never by rewriting.** A note saying what was previously claimed
   and why it was wrong is worth more than a clean-looking record — the reader learns the
   shape of the mistake, which is what stops it recurring.
6. **Numbers ≥ 1,000 carry thousands separators**, in prose as well as in program output.
   Exceptions: Rust literals, text quoted verbatim from another tool so it stays
   greppable, and years, model numbers, offsets and coordinates.
7. **A rule with no exception says MUST NOT, in capitals, per RFC 2119.** Requested by
   Terry 2026-08-05: *"if we mean NO NEVER, let's be consistent on that wording."* The
   capitals are the signal that judgment has no room here — `MUST`, `MUST NOT`, `SHALL NOT`
   for absolutes; `SHOULD` for a strong default a good argument may overrule; `MAY` for a
   genuine option. A document using them **states so once, at the top of the section that
   uses them.**

   **The discipline is in what you do *not* capitalize.** Prose "never" is usually
   descriptive — *"the operator is never practiced"*, *"Lightroom never opens one of the
   four"* — and those are facts about the world, not obligations; capitalizing them would be
   wrong. And a preference dressed as an absolute devalues the real ones, which is decision
   34's argument about exit 2 applied to prose: **spend the strong word on the things that
   have no exception, or it stops meaning anything.** The test: *can I imagine a good
   argument for doing this anyway?* If yes, it is a SHOULD.

   Where they are used today: the reboot prohibition ([`FULL-RUN.md`](FULL-RUN.md), global
   `CLAUDE.md`), binding constraints 1–4 ([`../CLAUDE.md`](../CLAUDE.md)), the `RUN-STATE.json`
   gate, the firmware freeze ([`TRIP-HYGIENE.md`](TRIP-HYGIENE.md)), the measured-run
   checklist ([`FULL-RUN.md`](FULL-RUN.md)) and daily hygiene ([`CONOPS.md`](CONOPS.md)).

8. **One name per concept, and these are the names.** A recurring thing gets one term and
   keeps it — in docs, code comments, commit messages, memory and conversation alike. Coining
   a fresh synonym is not a stylistic choice; it splits one searchable idea into several and
   turns one habit into several partly-performed ones.

   | The concept | The term | Never |
   |---|---|---|
   | The whole at-home pre-departure routine | **trip hygiene** ([`TRIP-HYGIENE.md`](TRIP-HYGIENE.md)) | "pre-trip prep", "the update pass", "getting ready" |
   | The camera-config check before each shooting session | **daily hygiene** ([`CONOPS.md`](CONOPS.md)) | "the morning check", "pre-shoot setup" |
   | The act the tool performs, and phase 3's heading on screen | **offload / offloading** | "ingesting", "importing", "copying" |
   | Phases 1 and 2, and their heading on screen | **pre-flight checks** | "startup checks", "validation", "the checks" |
   | All four copies written and read-back verified | **LANDED** ([`DESIGN.md`](DESIGN.md) decision 14) | "done", "finished", "complete" |
   | The end-of-day offload the operator performs | **the nightly ritual** ([`CONOPS.md`](CONOPS.md)) | "the workflow", "the process" |

   **The tool's own vocabulary is aviation, and that is deliberate rather than decorative.**
   A run opens with **pre-flight checks** and ends at **LANDED** — one metaphor, carried end to
   end, in the operator's own idiom. It earns its place by being *load-bearing*: a pre-flight
   check is understood to be a thing you complete before committing, and landing is understood
   to be the moment the risk is over. Both are exactly what those two stages mean here. A new
   stage name **SHOULD** come from the same place rather than starting a second metaphor.

   **`Eject` is the exception and MUST NOT be renamed to fit.** Terry spotted it: in aviation
   the word belongs to a very bad day, which makes the run read *pre-flight checks, landed
   safely, ejected* — backwards, and impossible in that order. It stays anyway, because it is
   not borrowed from aviation at all: it is what Windows calls the tray icon and what the API
   call is named (`CM_Request_Device_Eject`). **The operator's instrument wins over the
   metaphor's tidiness**, the same rule that puts sizes in GiB and rates in decimal.

   **The two hygienes are a deliberate pair and the names should stay parallel** — same
   noun, different cadence: *trip* hygiene runs once, at home, and covers the whole rig;
   *daily* hygiene runs every session, in the field, and covers only the camera config.
   Keeping the shape identical is what makes "which hygiene is this?" a question with an
   obvious answer.

   **Trip hygiene is the newest and the one most at risk of drifting**, since it names a
   routine that existed for months under no name at all and was described a different way
   each time it came up. The term is the operator's, adopted 2026-08-05, and it is
   deliberately a *noun for the whole thing* rather than a verb for one of its steps —
   which is what lets a calendar rule attach to it.

## Comments are documentation too

Same rules, one addition: a comment earns its place by explaining something the code
cannot. Names and signatures already say *what*; comments are for *why this and not the
obvious alternative*, and for the trap that is invisible at the call site.

The bar that has worked: **would a reader otherwise repeat this mistake?** A bare
`paths.sort_unstable()` needs a comment because deleting it breaks nothing that fails
loudly. `fn print_summary` does not need one.

## Signals you have buried the lead

Cheap to check, and each one has actually happened:

- The first runnable command is below the fold.
- The reader must scroll past a section marked *COMPLETE* or *retained as a record*.
- Two consecutive paragraphs make the same move — a scope caveat stated twice, a rule
  restated three times for emphasis.
- A "that last one" or "the table above" that no longer points where it did, because
  something was appended between them.
- A section's heading covers only its first third.
- A file is loaded into every session and nobody can say what it would cost to lose any
  given paragraph.

## When a document outgrows its home

In RawGeotag both the testing and writing standards began inside the design document. The
signal was the same in both cases: a section had become a fifth of a document whose
subject it was not, and it was still growing.

**Move it, do not copy it.** Leave a short pointer where it was, so the one-canonical-
place rule survives the split.
