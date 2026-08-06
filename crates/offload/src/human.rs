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

/// Bytes as **whole GiB, rounded up**, for anything answering *how much is there to move*.
///
/// **Whole rather than fractional at the operator's request** (2026-08-06): *"at no point will
/// I care about fractional GB."* A tenth of a GiB is 107 MB — below the resolution of any
/// decision made from this figure, and one more digit to skip past on every line carrying one.
///
/// **Up rather than to-nearest, and the direction is load-bearing.** This renders payloads and
/// requirements, where overstating is the safe error: `387` for a 386.6 GiB day can never
/// promise that something fits when it does not. Free space rounds the other way, in
/// [`gib_down`], so the two always move *apart*. That is what stops `NOT ENOUGH ROOM` from
/// printing two identical numbers while refusing the run — which is exactly what one shared
/// rounding would produce at 386.2 GiB free against 386.6 GiB needed.
///
/// **Integer arithmetic, not `f64::ceil`.** `div_ceil` is exact at every input, where a float
/// path has to be reasoned about at the boundary — the same class of trap that once had a test
/// in this file asserting `1,688.0` for a value stored as `1687.949999...`.
///
/// **Capacity is GiB and rates are decimal, and the split is deliberate.** Windows is what the
/// operator checks a figure against — Explorer, PowerShell's `/1GB`, the drive's own properties
/// dialog all divide by 2^30 — so a payload reported in decimal `GB` as `202` sends him to a
/// file manager that says `188` and invites him to wonder which is lying. Neither is; they are
/// the same bytes in two units, and the one matching his other instruments wins.
///
/// **Throughput stays decimal** (`GB/s`, `Gbps`) because a link's speed is decimal by
/// definition — 10 Gbps is 10^10 bits — and a rate expressed in GiB/s cannot be compared to
/// the number printed on the cable. So sizes are GiB, rates are GB, and each is the unit its
/// own question is asked in.
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
/// **Ten gibibytes is far below any real night, and that is the whole point.** Terry shoots
/// roughly thirty frames of each scene across a spread of settings, so even messing about
/// locally he comes home with 300–500 frames — `docs/CONOPS.md` has the shooting contract.
/// **A sub-10 GiB payload is therefore never a shooting day; it is a staged test slice**, and
/// that is exactly where the tenth earns its place: the 50-frame corpus is 2.6 GiB, renders as
/// `3` under a plain ceiling, and a 15 % overstatement cannot be checked against the source by
/// eye.
///
/// So the threshold is not a compromise between two preferences. **Whole GiB is what the
/// operator sees on every real run**, and the decimal exists for the development case he asked
/// it to keep working for.
const DECIMAL_BELOW: u64 = 10 * GIB;

/// Tenths of a GiB as `2.6`, with separators on the whole part.
fn tenths(n: u64) -> String {
    format!("{}.{}", count((n / 10) as usize), n % 10)
}

/// Bytes as **whole GiB, rounded down**, for anything answering *how much room is there*.
///
/// **Down because understating what you have is the safe error**, mirroring [`gib_up`], which
/// overstates what you need. Together they guarantee the pre-flight line can never read as
/// though tonight fits when it does not.
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
    fn gib_carries_separators_and_no_fraction() {
        assert_eq!(gib_up(201_252_000_000), "188");
        assert_eq!(gib_up(1_811_700_000_000), "1,688");
        assert_eq!(gib_down(1_620_000_000_000), "1,508");
    }

    /// **The staged 50-frame slice, which is the case the threshold exists for.** 2,796,966,092
    /// bytes is 2.605 GiB; a plain ceiling renders it `3` and overstates by 15 %, which cannot
    /// be checked against the source by eye. Terry, 2026-08-06, on why no real night lands here:
    /// *"at 30 shots per potential keeper and the very least I shoot ~300-500 shots even just
    /// messing around locally."*
    #[test]
    fn a_small_payload_keeps_one_decimal() {
        assert_eq!(gib_up(2_796_966_092), "2.7");
        assert_eq!(gib_down(2_796_966_092), "2.6");
        assert_eq!(gib_up(0), "0.0");
    }

    /// The threshold itself, from both sides — ten GiB exactly is already whole.
    #[test]
    fn ten_gibibytes_is_where_the_decimal_stops() {
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
