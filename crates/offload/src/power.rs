//! Keeping the machine awake for the length of a run.
//!
//! Decision 9. A laptop that suspends twenty minutes in turns a walk-away run into one that
//! is somehow unfinished on your return, with nothing in the report to explain it.

use windows::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};

/// Holds off system sleep until it is dropped.
///
/// **The display is deliberately allowed to sleep.** `ES_DISPLAY_REQUIRED` would keep the
/// screen lit for a run whose entire point is that nobody is watching it, on a laptop
/// that is often running on battery in a hotel room. Only `ES_SYSTEM_REQUIRED` is
/// claimed, so the machine stays awake and the screen does not.
///
/// **The request is per-thread and persists until reset**, so this MUST be held on a thread
/// that outlives the run — in practice `main`.
#[derive(Debug)]
pub struct StayAwake {
    /// False when the request was refused, which is reported rather than fatal — see
    /// [`StayAwake::engaged`].
    engaged: bool,
}

impl StayAwake {
    /// Ask the system to stay awake.
    ///
    /// **Never fails the run**, because a suspend is recoverable — the archives stay
    /// consistent through one and a re-run converges (decisions 13, 18). Reported, not fatal.
    pub fn request() -> Self {
        // SAFETY: no pointers and no borrows; this sets a flag on the calling thread.
        let previous = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };

        Self {
            // Zero is the documented failure return; anything else is the prior state.
            engaged: previous.0 != 0,
        }
    }

    /// Whether the request was granted, for the report to say so when it was not.
    pub fn engaged(&self) -> bool {
        self.engaged
    }
}

impl Drop for StayAwake {
    fn drop(&mut self) {
        if !self.engaged {
            return;
        }

        // `ES_CONTINUOUS` alone clears the standing request without asserting a new one,
        // which is how this API spells "back to normal".
        //
        // SAFETY: as above — no pointers, no borrows.
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Windows exposes no way to read the current execution state back**, so the only
    /// assertable thing is that the call is accepted. A false here means the flags are wrong.
    #[test]
    fn a_request_is_granted_and_releases_cleanly() {
        let awake = StayAwake::request();
        assert!(
            awake.engaged(),
            "SetThreadExecutionState refused the request"
        );
        drop(awake);
    }
}
