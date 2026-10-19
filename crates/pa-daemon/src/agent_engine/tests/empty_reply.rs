//! The empty-reply retry: a finished turn with no text and no tool call is
//! re-issued once with a request-only nudge, then surfaced as a warning.
use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(20);

fn read_request(listener: &TcpListener) -> (TcpStream, String) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + DEADLINE;
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "provider request never arrived");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("accept provider request: {error}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(DEADLINE)).unwrap();
    stream.set_write_timeout(Some(DEADLINE)).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "incomplete request"
        );
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    (stream, String::from_utf8(body).unwrap())
}

fn respond_sse(stream: &mut TcpStream, text: Option<&str>) {
    let mut frames = String::new();
    frames.push_str(&format!(
        "data: {}\n\n",
        json!({
            "id": "empty-reply-test", "object": "chat.completion.chunk", "created": 1,
            "model": "mock-1", "choices": [{"index": 0,
                "delta": {"role": "assistant"}, "finish_reason": null}]
        })
    ));
    if let Some(text) = text {
        frames.push_str(&format!(
            "data: {}\n\n",
            json!({
                "id": "empty-reply-test", "object": "chat.completion.chunk", "created": 1,
                "model": "mock-1", "choices": [{"index": 0,
                    "delta": {"content": text}, "finish_reason": null}]
            })
        ));
    }
    frames.push_str(&format!(
        "data: {}\n\n",
        json!({
            "id": "empty-reply-test", "object": "chat.completion.chunk", "created": 1,
            "model": "mock-1", "choices": [{"index": 0,
                "delta": {}, "finish_reason": "stop"}]
        })
    ));
    frames.push_str("data: [DONE]\n\n");
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{frames}"
    )
    .unwrap();
}

fn empty_reply_engine(
    dir: &std::path::Path,
    listener: &TcpListener,
) -> std::sync::Arc<AgentSessionEngine> {
    let agent_dir = dir.join("agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(
        agent_dir.join("models.json"),
        json!({
            "providers": {
                "empty-reply": {
                    "api": "openai-completions",
                    "baseUrl": format!("http://{}/v1", listener.local_addr().unwrap()),
                    "apiKey": "sk-empty-reply-test",
                    "models": [{
                        "id": "mock-1", "name": "empty reply",
                        "api": "openai-completions", "contextWindow": 128000, "maxTokens": 4096
                    }]
                }
            }
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        agent_dir.join("settings.json"),
        json!({"retry": {"enabled": false}}).to_string(),
    )
    .unwrap();
    let engine = AgentSessionEngine::new(AgentEngineConfig {
        cwd: dir.to_path_buf(),
        agent_dir,
        provider: Some("empty-reply".to_string()),
        model: Some("mock-1".to_string()),
        api_key: None,
        thinking: None,
        session_dir: None,
        session_file: None,
        faux_script: None,
        supervisor_link: None,
        telemetry_disabled: Some(true),
        cron_store: None,
        queued_steering_probe: None,
    })
    .unwrap();
    let model = engine.resolve_model().unwrap();
    engine.ensure_core_session(&model).unwrap();
    std::sync::Arc::new(engine)
}

fn collect_jsonl(root: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            found.push(path);
        }
    }
}

#[test]
fn retries_an_empty_finished_reply_once_with_a_nudge() {
    let dir = tempfile::TempDir::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let engine = empty_reply_engine(dir.path(), &listener);
    let server = std::thread::spawn(move || {
        let (mut first, _body) = read_request(&listener);
        respond_sse(&mut first, None);
        let (mut second, body) = read_request(&listener);
        respond_sse(&mut second, Some("pong"));
        body
    });
    let mut events: Vec<EngineEvent> = Vec::new();
    admit(
        &engine,
        "Reply with the single word: pong".to_string(),
        &mut events,
    );
    let retried_body = server.join().unwrap();
    assert!(
        retried_body.contains("previous reply was empty"),
        "the retried request must carry the nudge: {retried_body}"
    );
    assert_eq!(
        assistant_texts(&events).last().map(String::as_str),
        Some("pong"),
        "the retried turn must settle with the model's visible answer"
    );
    let mut sessions = Vec::new();
    collect_jsonl(dir.path(), &mut sessions);
    let persisted: Vec<_> = sessions
        .iter()
        .filter(|path| {
            std::fs::read_to_string(path)
                .is_ok_and(|text| text.contains("previous reply was empty"))
        })
        .collect();
    assert!(
        persisted.is_empty(),
        "the nudge must ride the request only, not the transcript: {persisted:?}"
    );
}

#[test]
fn a_stale_nudge_does_not_leak_into_the_next_turn() {
    let dir = tempfile::TempDir::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let engine = empty_reply_engine(dir.path(), &listener);
    // A retry that was queued but never issued (abort, worker shutdown)
    // leaves the slot armed; the next unrelated turn must not inherit it.
    let core = engine
        .runtime
        .block_on(async { engine.session.lock().await.clone() })
        .expect("core session");
    core.session.queue_retry_nudge("stale nudge");
    let server = std::thread::spawn(move || {
        let (mut stream, body) = read_request(&listener);
        respond_sse(&mut stream, Some("pong"));
        body
    });
    let mut events: Vec<EngineEvent> = Vec::new();
    admit(&engine, "Reply with pong".to_string(), &mut events);
    let body = server.join().unwrap();
    assert!(
        !body.contains("stale nudge"),
        "a nudge from an aborted retry must not reach a later request: {body}"
    );
    assert_eq!(
        assistant_texts(&events).last().map(String::as_str),
        Some("pong")
    );
}

#[test]
fn warns_when_the_retry_is_also_empty() {
    let dir = tempfile::TempDir::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let engine = empty_reply_engine(dir.path(), &listener);
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _body) = read_request(&listener);
            respond_sse(&mut stream, None);
        }
    });
    let mut events: Vec<EngineEvent> = Vec::new();
    admit(
        &engine,
        "Reply with the single word: pong".to_string(),
        &mut events,
    );
    server.join().unwrap();
    let warnings: Vec<&Value> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::CustomMessage(value) if value["customType"] == "model_prompt_error" => {
                Some(value)
            }
            _ => None,
        })
        .collect();
    assert_eq!(warnings.len(), 1, "exactly one visible warning");
    assert!(
        assistant_texts(&events).iter().all(String::is_empty),
        "no visible assistant text was produced"
    );
}

#[test]
fn a_stalled_subagent_notifies_its_parent() {
    let dir = tempfile::TempDir::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let engine = empty_reply_engine(dir.path(), &listener);
    // A child session (RLM depth 1) has no operator to nudge it: after the
    // one automatic retry it must tell the parent instead of idling unseen.
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
        },
    )
    .unwrap();
    let notices: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_notices = std::sync::Arc::clone(&notices);
    *engine.stalled_run_notice.lock().unwrap() = Some(std::sync::Arc::new(move |text: String| {
        let sink_notices = std::sync::Arc::clone(&sink_notices);
        Box::pin(async move {
            sink_notices.lock().unwrap().push(text);
            Ok(())
        })
    }));
    let server = std::thread::spawn(move || {
        // The original attempt plus exactly one automatic retry.
        for _ in 0..2 {
            let (mut stream, _body) = read_request(&listener);
            respond_sse(&mut stream, None);
        }
    });
    let mut events: Vec<EngineEvent> = Vec::new();
    admit(
        &engine,
        "Reply with the single word: pong".to_string(),
        &mut events,
    );
    server.join().unwrap();
    let sent = notices.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "the parent is told exactly once");
    assert!(
        sent[0].contains("re-task me"),
        "the notice asks the parent to re-task the child: {}",
        sent[0]
    );
    let warnings: Vec<&Value> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::CustomMessage(value) if value["customType"] == "model_prompt_error" => {
                Some(value)
            }
            _ => None,
        })
        .collect();
    assert_eq!(warnings.len(), 1, "exactly one visible warning");
    let content = warnings[0]["content"].as_str().unwrap_or_default();
    assert!(
        !content.contains("send a message"),
        "a subagent must not be told to send a message: {content}"
    );
    assert!(
        content.contains("parent was notified"),
        "the child row says the parent was told: {content}"
    );
}
