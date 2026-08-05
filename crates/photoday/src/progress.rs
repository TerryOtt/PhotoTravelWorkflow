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

/// Column widths matching the report's, so a bar lines up with the lines printed around it.
const TEMPLATE: &str = "  {prefix:<8} {bar:28} {pos:>5}/{len:<5} {msg}";

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

    /// One labelled tracker of `len` steps.
    pub fn bar(&self, prefix: &str, len: usize) -> Bar {
        match self {
            Self::Bars(multi) => {
                let bar = multi.add(ProgressBar::new(len as u64));
                // A malformed template is a programming error in the constant above, and the
                // honest response is a plain bar rather than aborting a run with hundreds of
                // gigabytes to move — nothing about the photographs depends on the rendering.
                if let Ok(style) = ProgressStyle::with_template(TEMPLATE) {
                    bar.set_style(style.progress_chars("=> "));
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
            println!(
                "  {:<8} {:>5}/{:<5} {}",
                plain.prefix,
                position,
                plain.len,
                plain.message.borrow()
            );
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

    /// Done — take the bar off the screen, leaving the report's own lines behind.
    pub fn finish(&self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}
