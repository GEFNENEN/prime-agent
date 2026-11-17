//! The Hindsight bridge behind the hidden `prime-agent hindsight-hook
//! prompt|stop` entry: the Rust replacement for the JavaScript
//! `before_agent_start` / `agent_end` extensions after the Rust build
//! stopped loading JS extensions.
//!
//! The two commands speak the same stdin/stdout contracts as the existing
//! `turnHook` / `stopHook` shell settings, so no session-engine wiring
//! changes are needed: point those settings at this binary and the hooks
//! behave exactly like any other command.
//!
//! Fail-open by construction: every error path returns no injection (or no
//! write-back), never an error that could fail a turn or a run.

mod bank;
mod client;
mod config;
mod prompt;
mod retain;
mod state;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

/// Which hook entry the hidden command ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    /// The per-turn prompt hook (`turnHook`).
    Prompt,
    /// The per-run write-back hook (`stopHook`).
    Stop,
}

/// Run one hook from its stdin payload and return the stdout text the host
/// must print (the prompt hook's JSON) or `None` for a quiet stop hook.
///
/// The function is deliberately silent on failure: a missing server, a
/// malformed payload, or a timed-out request must leave the turn untouched.
pub async fn handle(kind: HookKind, stdin: &str) -> Option<String> {
    let config = config::Config::load();
    if config.disabled {
        return None;
    }
    let state_root = state_root();
    match kind {
        HookKind::Prompt => {
            let input: prompt::PromptInput = serde_json::from_str(stdin).ok()?;
            let context = prompt::run(&config, &state_root, input).await?;
            Some(
                serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": context,
                    }
                })
                .to_string(),
            )
        }
        HookKind::Stop => {
            let input: retain::StopInput = serde_json::from_str(stdin).ok()?;
            retain::run(&config, &state_root, input).await;
            None
        }
    }
}

/// Per-session state under the agent directory; the temp fallback keeps a
/// session cache when no agent directory resolves (the JS hook's tmpdir
/// behavior).
fn state_root() -> PathBuf {
    pa_types::platform::dirs::agent_dir().map_or_else(
        || std::env::temp_dir().join("prime-agent-hindsight"),
        |dir| dir.join("hindsight"),
    )
}

/// Truncate on a character boundary, marking the cut with an ellipsis.
pub(crate) fn truncate_chars(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}
