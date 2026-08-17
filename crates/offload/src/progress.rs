//! Progress reporting — the difference between *working* and *hung*.
//!
//! **This exists because the operator watched a drive's activity LED to work out which phase was
//! running.** The run printed `ingesting 3,883 files...` and then nothing for twelve minutes.
//! **For a walk-away tool, a screen that cannot say whether it is working is the one thing that
//! makes an operator stay and watch it.**
//!
//! # Three modes, because a progress bar is useless in a log file
//!
//! **`indicatif` disables itself when its stream is not a terminal**, returning a hidden draw
//! target on `!term.is_term()` — so bars alone would render nothing in the mode that matters
//! most, since `CONOPS.md` has the operator running the offload captured to a file.
//!
//! | Mode | When | What it does |
//! |---|---|---|
//! | [`Progress::Bars`] | stderr is a terminal | live bars, one per destination |
//! | [`Progress::Lines`] | stderr is redirected | a plain line to **stdout** every tenth of a pass |
//! | [`Progress::Silent`] | tests | nothing |
//!
//! Two genuine behaviors and an absence is a plain enum, not the trait `CLAUDE.md` says to push
//! back on.

use std::cell::{Cell, RefCell};
use std::io::IsTerminal;
use std::sync::Mutex;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

/// How deep a heading and its rows sit, in spaces.
///
/// **Indent is a parameter rather than a constant because the phases are not all siblings.**
/// `Pre-Flight`, `Offloading`, `Corroborating` and `Geotagging` are peers at column 0, but
/// `Writing` and `Verifying` are the two *passes of* offloading — they belong under it, and a
/// flat list would claim otherwise. Rows always sit `STEP` further in than their heading, so
/// the two cannot drift apart at a call site.
pub const PHASE: usize = 0;

/// A heading nested under a phase — the `Writing` and `Verifying` passes of `Offloading`.
pub const PASS: usize = 4;

/// One level of the hierarchy.
const STEP: usize = 4;

/// The template a blank spacer line is drawn with.
///
/// **A literal space, never an empty string.** An empty template renders no line at all, so the
/// first attempt at blank lines produced no gap and looked exactly like the bug it was meant to
/// fix. Named rather than inlined so the test can read the value the code actually uses.
const SPACER: &str = " ";

/// Blank lines above a heading: two for a phase, one for a pass.
///
/// **A phase is a boundary in the run; a pass is a subdivision of one.** `Pre-Flight Checks`,
/// `Offloading`, `Corroborating` and `Geotagging` each start something new and get the wider
/// gap; `Writing` and `Verifying` are two halves of offloading and get the narrower one.
fn leading_blanks(indent: usize) -> usize {
    if indent == PHASE { 2 } else { 1 }
}

/// A blank line *under* a heading, for phases only.
///
/// A pass heading and its four destination rows read as one block; separating them would
/// break the group up rather than set it apart. A phase heading introduces a whole stage, and
/// its first line sat jammed against it until 2026-08-05.
fn trailing_blanks(indent: usize) -> usize {
    if indent == PHASE { 1 } else { 0 }
}

/// A heading at `indent` spaces.
fn section_template(indent: usize) -> String {
    format!("{:indent$}{{msg}}", "")
}

/// A destination row, one `STEP` in from its heading.
///
/// `human_pos` and `human_len` are `indicatif`'s own separator-formatted counters, which is
/// `WRITING.md` rule 6 applied to the bars — they rendered a bare `3883` until 2026-08-05 while
/// the report printed `3,883`. `pct` is registered in [`Progress::bar`] because `indicatif`'s
/// built-in percent is a whole number, and one decimal is what keeps the slowest destination
/// from appearing to stall (see [`crate::human::percent`]).
///
/// Widths hold to 6 digits — `999,999` — where the biggest day on record is 7,350 frames.
fn row_template(indent: usize) -> String {
    let pad = indent + STEP;
    format!(
        "{:pad$}{{prefix:<8}} {{bar:28}} {{human_pos:>6}}/{{human_len:<6}} {{pct:>6}}  {{etc:<16}}{{msg}}",
        ""
    )
}

/// How often the bars may repaint, in hertz.
///
/// **One, because the estimate is honest and unstable at the same time.** A row advances once
/// per file — six or so a second on this rig — and `indicatif` repaints on every advance, so
/// the ETC recomputed and flickered several times a second while it was still settling. Terry,
/// 2026-08-05: *"I don't mind the VALUE jumping often, it's the Hz of updates."*
///
/// **The fix is the paint rate, not the estimate.** Smoothing the number would be lying about
/// what is known; painting it less often is just not shouting. Capping the draw target also
/// costs nothing on the run itself — fewer terminal writes on the thread that is moving two
/// hundred gigabytes.
const REDRAW_HZ: u8 = 1;

/// How far into a pass an estimate has to be before it is worth showing.
///
/// **Ten per cent, and the number is the operator's observation rather than a guess.** Terry,
/// 2026-08-05: *"they start at 15m and settle around 10%."* Below that the sample is a handful
/// of files against a pipeline that is still filling its queues, so the figure swings wildly —
/// and a number that swings is worse than no number, because the reader cannot tell whether
/// the run changed or the estimate did.
///
/// This is the same judgment as the blank at 100 %, one pass earlier: **show the estimate only
/// while it means something.**
const ETC_FROM_FRACTION: f32 = 0.10;

/// Plain-text updates per pass. Ten is a deliberate ceiling rather than a rate: it bounds a
/// long run's log at a knowable number of lines regardless of how many frames the day holds,
/// where a time-based throttle would produce forty lines on a slow night and four on a fast
/// one for the same information.
const UPDATES_PER_PASS: usize = 10;

/// How the run reports progress. See the module note for why there are three of these.
#[derive(Debug)]
pub enum Progress {
    /// stderr is a terminal: live bars, and every bar and heading handed out so far.
    ///
    /// **The second field is what makes [`Progress::clear`] work.** `MultiProgress::clear`
    /// erases what is drawn and keeps owning the bars, so the next bar added anywhere causes a
    /// redraw of the whole set — which on 2026-08-05 put eight finished phase 3 rows back on
    /// screen underneath the LANDED report that had replaced them. Retiring a bar means
    /// removing it from the multi, and that needs a handle after the worker thread has dropped
    /// its own.
    Bars(MultiProgress, Mutex<Vec<ProgressBar>>),
    /// stderr is redirected: throttled plain lines on stdout.
    Lines,
    /// Tests, and anything that wants nothing.
    Silent,
}

impl Progress {
    /// Bars at a terminal, plain lines when redirected.
    ///
    /// **The check is on stderr because that is where bars are drawn**, and the two streams
    /// are deliberately different: the report is stdout and is meant to be kept, while bars
    /// are ephemeral and would litter a saved log with half-drawn frames.
    pub fn detect() -> Self {
        if std::io::stderr().is_terminal() {
            Self::Bars(
                MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(REDRAW_HZ)),
                Mutex::new(Vec::new()),
            )
        } else {
            Self::Lines
        }
    }

    /// Report nothing at all.
    pub fn silent() -> Self {
        Self::Silent
    }

    /// A pass heading, with the destination rows for that pass printed under it.
    ///
    /// **Both sections exist from the start and a destination appears in both.** Phase 3 has no
    /// barrier between the passes — each destination verifies the moment *its own* writes finish
    /// — so `Verifying` fills underneath `Writing` while rows above are still moving. **That
    /// overlap is the live form of the heterogeneity decision 14 calls the report's most useful
    /// number**, and a barrier to tidy it would idle the fast drives for nothing.
    ///
    /// **The returned [`Section`] MUST be held for as long as the heading should stay on
    /// screen.** `ProgressBar`'s `Drop` finishes an unfinished bar and takes its line with it, so
    /// dropping the handle removes the heading — silently, and only in `Bars`.
    #[must_use = "dropping the Section removes the heading from the screen"]
    pub fn section(&self, title: &str, indent: usize) -> Section {
        match self {
            Self::Bars(multi, drawn) => {
                // **The blanks belong to the heading rather than to the caller**, and how
                // many depends on the level: a phase is a boundary in the run, a pass is a
                // subdivision of one, and two blanks against one says that without a word. A
                // rule each call site has to remember is a rule one call site will forget.
                //
                let blank = || {
                    let spacer = multi.add(ProgressBar::new(0));
                    if let Ok(style) = ProgressStyle::with_template(SPACER) {
                        spacer.set_style(style);
                    }
                    spacer.tick();
                    spacer
                };

                let mut lines: Vec<ProgressBar> =
                    (0..leading_blanks(indent)).map(|_| blank()).collect();

                let heading = multi.add(ProgressBar::new(0));
                if let Ok(style) = ProgressStyle::with_template(&section_template(indent)) {
                    heading.set_style(style);
                }
                heading.set_message(title.to_owned());
                // Forces a first draw. A bar that is never advanced is never rendered, so a
                // heading — which by definition never advances — needs this or it is invisible.
                heading.tick();
                lines.push(heading);

                // A gap under the heading too, so its first row does not sit jammed against
                // it. Only phases get this: a pass heading and its four destination rows read
                // as one block, and separating them would break the thing up rather than
                // group it.
                lines.extend((0..trailing_blanks(indent)).map(|_| blank()));

                if let Ok(mut drawn) = drawn.lock() {
                    drawn.extend(lines.iter().cloned());
                }
                Section { lines }
            }
            Self::Lines => {
                for _ in 0..leading_blanks(indent) {
                    println!();
                }
                println!("{:indent$}{title}", "");
                for _ in 0..trailing_blanks(indent) {
                    println!();
                }
                Section { lines: Vec::new() }
            }
            Self::Silent => Section { lines: Vec::new() },
        }
    }

    /// Whether [`Progress::clear`] actually erased anything — and so whether the report section
    /// that follows has to reprint its own heading.
    ///
    /// **Only [`Progress::Bars`] clears.** At a terminal, `clear` takes the bars *and* the
    /// heading above them, so the record that follows must restate it or the section arrives
    /// unlabelled. In a captured log nothing is erased, the heading is still sitting there, and
    /// restating it printed `Corroborating` twice — a stutter in exactly the mode the operator
    /// actually reads, since he runs the offload through Claude whenever he has internet.
    ///
    /// **One source of truth rather than a second `is_terminal()` call**, which would be free
    /// to drift from the one in [`Progress::detect`] and produce a heading that is right in
    /// neither mode.
    pub fn heading_was_erased(&self) -> bool {
        matches!(self, Self::Bars(..))
    }

    /// Hand the terminal back, erasing every bar and heading this phase drew.
    ///
    /// **Call this before printing anything at the end of a phase.** `MultiProgress` owns a
    /// block of the screen and repaints it wherever the cursor happens to be, so an ordinary
    /// `println!` while it is live does not appear below the bars — it collides with them.
    /// On 2026-08-05 that put the LANDED banner in the middle of eight progress rows, with the
    /// rows drawn twice around it.
    ///
    /// **Nothing is lost by clearing, because every erased row has a durable counterpart** in the
    /// report — the per-destination counts and the badged `Writing` / `Verifying` records above
    /// `LANDED`. **The bars answer *which drive am I waiting on right now***, which stops being a
    /// question the moment the phase ends.
    pub fn clear(&self) {
        if let Self::Bars(multi, drawn) = self {
            // **Retire, then erase.** Removing each bar from the multi is what stops the next
            // phase's first redraw from bringing this phase's rows back; `clear` alone only
            // wipes the screen and leaves the multi still owning them.
            if let Ok(mut drawn) = drawn.lock() {
                for bar in drawn.drain(..) {
                    multi.remove(&bar);
                }
            }
            // A failure here means the terminal is already in a state we cannot improve.
            let _ = multi.clear();
        }
    }

    /// One labelled tracker of `len` steps.
    pub fn bar(&self, prefix: &str, len: usize, indent: usize) -> Bar {
        match self {
            Self::Bars(multi, drawn) => {
                let bar = multi.add(ProgressBar::new(len as u64));
                // A malformed template is a programming error in the constant above, and the
                // honest response is a plain bar rather than aborting a run with hundreds of
                // gigabytes to move — nothing about the photographs depends on the rendering.
                if let Ok(style) = ProgressStyle::with_template(&row_template(indent)) {
                    let style = style
                        .progress_chars("=> ")
                        .with_key(
                            "pct",
                            |state: &indicatif::ProgressState, out: &mut dyn std::fmt::Write| {
                                // Ignored deliberately: a formatting failure must not take down
                                // a run, for the same reason a malformed template does not.
                                let _ = write!(out, "{:.1}%", state.fraction() * 100.0);
                            },
                        )
                        // **Blank at both ends.** Below [`ETC_FROM_FRACTION`] the estimate
                        // describes a pipeline still filling rather than the drive; at 100 % a
                        // countdown to nothing is noise. In between it comes from this bar's own
                        // rate, which is what tells the operator the WD has eleven minutes left
                        // while the laptop has one.
                        .with_key(
                            "etc",
                            |state: &indicatif::ProgressState, out: &mut dyn std::fmt::Write| {
                                let done = state.fraction();
                                if !(ETC_FROM_FRACTION..1.0).contains(&done) {
                                    return;
                                }
                                // Two columns each for minutes and seconds, right-aligned and
                                // **not** zero-padded: `13m 48s` and ` 7m  4s` line up down the
                                // four rows, so the eye compares magnitudes rather than reading
                                // numbers. A leading zero would align just as well and reads as
                                // a clock, which this is not — it is a duration.
                                let seconds = state.eta().as_secs();
                                let _ =
                                    write!(out, "(ETC: {:>2}m {:>2}s)", seconds / 60, seconds % 60);
                            },
                        );
                    bar.set_style(style);
                }
                bar.set_prefix(prefix.to_owned());
                if let Ok(mut drawn) = drawn.lock() {
                    drawn.push(bar.clone());
                }
                Bar {
                    bar: Some(bar),
                    plain: None,
                }
            }
            Self::Lines => Bar {
                bar: None,
                plain: Some(Plain {
                    prefix: prefix.to_owned(),
                    indent: indent + STEP,
                    len,
                    position: Cell::new(0),
                    reported: Cell::new(0),
                    message: RefCell::new(String::new()),
                }),
            },
            Self::Silent => Bar {
                bar: None,
                plain: None,
            },
        }
    }
}

/// A pass heading, on screen for as long as this value is alive.
///
/// **Holding it is not bookkeeping — it is what keeps the line drawn.** `ProgressBar`'s `Drop`
/// finishes an unfinished bar and takes its line with it, so a heading dropped at the end of
/// the statement that made it is a heading nobody ever sees. `Lines` and `Silent` carry
/// nothing and exist only so callers can treat all three modes the same way.
#[derive(Debug)]
pub struct Section {
    #[allow(
        dead_code,
        reason = "held solely to keep the lines on screen until dropped"
    )]
    lines: Vec<ProgressBar>,
}

/// One phase's progress, however this run reports it.
///
/// Owned by the thread that advances it — one per destination in phase 3 — so the interior
/// mutability below needs no lock.
#[derive(Debug)]
pub struct Bar {
    bar: Option<ProgressBar>,
    plain: Option<Plain>,
}

#[derive(Debug)]
struct Plain {
    prefix: String,
    indent: usize,
    len: usize,
    position: Cell<usize>,
    reported: Cell<usize>,
    message: RefCell<String>,
}

impl Bar {
    /// Advance one step.
    pub fn inc(&self) {
        if let Some(bar) = &self.bar {
            bar.inc(1);
        }

        let Some(plain) = &self.plain else { return };

        let position = plain.position.get() + 1;
        plain.position.set(position);

        // Ceiling division, so a day with fewer frames than `UPDATES_PER_PASS` still steps by
        // at least one and cannot divide by zero.
        let step = plain.len.div_ceil(UPDATES_PER_PASS).max(1);
        if position - plain.reported.get() >= step || position == plain.len {
            plain.reported.set(position);
            // Same columns and the same formatting as the bars, so a log and a terminal
            // describe one run rather than looking like two tools.
            let indent = plain.indent;
            println!(
                "{:indent$}{:<8} {:>6}/{:<6} {:>6} {}",
                "",
                plain.prefix,
                crate::human::count(position),
                crate::human::count(plain.len),
                crate::human::percent(position, plain.len),
                plain.message.borrow()
            );
        }
    }

    /// Name this bar's pass **for the log only**.
    ///
    /// At a terminal the section heading above the row already says `Writing` or `Verifying`,
    /// so repeating it on every row is noise. A captured log has no headings interleaved with
    /// its rows — the two passes' lines arrive mixed together — so there the label is the only
    /// thing that says which pass a line belongs to. Same information, put where each mode
    /// needs it.
    pub fn set_pass(&self, pass: &str) {
        if let Some(plain) = &self.plain {
            pass.clone_into(&mut plain.message.borrow_mut());
        }
    }

    /// What this pass is doing, shown beside the count.
    pub fn set_message(&self, message: &str) {
        if let Some(bar) = &self.bar {
            bar.set_message(message.to_owned());
        }
        if let Some(plain) = &self.plain {
            message.clone_into(&mut plain.message.borrow_mut());
        }
    }

    /// Start counting again from zero, for a second pass over the same files.
    ///
    /// The write and verify passes are different work at different rates, so one tracker
    /// running to `2N` would move at two speeds and mean neither.
    pub fn restart(&self) {
        if let Some(bar) = &self.bar {
            bar.set_position(0);
        }
        if let Some(plain) = &self.plain {
            plain.position.set(0);
            plain.reported.set(0);
        }
    }

    /// Done — leave the bar on screen, full, carrying whatever message it already had.
    ///
    /// **It used to clear itself, and leaving it is worth more than the line it costs.** A
    /// finished destination vanishing reads as *something happened to it*; a full bar at
    /// `3,883/3,883 100.0%` is the tool showing its work. On a rig where the WD finishes
    /// minutes after the other three, those completed rows are also the clearest possible
    /// statement of *which* drive the run is waiting on — the question the operator used to
    /// answer by watching activity LEDs.
    ///
    /// **No completion verb is added**, because in phase 3 the `Writing` / `Verifying` heading
    /// above the row already says it and a row reading `100.0% written` under a `Writing`
    /// heading says it twice. Phases 4 and 5 have no heading and set their own message, which
    /// survives untouched — this method deliberately does not overwrite it.
    pub fn finish(&self) {
        if let Some(bar) = &self.bar {
            // `finish` rather than `finish_and_clear`: fills the bar to `len` and leaves it
            // drawn. `MultiProgress` keeps redrawing it and adds later phases' bars below,
            // so the screen accumulates a record of the run rather than replacing itself.
            bar.finish();
        }
        if let Some(plain) = &self.plain {
            // The log gets a closing line so a captured run and a watched one end the same
            // way. Position is forced to `len` for the case where a pass ends early.
            let repeat = closing_line_would_repeat(plain.reported.get(), plain.len);
            plain.position.set(plain.len);
            plain.reported.set(plain.len);
            if repeat {
                return;
            }

            let indent = plain.indent;
            println!(
                "{:indent$}{:<8} {:>6}/{:<6} {:>6} {}",
                "",
                plain.prefix,
                crate::human::count(plain.len),
                crate::human::count(plain.len),
                crate::human::percent(plain.len, plain.len),
                plain.message.borrow()
            );
        }
    }
}

/// Whether [`Bar::finish`]'s closing line would just repeat the one [`Bar::inc`] already wrote.
///
/// **A completed pass printed `100.0%` twice**, because `inc` always emits on the final step
/// and `finish` then emitted the same line again. Invisible at a terminal, where both are the
/// same redrawn bar; plainly duplicated in a captured log, which is every run Claude drives.
///
/// **A pass that ended early still gets its closing line**, which is the case `finish` forces
/// the position for — there `reported` sits below `len` and nothing has said the pass is over.
/// So does a zero-length pass, where nothing was ever reported and the guard would otherwise
/// swallow the only line.
fn closing_line_would_repeat(reported: usize, len: usize) -> bool {
    len > 0 && reported == len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_completed_pass_does_not_print_its_last_line_twice() {
        assert!(closing_line_would_repeat(3_883, 3_883));
    }

    /// The two cases the guard must NOT swallow.
    #[test]
    fn an_early_end_and_an_empty_pass_still_get_a_closing_line() {
        assert!(!closing_line_would_repeat(2_000, 3_883));
        assert!(!closing_line_would_repeat(0, 0));
    }

    /// **Templates are parsed at runtime and a bad one fails silently** — both call sites do
    /// `if let Ok(style)` deliberately, so a rendering fault cannot abort a run mid-copy. The
    /// cost is that a typo degrades to an unstyled bar with no error anywhere, so the parse is
    /// asserted here instead. **The only part of `Bars` testable without a terminal.**
    #[test]
    fn every_template_parses_at_every_depth_it_is_used_at() {
        for indent in [PHASE, PASS] {
            assert!(
                ProgressStyle::with_template(&section_template(indent)).is_ok(),
                "section template at indent {indent}"
            );
            assert!(
                ProgressStyle::with_template(&row_template(indent)).is_ok(),
                "row template at indent {indent}"
            );
        }
    }

    /// The spacer is a template too, and an **empty** one renders no line at all — which is how
    /// the first attempt at blank lines produced none.
    ///
    /// **Reads [`SPACER`] rather than re-typing it: a test that duplicates the value under test
    /// can only agree with itself.** Mutation-checked — `SPACER = ""` fails the second assertion.
    #[test]
    fn the_spacer_template_parses_and_is_not_empty() {
        assert!(ProgressStyle::with_template(SPACER).is_ok());
        assert!(!SPACER.is_empty(), "an empty spacer renders no line at all");
    }

    /// Rows sit one step in from their heading. Asserted rather than trusted because the two
    /// are computed in different functions and nothing else would catch them drifting apart.
    #[test]
    fn rows_are_indented_one_step_past_their_heading() {
        for indent in [PHASE, PASS] {
            let heading = section_template(indent);
            let row = row_template(indent);
            let lead = |s: &str| s.len() - s.trim_start().len();
            assert_eq!(
                lead(&row),
                lead(&heading) + STEP,
                "row at indent {indent} must sit one STEP past its heading"
            );
        }
    }
}
