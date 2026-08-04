//! Decision 28's test: **every manifest this tool has ever written stays readable.**
//!
//! This is the one test aimed at a defect that surfaces years from now — a reader that
//! quietly stops understanding an old archive. It qualifies on decision 18's own
//! criterion rather than as an exception to it: the damage is irreversible, because the
//! disk in the safe cannot be regenerated.
//!
//! **The fixture is committed, and that is the whole point.** It is not generated at
//! test time from the current structs — a fixture built by the code under test would
//! move whenever the code moved, and would agree with a schema 1 that had quietly
//! stopped being schema 1. This file is a schema-1 manifest as written in 2026, frozen,
//! and it must still read correctly on every build that ever ships.
//!
//! **Do not regenerate it to make a failing test pass.** A failure here means either a
//! genuine incompatibility — fix the reader — or a deliberate schema change, which
//! decision 28 says must be *additive* and must leave this file readable anyway. There
//! is no third case where updating the fixture is the right answer.

use photoday::manifest::{Corroborated, Manifest, Status};

const FIXTURE: &str = include_str!("fixtures/manifest-schema-1.json");

/// The whole promise, in one assertion: a 2026 manifest still reads.
#[test]
fn a_schema_1_manifest_from_2026_still_reads() {
    let manifest = Manifest::parse(FIXTURE).expect(
        "a committed schema-1 manifest must read on every build, forever. If this fails, \
         fix the reader — do not regenerate the fixture.",
    );

    assert_eq!(manifest.schema, 1);
    assert_eq!(manifest.body.date_utc, "2022-09-27");
    assert_eq!(manifest.body.destination, "OWC");
    assert_eq!(manifest.body.files.len(), 2);
}

/// The stable core decision 28 says no schema bump may redefine. Asserted field by field
/// rather than by count, because "the manifest parsed" would still be true if one of
/// these silently changed meaning.
#[test]
fn the_stable_core_fields_still_mean_what_they_meant() {
    let manifest = Manifest::parse(FIXTURE).expect("the fixture reads");
    let frame = &manifest.body.files[0];

    assert_eq!(frame.name, "1402Z_0001.CR3");
    assert_eq!(frame.status, Status::Present);
    assert_eq!(
        frame.sha256,
        "9f2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f809"
    );
    assert_eq!(frame.bytes, 47_185_920);
    assert_eq!(frame.captured_utc.as_deref(), Some("2022-09-27T14:02:05Z"));
    assert_eq!(frame.corroborated, Some(Corroborated::Matched));
}

/// A tombstone from 2026 must still read as a tombstone, or a `verify` years later
/// reports a file as missing that somebody deliberately deleted — the exact confusion
/// decision 12 keeps tombstones to prevent.
#[test]
fn a_tombstone_from_2026_still_reads_as_deleted_and_explained() {
    let manifest = Manifest::parse(FIXTURE).expect("the fixture reads");
    let tombstone = &manifest.body.files[1];

    assert_eq!(tombstone.status, Status::Deleted);
    assert_eq!(tombstone.corroborated, None);

    let deletion = tombstone
        .deletion
        .as_ref()
        .expect("a tombstone carries why it was deleted");

    assert_ne!(
        deletion.source_sha256, deletion.other_sha256,
        "a tombstone records the two hashes that disagreed"
    );
    assert_eq!(deletion.reason, "the two cards disagreed");
}

/// The fixture's own checksum still validates, which is what proves the *checksum
/// algorithm* has not drifted — a change to how the body is canonicalized would make
/// every archived manifest report as damaged, on photographs that are perfectly fine.
#[test]
fn the_2026_checksum_still_validates_under_todays_canonicalization() {
    // `parse` checks it; this asserts the value itself so a change is visible in the
    // diff rather than only in a failure.
    assert!(FIXTURE.contains("e1a3425ae18f5964d313894d47829ceadadd782eb866699c8436cd47d8d3cc2c"));
    assert!(Manifest::parse(FIXTURE).is_ok());
}
