//! `prime-agent session clean`: delete saved sessions that carry no
//! conversational payload.
//!
//! Every `prime-agent` launch creates a session file, so a launch that is
//! closed before the first prompt leaves a bootstrap-only file behind. They
//! accumulate silently: 82 of 136 sessions in one install, all under 1.3 KB.
//!
//! "Empty" is decided from the RECORD TYPES, never from a byte threshold. A
//! session is empty when every line is one of the bootstrap records below; a
//! single `message` or `custom_message` anywhere makes it real. That way a
//! small-but-meaningful session (a bare `/compact`, a one-line reply) is never
//! swept, and adding a new bootstrap record type to the writer can only ever
//! make this MORE conservative.
//!
//! Two guards keep the command from destroying something the user wanted:
//! a named session (`session_info`) is kept unless `--include-named` is passed,
//! and anything younger than `--min-age-hours` (default 1) is kept so a session
//! that is merely waiting for its first prompt survives.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Records a freshly created session writes before any conversation happens.
const BOOTSTRAP_TYPES: &[&str] = &[
    "session",
    "model_change",
    "thinking_level_change",
    "service_tier_change",
    "session_state",
];

pub struct Options {
    pub dry_run: bool,
    pub include_named: bool,
    pub min_age_hours: u64,
    pub json: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            dry_run: false,
            include_named: false,
            min_age_hours: 1,
            json: false,
        }
    }
}

/// Why one session file was kept.
pub enum Keep {
    /// A `message` or `custom_message` record is present.
    HasPayload,
    /// `session_info` gives the session a name the user may navigate to.
    Named,
    /// Too recent to be a leftover launch.
    TooYoung,
    /// The file could not be read; assume it matters.
    Unreadable,
}

pub struct Candidate {
    pub path: PathBuf,
    pub id: String,
    pub size: u64,
    pub age_hours: f64,
    pub cwd: Option<String>,
}

pub struct Report {
    pub scanned: usize,
    pub empty: Vec<Candidate>,
    pub kept: Vec<(String, Keep)>,
    pub bytes_freed: u64,
    pub deleted: usize,
}

/// Inspect one session file: `Ok(gate)` where the gate says whether it survived
/// and why, plus the display facts the report needs.
fn inspect(path: &Path, options: &Options, now: SystemTime) -> Result<Inspect, String> {
    let meta = fs::metadata(path).map_err(|e| format!("{e}"))?;
    let size = meta.len();
    let age = meta
        .modified()
        .ok()
        .and_then(|m| now.duration_since(m).ok())
        .unwrap_or_default();
    let age_hours = age.as_secs_f64() / 3600.0;

    let content = fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let mut has_payload = false;
    let mut named = false;
    let mut id = String::new();
    let mut cwd = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // A malformed line means this is not the bootstrap shape we understand;
        // treat the whole file as suspect rather than risk deleting it.
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
            return Ok(Inspect {
                keep: Some(Keep::Unreadable),
                size,
                age_hours,
                id,
                cwd,
            });
        };
        match record.get("type").and_then(serde_json::Value::as_str) {
            Some("session") => {
                if let Some(value) = record.get("id").and_then(serde_json::Value::as_str) {
                    id = value.to_string();
                }
                cwd = record
                    .get("cwd")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
            Some("session_info") => named = true,
            Some(kind) if BOOTSTRAP_TYPES.contains(&kind) => {}
            // Any other record type is conversational payload or bookkeeping we
            // do not recognise. Either way the file is not a bare bootstrap.
            Some(_) | None => has_payload = true,
        }
    }

    let keep = if has_payload {
        Some(Keep::HasPayload)
    } else if named && !options.include_named {
        Some(Keep::Named)
    } else if age < Duration::from_secs(options.min_age_hours.saturating_mul(3600)) {
        Some(Keep::TooYoung)
    } else {
        None
    };

    Ok(Inspect {
        keep,
        size,
        age_hours,
        id,
        cwd,
    })
}

struct Inspect {
    keep: Option<Keep>,
    size: u64,
    age_hours: f64,
    id: String,
    cwd: Option<String>,
}

/// Walk the session directory and build the plan. Deletion happens only when
/// `dry_run` is false.
pub fn plan(dir: &Path, options: &Options) -> Report {
    let now = SystemTime::now();
    let mut report = Report {
        scanned: 0,
        empty: Vec::new(),
        kept: Vec::new(),
        bytes_freed: 0,
        deleted: 0,
    };

    let Ok(entries) = fs::read_dir(dir) else {
        return report;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    paths.sort();

    for path in paths {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        report.scanned += 1;
        match inspect(&path, options, now) {
            Ok(facts) => {
                if let Some(reason) = facts.keep {
                    report.kept.push((stem, reason));
                    continue;
                }
                let id = if facts.id.is_empty() {
                    stem.clone()
                } else {
                    facts.id.clone()
                };
                let candidate = Candidate {
                    path: path.clone(),
                    id,
                    size: facts.size,
                    age_hours: facts.age_hours,
                    cwd: facts.cwd,
                };
                if options.dry_run {
                    report.bytes_freed += candidate.size;
                    report.empty.push(candidate);
                } else {
                    match fs::remove_file(&path) {
                        Ok(()) => {
                            report.deleted += 1;
                            report.bytes_freed += candidate.size;
                            report.empty.push(candidate);
                        }
                        // A file that cannot be removed is reported as kept: it
                        // is still on disk, so pretending it went is a lie.
                        Err(_) => report.kept.push((stem, Keep::Unreadable)),
                    }
                }
            }
            Err(_) => report.kept.push((stem, Keep::Unreadable)),
        }
    }

    report
}

fn keep_reason(reason: &Keep) -> &'static str {
    match reason {
        Keep::HasPayload => "has messages",
        Keep::Named => "named",
        Keep::TooYoung => "too new",
        Keep::Unreadable => "unreadable",
    }
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn print_report(report: &Report, options: &Options) {
    let verb = if options.dry_run {
        "would delete"
    } else {
        "deleted"
    };
    for candidate in &report.empty {
        let cwd = candidate.cwd.as_deref().unwrap_or("?");
        println!(
            "  {verb} {:>8}  {:>7.1}h  {}  {cwd}",
            human_bytes(candidate.size),
            candidate.age_hours,
            candidate.id
        );
    }
    if !report.empty.is_empty() {
        println!();
    }
    println!(
        "scanned {} session(s); {verb} {} ({})",
        report.scanned,
        report.empty.len(),
        human_bytes(report.bytes_freed)
    );
    if !report.kept.is_empty() {
        let mut counts: Vec<(&'static str, usize)> = Vec::new();
        for (_, reason) in &report.kept {
            let label = keep_reason(reason);
            match counts.iter_mut().find(|(name, _)| *name == label) {
                Some(slot) => slot.1 += 1,
                None => counts.push((label, 1)),
            }
        }
        counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let summary: Vec<String> = counts
            .into_iter()
            .map(|(label, n)| format!("{n} {label}"))
            .collect();
        println!("kept {}: {}", report.kept.len(), summary.join(", "));
    }
    if options.dry_run && !report.empty.is_empty() {
        println!();
        println!("Nothing was removed. Re-run without --dry-run to delete them.");
    }
}

fn print_json(report: &Report, options: &Options) {
    let payload = serde_json::json!({
        "dryRun": options.dry_run,
        "scanned": report.scanned,
        "deleted": report.empty.len(),
        "bytesFreed": report.bytes_freed,
        "kept": report.kept.len(),
        "sessions": report.empty.iter().map(|c| serde_json::json!({
            "id": c.id,
            "path": c.path.to_string_lossy(),
            "bytes": c.size,
            "ageHours": (c.age_hours * 10.0).round() / 10.0,
            "cwd": c.cwd,
        })).collect::<Vec<_>>(),
    });
    match serde_json::to_string_pretty(&payload) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("Error: {error}"),
    }
}

/// `prime-agent session clean [--dry-run] [--include-named] [--min-age-hours N] [--json]`
pub fn run(args: &[String], agent_dir: &Path) -> Result<i32, String> {
    let options = parse_options(args)?;
    let dir = agent_dir.join("sessions");
    if !dir.is_dir() {
        return Err(format!("No session directory at {}", dir.display()));
    }
    let report = plan(&dir, &options);
    if options.json {
        print_json(&report, &options);
    } else {
        print_report(&report, &options);
    }
    Ok(0)
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dry-run" => options.dry_run = true,
            "--include-named" => options.include_named = true,
            "--json" => options.json = true,
            "--min-age-hours" => {
                let Some(value) = rest.next() else {
                    return Err("--min-age-hours requires a number".to_string());
                };
                options.min_age_hours = value
                    .parse()
                    .map_err(|_| format!("--min-age-hours: {value:?} is not a number"))?;
            }
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
        let path = dir.join(name);
        let mut body = String::new();
        for line in lines {
            body.push_str(line);
            body.push('\n');
        }
        fs::write(&path, body).expect("write session");
        path
    }

    const SESSION: &str = r#"{"type":"session","version":3,"id":"s1","timestamp":"2026-01-01T00:00:00.000Z","cwd":"/w"}"#;
    const MODEL: &str = r#"{"type":"model_change","id":"m","parentId":null,"timestamp":"2026-01-01T00:00:00.000Z","provider":"p","modelId":"x"}"#;

    /// The bug this command exists for: a launch that never got a prompt leaves
    /// a bootstrap-only file. Age is forced past the guard by choosing a
    /// min-age of zero.
    #[test]
    fn deletes_a_bootstrap_only_session() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "01a-empty.jsonl", &[SESSION, MODEL]);
        let options = Options {
            min_age_hours: 0,
            ..Options::default()
        };
        let report = plan(dir.path(), &options);
        assert_eq!(report.empty.len(), 1);
        assert_eq!(report.scanned, 1);
        assert!(!dir.path().join("01a-empty.jsonl").exists());
    }

    /// A one-line conversation is small but real: a byte threshold would have
    /// swept it, the record-type rule does not.
    #[test]
    fn keeps_a_tiny_session_that_has_a_message() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(
            dir.path(),
            "01b-real.jsonl",
            &[
                SESSION,
                MODEL,
                r#"{"type":"message","id":"e1","parentId":null,"timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user","content":"hi"}}"#,
            ],
        );
        let options = Options {
            min_age_hours: 0,
            ..Options::default()
        };
        let report = plan(dir.path(), &options);
        assert!(report.empty.is_empty(), "a message makes it real");
        assert!(dir.path().join("01b-real.jsonl").exists());
    }

    /// A bare slash command is a `custom_message`, not a `message`; it still
    /// counts as payload.
    #[test]
    fn keeps_a_session_whose_only_content_is_a_custom_message() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(
            dir.path(),
            "01c-slash.jsonl",
            &[
                SESSION,
                MODEL,
                r#"{"type":"custom_message","customType":"session_slash_command","content":"/compact","display":true,"id":"c1","parentId":"m","timestamp":"2026-01-01T00:00:02.000Z"}"#,
            ],
        );
        let options = Options {
            min_age_hours: 0,
            ..Options::default()
        };
        let report = plan(dir.path(), &options);
        assert!(report.empty.is_empty());
    }

    /// A named session carries `session_info`; `/resume` can navigate to it, so
    /// it stays unless the caller opts in.
    #[test]
    fn keeps_a_named_session_unless_asked() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(
            dir.path(),
            "01d-named.jsonl",
            &[
                SESSION,
                r#"{"type":"session_info","id":"i","parentId":null,"timestamp":"2026-01-01T00:00:00.000Z","name":"twin"}"#,
            ],
        );
        let keep = Options {
            min_age_hours: 0,
            ..Options::default()
        };
        assert!(plan(dir.path(), &keep).empty.is_empty());

        let sweep = Options {
            min_age_hours: 0,
            include_named: true,
            ..Options::default()
        };
        assert_eq!(plan(dir.path(), &sweep).empty.len(), 1);
    }

    /// A fresh file may be a session waiting for its first prompt; the default
    /// one-hour floor keeps it.
    #[test]
    fn keeps_a_session_younger_than_the_floor() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "01e-new.jsonl", &[SESSION, MODEL]);
        let report = plan(dir.path(), &Options::default());
        assert!(report.empty.is_empty(), "default min age is 1h");
        assert_eq!(report.kept.len(), 1);
    }

    /// `--dry-run` reports without touching the disk.
    #[test]
    fn dry_run_reports_the_plan_and_keeps_the_files() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "01f-empty.jsonl", &[SESSION, MODEL]);
        let options = Options {
            dry_run: true,
            min_age_hours: 0,
            ..Options::default()
        };
        let report = plan(dir.path(), &options);
        assert_eq!(report.empty.len(), 1);
        assert_eq!(report.deleted, 0, "dry run must not count deletions");
        assert!(dir.path().join("01f-empty.jsonl").exists());
    }

    /// An unparsable line means the file is not the shape we understand; it is
    /// kept rather than deleted.
    #[test]
    fn keeps_a_file_with_a_malformed_line() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "01g-broken.jsonl", &[SESSION, "{not json"]);
        let options = Options {
            min_age_hours: 0,
            ..Options::default()
        };
        let report = plan(dir.path(), &options);
        assert!(report.empty.is_empty());
    }

    #[test]
    fn parses_the_option_set() {
        let options = parse_options(&[
            "--dry-run".to_string(),
            "--include-named".to_string(),
            "--min-age-hours".to_string(),
            "48".to_string(),
            "--json".to_string(),
        ])
        .expect("valid options");
        assert!(options.dry_run);
        assert!(options.include_named);
        assert!(options.json);
        assert_eq!(options.min_age_hours, 48);

        assert!(parse_options(&["--nope".to_string()]).is_err());
        assert!(parse_options(&["--min-age-hours".to_string()]).is_err());
        assert!(parse_options(&["--min-age-hours".to_string(), "x".to_string()]).is_err());
    }
}
