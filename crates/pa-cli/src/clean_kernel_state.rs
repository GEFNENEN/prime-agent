//! `prime-agent clean-kernel-state`: delete stale `kernel-state` snapshot
//! files from the session artifacts tree.
//!
//! A session's kernel namespace snapshot lives in its artifact directory as
//! `kernel-state.dill` (the payload, up to the 256 MiB aggregate cap) plus
//! `kernel-state.json` (the manifest); an interrupted write can also leave a
//! `kernel-state.*.tmp` staging file behind. This command removes those files
//! once they have been untouched for longer than the threshold. Sessions,
//! transcripts, scheduled jobs, and directory structure are never touched;
//! a session restored after its snapshot was removed simply starts with an
//! empty kernel namespace.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use crate::config::{get_agent_dir, APP_NAME};

/// The artifacts tree under the agent dir (`<agent-dir>/session-artifacts`
/// holds one directory per persisted session, nested for RLM children).
const SESSION_ARTIFACTS_DIR: &str = "session-artifacts";

/// The default `--days` threshold.
const DEFAULT_DAYS: u64 = 1;

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Options {
    days: u64,
    dry_run: bool,
    json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UsageError(String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One removable file: the snapshot payload, its manifest, or a leftover
/// staging temp.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    path: PathBuf,
    bytes: u64,
}

/// Parse `clean-kernel-state [--days <n>] [--dry-run] [--json]` arguments.
///
/// # Errors
///
/// Returns [`UsageError`] for an unknown option, a missing/empty value, or a
/// non-numeric `--days`.
fn parse_options(args: &[String]) -> Result<Options, UsageError> {
    let mut options = Options {
        days: DEFAULT_DAYS,
        dry_run: false,
        json: false,
    };
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let mut name = arg;
        let mut value: Option<String> = None;
        if arg.starts_with("--") {
            if let Some(equals_index) = arg.find('=') {
                name = &arg[..equals_index];
                value = Some(arg[equals_index + 1..].to_string());
            }
        }
        match name {
            "--dry-run" => {
                if value.is_some() {
                    return Err(UsageError(format!("Option {name} takes no value.")));
                }
                options.dry_run = true;
            }
            "--json" => {
                if value.is_some() {
                    return Err(UsageError(format!("Option {name} takes no value.")));
                }
                options.json = true;
            }
            "--days" => {
                let value = if let Some(value) = value {
                    value
                } else {
                    let Some(next) = args.get(index + 1) else {
                        return Err(UsageError(format!("Option {name} requires a value.")));
                    };
                    index += 1;
                    next.clone()
                };
                options.days = value.parse::<u64>().map_err(|_| {
                    UsageError(format!(
                        "Option {name} requires a whole number of days (got \"{value}\")."
                    ))
                })?;
            }
            _ => {
                return Err(UsageError(format!(
                    "Unknown option for clean-kernel-state: {arg}"
                )));
            }
        }
        index += 1;
    }
    Ok(options)
}

/// `kernel-state.dill`, `kernel-state.json`, or a `kernel-state.*.tmp`
/// staging leftover (the runtime writes `kernel-state.dill.<rand>.tmp` /
/// `kernel-state.json.<rand>.tmp` via `mkstemp`).
fn is_kernel_state_file(name: &str) -> bool {
    name == "kernel-state.dill"
        || name == "kernel-state.json"
        || (name.starts_with("kernel-state.")
            && Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tmp")))
}

/// Collect every matching file under `root` (recursively) whose mtime is at
/// least `min_age` old. Symlinks are never followed: the sweep stays inside
/// the tree it was pointed at.
fn collect_targets(root: &Path, min_age: Duration, targets: &mut Vec<Target>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_targets(&path, min_age, targets);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_kernel_state_file(name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        // A future mtime reads as young, never as stale.
        let Ok(age) = modified.elapsed() else {
            continue;
        };
        if age >= min_age {
            targets.push(Target {
                path,
                bytes: metadata.len(),
            });
        }
    }
}

/// One removed file with the bytes it freed.
type RemovedFile = (PathBuf, u64);
/// One file the sweep could not remove, with the OS error.
type FailedRemoval = (PathBuf, String);

/// Remove every target; files that fail to delete (in use, permissions) are
/// reported instead of failing the whole run.
fn remove_targets(targets: &[Target]) -> (Vec<RemovedFile>, Vec<FailedRemoval>) {
    let mut removed: Vec<RemovedFile> = Vec::new();
    let mut failed: Vec<FailedRemoval> = Vec::new();
    for target in targets {
        match fs::remove_file(&target.path) {
            Ok(()) => removed.push((target.path.clone(), target.bytes)),
            // Already gone (a concurrent clean, or a session deleting its
            // own artifacts) still satisfies the command's goal.
            Err(error) if error.kind() == ErrorKind::NotFound => {
                removed.push((target.path.clone(), target.bytes));
            }
            Err(error) => failed.push((target.path.clone(), error.to_string())),
        }
    }
    (removed, failed)
}

#[allow(clippy::cast_precision_loss)]
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn threshold_text(days: u64) -> String {
    match days {
        0 => "any age".to_string(),
        1 => "older than 1 day".to_string(),
        days => format!("older than {days} days"),
    }
}

fn target_entries<'a>(targets: impl IntoIterator<Item = &'a Target>) -> Vec<Value> {
    targets
        .into_iter()
        .map(|target| {
            json!({
                "path": target.path.display().to_string(),
                "bytes": target.bytes,
            })
        })
        .collect()
}

fn dry_run_json(root: &Path, options: &Options, targets: &[Target]) -> Value {
    let total: u64 = targets.iter().map(|target| target.bytes).sum();
    json!({
        "root": root.display().to_string(),
        "days": options.days,
        "dryRun": true,
        "candidates": target_entries(targets),
        "candidateCount": targets.len(),
        "candidateBytes": total,
    })
}

fn removal_json(
    root: &Path,
    options: &Options,
    removed: &[RemovedFile],
    failed: &[FailedRemoval],
) -> Value {
    let freed: u64 = removed.iter().map(|(_, bytes)| *bytes).sum();
    json!({
        "root": root.display().to_string(),
        "days": options.days,
        "dryRun": false,
        "removed": removed
            .iter()
            .map(|(path, bytes)| json!({
                "path": path.display().to_string(),
                "bytes": bytes,
            }))
            .collect::<Vec<_>>(),
        "failed": failed
            .iter()
            .map(|(path, error)| json!({
                "path": path.display().to_string(),
                "error": error,
            }))
            .collect::<Vec<_>>(),
        "removedCount": removed.len(),
        "removedBytes": freed,
        "failedCount": failed.len(),
    })
}

pub(crate) fn run_clean_kernel_state(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("Error: {error}");
            eprintln!("Run \"{APP_NAME} help clean-kernel-state\" for usage.");
            return 1;
        }
    };
    let root = get_agent_dir().join(SESSION_ARTIFACTS_DIR);
    let min_age = Duration::from_secs(options.days.saturating_mul(SECONDS_PER_DAY));
    let mut targets: Vec<Target> = Vec::new();
    if root.is_dir() {
        collect_targets(&root, min_age, &mut targets);
    }
    targets.sort_by(|left, right| left.path.cmp(&right.path));

    if options.dry_run {
        if options.json {
            println!("{}", dry_run_json(&root, &options, &targets));
        } else if targets.is_empty() {
            println!(
                "Nothing to clean: no kernel-state files {} under {}.",
                threshold_text(options.days),
                root.display()
            );
        } else {
            let total: u64 = targets.iter().map(|target| target.bytes).sum();
            println!(
                "Would remove {} kernel-state file(s) {}, freeing {}:",
                targets.len(),
                threshold_text(options.days),
                format_bytes(total)
            );
            for target in &targets {
                println!(
                    "  {} ({})",
                    target.path.display(),
                    format_bytes(target.bytes)
                );
            }
        }
        return 0;
    }

    let (removed, failed) = remove_targets(&targets);
    if options.json {
        println!("{}", removal_json(&root, &options, &removed, &failed));
        return 0;
    }
    if removed.is_empty() && failed.is_empty() {
        println!(
            "Nothing to clean: no kernel-state files {} under {}.",
            threshold_text(options.days),
            root.display()
        );
        return 0;
    }
    if !removed.is_empty() {
        let freed: u64 = removed.iter().map(|(_, bytes)| *bytes).sum();
        println!(
            "Removed {} kernel-state file(s), freeing {}.",
            removed.len(),
            format_bytes(freed)
        );
    }
    if !failed.is_empty() {
        println!("Skipped {} file(s):", failed.len());
        for (path, error) in &failed {
            println!("  {} ({error})", path.display());
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::time::SystemTime;

    fn touch(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn backdate(path: &Path, age: Duration) {
        let file = OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    fn parse(args: &[&str]) -> Result<Options, UsageError> {
        parse_options(&args.iter().map(|arg| (*arg).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn options_default_to_one_day() {
        let options = parse(&[]).unwrap();
        assert_eq!(
            options,
            Options {
                days: 1,
                dry_run: false,
                json: false,
            }
        );
    }

    #[test]
    fn options_accept_days_in_both_forms() {
        assert_eq!(parse(&["--days", "7"]).unwrap().days, 7);
        assert_eq!(parse(&["--days=7"]).unwrap().days, 7);
        assert_eq!(parse(&["--days", "0"]).unwrap().days, 0);
        let flags = parse(&["--dry-run", "--json"]).unwrap();
        assert!(flags.dry_run && flags.json && flags.days == 1);
    }

    #[test]
    fn options_reject_unknown_flags_and_bad_days() {
        assert!(parse(&["--weeks", "1"]).is_err());
        assert!(parse(&["--days"]).is_err());
        assert!(parse(&["--days", "soon"]).is_err());
        assert!(parse(&["--days=1.5"]).is_err());
        assert!(parse(&["--dry-run=yes"]).is_err());
    }

    #[test]
    fn collection_matches_only_stale_kernel_state_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let old = Duration::from_secs(2 * SECONDS_PER_DAY);
        let fresh = Duration::from_secs(60);

        for stale in [
            "sess-a/kernel-state.dill",
            "sess-a/kernel-state.json",
            "sess-a/kernel-state.dill.abcd1234.tmp",
            "sess-a/session-artifacts/child/kernel-state.dill",
        ] {
            let path = root.join(stale);
            touch(&path, b"payload");
            backdate(&path, old);
        }
        for young in [
            "sess-b/kernel-state.dill",
            "sess-b/kernel-state.dill.abcd1234.tmp",
        ] {
            let path = root.join(young);
            touch(&path, b"payload");
            backdate(&path, fresh);
        }
        for never in [
            "sess-a/scheduled-jobs.json",
            "sess-a/semantic-edges.jsonl",
            "sess-a/notes.tmp",
            "sess-a/kernel-state.bak",
        ] {
            let path = root.join(never);
            touch(&path, b"untouched");
            backdate(&path, old);
        }

        let mut targets = Vec::new();
        collect_targets(root, Duration::from_secs(SECONDS_PER_DAY), &mut targets);
        let mut names: Vec<String> = targets
            .iter()
            .map(|target| {
                target
                    .path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "sess-a/kernel-state.dill",
                "sess-a/kernel-state.dill.abcd1234.tmp",
                "sess-a/kernel-state.json",
                "sess-a/session-artifacts/child/kernel-state.dill",
            ]
        );
    }

    #[test]
    fn zero_days_matches_every_age_but_never_future_mtimes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let fresh = root.join("sess/kernel-state.dill");
        touch(&fresh, b"payload");
        let future = root.join("sess/kernel-state.json");
        touch(&future, b"{}");
        let file = OpenOptions::new().write(true).open(&future).unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(3600))
            .unwrap();

        let mut targets = Vec::new();
        collect_targets(root, Duration::ZERO, &mut targets);
        let names: Vec<String> = targets
            .iter()
            .map(|target| target.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["kernel-state.dill".to_string()]);
    }

    #[test]
    fn removal_deletes_targets_and_counts_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let old = Duration::from_secs(2 * SECONDS_PER_DAY);
        let payload = root.join("sess/kernel-state.dill");
        touch(&payload, b"payload!");
        backdate(&payload, old);
        let manifest = root.join("sess/kernel-state.json");
        touch(&manifest, b"{}");
        backdate(&manifest, old);
        let unrelated = root.join("sess/scheduled-jobs.json");
        touch(&unrelated, b"keep me");
        backdate(&unrelated, old);

        let mut targets = Vec::new();
        collect_targets(root, Duration::from_secs(SECONDS_PER_DAY), &mut targets);
        assert_eq!(targets.len(), 2);
        let (removed, failed) = remove_targets(&targets);
        assert_eq!(removed.len(), 2);
        assert!(failed.is_empty());
        assert!(!payload.exists());
        assert!(!manifest.exists());
        assert!(unrelated.exists());
        let freed: u64 = removed.iter().map(|(_, bytes)| bytes).sum();
        assert_eq!(freed, 8 + 2);
    }
}
