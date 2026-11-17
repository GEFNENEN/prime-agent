//! Durable per-session state: the prompt cache (one reflection + the page
//! roster) and the write-back cursor. Files live under
//! `<agent dir>/hindsight/sessions/<sanitized session id>/` so a worker
//! restart resumes the same cache instead of reflecting again.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// One knowledge page title, cached for the roster refresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PageTitle {
    pub id: String,
    pub name: String,
}

/// The JS `sessionCacheFile` shape: turn counter, cached reflection (a
/// present-but-empty answer still means "already ran"), and the roster.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PromptCache {
    pub turns: u64,
    pub reflect_answer: Option<String>,
    pub pages: Option<PageCache>,
    pub bank: Option<String>,
}

/// The roster snapshot and the turn it was fetched at.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PageCache {
    pub at_turn: u64,
    pub list: Vec<PageTitle>,
}

/// The write-back cursor: how many turns of the transcript are already in
/// the bank, the prefix fingerprint they had, and which bank holds them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct RetainCursor {
    pub turns: u64,
    pub fingerprint: String,
    pub bank: String,
}

/// The per-session state directory. Session ids are UUIDs, but the
/// sanitizer keeps other shapes (tests, resumed ids) from escaping the
/// state root.
pub(crate) fn session_dir(root: &Path, session_id: &str) -> PathBuf {
    let mut name = String::new();
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    let name = name.trim_matches('.').to_string();
    let name = if name.is_empty() {
        "no-session".to_string()
    } else {
        name.chars().take(120).collect()
    };
    root.join("sessions").join(name)
}

/// Read one JSON state file; any failure is an absent cache.
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write one JSON state file atomically (temp beside it, then rename).
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let file_name = path.file_name().map_or_else(
        || "state.json".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let temp = path.with_file_name(format!("{file_name}.{}.tmp", std::process::id()));
    std::fs::write(&temp, text)?;
    crate::platform::rename_onto(&temp, path)
}
