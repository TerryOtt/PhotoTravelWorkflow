//! Number formatting shared by the report and the progress bars.
//!
//! **This exists because the bars were not held to the rule the report follows.**
//! [`docs/WRITING.md`](../../../docs/WRITING.md) rule 6 requires thousands separators in
//! program output as well as prose, and `landed()` had obeyed it since it was written —
//! while the progress line beside it rendered a bare `3883`, because `count` was a private
//! helper in `main.rs` and `progress.rs` lives in the library. **The rule was fine; the
//! function was in the wrong file.** Spotted by the operator on 2026-08-05, the first time
//! he ran the tool himself.

/// Thousands separators, per `docs/WRITING.md` rule 6.
///
/// Hand-written rather than pulled from a crate, per decision 29's note on what stays
/// hand-rolled: this is a dozen lines with a unit test against a crate and a version to
/// track for the rest of the project's life.
pub fn count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Bytes as **GiB**, one decimal place, for anything that answers *how much space*.
///
/// **Capacity is GiB and rates are decimal, and the split is deliberate.** Windows is what
/// the operator checks a figure against — Explorer, PowerShell's `/1GB`, the drive's own
/// properties dialog all divide by 2^30 — so a payload reported as `201.3 GB` sends him to a
/// file manager that says `187` and invites him to wonder which is lying. Neither is; they are
/// the same bytes in two units, and the one that matches his other instruments wins.
///
/// **Throughput stays decimal** (`GB/s`, `Gbps`) because a link's speed is decimal by
/// definition — 10 Gbps is 10^10 bits — and a rate expressed in GiB/s cannot be compared to
/// the number printed on the cable. So sizes are GiB, rates are GB, and each is the unit its
/// own question is asked in.
pub fn gib(bytes: u64) -> String {
    // Rounded to one decimal *first*, then separated, so the separator is applied to the
    // digits actually printed. Doing it the other way rounds 1,687.95 to a whole 1,687 and a
    // fraction 10, which renders as `1,687.10`.
    let text = format!("{:.1}", bytes as f64 / (1u64 << 30) as f64);
    match text.split_once('.') {
        Some((whole, fraction)) => match whole.parse::<usize>() {
            Ok(whole) => format!("{}.{fraction}", count(whole)),
            Err(_) => text,
        },
        None => text,
    }
}

/// Whole GiB with thousands separators, for the free-space column.
pub fn gib_whole(bytes: u64) -> String {
    count((bytes as f64 / (1u64 << 30) as f64).round() as usize)
}

/// `done` of `total` as a percentage, one decimal place.
///
/// **One decimal rather than none because a whole-number percent stalls visibly.** A 3,883
/// file pass moves 0.0258 % per file, so a whole number sits unchanged for **~39 files** at a
/// time while one decimal advances every **~3.9** — ten times more often. On the slowest
/// destination 39 files is long enough to read as stuck, which is the exact impression the
/// progress output exists to prevent (decision 22, on an unlabeled silence).
///
/// **It does not step per file, and an earlier version of this comment claimed it did.** The
/// test below asserted that adjacent files render differently and failed on the first run:
/// 1,000 and 1,001 of 3,883 are 25.7533 % and 25.7790 %, both `25.8%`. Ten times more often
/// is the real benefit and it is enough; a second decimal would buy per-file movement at the
/// cost of a number nobody reads.
///
/// A zero total reports `0.0%` rather than dividing: an empty card is refused long before
/// this by pre-flight, so the branch is unreachable in a real run and a panic here would be
/// a rendering bug taking down a run with hundreds of gigabytes to move.
pub fn percent(done: usize, total: usize) -> String {
    if total == 0 {
        return "0.0%".to_owned();
    }
    format!("{:.1}%", done as f64 * 100.0 / total as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_land_every_three_digits_from_the_right() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1_000), "1,000");
        assert_eq!(count(3_883), "3,883");
        assert_eq!(count(15_532), "15,532");
        assert_eq!(count(1_000_000), "1,000,000");
    }

    /// The four-digit case is the one that matters: a full run moves ~1,687 GiB, and the
    /// figure shipped without a separator until the operator spotted it in real output.
    #[test]
    fn gib_carries_separators_and_one_decimal() {
        assert_eq!(gib(0), "0.0");
        assert_eq!(gib(201_252_000_000), "187.4");
        assert_eq!(gib(1_811_700_000_000), "1,687.3");
        assert_eq!(gib_whole(1_620_000_000_000), "1,509");
    }

    /// Separating before rounding would carry the fraction wrong and render `1,687.10`.
    ///
    /// One byte under 1,688 GiB, so the rounding is exact rather than resting on how a
    /// decimal literal lands in binary — an earlier version of this test asserted `1,688.0`
    /// for `1687.95`, which is stored as `1687.949999…` and formats to `1687.9`.
    #[test]
    fn a_fraction_that_rounds_up_does_not_corrupt_the_separator() {
        assert_eq!(gib(1_688 * (1u64 << 30) - 1), "1,688.0");
    }

    #[test]
    fn percent_carries_one_decimal() {
        assert_eq!(percent(0, 3_883), "0.0%");
        assert_eq!(percent(389, 3_883), "10.0%");
        assert_eq!(percent(3_883, 3_883), "100.0%");
    }

    /// The reason the decimal is there, asserted as the thing that is actually true.
    ///
    /// **This test failed the first time it ran, and it was the assertion that was wrong.**
    /// It claimed adjacent files render differently; they do not — 1,000 and 1,001 of 3,883
    /// are both `25.8%`. What one decimal buys is ten times more movement, not per-file
    /// movement, and that is what is checked here.
    #[test]
    fn one_decimal_advances_about_ten_times_more_often_than_a_whole_number() {
        let total = 3_883;
        let steps = |render: &dyn Fn(usize) -> String| {
            (0..total).filter(|&n| render(n) != render(n + 1)).count()
        };

        let one_decimal = steps(&|n| percent(n, total));
        let whole = steps(&|n| format!("{}%", n * 100 / total));

        assert!(
            one_decimal >= whole * 9,
            "one decimal gave {one_decimal} visible steps against {whole} for a whole number"
        );
    }

    /// Pre-flight refuses an empty card, so this is a rendering guard rather than a case.
    #[test]
    fn an_empty_total_does_not_divide_by_zero() {
        assert_eq!(percent(0, 0), "0.0%");
    }
}
