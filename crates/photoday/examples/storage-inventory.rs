//! Everything the storage layer can see, printed.
//!
//! ```text
//! cargo run --example storage-inventory
//! ```
//!
//! This is how the identity layer is checked against a real rig, and there is no
//! substitute for it: the module's whole job is to describe hardware, so a test can
//! assert that its answers are self-consistent but never that they are *true*. Only a
//! person looking at the output and recognizing their own disks can do that.
//!
//! Run it with the Thunderbolt hub populated before a trip and confirm three things —
//! that each archive SSD reports a serial, that all of them report *different* disk
//! numbers, and that the two card readers show up as removable with a `DCIM` on them.
//! Those are exactly the assertions decisions 6 and 7 will make; this shows the inputs
//! they will make them from. `photoday --dry-run` subsumes this once pre-flight exists.

use std::path::Path;

use photoday::storage::{self, Volume, device_of};

fn main() {
    let volumes = match storage::volumes() {
        Ok(volumes) => volumes,
        Err(error) => {
            eprintln!("could not enumerate volumes: {error:#}");
            std::process::exit(1);
        }
    };

    println!("{} volumes\n", volumes.len());

    for volume in &volumes {
        describe(volume);
    }

    summarize(&volumes);
}

fn describe(volume: &Volume) {
    let mounts = if volume.mount_points.is_empty() {
        "(not mounted)".to_string()
    } else {
        volume
            .mount_points
            .iter()
            .map(|mount| mount.display().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };

    println!(
        "{mounts}  {}  {}",
        volume.label.as_deref().unwrap_or("(no label)"),
        volume.filesystem.as_deref().unwrap_or("?")
    );
    println!("   {}", volume.guid_path);
    println!(
        "   volume serial {:08X}   {}   {:.1} GB free of {:.1} GB",
        volume.volume_serial,
        if volume.removable {
            "REMOVABLE"
        } else {
            "fixed"
        },
        volume.free_bytes as f64 / 1e9,
        volume.total_bytes as f64 / 1e9,
    );

    match device_of(volume) {
        Ok(device) => println!(
            "   disk {}   serial {}",
            device.disk_number,
            device.serial.as_deref().unwrap_or("(none reported)")
        ),
        Err(error) => println!("   device query failed: {error:#}"),
    }

    if has_dcim(volume) {
        println!("   *** has DCIM — this is a camera card (decision 7)");
    }

    println!();
}

/// The three assertions pre-flight will make, answered against what is plugged in now.
fn summarize(volumes: &[Volume]) {
    let devices: Vec<_> = volumes.iter().filter_map(|v| device_of(v).ok()).collect();

    let mut disks: Vec<u32> = devices.iter().map(|d| d.disk_number).collect();
    disks.sort_unstable();
    disks.dedup();

    let with_serial = devices.iter().filter(|d| d.serial.is_some()).count();
    let cards = volumes.iter().filter(|v| has_dcim(v)).count();

    println!("---");
    println!("{} distinct physical disks", disks.len());
    println!(
        "{with_serial} of {} volumes sit on a device reporting a serial",
        devices.len()
    );
    println!("{cards} volume(s) carry a DCIM directory");
}

fn has_dcim(volume: &Volume) -> bool {
    volume
        .mount_points
        .iter()
        .any(|mount| Path::new(mount).join("DCIM").is_dir())
}
