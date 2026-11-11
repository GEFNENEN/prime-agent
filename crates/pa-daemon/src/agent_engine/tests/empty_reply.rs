//! A subagent whose reply to an injected message is empty ends its run with
//! nobody told: the incomplete-reply verdict only owes an answer to a user
//! prompt, so silence there settles cleanly and the sender believes the child
//! took the work. The child tells its parent instead, once per stall, so the
//! run can be re-tasked.

use super::*;

fn thinking_only_stop() -> serde_json::Value {
    json!({
        "content": [{"type": "thinking", "thinking": "Nothing to add here."}],
        "stopReason": "stop",
    })
}

fn agent_message_row(text: &str) -> serde_json::Value {
    json!({
        "role": "custom",
        "customType": "agent_message",
        "content": text,
        "display": true,
        "timestamp": 0
    })
}

/// A child session (RLM depth 1) whose stalled-run notices land in a sink.
fn stalled_child(
    dir: &std::path::Path,
    responses: &serde_json::Value,
) -> (
    std::sync::Arc<AgentSessionEngine>,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    let engine = AgentSessionEngine::new(AgentEngineConfig {
        cwd: dir.to_path_buf(),
        agent_dir: dir.join("agent"),
        provider: None,
        model: None,
        api_key: None,
        thinking: None,
        session_dir: None,
        session_file: None,
        faux_script: Some(json!({"engine": "faux", "responses": responses}).to_string()),
        supervisor_link: None,
        telemetry_disabled: None,
        cron_store: None,
        queued_steering_probe: None,
    })
    .unwrap();
    let engine = std::sync::Arc::new(engine);
    engine.register_arc();
    <AgentSessionEngine as crate::engine::SessionEngine>::configure_rlm_identity(
        &engine,
        crate::engine::RlmSessionIdentity {
            rlm_depth: 1,
            rlm_max_depth: None,
            cwd: None,
            session_id: None,
            session_file: None,
            thinking: None,
            child_script: None,
            semantic_spawn: None,
            factory_child: false,
        },
    )
    .unwrap();
    let notices: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&notices);
    *engine.stalled_run_notice.lock().unwrap() = Some(std::sync::Arc::new(move |text: String| {
        let sink = std::sync::Arc::clone(&sink);
        Box::pin(async move {
            sink.lock().unwrap().push(text);
            Ok(())
        })
    }));
    (engine, notices)
}

fn run_injected(engine: &AgentSessionEngine, text: &str, events: &mut Vec<EngineEvent>) {
    engine.run_prompt(
        0,
        PromptRequest {
            batch: Vec::new(),
            images: Vec::new(),
            message: text.to_string(),
            source: "user".to_string(),
            agent_message_id: None,
            custom_message: Some(agent_message_row(text)),
        },
        &|| false,
        &mut |event| {
            events.push(event);
            true
        },
    );
}

#[test]
fn a_stalled_subagent_notifies_its_parent() {
    let _faux = FAUX_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::TempDir::new().unwrap();
    let (engine, notices) = stalled_child(dir.path(), &json!([thinking_only_stop()]));
    let mut events: Vec<EngineEvent> = Vec::new();
    run_injected(&engine, "sibling update: the queue drained", &mut events);
    let sent = notices.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "the parent is told exactly once");
    assert!(
        sent[0].contains("re-task me"),
        "the notice asks the parent to re-task the child: {}",
        sent[0]
    );
    assert_eq!(
        events.last(),
        Some(&EngineEvent::Done(Ok(()))),
        "the silent acknowledgement still ends the turn cleanly"
    );
}

#[test]
fn a_visible_reply_clears_the_stall_gate() {
    let _faux = FAUX_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::TempDir::new().unwrap();
    let (engine, notices) = stalled_child(
        dir.path(),
        &json!([
            thinking_only_stop(),
            {"text": "Done: the queue is drained."},
            thinking_only_stop(),
        ]),
    );
    let mut first: Vec<EngineEvent> = Vec::new();
    run_injected(&engine, "sibling update: the queue drained", &mut first);
    let mut second: Vec<EngineEvent> = Vec::new();
    run_injected(&engine, "sibling update: tests pass", &mut second);
    let mut third: Vec<EngineEvent> = Vec::new();
    run_injected(&engine, "sibling update: the log rotated", &mut third);
    let sent = notices.lock().unwrap().clone();
    assert_eq!(
        sent.len(),
        2,
        "a reply in between re-arms the notice: {sent:?}"
    );
}

#[test]
fn a_repeat_stall_does_not_flood_the_parent() {
    let _faux = FAUX_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::TempDir::new().unwrap();
    let (engine, notices) = stalled_child(
        dir.path(),
        &json!([thinking_only_stop(), thinking_only_stop()]),
    );
    let mut first: Vec<EngineEvent> = Vec::new();
    run_injected(&engine, "First message", &mut first);
    let mut second: Vec<EngineEvent> = Vec::new();
    run_injected(&engine, "Second message", &mut second);
    let sent = notices.lock().unwrap().clone();
    assert_eq!(
        sent.len(),
        1,
        "a still-stalled child announces once, not once per message: {sent:?}"
    );
}

#[test]
fn a_silent_user_prompt_is_left_to_the_incomplete_reply_verdict() {
    let _faux = FAUX_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::TempDir::new().unwrap();
    let (engine, notices) = stalled_child(
        dir.path(),
        &json!([thinking_only_stop(), thinking_only_stop()]),
    );
    let mut events: Vec<EngineEvent> = Vec::new();
    admit(&engine, "explain the slow query".to_string(), &mut events);
    let sent = notices.lock().unwrap().clone();
    assert!(
        sent.is_empty(),
        "a user prompt surfaces its own error; the child must not double-report: {sent:?}"
    );
    assert!(
        matches!(events.last(), Some(EngineEvent::Done(Err(_)))),
        "the retried silent reply ends the turn with an error: {:?}",
        events.last()
    );
}
