//! The `turnHook` settings surface: a user-configured shell command the
//! session runs once per admitted user turn, with a `{prompt, cwd,
//! session_id}` JSON payload on stdin. The `hookSpecificOutput
//! .additionalContext` of its stdout JSON is appended to that turn's
//! system prompt through the agent loop's `get_system_prompt` seam (the
//! Rust parity of the TS `before_agent_start` extension hook).
//!
//! The command runs inside the turn's first LLM request (the loop's
//! transform-context seam), not at admission: prompt acceptance and
//! streaming start immediately, and the hook's model latency rides the
//! turn instead of delaying the dispatch.
//!
//! Fail-open by contract: a missing command, spawn failure, timeout,
//! stderr-only output, malformed JSON, or an empty `additionalContext`
//! injects nothing and never fails the turn.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use pa_agent::abort::AbortSignal;
use pa_agent::agent_loop::{GetSystemPromptFn, TransformContextFn};
use pa_agent::types::AgentMessage;
use tokio::io::AsyncWriteExt;

use super::AgentSession;
use crate::settings::TurnHookSettings;

/// Per-turn wall-clock bound when `turnHook.timeoutMs` is unset: the
/// console-side hooks cap a first-turn reflect at 25s, so 30s keeps the
/// documented budget with a margin.
pub const DEFAULT_TURN_HOOK_TIMEOUT_MS: u64 = 30_000;

/// One admitted turn waiting for its hook run (the payload inputs).
struct PendingTurn {
    prompt: String,
    cwd: String,
    session_id: String,
}

#[derive(Default)]
struct TurnHookState {
    /// Set at admission, consumed by the turn's first request transform.
    pending: Option<PendingTurn>,
    /// The current turn's injection text, read by the resolver on every
    /// request of the run.
    injection: Option<String>,
}

/// One configured turn hook: the command line, its per-turn timeout, and
/// the shared state the resolver + transform closures reach.
pub struct TurnHook {
    command: String,
    timeout_ms: u64,
    state: Arc<std::sync::Mutex<TurnHookState>>,
}

fn lock_state(state: &std::sync::Mutex<TurnHookState>) -> std::sync::MutexGuard<'_, TurnHookState> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl TurnHook {
    /// Build the hook from the `turnHook` settings; `None` (or a blank
    /// command) keeps the feature off.
    #[must_use]
    pub fn from_settings(settings: Option<&TurnHookSettings>) -> Option<TurnHook> {
        let settings = settings?;
        let command = settings.command.as_ref()?.trim();
        if command.is_empty() {
            return None;
        }
        Some(TurnHook {
            command: command.to_string(),
            timeout_ms: settings
                .timeout_ms
                .unwrap_or(DEFAULT_TURN_HOOK_TIMEOUT_MS),
            state: Arc::new(std::sync::Mutex::new(TurnHookState::default())),
        })
    }

    /// The `get_system_prompt` resolver this hook injects through: with an
    /// injection pending it returns `base` plus the hook text after a blank
    /// line; otherwise the untouched base prompt.
    #[must_use]
    pub fn resolver(&self, base: &str) -> GetSystemPromptFn {
        let base = base.to_string();
        let state = Arc::clone(&self.state);
        Arc::new(move || {
            let injection = lock_state(&state).injection.clone();
            match injection {
                Some(text) if !text.trim().is_empty() => format!("{base}\n\n{text}"),
                _ => base.clone(),
            }
        })
    }

    /// Wrap the loop's context transform so the turn's first request runs
    /// the hook: the pending note is consumed (later requests of the same
    /// run are no-ops), the command runs, and its injection replaces the
    /// slot for the resolver that fires immediately after. Messages pass
    /// through untouched.
    #[must_use]
    pub fn wrap_transform(&self, inner: TransformContextFn) -> TransformContextFn {
        let command = self.command.clone();
        let timeout_ms = self.timeout_ms;
        let state = Arc::clone(&self.state);
        Arc::new(
            move |messages: Vec<AgentMessage>, signal: AbortSignal| {
                let command = command.clone();
                let state = Arc::clone(&state);
                let inner = Arc::clone(&inner);
                Box::pin(async move {
                    let pending = lock_state(&state).pending.take();
                    if let Some(pending) = pending {
                        let injection = run_command(&command, timeout_ms, &pending).await;
                        lock_state(&state).injection = injection;
                    }
                    inner(messages, signal).await
                })
            },
        )
    }

    /// Note an admitted turn: the next request's transform runs the hook
    /// for it. A blank prompt notes nothing (the console hooks skip empty
    /// prompts the same way).
    pub fn note_turn(&self, prompt: &str, cwd: &str, session_id: &str) {
        if prompt.trim().is_empty() {
            return;
        }
        lock_state(&self.state).pending = Some(PendingTurn {
            prompt: prompt.to_string(),
            cwd: cwd.to_string(),
            session_id: session_id.to_string(),
        });
    }

    /// Drop a pending note (a failed admission must not fire the next
    /// turn's hook with this turn's prompt).
    pub fn discard_pending(&self) {
        lock_state(&self.state).pending = None;
    }
}

/// Spawn the command with the payload on stdin, bounded by the
/// configured timeout; any failure path reports no injection.
async fn run_command(command: &str, timeout_ms: u64, turn: &PendingTurn) -> Option<String> {
    let payload = serde_json::json!({
        "prompt": turn.prompt,
        "cwd": turn.cwd,
        "session_id": turn.session_id,
    });
    let mut process = hidden_shell_command(command);
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::debug!(%error, command, "turn hook spawn failed");
            return None;
        }
    };
    {
        let mut stdin = child.stdin.take()?;
        if let Err(error) = stdin.write_all(payload.to_string().as_bytes()).await {
            tracing::debug!(%error, "turn hook stdin write failed");
            return None;
        }
        // Dropping the handle closes the pipe so the hook sees EOF.
    }
    let waited =
        tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait_with_output()).await;
    let output = match waited {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            tracing::debug!(%error, "turn hook wait failed");
            return None;
        }
        Err(_elapsed) => {
            // The dropped future reaps the child (`kill_on_drop`).
            tracing::debug!(
                timeout_ms,
                "turn hook timed out; injecting nothing"
            );
            return None;
        }
    };
    if !output.status.success() {
        tracing::debug!(
            status = ?output.status.code(),
            "turn hook exited non-zero; injecting nothing"
        );
        return None;
    }
    parse_additional_context(&String::from_utf8_lossy(&output.stdout))
}

/// The stdout contract: one JSON object, `hookSpecificOutput
/// .additionalContext` carrying the injection text. Empty output (the
/// console hooks' cache-hit turns) and every other shape inject nothing.
fn parse_additional_context(stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let context = value
        .get("hookSpecificOutput")?
        .get("additionalContext")?
        .as_str()?;
    (!context.trim().is_empty()).then(|| context.to_string())
}

impl AgentSession {
    /// Note the configured turn hook's next run for one admitted user turn.
    /// Without a wired hook this is a no-op.
    pub(super) async fn note_turn_hook(&self, prompt: &str) {
        let Some(hook) = self.turn_hook.as_ref() else {
            return;
        };
        let (cwd, session_id) = {
            let session = self.session.lock().await;
            (
                session.get_cwd().display().to_string(),
                session.get_session_id().to_string(),
            )
        };
        hook.note_turn(prompt, &cwd, &session_id);
    }

    /// Drop a pending hook note after a failed admission.
    pub(super) fn discard_turn_hook_note(&self) {
        if let Some(hook) = self.turn_hook.as_ref() {
            hook.discard_pending();
        }
    }
}

/// Hidden shell invocation for the hook command lines (shared with the
/// agent-end `stopHook` write-back): Unix uses `bash -c`; Windows prefers
/// the configured shell and falls back to `ComSpec`, with
/// `CREATE_NO_WINDOW` so the hook never pops a console.
#[cfg(unix)]
pub(super) fn hidden_shell_command(command: &str) -> tokio::process::Command {
    let mut process = tokio::process::Command::new("bash");
    process.arg("-c").arg(command);
    process
}

#[cfg(windows)]
pub(super) fn hidden_shell_command(command: &str) -> tokio::process::Command {
    let configured = crate::platform::get_shell_config(None);
    let mut process = if let Ok(config) = configured {
        let mut process = tokio::process::Command::new(config.shell);
        process.args(config.args).arg(command);
        process
    } else {
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
        let mut process = tokio::process::Command::new(comspec);
        process.args(["/d", "/s", "/c"]).arg(command);
        process
    };
    crate::platform::process::set_no_window(process.as_std_mut());
    process
}

#[cfg(not(any(unix, windows)))]
pub(super) fn hidden_shell_command(command: &str) -> tokio::process::Command {
    let mut process = tokio::process::Command::new("sh");
    process.arg("-c").arg(command);
    process
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_hook(command: &str, timeout_ms: Option<u64>) -> TurnHook {
        TurnHook::from_settings(Some(&TurnHookSettings {
            command: Some(command.to_string()),
            timeout_ms,
        }))
        .expect("a non-blank command builds the hook")
    }

    /// The hook command line runs through the platform shell (bash on Unix
    /// and whenever Windows resolves Git Bash; `cmd.exe` otherwise), so the
    /// test payloads must speak that shell's quoting.
    #[cfg(windows)]
    fn shell_is_bash() -> bool {
        crate::platform::get_shell_config(None).map_or(false, |config| {
            config.shell.to_ascii_lowercase().contains("bash")
        })
    }

    /// A command that prints the given JSON on stdout in the configured
    /// shell's quoting.
    fn print_json(json: &str) -> String {
        #[cfg(windows)]
        if !shell_is_bash() {
            // `cmd.exe` prints the line literally, double quotes included.
            return format!("echo {json}");
        }
        format!("printf '%s' '{json}'")
    }

    /// A command that outlives any small test timeout.
    fn slow_command() -> &'static str {
        #[cfg(windows)]
        if !shell_is_bash() {
            // ~5s of pings: the classic cmd.exe sleep.
            return "ping -n 6 127.0.0.1";
        }
        "sleep 5"
    }

    fn injected(payload: &str) -> String {
        print_json(&format!(
            r#"{{"hookSpecificOutput":{{"additionalContext":"{payload}"}}}}"#
        ))
    }

    async fn run_transform(hook: &TurnHook) {
        let transform = hook.wrap_transform(Arc::new(|messages, _signal| {
            Box::pin(async move { Ok(messages) })
        }));
        let messages = transform(Vec::new(), AbortSignal::default())
            .await
            .unwrap();
        assert!(messages.is_empty(), "the transform passes messages through");
    }

    #[test]
    fn additional_context_parses_and_empty_shapes_inject_nothing() {
        let payload = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"memory text"}}"#;
        assert_eq!(
            parse_additional_context(payload).as_deref(),
            Some("memory text")
        );
        // The console hooks emit nothing on cache-hit turns.
        assert_eq!(parse_additional_context(""), None);
        assert_eq!(parse_additional_context("   \n"), None);
        assert_eq!(parse_additional_context("not json"), None);
        assert_eq!(parse_additional_context(r#"{"hookSpecificOutput":{}}"#), None);
        assert_eq!(
            parse_additional_context(r#"{"hookSpecificOutput":{"additionalContext":"   "}}"#),
            None
        );
        assert_eq!(
            parse_additional_context(r#"{"systemMessage":"notice only"}"#),
            None
        );
    }

    #[test]
    fn resolver_appends_the_injection_and_falls_back_to_the_base() {
        let hook = echo_hook("echo", None);
        let resolver = hook.resolver("BASE PROMPT");
        assert_eq!(resolver(), "BASE PROMPT");

        lock_state(&hook.state).injection =
            Some("<hindsight_memory>facts</hindsight_memory>".to_string());
        assert_eq!(
            resolver(),
            "BASE PROMPT\n\n<hindsight_memory>facts</hindsight_memory>"
        );

        lock_state(&hook.state).injection = None;
        assert_eq!(resolver(), "BASE PROMPT");
    }

    #[test]
    fn blank_commands_and_absent_settings_disable_the_hook() {
        assert!(TurnHook::from_settings(None).is_none());
        assert!(TurnHook::from_settings(Some(&TurnHookSettings::default())).is_none());
        assert!(
            TurnHook::from_settings(Some(&TurnHookSettings {
                command: Some("   ".to_string()),
                timeout_ms: None,
            }))
            .is_none()
        );
    }

    #[test]
    fn noting_a_turn_skips_blank_prompts_and_discard_clears() {
        let hook = echo_hook("echo", None);
        hook.note_turn("   ", "/tmp", "s");
        assert!(lock_state(&hook.state).pending.is_none());
        hook.note_turn("real", "/tmp", "s");
        assert!(lock_state(&hook.state).pending.is_some());
        hook.discard_pending();
        assert!(lock_state(&hook.state).pending.is_none());
    }

    #[tokio::test]
    async fn the_turns_first_request_runs_the_command_and_publishes_the_injection() {
        let hook = echo_hook(&injected("remembered"), None);
        let resolver = hook.resolver("BASE");
        hook.note_turn("do the thing", "/tmp", "session-1");
        run_transform(&hook).await;
        assert_eq!(resolver(), "BASE\n\nremembered");

        // Later requests of the same run keep the injection without
        // re-running the command.
        run_transform(&hook).await;
        assert_eq!(resolver(), "BASE\n\nremembered");
    }

    #[tokio::test]
    async fn a_quiet_turn_clears_the_previous_injection() {
        let hook = echo_hook(&injected("remembered"), None);
        let resolver = hook.resolver("BASE");
        hook.note_turn("first", "/tmp", "session-1");
        run_transform(&hook).await;
        assert_eq!(resolver(), "BASE\n\nremembered");

        // The next turn's hook emits nothing (the console hooks' cache-hit
        // shape): the stale injection must not survive the turn.
        let hook = echo_hook("echo", None);
        let resolver = hook.resolver("BASE");
        hook.note_turn("second", "/tmp", "session-1");
        run_transform(&hook).await;
        assert_eq!(resolver(), "BASE");
    }

    #[tokio::test]
    async fn a_timed_out_command_injects_nothing() {
        let hook = echo_hook(slow_command(), Some(200));
        let resolver = hook.resolver("BASE");
        hook.note_turn("slow", "/tmp", "session-1");
        run_transform(&hook).await;
        assert_eq!(resolver(), "BASE");
    }

    #[tokio::test]
    async fn a_failing_command_injects_nothing() {
        let hook = echo_hook("exit 3", None);
        let resolver = hook.resolver("BASE");
        hook.note_turn("boom", "/tmp", "session-1");
        run_transform(&hook).await;
        assert_eq!(resolver(), "BASE");
    }
}
