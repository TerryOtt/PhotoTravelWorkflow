//! Can this tool read the camera body's identity out of a CR3? Decision 34 depends on it.
//!
//! ```text
//! cargo run --release --example body-identity -- E:\DCIM\100EOS5D\_50A0001.CR3
//! ```
//!
//! **✔ Answered — decision 34 is blocked on nothing.** The R5 writes `CameraSerialNumber`
//! (0xa431) into **standard EXIF**, so no MakerNote decoding and no new dependency. Confirmed
//! 2026-08-07 across nine frames spanning 2021 to 2026.
//!
//! **The config value is `082021001047`**, stable from 2024-09-29 through the newest frame in
//! the archive. **A serial recorded in `DESIGN.md` as this probe's result — `092023000050` —
//! matched none of the four R5 bodies Terry has shot** and was never read off his rig. Take the
//! value from a real frame, never from a document: a wrong serial mismatches on every run
//! forever and is indistinguishable from the feature working.
//!
//! The serial is the field that makes the check worth having — a *model* check passes cleanly
//! on a rented R5, and he rented one from 2021 until buying his own in 2024. The concern that
//! made this probe necessary: **Canon has historically written the body serial into MakerNotes
//! rather than standard EXIF**, and in a CR3 those are different boxes — `CMT1`/`CMT2` are the
//! EXIF IFDs, `CMT3` is MakerNotes. `nom-exif` collects the standard tag and *locates* CMT3
//! without necessarily decoding Canon's structure inside it.
//!
//! **That Lightroom displays a serial proves it is in the file, not which box it is in** —
//! Lightroom reads MakerNotes heavily. And binding constraint 1 is what makes the difference
//! expensive rather than academic: there is no ExifTool to fall back on, so a MakerNotes-only
//! serial would mean decoding Canon's structure by hand.
//!
//! **Reads one file and writes nothing**, so it is safe to point at a live card — which is
//! the only place a real frame lives, since no CR3 is committed to this repository.

use std::path::Path;
use std::process::ExitCode;

use geotag::format::RawFormat;
use geotag::raw::{self, MediaParser};
use nom_exif::{Exif, ExifIter, ExifTag, MediaSource};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: body-identity <path to a .CR3>");
        return ExitCode::from(2);
    };

    let mut parser = MediaParser::new();

    // **The body fields go through `raw::body_identity`, which is what pre-flight calls.**
    // A probe with its own copy of the extraction can agree with the engine on this rig and
    // disagree on the next body — and it would be the probe everyone believed. The lens
    // fields below are read directly because the engine has no reason to expose them.
    let body = match raw::body_identity(&mut parser, Path::new(&path), RawFormat::Cr3) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    println!();
    println!("  {path}");
    println!();

    for (label, value) in [
        ("Make", &body.make),
        ("Model", &body.model),
        ("CameraSerialNumber", &body.serial),
    ] {
        match value {
            Some(text) => println!("  {label:<20} {text}"),
            None => println!("  {label:<20} —  (absent from standard EXIF)"),
        }
    }

    // The lens serial is the **control**: present alongside a missing body serial, that is
    // Canon's tag layout rather than this parser failing to reach the block. Decision 34
    // records why the lens is deliberately never *checked* — Terry rents glass constantly.
    let lens = lens_fields(&mut parser, &path);
    for (label, value) in lens {
        match value {
            Some(text) => println!("  {label:<20} {text}"),
            None => println!("  {label:<20} —  (absent from standard EXIF)"),
        }
    }

    let have_serial = body.serial.is_some();

    println!();
    if have_serial {
        println!("  ✔ CameraSerialNumber is in standard EXIF — decision 34 can record a serial");
    } else {
        println!("  ✗ no CameraSerialNumber in standard EXIF");
        println!("    Canon put it in MakerNotes, which binding constraint 1 makes expensive.");
        println!("    Decision 34 falls back to Model alone, which still carries the");
        println!("    decision 23 payoff — a body that records no UTC offset is the case");
        println!("    that actually costs a night, and Model catches it.");
    }

    ExitCode::SUCCESS
}

/// The two lens tags, read straight from `nom-exif`.
///
/// Deliberately *not* on [`geotag::raw::BodyIdentity`]: decision 34 refuses to check the
/// lens, and a field on the shipping type would be an invitation to start. Here it is a
/// control for this probe and nothing else, so a failure to read it costs an em dash.
fn lens_fields(parser: &mut MediaParser, path: &str) -> [(&'static str, Option<String>); 2] {
    let exif: Option<Exif> = std::fs::File::open(path)
        .ok()
        .and_then(|file| MediaSource::seekable(file).ok())
        .and_then(|source| parser.parse_exif(source).ok())
        .map(|iter: ExifIter| iter.into());

    let read = |tag: ExifTag| {
        exif.as_ref()
            .and_then(|exif| exif.get(tag))
            .and_then(|value| value.as_str().map(str::to_owned))
    };

    [
        ("LensModel", read(ExifTag::LensModel)),
        ("LensSerialNumber", read(ExifTag::LensSerialNumber)),
    ]
}
