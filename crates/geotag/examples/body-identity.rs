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

use std::process::ExitCode;

use geotag::raw::MediaParser;
use nom_exif::{Exif, ExifIter, ExifTag, MediaSource};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: body-identity <path to a .CR3>");
        return ExitCode::from(2);
    };

    let mut parser = MediaParser::new();

    // The same two calls `raw::capture_time` makes for a CR3 — a seekable source and
    // `parse_exif` — so this probe reads the file exactly the way the engine does. Guessing
    // a tidier-looking API here cost a compile; the engine was the authority all along.
    let source = match std::fs::File::open(&path)
        .map_err(|e| e.to_string())
        .and_then(|file| MediaSource::seekable(file).map_err(|e| e.to_string()))
    {
        Ok(source) => source,
        Err(error) => {
            eprintln!("opening {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let iter: ExifIter = match parser.parse_exif(source) {
        Ok(iter) => iter,
        Err(error) => {
            eprintln!("parsing EXIF from {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let exif: Exif = iter.into();

    println!();
    println!("  {path}");
    println!();

    // The three that matter to decision 34, plus the lens serial as a control: if the lens
    // one is present and the body one is not, that is Canon's tag layout rather than this
    // parser failing to reach the block.
    let fields = [
        ("Make", ExifTag::Make),
        ("Model", ExifTag::Model),
        ("CameraSerialNumber", ExifTag::CameraSerialNumber),
        ("LensModel", ExifTag::LensModel),
        ("LensSerialNumber", ExifTag::LensSerialNumber),
    ];

    let mut have_serial = false;
    for (label, tag) in fields {
        match exif.get(tag).and_then(|v| v.as_str().map(str::to_owned)) {
            Some(value) => {
                println!("  {label:<20} {value}");
                if matches!(tag, ExifTag::CameraSerialNumber) && !value.trim().is_empty() {
                    have_serial = true;
                }
            }
            None => println!("  {label:<20} —  (absent from standard EXIF)"),
        }
    }

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
