//! The `stopHook` settings surface: a user-configured shell command the
//! session runs once per finished run (`agent_end`) with a
//! `{session_id, transcript_path, cwd}` JSON payload on stdin — the
//! write-back half of the TS extension parity (`pi.on("agent_end")` ->
//! `core.onTranscript`).
//!
//! The run's new messages are rendered into a dcode-format adapter
//! transcript (`<agent_dir>/stop-hook/<session_id>.jsonl`: one
//! `schema_version: 1` record per line) that console-side stop hooks
//! read through their `transcript_path` contract; the command's stdout
//! is ignored.
//!
//! The listener never blocks the loop: the write-back rides its own
//! detached task, and overlapping runs serialize on a per-hook flight
//! mutex (the adapter file is append-only). Fail-open by contract: a
//! missing command, spawn failure, timeout, or a bad transcript write
//! is logged and never fails the turn.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use pa_agent::types::{AgentMessage, AssistantContent, Message, ToolCall, UserContent, UserPart};
use tokio::io::AsyncWriteExt;

use crate::settings::StopHookSettings;

/// Wall-clock backstop when `stopHook.timeoutMs` is unset: the console
/// stop hooks carry a 60s host budget of their own and bound their retry
/// loop under it, so 70s only reaps a wedged child.
pub const DEFAULT_STOP_HOOK_TIMEOUT_MS: u64 = 70_000;

/// One configured agent-end write-back hook.
pub struct StopHook {
    command: String,
    timeout_ms: u64,
    transcripts_dir: PathBuf,
    /// Serializes overlapping write-backs: the adapter file is
    /// append-only, so one run's rows must land before the next's.
    flight: tokio::sync::Mutex<()>,
}

impl StopHook {
    /// Build the hook from the `stopHook` settings; `None` (or a blank
    /// command) keeps the feature off.
    #[must_use]
    pub fn from_settings(settings: Option<&StopHookSettings>, agent_dir: &Path) -> Option<StopHook> {
        let settings = settings?;
        let command = settings.command.as_ref()?.trim();
        if command.is_empty() {
            return None;
        }
        Some(StopHook {
            command: command.to_string(),
            timeout_ms: settings
                .timeout_ms
                .unwrap_or(DEFAULT_STOP_HOOK_TIMEOUT_MS),
            transcripts_dir: agent_dir.join("stop-hook"),
            flight: tokio::sync::Mutex::new(()),
        })
    }

    /// Run the hook for one finished run on its own task: the loop's
    /// final `agent_end` emission never waits on it.
    pub fn handle_agent_end(
        self: &Arc<Self>,
        session: Arc<tokio::sync::Mutex<crate::session::manager::SessionManager>>,
        messages: Vec<AgentMessage>,
    ) {
        let hook = Arc::clone(self);
        tokio::spawn(async move {
            let (session_id, cwd) = {
                let session = session.lock().await;
                (
                    session.get_session_id().to_string(),
                    session.get_cwd().display().to_string(),
                )
            };
            hook.run_with(&session_id, &cwd, &messages).await;
        });
    }

    /// Append the run's adapter rows and run the command. A run that
    /// rendered no rows (custom-only messages) skips both: the console
    /// hook would find no turns in the transcript anyway.
    pub(crate) async fn run_with(&self, session_id: &str, cwd: &str, messages: &[AgentMessage]) {
        let _flight = self.flight.lock().await;
        let lines = adapter_lines(messages);
        if lines.is_empty() {
            return;
        }
        let transcript_path = self.transcripts_dir.join(format!("{session_id}.jsonl"));
        let path = transcript_path.clone();
        match tokio::task::spawn_blocking(move || append_lines(&path, &lines)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::debug!(%error, "stop hook transcript append failed");
                return;
            }
            Err(error) => {
                tracing::debug!(%error, "stop hook transcript task failed");
                return;
            }
        }
        let payload = serde_json::json!({
            "session_id": session_id,
            "transcript_path": transcript_path.display().to_string(),
            "cwd": cwd,
        });
        run_command(&self.command, self.timeout_ms, &payload).await;
    }
}

/// Append one adapter line per rendered message to the transcript file.
fn append_lines(path: &Path, lines: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for line in lines {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

/// Render the run's messages into dcode-format transcript rows: user and
/// assistant text becomes `role: user|assistant`, every tool call
/// becomes an `role: tool` action row (`content` carries the call's
/// arguments, `name` the tool), and everything else (tool results,
/// custom rows, thinking, images) drops.
fn adapter_lines(messages: &[AgentMessage]) -> Vec<String> {
    let mut lines = Vec::new();
    for message in messages {
        let AgentMessage::Standard(message) = message else {
            continue;
        };
        match message {
            Message::User(user) => {
                let text = user_text(&user.content);
                if !text.is_empty() {
                    lines.push(record_line("user", &text, &timestamp_of(message)));
                }
            }
            Message::Assistant(assistant) => {
                let timestamp = timestamp_of(message);
                let text = assistant_text(&assistant.content);
                if !text.is_empty() {
                    lines.push(record_line("assistant", &text, &timestamp));
                }
                for call in assistant.tool_calls() {
                    lines.push(tool_line(call, &timestamp));
                }
            }
            // Action rows come from the calls; results would duplicate them.
            Message::ToolResult(_) => {}
        }
    }
    lines
}

fn record_line(role: &str, content: &str, timestamp: &str) -> String {
    serde_json::json!({
        "schema_version": 1,
        "role": role,
        "content": content,
        "timestamp": timestamp,
    })
    .to_string()
}

fn tool_line(call: &ToolCall, timestamp: &str) -> String {
    serde_json::json!({
        "schema_version": 1,
        "role": "tool",
        "name": call.name.clone(),
        "content": call.arguments.clone(),
        "timestamp": timestamp,
    })
    .to_string()
}

/// The message's ISO-8601 stamp; a missing one (the `Custom` role reads
/// as 0) falls back to now, like the console transcripts' fallback.
fn timestamp_of(message: &Message) -> String {
    let millis = match message {
        Message::User(user) => user.timestamp,
        Message::Assistant(assistant) => assistant.timestamp,
        Message::ToolResult(result) => result.timestamp,
    };
    if millis <= 0 {
        crate::session::manager::format_iso_now()
    } else {
        crate::session::manager::format_iso(millis)
    }
}

fn user_text(content: &UserContent) -> String {
    match content {
        UserContent::Text(text) => text.trim().to_string(),
        UserContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                UserPart::Text(text) => Some(text.text.as_str()),
                UserPart::Image(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
    }
}

fn assistant_text(content: &[AssistantContent]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            AssistantContent::Text(text) => Some(text.text.as_str()),
            AssistantContent::Thinking(_) | AssistantContent::ToolCall(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Spawn the command with the payload on stdin, bounded by the
/// configured timeout; every failure path is logged and ignored.
async fn run_command(command: &str, timeout_ms: u64, payload: &serde_json::Value) {
    let mut process = super::turn_hook::hidden_shell_command(command);
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::debug!(%error, command, "stop hook spawn failed");
            return;
        }
    };
    {
        let Some(mut stdin) = child.stdin.take() else {
            return;
        };
        if let Err(error) = stdin.write_all(payload.to_string().as_bytes()).await {
            tracing::debug!(%error, "stop hook stdin write failed");
            return;
        }
        // Dropping the handle closes the pipe so the hook sees EOF.
    }
    match tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait()).await {
        Ok(Ok(status)) if status.success() => {}
        Ok(Ok(status)) => {
            tracing::debug!(code = status.code(), "stop hook exited non-zero");
        }
        Ok(Err(error)) => {
            tracing::debug!(%error, "stop hook wait failed");
        }
        Err(_elapsed) => {
            // The dropped future reaps the child (`kill_on_drop`).
            tracing::debug!(timeout_ms, "stop hook timed out");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hook command line runs through the platform shell (bash on Unix
    /// and whenever Windows resolves Git Bash; `cmd.exe` otherwise), so the
    /// test commands must speak that shell's quoting.
    #[cfg(windows)]
    fn shell_is_bash() -> bool {
        crate::platform::get_shell_config(None).map_or(false, |config| {
            config.shell.to_ascii_lowercase().contains("bash")
        })
    }

    /// A command that copies stdin into `capture`.
    fn capture_command(capture: &Path) -> String {
        #[cfg(windows)]
        if !shell_is_bash() {
            return format!("more > \"{}\"", capture.display());
        }
        format!("cat > '{}'", capture.display().to_string().replace('\\', "/"))
    }

    fn hook(command: &str, dir: &Path) -> StopHook {
        StopHook::from_settings(
            Some(&StopHookSettings {
                command: Some(command.to_string()),
                timeout_ms: None,
            }),
            dir,
        )
        .expect("a non-blank command builds the hook")
    }

    fn user_message(text: &str) -> AgentMessage {
        serde_json::from_value(serde_json::json!({
            "role": "user",
            "content": text,
            "timestamp": 1_704_067_200_012_i64,
        }))
        .unwrap()
    }

    fn assistant_message() -> AgentMessage {
        serde_json::from_value(serde_json::json!({
            "role": "assistant",
            "content": [
                { "type": "text", "text": "on it" },
                { "type": "toolCall", "id": "c1", "name": "Read",
                  "arguments": { "file_path": "a.txt" } },
            ],
            "usage": {},
            "stopReason": "toolUse",
            "timestamp": 1_704_067_200_099_i64,
        }))
        .unwrap()
    }

    fn tool_result_message() -> AgentMessage {
        serde_json::from_value(serde_json::json!({
            "role": "toolResult",
            "toolCallId": "c1",
            "toolName": "Read",
            "content": [{ "type": "text", "text": "file body" }],
            "isError": false,
            "timestamp": 1_704_067_200_150_i64,
        }))
        .unwrap()
    }

    #[test]
    fn blank_commands_and_absent_settings_disable_the_hook() {
        let dir = tempfile::tempdir().unwrap();
        assert!(StopHook::from_settings(None, dir.path()).is_none());
        assert!(StopHook::from_settings(Some(&StopHookSettings::default()), dir.path()).is_none());
        assert!(
            StopHook::from_settings(
                Some(&StopHookSettings {
                    command: Some("   ".to_string()),
                    timeout_ms: None,
                }),
                dir.path(),
            )
            .is_none()
        );
    }

    #[test]
    fn adapter_lines_render_user_assistant_and_tool_rows() {
        let messages = vec![
            user_message("hi there"),
            assistant_message(),
            tool_result_message(),
        ];
        let lines = adapter_lines(&messages);
        assert_eq!(lines.len(), 3, "results and customs render nothing: {lines:?}");

        let rows: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows[0]["schema_version"], 1);
        assert_eq!(rows[0]["role"], "user");
        assert_eq!(rows[0]["content"], "hi there");
        assert_eq!(rows[0]["timestamp"], "2024-01-01T00:00:00.012Z");
        assert_eq!(rows[1]["role"], "assistant");
        assert_eq!(rows[1]["content"], "on it");
        assert_eq!(rows[2]["role"], "tool");
        assert_eq!(rows[2]["name"], "Read");
        assert_eq!(rows[2]["content"]["file_path"], "a.txt");
    }

    #[test]
    fn a_custom_only_run_renders_no_rows() {
        let custom: AgentMessage = serde_json::from_value(serde_json::json!({
            "role": "custom-role",
            "kind": "notice",
        }))
        .unwrap();
        assert!(adapter_lines(&[custom]).is_empty());
    }

    #[tokio::test]
    async fn running_the_hook_appends_the_transcript_and_feeds_the_payload() {
        let dir = tempfile::tempdir().unwrap();
        let capture = dir.path().join("payload.json");
        let hook = hook(&capture_command(&capture), dir.path());
        let messages = vec![user_message("hi there"), assistant_message()];

        hook.run_with("sess-1", "/tmp/work", &messages).await;

        let transcript = dir.path().join("stop-hook").join("sess-1.jsonl");
        let body = std::fs::read_to_string(&transcript).expect("adapter transcript exists");
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("\"content\":\"hi there\""));

        let payload =
            std::fs::read_to_string(&capture).expect("the command captured its stdin payload");
        assert!(
            payload.contains("\"session_id\":\"sess-1\""),
            "payload: {payload}"
        );
        assert!(payload.contains("\"cwd\":\"/tmp/work\""), "payload: {payload}");
        assert!(
            payload.contains(&transcript.display().to_string().replace('\\', "\\\\")),
            "payload: {payload}"
        );
    }

    #[tokio::test]
    async fn a_failing_command_never_fails_the_write_back() {
        let dir = tempfile::tempdir().unwrap();
        let hook = hook("exit 3", dir.path());
        hook.run_with("sess-2", "/tmp/work", &[user_message("boom")])
            .await;
        let transcript = dir.path().join("stop-hook").join("sess-2.jsonl");
        assert!(transcript.exists(), "rows land before the command runs");
    }

    #[tokio::test]
    async fn a_custom_only_run_skips_the_command() {
        let dir = tempfile::tempdir().unwrap();
        let capture = dir.path().join("payload.json");
        let hook = hook(&capture_command(&capture), dir.path());
        let custom: AgentMessage = serde_json::from_value(serde_json::json!({
            "role": "custom-role",
            "kind": "notice",
        }))
        .unwrap();
        hook.run_with("sess-3", "/tmp/work", &[custom]).await;
        assert!(
            !dir.path().join("stop-hook").join("sess-3.jsonl").exists(),
            "no rows means no transcript"
        );
        assert!(!capture.exists(), "no rows means no command run");
    }
}
