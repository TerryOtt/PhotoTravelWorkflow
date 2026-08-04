//! Where a photo lands, decided entirely by the photo.
//!
//! Decision 5: the output path is a pure function of the capture instant and the
//! camera's own file name, and of nothing that is already present in the destination.
//! That is what makes it deterministic, idempotent, and identical across all four
//! destinations with no coordination between them — a crashed run re-offloaded produces
//! the same names, and so does the same card offloaded twice.
//!
//! Nothing here touches the filesystem. Collision *resolution* needs to know what is
//! already on a disk and therefore lives in the pipeline; what lives here is the naming
//! rule itself, which decision 18 singles out as one of the four things worth testing
//! because it decides where irreplaceable files land.

use std::path::PathBuf;

use chrono::{DateTime, Utc};

/// `2026/2026-08-03/1422Z_0001.CR3` — the path a photo takes inside every
/// destination root.
///
/// **The two-level shape is not arbitrary and is not free to change** (decision 31): it
/// is the layout the Lightroom catalog is already configured to use, and writing it
/// directly is what lets the import at home run as *Add* rather than *Copy* — measured
/// in the field at more than 10x faster. Flattening or restyling this silently returns
/// the operator to the slow import.
///
/// The date folder comes from the **UTC** capture instant (decision 23 derives that
/// instant per photo from EXIF wall time minus the recorded offset, so no timezone
/// logic survives this far). Only the time of day goes in the file name, because the
/// date is already carried by the directory.
///
/// Minute resolution is deliberate and sufficient: it restores shooting order that the
/// camera's bare counter loses when a mid-day format resets it, and within any single
/// minute the counter is still monotonic, so ties break correctly on the sequence
/// number that [`prefixed_name`] keeps.
pub fn destination_path(captured: DateTime<Utc>, source_file_name: &str) -> PathBuf {
    let year = captured.format("%Y");
    let day = captured.format("%Y-%m-%d");

    PathBuf::from(year.to_string())
        .join(day.to_string())
        .join(prefixed_name(captured, source_file_name))
}

/// `_50A0001.CR3` -> `1422Z_0001.CR3`.
///
/// **`HHMMZ`, an underscore of ours, and the camera's sequence number — nothing else.**
/// The rest of the camera's stem is the body prefix, and with the fleet fixed at one R5
/// (`CONOPS.md`) it is the same three characters on every frame ever shot: it
/// distinguishes nothing and costs four characters in every filename in the archive.
/// The sequence number is the part that carries information, because it is what breaks
/// ties within a minute.
///
/// The separator is ours rather than borrowed from the camera, which is what makes this
/// readable for any body: `IMG_1234.CR3` becomes `1422Z_1234.CR3` and not
/// `1422ZIMG_1234.CR3`.
///
/// The extension is carried through untouched, uppercase `.CR3` included, so the
/// archive stays consistent with everything already in it and with any filesystem that
/// ever cares about case.
fn prefixed_name(captured: DateTime<Utc>, source_file_name: &str) -> String {
    let (stem, extension) = match source_file_name.rsplit_once('.') {
        Some((stem, extension)) => (stem, Some(extension)),
        None => (source_file_name, None),
    };

    let sequence = sequence_number(stem);

    match extension {
        Some(extension) => format!("{}Z_{sequence}.{extension}", captured.format("%H%M")),
        None => format!("{}Z_{sequence}", captured.format("%H%M")),
    }
}

/// The trailing run of digits in a camera stem: `_50A0001` -> `0001`, `IMG_1234` ->
/// `1234`.
///
/// Every run of digits is taken rather than a fixed four, so a body with a five-digit
/// counter keeps all of it rather than silently colliding on the last four.
///
/// **A stem with no trailing digits keeps the whole stem instead.** That case should not
/// occur — every camera numbers its files — but the alternative to a fallback is an
/// empty sequence, which would name every such frame in a minute identically and push
/// two distinct photos onto decision 5's collision path for no reason. Losing the
/// tidiness on a file nobody expects is the cheaper failure.
fn sequence_number(stem: &str) -> &str {
    let digits_start = stem
        .rfind(|character: char| !character.is_ascii_digit())
        .map_or(0, |last_other| last_other + 1);

    if digits_start == stem.len() {
        stem
    } else {
        &stem[digits_start..]
    }
}

/// `_unfiled/<run-id>/<original name>` — decision 21's home for a CR3 whose EXIF
/// cannot be read, or which carries no UTC offset to resolve (decision 23).
///
/// Outside the `YYYY/` tree — which is load-bearing rather than tidy, because
/// Lightroom's import points at the year folders, so a frame parked outside them stays
/// out of the catalog (decision 31). Under a per-run subfolder
/// so name collisions are impossible without any collision logic at all. The camera's
/// name is kept verbatim: there is no capture time to build a better one from, and that
/// is precisely why the file is here.
pub fn unfiled_path(run_id: &str, source_file_name: &str) -> PathBuf {
    PathBuf::from("_unfiled")
        .join(run_id)
        .join(source_file_name)
}

/// `1422Z_0001.CR3` -> `1422Z_0001_001.CR3`, decision 5's escape hatch for two
/// genuinely different photos that share a sequence number within one minute.
///
/// Pathological rather than impossible, and it should effectively never fire — the
/// mid-day-format collision that once motivated a whole rename scheme cannot reach it,
/// because two photos taken in different minutes now get different names anyway.
pub fn with_collision_suffix(name: &str, nth: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) => format!("{stem}_{nth:03}.{extension}"),
        None => format!("{name}_{nth:03}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worked example from decision 5, end to end. If this moves, the archive's
    /// naming scheme moved with it.
    #[test]
    fn the_design_example_lands_exactly_where_the_design_says() {
        let captured = "2026-08-03T14:22:37Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(
            destination_path(captured, "_50A0001.CR3"),
            PathBuf::from("2026")
                .join("2026-08-03")
                .join("1422Z_0001.CR3")
        );
    }

    /// The body prefix is dropped and the separator is ours, so the name reads the same
    /// way whichever of Canon's two stem shapes the camera is writing. This is the case
    /// the previous scheme got ugly on — it produced `1422ZIMG_1234.CR3`.
    #[test]
    fn only_the_sequence_number_survives_whatever_shape_the_stem_has() {
        let captured = "2026-08-03T14:22:37Z".parse::<DateTime<Utc>>().unwrap();

        for stem in ["_50A0001", "IMG_0001", "_MG_0001", "100_0001"] {
            assert_eq!(
                prefixed_name(captured, &format!("{stem}.CR3")),
                "1422Z_0001.CR3",
                "{stem}"
            );
        }
    }

    /// All the trailing digits, not the last four — a five-digit counter must not
    /// collide two frames onto one name.
    #[test]
    fn a_longer_counter_keeps_all_of_its_digits() {
        let captured = "2026-08-03T14:22:37Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(prefixed_name(captured, "ABC12345.CR3"), "1422Z_12345.CR3");
        assert_eq!(sequence_number("ABC12345"), "12345");
    }

    /// The fallback: a stem with no trailing digits keeps the whole stem, because the
    /// alternative is an empty sequence that names every such frame in a minute alike.
    #[test]
    fn a_stem_with_no_trailing_digits_keeps_the_whole_stem() {
        let captured = "2026-08-03T14:22:37Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(prefixed_name(captured, "SCAN.CR3"), "1422Z_SCAN.CR3");
        assert_eq!(
            prefixed_name(captured, "IMG_0001A.CR3"),
            "1422Z_IMG_0001A.CR3"
        );
        assert_eq!(
            sequence_number("0001"),
            "0001",
            "an all-digit stem is its own"
        );
    }

    /// The whole point of UTC foldering, and the consequence decision 23 accepts by
    /// name: a frame shot early in the morning east of UTC files under the previous
    /// day. Asserted rather than left implicit, because it is the behavior most likely
    /// to be "fixed" by someone who thinks it is a bug.
    #[test]
    fn an_instant_just_after_utc_midnight_files_under_the_new_utc_day() {
        let before = "2026-08-03T23:58:00Z".parse::<DateTime<Utc>>().unwrap();
        let after = "2026-08-04T00:01:00Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(
            destination_path(before, "_50A0001.CR3"),
            PathBuf::from("2026")
                .join("2026-08-03")
                .join("2358Z_0001.CR3")
        );
        assert_eq!(
            destination_path(after, "_50A0002.CR3"),
            PathBuf::from("2026")
                .join("2026-08-04")
                .join("0001Z_0002.CR3")
        );
    }

    /// Midnight and noon in the same day, because a `%H%M` that used a 12-hour clock
    /// would pass every other test here and silently collide two frames twelve hours
    /// apart.
    #[test]
    fn the_time_prefix_is_a_24_hour_clock_and_always_four_digits() {
        let midnight = "2026-08-03T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let noon = "2026-08-03T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let evening = "2026-08-03T23:59:00Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(prefixed_name(midnight, "_50A0001.CR3"), "0000Z_0001.CR3");
        assert_eq!(prefixed_name(noon, "_50A0001.CR3"), "1200Z_0001.CR3");
        assert_eq!(prefixed_name(evening, "_50A0001.CR3"), "2359Z_0001.CR3");
    }

    /// Seconds are deliberately absent from the name. Two frames in the same minute
    /// therefore collide by design, and the camera's monotonic counter is what orders
    /// them — which is the property that makes minute resolution sufficient.
    #[test]
    fn frames_in_one_minute_share_a_prefix_and_are_ordered_by_the_camera_counter() {
        let early = "2026-08-03T14:22:01Z".parse::<DateTime<Utc>>().unwrap();
        let late = "2026-08-03T14:22:59Z".parse::<DateTime<Utc>>().unwrap();

        let first = prefixed_name(early, "_50A0001.CR3");
        let second = prefixed_name(late, "_50A0002.CR3");

        assert_eq!(first, "1422Z_0001.CR3");
        assert_eq!(second, "1422Z_0002.CR3");
        assert!(first < second, "the counter has to break the tie");
    }

    /// The reason the scheme exists: after a mid-day format resets the counter, the
    /// afternoon's `_50A0001` sorts *after* the morning's `_50A3999`, which the
    /// camera's bare filename gets exactly backwards.
    #[test]
    fn a_post_format_afternoon_frame_sorts_after_a_morning_one() {
        let morning = "2026-08-03T09:15:00Z".parse::<DateTime<Utc>>().unwrap();
        let afternoon = "2026-08-03T16:40:00Z".parse::<DateTime<Utc>>().unwrap();

        let morning_name = prefixed_name(morning, "_50A3999.CR3");
        let afternoon_name = prefixed_name(afternoon, "_50A0001.CR3");

        assert!(
            morning_name < afternoon_name,
            "{morning_name} should sort before {afternoon_name}"
        );
        assert!(
            "_50A3999.CR3" > "_50A0001.CR3",
            "the camera's own name sorts these the wrong way round, which is the point"
        );
    }

    /// Uppercase in, uppercase out. Normalizing would leave the archive holding two
    /// spellings of the same extension for no gain.
    #[test]
    fn the_cameras_extension_case_is_preserved() {
        let captured = "2026-08-03T14:22:37Z".parse::<DateTime<Utc>>().unwrap();

        assert!(
            destination_path(captured, "_50A0001.CR3")
                .to_string_lossy()
                .ends_with(".CR3")
        );
    }

    #[test]
    fn unfiled_files_keep_their_name_under_the_run_that_found_them() {
        assert_eq!(
            unfiled_path("2026-08-03T18-22-04", "_50A0001.CR3"),
            PathBuf::from("_unfiled")
                .join("2026-08-03T18-22-04")
                .join("_50A0001.CR3")
        );
    }

    #[test]
    fn the_collision_suffix_goes_before_the_extension() {
        assert_eq!(
            with_collision_suffix("1422Z_0001.CR3", 1),
            "1422Z_0001_001.CR3"
        );
        assert_eq!(
            with_collision_suffix("1422Z_0001.CR3", 42),
            "1422Z_0001_042.CR3"
        );
    }

    /// `rsplit_once` rather than `split_once`, so a name with more than one dot keeps
    /// all but the last as part of its stem.
    #[test]
    fn the_collision_suffix_splits_on_the_last_dot_not_the_first() {
        assert_eq!(
            with_collision_suffix("1422Z_50A.0001.CR3", 1),
            "1422Z_50A.0001_001.CR3"
        );
        assert_eq!(with_collision_suffix("NOEXTENSION", 1), "NOEXTENSION_001");
    }
}
