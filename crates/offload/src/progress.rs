//! Progress reporting — the difference between *working* and *hung*.
//!
//! **This exists because the operator watched a drive's activity LED to work out which phase
//! was running.** The run printed `ingesting 3,883 files…` and then nothing for twelve
//! minutes, then nothing again for the sixteen phase 4 takes. His words are the whole
//! specification: *"I feel like I shouldn't need to guess at that."*
//!
//! Decision 22 had already won this argument for a different stage — eject became a *timed*
//! stage because "an unlabeled twenty-minute silence reads as a hang while a timed one reads
//! as persistence" — and the conclusion was never carried across to the phases that take
//! twelve and sixteen minutes rather than fifteen seconds. **For a walk-away tool, a screen
//! that cannot say whether it is working is the one thing that makes an operator stay and
//! watch it.**
//!
//! # Three modes, because a progress bar is useless in a log file
//!
//! `indicatif` disables itself when its stream is not a terminal — `draw_target.rs` returns a
//! hidden target on `!term.is_term()`. **That would have made this feature invisible in
//! exactly the mode it is most needed**: `CONOPS.md`'s shooting-day contract has the operator
//! running the offload through Claude whenever there is internet, which means captured to a
//! file, which means not a terminal. The bars would have looked right when he ran them by hand
//! and rendered nothing every time they ran for him.
//!
//! So there are three real modes rather than a display and a no-op:
//!
//! | Mode | When | What it does |
//! |---|---|---|
//! | [`Progress::Bars`] | stderr is a terminal | live bars, one per destination |
//! | [`Progress::Lines`] | stderr is redirected | a plain line to **stdout** every tenth of a pass |
//! | [`Progress::Silent`] | tests | nothing |
//!
//! **The log arguably needs this more than the terminal does.** At a terminal you can at least
//! glance at the drive lights — which is precisely what the operator was reduced to — while a
//! captured log is the only evidence a session that was not watching will ever have.
//!
//! That three-way split is also why there is no `Progress` *trait*: two of these are genuine,
//! different behaviors and the third is absent, which is a plain enum rather than the
//! speculative extension point `CLAUDE.md` says to push back on.

use std::cell::{Cell, RefCell};
use std::io::IsTerminal;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

/// A pass heading — `Writing`, `Verifying` — indented one level under the ingest line.
const SECTION_TEMPLATE: &str = "    {msg}";

/// Column widths matching the report's, so a bar lines up with the lines printed around it.
///
/// `human_pos` and `human_len` are `indicatif`'s own separator-formatted counters, which is
/// `WRITING.md` rule 6 applied to the bars — they rendered a bare `3883` until 2026-08-05 while
/// the report two lines below printed `3,883`. `pct` is registered in [`Progress::bar`] because
/// `indicatif`'s built-in percent is a whole number, and one decimal is what keeps the slowest
/// destination from appearing to stall (see [`crate::human::percent`]).
///
/// Widths hold to 6 digits — `999,999` — where the biggest day on record is 7,350 frames.
const TEMPLATE: &str = "        {prefix:<8} {bar:28} {human_pos:>6}/{human_len:<6} {pct:>6} {msg}";

/// Plain-text updates per pass. Ten is a deliberate ceiling rather than a rate: it bounds a
/// long run's log at a knowable number of lines regardless of how many frames the day holds,
/// where a time-based throttle would produce forty lines on a slow night and four on a fast
/// one for the same information.
const UPDATES_PER_PASS: usize = 10;

/// How the run reports progress. See the module note for why there are three of these.
pub enum Progress {
    /// stderr is a terminal: live bars.
    Bars(MultiProgress),
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
            Self::Bars(MultiProgress::new())
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
    /// **Both sections exist from the start and a destination appears in both**, which is not
    /// merely a rendering choice. Phase 3 has no barrier between the passes: each destination
    /// begins verifying the moment *its own* writes finish, so the laptop's NVMe is reading
    /// back while the slowest USB drive is still being written. Adding a barrier to make the
    /// sections tidy would idle the fast drives and cost real wall clock.
    ///
    /// So `Writing` fills top-to-bottom and `Verifying` starts filling underneath it while
    /// some rows above are still moving. **That overlap is the most useful thing on the
    /// screen** — it is the same heterogeneity decision 14 calls the report's most useful
    /// number, shown live.
    ///
    /// In `Bars` this is a message-only bar that is never advanced, so `MultiProgress` keeps
    /// it in place as an ordinary line. In `Lines` it is one heading printed once.
    pub fn section(&self, title: &str) {
        match self {
            Self::Bars(multi) => {
                let heading = multi.add(ProgressBar::new(0));
                if let Ok(style) = ProgressStyle::with_template(SECTION_TEMPLATE) {
                    heading.set_style(style);
                }
                heading.set_message(title.to_owned());
                // The handle is dropped here and the line stays: `MultiProgress` owns the
                // bar's state once added, so a heading needs no owner. Never ticked and never
                // finished — it is a label that happens to be a bar.
            }
            Self::Lines => println!("    {title}"),
            Self::Silent => {}
        }
    }

    /// One labelled tracker of `len` steps.
    pub fn bar(&self, prefix: &str, len: usize) -> Bar {
        match self {
            Self::Bars(multi) => {
                let bar = multi.add(ProgressBar::new(len as u64));
                // A malformed template is a programming error in the constant above, and the
                // honest response is a plain bar rather than aborting a run with hundreds of
                // gigabytes to move — nothing about the photographs depends on the rendering.
                if let Ok(style) = ProgressStyle::with_template(TEMPLATE) {
                    let style = style.progress_chars("=> ").with_key(
                        "pct",
                        |state: &indicatif::ProgressState, out: &mut dyn std::fmt::Write| {
                            // Ignored deliberately: a formatting failure must not take down a
                            // run, for the same reason a malformed template does not above.
                            let _ = write!(out, "{:.1}%", state.fraction() * 100.0);
                        },
                    );
                    bar.set_style(style);
                }
                bar.set_prefix(prefix.to_owned());
                Bar {
                    bar: Some(bar),
                    plain: None,
                }
            }
            Self::Lines => Bar {
                bar: None,
                plain: Some(Plain {
                    prefix: prefix.to_owned(),
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

/// One phase's progress, however this run reports it.
///
/// Owned by the thread that advances it — one per destination in phase 3 — so the interior
/// mutability below needs no lock.
pub struct Bar {
    bar: Option<ProgressBar>,
    plain: Option<Plain>,
}

struct Plain {
    prefix: String,
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
            println!(
                "  {:<8} {:>6}/{:<6} {:>6} {}",
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
            *plain.message.borrow_mut() = pass.to_owned();
        }
    }

    /// What this pass is doing, shown beside the count.
    pub fn set_message(&self, message: &str) {
        if let Some(bar) = &self.bar {
            bar.set_message(message.to_owned());
        }
        if let Some(plain) = &self.plain {
            *plain.message.borrow_mut() = message.to_owned();
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
            plain.position.set(plain.len);
            println!(
                "  {:<8} {:>6}/{:<6} {:>6} {}",
                plain.prefix,
                crate::human::count(plain.len),
                crate::human::count(plain.len),
                crate::human::percent(plain.len, plain.len),
                plain.message.borrow()
            );
        }
    }
}
