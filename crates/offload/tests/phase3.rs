//! Phase 3 end to end — decision 18's third test.
//!
//! Two synthetic cards, four temporary destinations, four identical trees out. The
//! design words it as "against real CR3 fixtures", and this deliberately does not
//! commit any: a raw frame is tens of megabytes, it belongs in neither repository, and
//! RawGeotag already holds the corpus and the harness that hashes it.
//!
//! **What that costs, stated plainly rather than glossed:** with no real EXIF in these
//! files every photo takes the `_unfiled` path (decision 21), so this exercises read →
//! hash → write-through → unbuffered verify → run log, and *not* the date-folder naming
//! that [`offload::naming`]'s own tests cover exhaustively. The two together cover the
//! path a real frame takes; neither does alone, and that seam is where a bug would hide
//! if this file ever became the only check.

use std::fs;
use std::path::{Path, PathBuf};

use offload::hash::{hex, sha256};
use offload::pipeline::{self, Destination, Source};
use offload::runlog::{self, RunLog};
use tempfile::TempDir;

const RUN_ID: &str = "2026-08-03T18-22-04";

/// What phase 3 records about the card that fed it (decision 12): the role it
/// played and the volume serial observed at pre-flight — never a card type,
/// which the tool cannot know.
const SOURCE: Source<'static> = Source {
    role: "primary",
    volume_serial: "A4E2-91CC",
};

/// Deterministic bytes that differ per frame, so a pipeline that mixed two files up
/// would fail rather than coincidentally match.
fn frame(seed: u8, size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// A card with `count` frames on it, named the way the R5 names them.
fn card(dir: &Path, count: u8) -> Vec<Vec<u8>> {
    fs::create_dir_all(dir.join("DCIM").join("100EOSR5")).expect("creating the card tree");

    (0..count)
        .map(|n| {
            // Sizes that are not sector multiples, because no real frame is.
            let bytes = frame(n, 100_000 + usize::from(n) * 1_337);
            let name = format!("_50A{:04}.CR3", n + 1);
            fs::write(dir.join("DCIM").join("100EOSR5").join(name), &bytes)
                .expect("writing a frame");
            bytes
        })
        .collect()
}

fn destinations(root: &Path) -> Vec<Destination> {
    ["laptop", "SSD-A", "SSD-B", "SSD-C"]
        .iter()
        .map(|label| Destination {
            label: (*label).to_string(),
            root: root.join(label),
        })
        .collect()
}

/// Every file under `root`, relative and slash-normalized, sorted.
fn tree(root: &Path) -> Vec<String> {
    let mut found: Vec<String> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    found.sort();
    found
}

#[test]
fn one_card_lands_four_identical_verified_trees() {
    let scratch = TempDir::new().expect("a scratch directory");
    let card_dir = scratch.path().join("cfexpress");
    let frames = card(&card_dir, 5);

    let sources = pipeline::cr3_files(&card_dir).expect("walking the card");
    assert_eq!(sources.len(), 5, "the walk must find every CR3");

    let destinations = destinations(scratch.path());
    let log_path = scratch.path().join("_runs").join(RUN_ID).join("run.jsonl");
    let log = RunLog::open(&log_path).expect("opening the run log");

    let outcome = pipeline::run(
        &sources,
        &destinations,
        RUN_ID,
        SOURCE,
        &card_dir,
        &log,
        &offload::progress::Progress::silent(),
    )
    .expect("phase 3 must complete");

    assert!(outcome.landed(), "LANDED is the product: {outcome:?}");
    assert_eq!(outcome.files, 5);
    assert_eq!(
        outcome.bytes,
        frames.iter().map(|f| f.len() as u64).sum::<u64>()
    );

    // Four trees, identical to each other — the thing the whole phase exists to produce.
    let first = tree(&destinations[0].root);
    assert_eq!(
        first.len(),
        6,
        "five frames plus the date folder's manifest: {first:?}"
    );
    for destination in &destinations[1..] {
        assert_eq!(
            tree(&destination.root),
            first,
            "{} diverged",
            destination.label
        );
    }

    // And identical to the card, byte for byte, read back off the disk rather than
    // trusted from the write.
    let raws: Vec<&String> = first.iter().filter(|name| name.ends_with(".CR3")).collect();
    assert_eq!(raws.len(), 5);
    for destination in &destinations {
        for (relative, expected) in raws.iter().zip(&frames) {
            let landed = fs::read(destination.root.join(relative)).expect("reading back");
            assert_eq!(sha256(&landed), sha256(expected), "{relative}");
        }
    }

    // Every copy is self-describing: decision 12's durable artifact, sealed and readable.
    // Without this a disk pulled from the safe has photographs and no way to prove them.
    for destination in &destinations {
        let folder = destination.root.join("_unfiled").join(RUN_ID);
        let manifest = offload::manifest::Manifest::read(&offload::manifest::path_in(&folder))
            .unwrap_or_else(|e| panic!("{} has no readable manifest: {e}", destination.label));

        assert_eq!(manifest.schema, 1);
        assert_eq!(manifest.body.destination, destination.label);
        assert_eq!(manifest.body.files.len(), 5);
        assert_eq!(manifest.body.runs.len(), 1);
        assert_eq!(manifest.body.runs[0].run_id, RUN_ID);

        // Corroboration is pending, not absent — phase 4 has not run (decision 12).
        assert!(manifest.body.files.iter().all(|f| f.corroborated.is_none()));
    }

    for destination in &outcome.destinations {
        assert_eq!(destination.written, 5, "{}", destination.label);
        assert_eq!(destination.verified, 5, "{}", destination.label);
        assert!(destination.failed.is_empty(), "{}", destination.label);
    }
}

/// One record per `(file, destination)` — twenty for five frames across four disks —
/// and every one of them carrying the hash that was actually verified.
#[test]
fn the_run_log_records_every_file_on_every_destination() {
    let scratch = TempDir::new().expect("a scratch directory");
    let card_dir = scratch.path().join("cfexpress");
    let frames = card(&card_dir, 5);

    let sources = pipeline::cr3_files(&card_dir).expect("walking the card");
    let destinations = destinations(scratch.path());
    let log_path = scratch.path().join("run.jsonl");
    let log = RunLog::open(&log_path).expect("opening the run log");

    pipeline::run(
        &sources,
        &destinations,
        RUN_ID,
        SOURCE,
        &card_dir,
        &log,
        &offload::progress::Progress::silent(),
    )
    .expect("phase 3");

    let records = runlog::read(&log_path).expect("reading the run log");
    assert_eq!(records.len(), 20, "5 frames x 4 destinations");

    let expected: Vec<String> = frames.iter().map(|f| hex(&sha256(f))).collect();
    for record in &records {
        assert_eq!(record.run_id, RUN_ID);
        assert_eq!(record.source_card, "primary");
        assert!(
            expected.contains(&record.sha256),
            "unknown hash {}",
            record.sha256
        );
    }

    for label in ["laptop", "SSD-A", "SSD-B", "SSD-C"] {
        assert_eq!(
            records.iter().filter(|r| r.destination == label).count(),
            5,
            "{label}"
        );
    }
}

/// Decision 5's idempotency, which is what makes "just run it again" the answer to
/// every recovery in `CONOPS.md`: a second pass over the same card writes nothing new,
/// skips on identical content, and still verifies everything.
#[test]
fn a_second_run_over_the_same_card_skips_rather_than_rewrites() {
    let scratch = TempDir::new().expect("a scratch directory");
    let card_dir = scratch.path().join("cfexpress");
    card(&card_dir, 4);

    let sources = pipeline::cr3_files(&card_dir).expect("walking the card");
    let destinations = destinations(scratch.path());
    let log = RunLog::open(&scratch.path().join("run.jsonl")).expect("opening the run log");

    let first = pipeline::run(
        &sources,
        &destinations,
        RUN_ID,
        SOURCE,
        &card_dir,
        &log,
        &offload::progress::Progress::silent(),
    )
    .expect("run 1");
    let before = tree(&destinations[0].root);

    let second = pipeline::run(
        &sources,
        &destinations,
        RUN_ID,
        SOURCE,
        &card_dir,
        &log,
        &offload::progress::Progress::silent(),
    )
    .expect("run 2");

    for destination in &first.destinations {
        assert_eq!(destination.written, 4);
        assert_eq!(destination.skipped, 0);
    }
    for destination in &second.destinations {
        assert_eq!(
            destination.written, 0,
            "{} rewrote files",
            destination.label
        );
        assert_eq!(destination.skipped, 4, "{}", destination.label);
        assert_eq!(destination.verified, 4, "{}", destination.label);
    }

    assert!(second.landed());
    assert_eq!(
        tree(&destinations[0].root),
        before,
        "a re-run must not add a suffixed duplicate"
    );
}

/// A frame the camera wrote but whose EXIF says nothing lands in `_unfiled` — still
/// hashed, still on all four disks, still verified (decision 21). Every file in this
/// suite takes that path, so this asserts the placement the others rely on.
#[test]
fn a_file_with_no_readable_capture_time_is_kept_under_unfiled() {
    let scratch = TempDir::new().expect("a scratch directory");
    let card_dir = scratch.path().join("cfexpress");
    card(&card_dir, 2);

    let sources = pipeline::cr3_files(&card_dir).expect("walking the card");
    let destinations = destinations(scratch.path());
    let log = RunLog::open(&scratch.path().join("run.jsonl")).expect("opening the run log");

    let outcome = pipeline::run(
        &sources,
        &destinations,
        RUN_ID,
        SOURCE,
        &card_dir,
        &log,
        &offload::progress::Progress::silent(),
    )
    .expect("phase 3");

    assert_eq!(outcome.unfiled.len(), 2, "both frames are unnameable");
    assert!(outcome.landed(), "unnameable is not unsafe");

    for destination in &destinations {
        let expected = PathBuf::from("_unfiled").join(RUN_ID).join("_50A0001.CR3");
        assert!(
            destination.root.join(&expected).exists(),
            "{} is missing {}",
            destination.label,
            expected.display()
        );
    }
}
