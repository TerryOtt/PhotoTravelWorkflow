//! `offload` — the end-of-day offload, as a library.
//!
//! The binary beside this file is the CLI of decision 8 and little else; the phases
//! live here so they can be driven by tests over ordinary directories rather than only
//! by a run against two card readers and three SSDs on a Thunderbolt hub.
//!
//! That split is not merely convenient — it is what lets phase 3 be tested at all.
//! [`pipeline::run`] takes source paths and destination roots, so the hardware identity
//! that decision 6 resolves a destination by is phase 2's problem and never this
//! phase's. A destination arrives here as a label and a path.

pub mod cards;
pub mod config;
pub mod destinations;
pub mod eject;
pub mod hash;
pub mod human;
pub mod manifest;
pub mod marker;
pub mod naming;
pub mod phase4;
pub mod phase5;
pub mod pipeline;
pub mod preflight;
pub mod progress;
pub mod runlog;
pub mod storage;
pub mod verify;
pub mod winio;
