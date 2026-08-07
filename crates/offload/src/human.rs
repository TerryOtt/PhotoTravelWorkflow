//! Number formatting shared by the report and the progress bars.
//!
//! **Shared so the two cannot drift**, which they did: `count` was private to `main.rs`, so the
//! report obeyed [`docs/WRITING.md`](../../../docs/WRITING.md) rule 6's thousands separators
//! while the progress line beside it rendered a bare `3883`.

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

/// Bytes as **whole GiB, rounded up**, for anything answering *how much is there to move*.
///
/// Up because overstating what you *need* is the safe error; [`gib_down`] rounds the other way.
/// The pair's invariant is asserted by `up_and_down_straddle_the_true_value_and_never_cross`.
///
/// **Sizes are GiB, rates are decimal, and nothing asserts that.** Windows is what a size gets
/// checked against — Explorer divides by 2^30 — so a payload printed as decimal `202` sends the
/// operator to a file manager saying `188`. Throughput stays decimal because a link's speed is
/// decimal by definition. **Each is the unit its own question is asked in.**
pub fn gib_up(bytes: u64) -> String {
    if bytes < DECIMAL_BELOW {
        return tenths((bytes * 10).div_ceil(GIB));
    }
    count(bytes.div_ceil(GIB) as usize)
}

/// One gibibyte.
const GIB: u64 = 1 << 30;

/// Below this, sizes keep one decimal place.
///
/// **Ten gibibytes is far below any real night, so this is not a compromise between two
/// preferences.** The shooting contract in `docs/CONOPS.md` puts even a local afternoon at
/// 300–500 frames, so **a sub-10 GiB payload is never a shooting day — it is a staged test
/// slice**, and that is where the tenth earns its place: the 50-frame corpus is 2.6 GiB, renders
/// as `3` under a plain ceiling, and a 15 % overstatement cannot be eyeballed against the source.
/// Whole GiB remains what the operator sees on every real run.
const DECIMAL_BELOW: u64 = 10 * GIB;

/// Tenths of a GiB as `2.6`, with separators on the whole part.
fn tenths(n: u64) -> String {
    format!("{}.{}", count((n / 10) as usize), n % 10)
}

/// Bytes as **whole GiB, rounded down**, for anything answering *how much room is there*.
///
/// Down because understating what you *have* is the safe error, mirroring [`gib_up`]. Together
/// they guarantee the pre-flight line can never read as though tonight fits when it does not.
pub fn gib_down(bytes: u64) -> String {
    if bytes < DECIMAL_BELOW {
        return tenths(bytes * 10 / GIB);
    }
    count((bytes / GIB) as usize)
}

/// `done` of `total` as a percentage, one decimal place.
///
/// **One decimal rather than none because a whole-number percent stalls visibly.** A 3,883
/// file pass moves 0.0258 % per file, so a whole number sits unchanged for **~39 files** at a
/// time while one decimal advances every **~3.9** — ten times more often. On the slowest
/// destination 39 files is long enough to read as stuck, which is the exact impression the
/// progress output exists to prevent (decision 22, on an unlabeled silence).
///
/// **It does not step per file** — 1,000 and 1,001 of 3,883 are both `25.8%`. Ten times more
/// movement is the real benefit and it is enough; a second decimal would buy per-file stepping at
/// the cost of a number nobody reads.
///
/// A zero total reports `0.0%` rather than dividing: pre-flight refuses an empty card long before
/// this, so the branch is unreachable in a real run and a panic here would be a rendering bug
/// taking down a run mid-copy.
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

    /// Rendering across the whole range: four-digit separators, the sub-10 GiB decimal, and the
    /// threshold from both sides. `2_796_966_092` is the staged 50-frame slice — 2.605 GiB, which
    /// a plain ceiling would render `3`.
    #[test]
    fn gib_renders_separators_above_the_threshold_and_one_decimal_below() {
        assert_eq!(gib_up(201_252_000_000), "188");
        assert_eq!(gib_up(1_811_700_000_000), "1,688");
        assert_eq!(gib_down(1_620_000_000_000), "1,508");

        assert_eq!(gib_up(2_796_966_092), "2.7");
        assert_eq!(gib_down(2_796_966_092), "2.6");
        assert_eq!(gib_up(0), "0.0");

        assert_eq!(gib_up(10 * (1u64 << 30) - 1), "10.0");
        assert_eq!(gib_up(10 * (1u64 << 30)), "10");
        assert_eq!(gib_down(10 * (1u64 << 30)), "10");
    }

    /// An exact multiple must not be inflated — `div_ceil`'s boundary, asserted from both
    /// sides. One byte more is a whole GiB more, which is the cost of rounding up and is
    /// the intended behavior rather than an accident of it.
    #[test]
    fn an_exact_multiple_is_not_rounded_up() {
        assert_eq!(gib_up(387 * (1u64 << 30)), "387");
        assert_eq!(gib_up(387 * (1u64 << 30) + 1), "388");
        assert_eq!(gib_down(387 * (1u64 << 30)), "387");
        assert_eq!(gib_down(388 * (1u64 << 30) - 1), "387");
    }

    /// **The invariant the pair exists to hold.** For the same bytes, what you *need* must
    /// never render below what you *have*, or `NOT ENOUGH ROOM` can print two equal numbers
    /// while refusing the run. They must also never differ by more than one whole GiB, which
    /// is what keeps the overstatement honest rather than merely safe.
    #[test]
    fn up_and_down_straddle_the_true_value_and_never_cross() {
        let plain = |text: String| text.replace(',', "").parse::<f64>().unwrap();

        for bytes in [
            0,
            1,
            (1u64 << 30) - 1,
            1u64 << 30,
            (1u64 << 30) + 1,
            2_796_966_092,
            10 * (1u64 << 30) - 1,
            10 * (1u64 << 30),
            386_600_000_000,
            415_137_034_818,
            u64::MAX / 4,
        ] {
            let (up, down) = (plain(gib_up(bytes)), plain(gib_down(bytes)));
            let truth = bytes as f64 / (1u64 << 30) as f64;

            // The granularity of whichever side of the threshold this landed on, which is the
            // most the two may ever differ by.
            let step = if bytes < 10 * (1u64 << 30) { 0.1 } else { 1.0 };

            assert!(up >= down, "{bytes}: up {up} below down {down}");
            assert!(
                up + 1e-9 >= truth && down <= truth + 1e-9,
                "{bytes}: {down} and {up} do not straddle {truth}"
            );
            assert!(
                up - down <= step + 1e-9,
                "{bytes}: up {up} and down {down} differ by more than {step}"
            );
        }
    }

    #[test]
    fn percent_carries_one_decimal() {
        assert_eq!(percent(0, 3_883), "0.0%");
        assert_eq!(percent(389, 3_883), "10.0%");
        assert_eq!(percent(3_883, 3_883), "100.0%");
        // Pre-flight refuses an empty card, so this is a rendering guard rather than a case.
        assert_eq!(percent(0, 0), "0.0%");
    }

    /// The reason the decimal is there, asserted as the thing that is actually true: **ten times
    /// more movement, not per-file movement.** Adjacent files often render identically.
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
}
