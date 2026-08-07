# Documentation standard

*Two standing orders share one diff: [`REVIEWING.md`](REVIEWING.md) for the code and its
tests, this file for the prose.*

*Carried over from RawGeotag, this project's predecessor, where these rules were derived
from real mistakes rather than from a style guide. Where a rule cites an incident, that
incident happened there.*

## The standing order

> **Every document leads with what its reader came for. Everything else comes after.**

## The second standing order: prose earns its place or goes

**Added 2026-08-06 at Terry's request, and the framing is his:** *"doc and test bloat seem to be
a side effect of vibe coding... this is a hobby project, we aren't launching nuclear missiles,
nobody's gonna die. Use a fresh pair of skeptical eyes on what REALLY is justified."*

### ⚠ STANDING ORDER 2026-08-07: the default stance is REMOVE

**Terry, verbatim:** *"start with a default stance to remove. If you can't justify removal, make it
as concise as possible. Standing order for this project."*

**Applies to all three: prose in `docs/`, code comments, and unit tests.** RFC 2119 keywords, and
the capitals are load-bearing.

**The burden of proof is inverted, and that inversion is the whole point.** The question is no
longer *can I find a reason to cut this?* — it is ***what argument keeps this?*** Anything that
cannot answer the second question **MUST** go, and anything that survives it **MUST** then be cut
to its shortest honest form.

| Stance | |
|---|---|
| **Default** | **Remove.** No argument needed to delete |
| **Keeping** | **MUST** be justified — say what it prevents *now* |
| **Kept** | **MUST** then be made as concise as it can be without losing what justified it |

**Why the inversion rather than a stricter version of the old rule:** a bar like *"prose earns its
place"* is still evaluated by a reader looking for reasons to keep, and a reason can always be
found — every comment in this repository was written because something went wrong. **Two review
passes applied that bar and the comment share did not move.** Flipping the default is what makes
the third pass different.

**This does NOT license deleting a rule, a measurement, or a safety-critical prohibition** because
it is inconvenient to justify. Those answer *what does this prevent* in one sentence, and the
answer is the justification. **The order removes the benefit of the doubt, not the evidence.**

### Sharpened 2026-08-07: past necessity is no justification at all

**Terry, verbatim:** *"diet means shrink if necessary, remove if not — just because a comment was
necessary in the past is zero justification to keep it."*

**The test is not *was this worth writing*. It is: delete it, and what breaks?**

| If deleting it would... | Then |
|---|---|
| let a reader make a mistake nothing else catches | **keep it** |
| change nothing, because **a test asserts it** | **cut it** — the test is the documentation, and it cannot go stale |
| change nothing, because **the code plainly says it** | **cut it** |
| change nothing, because **the mistake is no longer reachable** | **cut it** |
| lose an argument that `DESIGN.md` already owns | **cut to a one-line citation** |

**The trap this closes is that every surviving comment has a good origin story.** Each was written
because something went wrong, which makes every one of them feel load-bearing on inspection —
and that feeling is what kept a 31 % comment share sitting still through two review passes.
**A comment earns its place by what it prevents *now*, never by what it once explained.**

**The rows below still apply — they say what "would let a reader make a mistake" means in
practice.** They are the *shape* of a keeper, not a licence for one.

**A comment or a paragraph MUST buy something a reader could not get from the code.** Four things
qualify, and nothing else does:

| Keep | Because |
|---|---|
| **A finding that cost time to learn** | bold black renders as grey; the slow card was a bad card; the veto came from re-dismounting |
| **A rule, with enough why that it is not re-litigated** | never write to a camera card; red is banned |
| **A mechanism a reader would get wrong** | `MultiProgress` repaints wherever the cursor is |
| **A decision that looks wrong until explained** | cards excluded from the exit code |

**Cut on sight:**

- **The same argument in three places.** A finding belongs in *one* home — usually `DESIGN.md` —
  and everywhere else cites it. Code comments restating a decision in full are the main offender.
- **Narration of what the code says.** If the sentence tracks the statement below it, delete it.
- **Justification nobody asked for.** Defending an obvious choice teaches the reader that this
  codebase argues with itself.
- **Long quotation where a clause would do.** One vivid sentence of Terry's earns its place; three
  paragraphs of transcript do not.
- **Ceremony around a five-line function.** Thirty lines of doc on a function that sets two
  colours is the shape to watch for.

**Two guards, because the failure mode is regrowth rather than a bad first draft:**

- **Length is not thoroughness.** The instinct that produced the bloat is the same one that feels
  like diligence while producing it — every paragraph seemed worth writing at the time.
- **Deleting prose MUST NOT delete the finding.** If a paragraph is the only record of something
  measured, compress it, move it, or leave it — never drop it. **When in doubt about a finding,
  keep it; when in doubt about an explanation, cut it.**

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
| `RUNS.md` | someone asking what a full run cost | the narratives, newest first |
| `BACKLOG.md` | a session picking up work | what is in flight and what state it is in |
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
   **A document that says what a program prints MUST have those strings grepped against the
   source before the claim lands, and again whenever the section is touched.** They are
   checkable, so check them; **nothing in a Markdown table fails a test**, and a spec is
   uniquely good at hiding this — every row reads as a description of the tool, and a false
   one that has sat there a while looks the most settled of all. Four rows of decision 14's
   verdict table described output that had never been printed, and four rows of `DESIGN.md`
   described modules that did not exist. **Prove the pattern can find something before
   believing it found nothing** — a grep that was pointed at the wrong thing and a grep that
   found nothing are the same empty result, and the first one reads as *clean*.
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
