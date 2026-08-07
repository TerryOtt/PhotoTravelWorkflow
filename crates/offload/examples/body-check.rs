//! Decision 34's check, run against a real frame, printing the row the report prints.
//!
//! ```text
//! cargo run --release --example body-check -- <a .CR3> "Canon EOS R5" 082021001047
//! ```
//!
//! **Why this exists: no CR3 is committed to this repository**, so the unit tests can only
//! reach `compare_body` — the pure half. Everything between *a path on a card* and *the string
//! in the report* was type-checked and never executed until this probe ran it. `REVIEWING.md`
//! collects that shape under *a diagnostic that cannot fail*; this project's memory puts it
//! more bluntly: **a feature checked only in the convenient mode is unchecked.**
//!
//! **It calls the shipping functions**, `preflight::check_body` and `BodyReport`'s `Display`,
//! not copies of them. A probe with its own extraction or its own formatting can agree on this
//! rig and drift on the next body — and it would be the probe everyone believed.
//!
//! **What it still does not prove:** that the row lands in the right column of a real
//! pre-flight block. That needs a card in a reader and `offload --dry-run`.
//!
//! **Reads one file and writes nothing**, so it is safe to point at a live card — and at the
//! archive, which permits reads.

use std::path::PathBuf;
use std::process::ExitCode;

use offload::config::Body;
use offload::preflight::{self, BodyReport};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [frame, model, serial] = args.as_slice() else {
        eprintln!(r#"usage: body-check <a .CR3> "<model>" <serial>"#);
        eprintln!(r#"   eg: body-check Q:\...\_DOO0001.CR3 "Canon EOS R5" 082021001047"#);
        return ExitCode::from(2);
    };

    let configured = Body {
        model: model.clone(),
        serial: serial.clone(),
    };
    let path = PathBuf::from(frame);

    let report = preflight::check_body(Some(&path), &configured);

    println!();
    println!("  Frame     {frame}");
    println!("  Config    {} · {}", configured.model, configured.serial);
    println!();

    // Exactly what pre-flight prints, at the column it prints it in — `Body` padded to the
    // width the card labels set. The alignment is illustrative here; only a real run proves it.
    println!("      Camera Cards");
    println!("          {:<12}{report}", "Body");
    println!();

    // The arm is named as well as rendered, because two of the four read similarly at a glance
    // and they mean different things to the operator.
    let (verdict, code) = match &report {
        BodyReport::AsConfigured { .. } => ("MATCHES — this is the configured body", 0),
        BodyReport::Unexpected { .. } => ("DIFFERS — reported, and the run would proceed", 0),
        BodyReport::FrameSaysNothing => ("the frame carries no camera tags", 0),
        BodyReport::Unreadable(_) => ("the frame could not be read", 1),
    };
    println!("  {verdict}");
    println!();
    println!("  INFO in every arm: never the verdict, never the exit code, no badge.");

    ExitCode::from(code)
}
