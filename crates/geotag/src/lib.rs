//! Capture time from camera raw files, positions from GPX tracks, and the XMP
//! sidecars that marry the two.
//!
//! **Lifted whole from RawGeotag, deliberately unrewritten.** `DESIGN.md` decision 17
//! takes this engine as already-solved on the strength of its validation — CR3 capture
//! times over thousands of real frames on two bodies, and XMP packets diffed against
//! Lightroom Classic 15.4.1's own output to 0.02–0.12 m. A lift that rewrote what it
//! moved would forfeit exactly that, so the modules arrived as they were, tests
//! included, and the changes made to them are the two named below and nothing else.
//!
//! # The project mantra it enforces
//!
//! **A geotag off by more than 5 m is worse than no geotag.** A missing tag is visibly
//! missing; a wrong one looks authoritative and silently corrupts the photo's
//! provenance. So a tag is earned, never assumed: [`track`] refuses to interpolate
//! across a hole too wide in time *or* distance, refuses to bridge a `<trkseg>` break
//! at all, and never clamps or extrapolates past the ends of a track.
//!
//! # What changed in the lift
//!
//! - [`raw::capture_time_in_memory`] is new. `offload` holds every file in RAM to hash
//!   it (`DESIGN.md` decision 10), so re-reading it from disk to find its capture time
//!   would be pure waste. It shares all of its logic with the path-based
//!   [`raw::capture_time`]; only how the bytes reach the parser differs.
//! - [`xmp::render`] takes the writing tool's identity rather than baking in
//!   `rawgeotag`. Two tools now emit these packets, and a sidecar that names the wrong
//!   one is a small lie in a file whose whole job is provenance.
//!
//! # Why two time crates
//!
//! Neither is this crate's choice: `nom-exif` returns `chrono` types and `gpx` returns
//! `time` types, so both arrive at the API boundary. `chrono` is the one the code
//! speaks — `DateTime<Utc>` for instants, `TimeDelta` for durations — and `gpx`'s
//! `OffsetDateTime` is converted at exactly one function, `track_point`.

pub mod format;
pub mod raw;
pub mod track;
pub mod xmp;
