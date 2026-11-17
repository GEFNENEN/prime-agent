//! The `stop` hook: retain the run's new turns into the session document.
//!
//! The harness hands over a dcode-format adapter transcript that grows one
//! run at a time. A per-session cursor records the turn count and a prefix
//! fingerprint, so a normal run appends only the new turns (with an
//! idempotent `operation_id`) and a rewritten transcript falls back to a
//! full replace.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::bank;
use super::client::{Client, RetainFailure, RetainItem, RetainRequest};
use super::config::Config;
use super::state::{self, RetainCursor};
use super::truncate_chars;

/// Bound one retain request (the JS `REQUEST_BUDGET_MS`).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// Read at most this much of a long transcript (the JS tail window).
const MAX_TRANSCRIPT_BYTES: u64 = 10 * 1024 * 1024;
/// The JS uuid-v5 namespace for operation ids.
const OPERATION_NAMESPACE: &str = "e75686a4-e923-4326-a0d1-358f1c6c3eb4";
/// The injected-memory tags removed before retaining.
const MEMORY_TAG_PATTERN: &str = r"<(hook_prompt|task-notification|system-reminder|hindsight_memory|hindsight_memories|hindsight_bank|relevant_memories|user_feedback|hindsight_knowledge|hindsight_knowledge_refresh)\b[\s\S]*?</\1>";
/// Tool-input keys the adapter renders as an action's target, in order.
const TARGET_KEYS: [&str; 9] = [
    "file_path",
    "path",
    "notebook_path",
    "command",
    "pattern",
    "query",
    "url",
    "name",
    "id",
];

/// The stop hook's stdin payload.
#[derive(Debug, Deserialize)]
pub(crate) struct StopInput {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub transcript_path: String,
    #[serde(default)]
    pub cwd: String,
}

/// One retained turn (the document's JSONL row).
#[derive(Debug, Clone, Serialize)]
struct Turn {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

/// One adapter transcript line.
#[derive(Debug, Deserialize)]
struct Record {
    schema_version: Option<u32>,
    role: Option<String>,
    content: Option<serde_json::Value>,
    name: Option<String>,
    timestamp: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum Plan {
    Skip,
    Replace,
    Append { from: usize },
}

struct Payload {
    mode: &'static str,
    content: String,
}

/// Run one write-back. Every failure is logged and swallowed: the hook must
/// never fail a finished run.
pub(crate) async fn run(config: &Config, state_root: &Path, input: StopInput) {
    let transcript_path = input.transcript_path.trim();
    if transcript_path.is_empty() {
        return;
    }
    let cwd = if input.cwd.trim().is_empty() {
        std::env::current_dir().unwrap_or_default()
    } else {
        std::path::PathBuf::from(input.cwd.trim())
    };
    let Some(bank) = bank::resolve(config, &cwd).await else {
        return;
    };
    let (config, bank) = config.for_bank(&bank);
    if config.disabled || !config.retain_sessions {
        return;
    }
    let Some(text) = read_transcript(Path::new(transcript_path)) else {
        return;
    };
    let turns = parse_turns(&text);
    if turns.is_empty() {
        return;
    }
    let session_id = if input.session_id.trim().is_empty() {
        "no-session"
    } else {
        input.session_id.trim()
    };
    let dir = state::session_dir(state_root, session_id);
    let cursor_path = dir.join("retain.json");
    let cursor: Option<RetainCursor> = state::read_json(&cursor_path);
    let plan = plan_turns(&turns, cursor.as_ref(), &bank);
    if matches!(plan, Plan::Skip) {
        return;
    }
    let ref_id = format!("conversation:{session_id}");
    let start_ts = turns.first().and_then(|turn| turn.timestamp.clone());
    let Some(payload) = build_payload(&turns, plan, &ref_id, start_ts.as_deref()) else {
        return;
    };
    let client = Client::new(&config, &bank);
    let first = submit(&client, &config, &bank, &ref_id, &payload).await;
    let sent = match first {
        Ok(()) => true,
        Err(failure) if payload.mode == "append" && append_mismatch(failure.status) => {
            let replace = build_payload(&turns, Plan::Replace, &ref_id, start_ts.as_deref());
            match replace {
                Some(replace) => match submit(&client, &config, &bank, &ref_id, &replace).await {
                    Ok(()) => true,
                    Err(failure) => {
                        tracing::debug!(%failure.message, "hindsight retain replace failed");
                        false
                    }
                },
                None => false,
            }
        }
        Err(failure) => {
            tracing::debug!(%failure.message, "hindsight retain failed");
            false
        }
    };
    if !sent {
        return;
    }
    let next = RetainCursor {
        turns: u64::try_from(turns.len()).unwrap_or(u64::MAX),
        fingerprint: fingerprint(&turns),
        bank,
    };
    if let Err(error) = state::write_json(&cursor_path, &next) {
        tracing::debug!(%error, "hindsight retain cursor write failed");
    }
}

async fn submit(
    client: &Client,
    config: &Config,
    bank: &str,
    ref_id: &str,
    payload: &Payload,
) -> Result<(), RetainFailure> {
    let operation = format!("{bank}\n{ref_id}\n{}\n{}", payload.mode, payload.content);
    let namespace = Uuid::parse_str(OPERATION_NAMESPACE).map_err(|error| RetainFailure {
        status: None,
        message: error.to_string(),
    })?;
    let item = RetainItem {
        content: payload.content.clone(),
        context: "coding agent session".to_string(),
        document_id: ref_id.to_string(),
        tags: vec![
            "source:chat".to_string(),
            format!("harness:{}", config.harness),
        ],
        strategy: "conversation".to_string(),
        observation_scopes: "shared".to_string(),
        timestamp: None,
        update_mode: (payload.mode == "append").then(|| "append".to_string()),
        metadata: serde_json::json!({
            "source": "chat",
            "session_id": ref_id.trim_start_matches("conversation:"),
            "ref_id": ref_id,
            "harness": config.harness,
        }),
    };
    let body = RetainRequest {
        items: vec![item],
        asynchronous: true,
        operation_id: Some(Uuid::new_v5(&namespace, operation.as_bytes()).to_string()),
    };
    client.retain(&body, REQUEST_TIMEOUT).await
}

/// A rejected `update_mode` means the server predates append; retry as a
/// full replace. Rate limits and auth failures stay retryable later.
fn append_mismatch(status: Option<u16>) -> bool {
    matches!(status, Some(400 | 404 | 405 | 409 | 422))
}

fn plan_turns(turns: &[Turn], cursor: Option<&RetainCursor>, bank: &str) -> Plan {
    let Some(cursor) = cursor else {
        return Plan::Replace;
    };
    if cursor.bank != bank {
        return Plan::Replace;
    }
    let Ok(sent) = usize::try_from(cursor.turns) else {
        return Plan::Replace;
    };
    if sent > turns.len() || fingerprint(&turns[..sent]) != cursor.fingerprint {
        return Plan::Replace;
    }
    if sent == turns.len() {
        Plan::Skip
    } else {
        Plan::Append { from: sent }
    }
}

fn build_payload(
    turns: &[Turn],
    plan: Plan,
    ref_id: &str,
    start_ts: Option<&str>,
) -> Option<Payload> {
    match plan {
        Plan::Skip => None,
        Plan::Replace => {
            let mut lines = Vec::with_capacity(turns.len() + 1);
            lines.push(Turn {
                role: "system".to_string(),
                content: format!("REF-ID: {ref_id}"),
                timestamp: start_ts.map(ToString::to_string),
            });
            lines.extend(turns.iter().cloned());
            Some(Payload {
                mode: "replace",
                content: render_lines(&lines),
            })
        }
        Plan::Append { from } => Some(Payload {
            mode: "append",
            content: render_lines(&turns[from..]),
        }),
    }
}

fn render_lines(turns: &[Turn]) -> String {
    turns
        .iter()
        .filter_map(|turn| serde_json::to_string(turn).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

fn fingerprint(turns: &[Turn]) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(turns.len().to_string().as_bytes());
    for turn in turns {
        hasher.update(b"\n");
        if let Ok(text) = serde_json::to_string(turn) {
            hasher.update(text.as_bytes());
        }
    }
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn parse_turns(text: &str) -> Vec<Turn> {
    let mut turns = Vec::new();
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<Record>(line) else {
            continue;
        };
        if record.schema_version != Some(1) {
            continue;
        }
        let timestamp = record
            .timestamp
            .clone()
            .filter(|value| !value.trim().is_empty());
        match record.role.as_deref() {
            Some("user" | "assistant") => {
                let role = record.role.clone().unwrap_or_default();
                let content = strip_injected(&content_text(record.content.as_ref()));
                let content = content.trim().to_string();
                if !content.is_empty() {
                    turns.push(Turn {
                        role,
                        content,
                        timestamp,
                    });
                }
            }
            Some("tool") => {
                let name = record.name.clone().unwrap_or_default();
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                turns.push(Turn {
                    role: "action".to_string(),
                    content: action_line(name, record.content.as_ref()),
                    timestamp,
                });
            }
            _ => {}
        }
    }
    turns
}

fn content_text(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                serde_json::Value::String(text) => Some(text.as_str()),
                serde_json::Value::Object(object) => {
                    object.get("text").and_then(serde_json::Value::as_str)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn action_line(tool: &str, content: Option<&serde_json::Value>) -> String {
    let target = match content {
        Some(serde_json::Value::Object(object)) => TARGET_KEYS
            .iter()
            .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
            .unwrap_or_default(),
        Some(serde_json::Value::String(text)) => text.as_str(),
        _ => "",
    };
    let target = target.trim().lines().next().unwrap_or_default().trim();
    if target.is_empty() {
        return tool.to_string();
    }
    format!("{tool} {}", truncate_chars(target, 100))
}

fn strip_injected(text: &str) -> String {
    static PATTERN: std::sync::OnceLock<Option<fancy_regex::Regex>> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| fancy_regex::Regex::new(MEMORY_TAG_PATTERN).ok());
    match pattern {
        Some(pattern) => pattern.replace_all(text, "").into_owned(),
        None => text.to_string(),
    }
}

fn read_transcript(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let length = std::fs::metadata(path).ok()?.len();
    let mut file = std::fs::File::open(path).ok()?;
    if length <= MAX_TRANSCRIPT_BYTES {
        let mut text = String::new();
        file.read_to_string(&mut text).ok()?;
        return Some(text);
    }
    file.seek(SeekFrom::Start(length - MAX_TRANSCRIPT_BYTES))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let tail = String::from_utf8_lossy(&bytes);
    Some(
        tail.split_once('\n')
            .map_or_else(String::new, |(_, rest)| rest.to_string()),
    )
}
