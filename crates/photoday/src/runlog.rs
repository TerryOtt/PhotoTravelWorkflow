//! The append-only record of what has actually landed.
//!
//! Decision 12 splits the two artifacts by durability requirement, and this is the
//! fragile one: **JSON Lines, appended a record at a time, so a crash mid-phase-3
//! leaves a valid partial record rather than a truncated array.** The durable artifact
//! is the per-date-folder manifest, written atomically at the end of a run.
//!
//! A record is appended only **after** that file's verify read completed on that
//! destination (decision 13), so nothing in the log ever describes work still in
//! flight. That is what makes the log trustworthy up to its last intact line, and the
//! only artifact a crash can leave is a torn final line.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One `(file, destination)` pair, proven.
///
/// Per *pair* rather than per file, because a file counts as done only for a specific
/// destination (decision 13) — so a crash partway through fan-out means redoing that
/// file on one disk, not redoing the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verified {
    pub run_id: String,
    /// Relative to the destination root, e.g. `2026/2026-08-03/1422Z_0001.CR3`.
    pub name: String,
    /// The destination's config label, which survives a drive-letter change.
    pub destination: String,
    pub sha256: String,
    pub bytes: u64,
    /// Absent for a file in `_unfiled`, which is there precisely because it has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_utc: Option<String>,
    pub source_card: String,
    pub verified_utc: String,
}

/// An open run log, appendable from every destination thread at once.
///
/// The mutex is not a bottleneck and is not trying to be: one line per file per
/// destination is ~5,000 writes spread over minutes, against four threads that spend
/// their time moving gigabytes. A channel to a dedicated writer thread would buy
/// nothing and add a shutdown ordering problem.
pub struct RunLog {
    file: Mutex<File>,
}

impl RunLog {
    /// Open for append, creating it if this is a fresh run.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the run log {}", path.display()))?;

        Ok(Self {
            file: Mutex::new(file),
        })
    }

    /// Append one proven pair and put it beyond the reach of a crash.
    ///
    /// Flushed and synced per record rather than buffered. The whole value of this file
    /// is that it survives whatever killed the process, and a record sitting in a
    /// `BufWriter` when the power goes describes work that is done but will be redone —
    /// which is merely wasteful — while one sitting there when the *disk* goes is worse.
    /// ~5,000 syncs against a run that moves hundreds of gigabytes is not a cost.
    pub fn append(&self, record: &Verified) -> Result<()> {
        let line = serde_json::to_string(record).context("serializing a run log record")?;

        let mut file = self
            .file
            .lock()
            .expect("the run log mutex is never poisoned");
        writeln!(file, "{line}").context("appending to the run log")?;
        file.sync_data().context("syncing the run log")?;

        Ok(())
    }
}

/// Every intact record, in the order it was appended.
///
/// **A torn final line is discarded, not an error** (decision 13). It is the one thing
/// a crash can leave behind, it describes work that was in flight rather than proven,
/// and refusing to read the log because of it would turn a recoverable interruption
/// into a manual repair job at 11pm.
///
/// A torn line anywhere *else* is a different matter and is reported: records are
/// appended under a lock, one `writeln!` and one sync at a time, so corruption in the
/// middle of the file means something damaged the log rather than merely interrupted it.
pub fn read(path: &Path) -> Result<Vec<Verified>> {
    let file = match File::open(path) {
        Ok(file) => file,
        // No log is the normal case for a fresh run, not a failure.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("opening the run log {}", path.display()));
        }
    };

    let lines: Vec<String> = BufReader::new(file)
        .lines()
        .collect::<std::io::Result<_>>()
        .with_context(|| format!("reading the run log {}", path.display()))?;

    let mut records = Vec::with_capacity(lines.len());
    let last = lines.len().saturating_sub(1);

    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Verified>(line) {
            Ok(record) => records.push(record),
            Err(_) if index == last => break,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "the run log {} is damaged at line {}: a torn line is only \
                         expected as the very last one, where a crash leaves it",
                        path.display(),
                        index + 1
                    )
                });
            }
        }
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn record(name: &str, destination: &str) -> Verified {
        Verified {
            run_id: "2026-08-03T18:22:04Z".into(),
            name: name.into(),
            destination: destination.into(),
            sha256: "9f2b".into(),
            bytes: 47_185_920,
            captured_utc: Some("2026-08-03T14:22:37Z".into()),
            source_card: "cfexpress".into(),
            verified_utc: "2026-08-03T18:23:31Z".into(),
        }
    }

    #[test]
    fn records_round_trip_in_the_order_they_were_appended() {
        let dir = TempDir::new().expect("a scratch directory");
        let path = dir.path().join("_runs").join("run.jsonl");

        let log = RunLog::open(&path).expect("opening");
        log.append(&record("a.CR3", "SSD-A")).expect("appending");
        log.append(&record("b.CR3", "SSD-A")).expect("appending");
        log.append(&record("a.CR3", "SSD-B")).expect("appending");

        let back = read(&path).expect("reading");
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].name, "a.CR3");
        assert_eq!(back[2].destination, "SSD-B");
    }

    /// The crash shape decision 13 describes: the process died mid-`writeln!`. Every
    /// complete record before it must survive.
    #[test]
    fn a_torn_final_line_is_discarded_and_the_rest_survives() {
        let dir = TempDir::new().expect("a scratch directory");
        let path = dir.path().join("run.jsonl");

        let log = RunLog::open(&path).expect("opening");
        log.append(&record("a.CR3", "SSD-A")).expect("appending");
        log.append(&record("b.CR3", "SSD-A")).expect("appending");
        drop(log);

        let mut raw = fs::read_to_string(&path).expect("reading");
        raw.push_str("{\"run_id\":\"2026-08-03T18:22:04Z\",\"name\":\"c.CR");
        fs::write(&path, raw).expect("writing the torn log");

        let back = read(&path).expect("a torn tail must not be an error");
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].name, "b.CR3");
    }

    /// Damage in the *middle* is not a crash artifact and must not be waved through —
    /// the difference between "this run was interrupted" and "this file is corrupt".
    #[test]
    fn a_torn_line_that_is_not_the_last_is_reported() {
        let dir = TempDir::new().expect("a scratch directory");
        let path = dir.path().join("run.jsonl");

        let log = RunLog::open(&path).expect("opening");
        log.append(&record("a.CR3", "SSD-A")).expect("appending");
        log.append(&record("b.CR3", "SSD-A")).expect("appending");
        drop(log);

        let raw = fs::read_to_string(&path).expect("reading");
        let mut lines: Vec<&str> = raw.lines().collect();
        lines.insert(1, "{ this is not json");
        fs::write(&path, lines.join("\n") + "\n").expect("writing the damaged log");

        assert!(read(&path).is_err(), "mid-file damage must be reported");
    }

    #[test]
    fn a_missing_log_reads_as_empty_rather_than_failing() {
        let dir = TempDir::new().expect("a scratch directory");
        assert!(
            read(&dir.path().join("nothing-here.jsonl"))
                .unwrap()
                .is_empty()
        );
    }
}
