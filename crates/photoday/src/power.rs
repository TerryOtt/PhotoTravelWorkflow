//! Keeping the machine awake for the length of a run.
//!
//! Decision 9 lists this among pre-flight's jobs, and the reason is the whole premise of
//! the tool: you start it and go to dinner. A laptop that suspends twenty minutes in
//! turns a walk-away run into a run that is somehow not finished when you get back, with
//! nothing in the report to explain it.

use windows::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};

/// Holds off system sleep until it is dropped.
///
/// **The display is deliberately allowed to sleep.** `ES_DISPLAY_REQUIRED` would keep the
/// screen lit for a run whose entire point is that nobody is watching it, on a laptop
/// that is often running on battery in a hotel room. Only `ES_SYSTEM_REQUIRED` is
/// claimed, so the machine stays awake and the screen does not.
///
/// The request is per-thread and persists until reset, so this must be held on a thread
/// that outlives the run — in practice the one that owns the run, which is `main`.
/// Dropping it restores the machine's normal idle behavior rather than forcing the
/// machine awake and leaving it that way, which is the failure mode of calling the API
/// without an RAII guard around it.
#[derive(Debug)]
pub struct StayAwake {
    /// False when the request was refused, which is reported rather than fatal — see
    /// [`StayAwake::engaged`].
    engaged: bool,
}

impl StayAwake {
    /// Ask the system to stay awake.
    ///
    /// Never fails the run. A refused request means the machine may sleep mid-offload,
    /// which is recoverable — the archives stay consistent through a suspend and a
    /// re-run converges (decisions 13 and 18) — so this is a thing to *report*, not a
    /// thing to stop for. Pre-flight exists to catch what makes a run impossible, and
    /// this is not that.
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

    /// The guard has no observable effect to assert — Windows exposes no way to read
    /// back the current execution state — so what is tested is the one thing that can
    /// silently break: that requesting and dropping does not panic, and that the
    /// granted flag is set on a machine where the call works. If this ever reports
    /// false on a normal desktop session, the flags are wrong.
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
