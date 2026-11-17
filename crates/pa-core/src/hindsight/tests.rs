//! The bridge's unit tests: a tiny in-process HTTP server (no new test
//! dependency) and temp state roots, so nothing here touches the real
//! Hindsight service or the user's settings.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::bank;
use super::config::{Config, Env, DEFAULT_BANK_TEMPLATE};
use super::prompt::{self, PromptInput};
use super::retain::{self, StopInput};
use super::state::{self, PromptCache, RetainCursor};

// ---------------------------------------------------------------------------
// The fake server
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Route {
    method: &'static str,
    path_contains: &'static str,
    status: u16,
    body: Value,
    delay: Duration,
}

impl Route {
    fn ok(method: &'static str, path_contains: &'static str, body: Value) -> Route {
        Route {
            method,
            path_contains,
            status: 200,
            body,
            delay: Duration::ZERO,
        }
    }

    fn status(method: &'static str, path_contains: &'static str, status: u16) -> Route {
        Route {
            method,
            path_contains,
            status,
            body: json!({"detail": "boom"}),
            delay: Duration::ZERO,
        }
    }

    fn slow(
        method: &'static str,
        path_contains: &'static str,
        body: Value,
        delay: Duration,
    ) -> Route {
        Route {
            method,
            path_contains,
            status: 200,
            body,
            delay,
        }
    }
}

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    body: Value,
}

struct State {
    routes: Vec<Route>,
    requests: Vec<Recorded>,
}

struct TestServer {
    url: String,
    state: Arc<Mutex<State>>,
}

impl TestServer {
    async fn start(routes: Vec<Route>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let state = Arc::new(Mutex::new(State {
            routes,
            requests: Vec::new(),
        }));
        let shared = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let shared = Arc::clone(&shared);
                tokio::spawn(async move {
                    let Some(request) = read_request(&mut socket).await else {
                        return;
                    };
                    let route = {
                        let mut state = shared.lock().expect("server state");
                        state.requests.push(request.clone());
                        state
                            .routes
                            .iter()
                            .find(|route| {
                                route.method == request.method
                                    && request.path.contains(route.path_contains)
                            })
                            .cloned()
                    };
                    if let Some(route) = route {
                        if !route.delay.is_zero() {
                            tokio::time::sleep(route.delay).await;
                        }
                        let _ = socket
                            .write_all(render(route.status, &route.body).as_bytes())
                            .await;
                    } else {
                        let _ = socket
                            .write_all(render(404, &json!({"detail": "not found"})).as_bytes())
                            .await;
                    }
                    let _ = socket.shutdown().await;
                });
            }
        });
        TestServer {
            url: format!("http://{address}"),
            state,
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.state.lock().expect("server state").requests.clone()
    }

    fn count(&self, method: &str, path_contains: &str) -> usize {
        self.requests()
            .iter()
            .filter(|request| request.method == method && request.path.contains(path_contains))
            .count()
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Recorded> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find(&buffer, b"\r\n\r\n") {
            break position + 4;
        }
        if buffer.len() > 64 * 1024 {
            return None;
        }
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut content_length = 0usize;
    for line in headers.lines() {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok()?;
        } else if let Some(value) = line.strip_prefix("content-length:") {
            content_length = value.trim().parse().ok()?;
        }
    }
    while buffer.len() < header_end + content_length {
        let mut chunk = [0u8; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let request_line = headers.lines().next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let body_text = String::from_utf8_lossy(&buffer[header_end..]).into_owned();
    let body = serde_json::from_str(&body_text).unwrap_or(Value::Null);
    Some(Recorded { method, path, body })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn render(status: u16, body: &Value) -> String {
    let body = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());
    let reason = if status == 200 { "OK" } else { "Error" };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn config(url: &str) -> Config {
    Config {
        api_url: url.to_string(),
        api_token: None,
        bank_id: Some("coding-agent::test".to_string()),
        dynamic_bank_id: false,
        bank_id_template: DEFAULT_BANK_TEMPLATE.to_string(),
        map_path_to_bank: BTreeMap::new(),
        banks: BTreeMap::new(),
        disabled: false,
        auto_reflect: true,
        reflect_timeout_ms: 20_000,
        retain_sessions: true,
        resolve_worktrees: false,
        page_refresh_every_turns: 10,
        opt_in_only: false,
        opt_in_paths: Vec::new(),
        harness: "prime-agent".to_string(),
    }
}

fn prompt_input(prompt: &str, cwd: &Path, session: &str) -> PromptInput {
    PromptInput {
        prompt: prompt.to_string(),
        cwd: cwd.to_string_lossy().into_owned(),
        session_id: session.to_string(),
    }
}

fn user_row(text: &str) -> Value {
    json!({
        "schema_version": 1,
        "role": "user",
        "content": [{"type": "text", "text": text}],
        "timestamp": "2026-11-17T00:00:00.000Z",
    })
}

fn assistant_row(text: &str) -> Value {
    json!({
        "schema_version": 1,
        "role": "assistant",
        "content": [{"type": "text", "text": text}],
        "timestamp": "2026-11-17T00:00:05.000Z",
    })
}

fn write_rows(path: &Path, rows: &[Value]) {
    let text = rows
        .iter()
        .map(|row| serde_json::to_string(row).expect("row"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{text}\n")).expect("write transcript");
}

fn stop_input(path: &Path, cwd: &Path, session: &str) -> StopInput {
    StopInput {
        session_id: session.to_string(),
        transcript_path: path.to_string_lossy().into_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
    }
}

fn cursor(state_root: &Path, session: &str) -> Option<RetainCursor> {
    state::read_json(&state::session_dir(state_root, session).join("retain.json"))
}

// ---------------------------------------------------------------------------
// Prompt hook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_first_turn_reflects_once_and_later_turns_stay_quiet() {
    let server = TestServer::start(vec![
        Route::ok("POST", "/reflect", json!({"text": "DECIDED: use rustls"})),
        Route::ok("GET", "/knowledge-base/tree", json!({"roots": []})),
    ])
    .await;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");
    let config = config(&server.url);

    let first = prompt::run(
        &config,
        state_root.path(),
        prompt_input("how does the hook work?", cwd.path(), "s1"),
    )
    .await
    .expect("first turn injects");
    assert!(first.contains("<hindsight_memory>"));
    assert!(first.contains("DECIDED: use rustls"));
    assert!(
        !first.contains("hindsight_ingest_document"),
        "the injection must not call a tool this bridge never registers"
    );
    assert!(first.contains("stop hook writes this conversation back"));

    let second = prompt::run(
        &config,
        state_root.path(),
        prompt_input("follow up", cwd.path(), "s1"),
    )
    .await;
    assert!(
        second.is_none(),
        "later turns do not re-inject the synthesis"
    );
    assert_eq!(server.count("POST", "/reflect"), 1);

    let cache: PromptCache =
        state::read_json(&state::session_dir(state_root.path(), "s1").join("prompt.json"))
            .expect("cache written");
    assert_eq!(cache.turns, 2);
    assert_eq!(cache.reflect_answer.as_deref(), Some("DECIDED: use rustls"));
}

#[tokio::test]
async fn a_cached_answer_survives_a_restart_without_another_reflect() {
    let server =
        TestServer::start(vec![Route::ok("POST", "/reflect", json!({"text": "NEW"}))]).await;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");
    let dir = state::session_dir(state_root.path(), "s1");
    std::fs::create_dir_all(&dir).expect("session dir");
    state::write_json(
        &dir.join("prompt.json"),
        &PromptCache {
            turns: 1,
            reflect_answer: Some("CACHED".to_string()),
            pages: Some(state::PageCache {
                at_turn: 1,
                list: Vec::new(),
            }),
            bank: Some("coding-agent::test".to_string()),
        },
    )
    .expect("seed cache");

    let output = prompt::run(
        &config(&server.url),
        state_root.path(),
        prompt_input("next turn", cwd.path(), "s1"),
    )
    .await;
    assert!(output.is_none());
    assert_eq!(server.count("POST", "/reflect"), 0);
    let cache: PromptCache = state::read_json(&dir.join("prompt.json")).expect("cache survives");
    assert_eq!(cache.turns, 2);
    assert_eq!(cache.reflect_answer.as_deref(), Some("CACHED"));
}

#[tokio::test]
async fn a_timed_out_reflect_falls_back_to_page_search() {
    let server = TestServer::start(vec![
        Route::slow(
            "POST",
            "/reflect",
            json!({"text": "too late"}),
            Duration::from_millis(200),
        ),
        Route::ok(
            "GET",
            "/knowledge-base/search",
            json!({"results": [
                {"id": "kp-1", "name": "Component map", "snippet": "the engine owns sessions"},
            ]}),
        ),
    ])
    .await;
    let mut config = config(&server.url);
    config.reflect_timeout_ms = 20;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");

    let output = prompt::run(
        &config,
        state_root.path(),
        prompt_input("how does the engine work?", cwd.path(), "s1"),
    )
    .await
    .expect("fallback injects");
    assert!(output.starts_with("<hindsight_memory>"));
    assert!(output.contains("synthesis was unavailable"));
    assert!(output.contains("Component map (kp-1)"));
    let requests = server.requests();
    let search = requests
        .iter()
        .find(|request| request.path.contains("/knowledge-base/search"))
        .expect("page search sent");
    assert!(search.path.contains("limit=3"));
    assert!(search
        .path
        .contains("q=how%20does%20the%20engine%20work%3F"));
}

#[tokio::test]
async fn every_failed_tier_injects_nothing_and_remembers_the_attempt() {
    let server = TestServer::start(vec![
        Route::status("POST", "/reflect", 500),
        Route::status("GET", "/knowledge-base/search", 500),
        Route::status("POST", "/memories/recall", 500),
    ])
    .await;
    let mut config = config(&server.url);
    config.reflect_timeout_ms = 20;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");

    let output = prompt::run(
        &config,
        state_root.path(),
        prompt_input("anything", cwd.path(), "s1"),
    )
    .await;
    assert!(output.is_none());
    let cache: PromptCache =
        state::read_json(&state::session_dir(state_root.path(), "s1").join("prompt.json"))
            .expect("cache");
    assert_eq!(cache.reflect_answer.as_deref(), Some(""));
    assert_eq!(cache.turns, 1);
}

#[tokio::test]
async fn the_roster_refreshes_on_the_configured_cadence_not_every_turn() {
    let server = TestServer::start(vec![
        Route::ok(
            "GET",
            "/knowledge-base/tree",
            json!({"roots": [{"id": "kp-1", "kind": "page", "name": "Component map"}]}),
        ),
        Route::ok("POST", "/reflect", json!({"text": "never"})),
    ])
    .await;
    let mut config = config(&server.url);
    config.auto_reflect = false;
    config.page_refresh_every_turns = 2;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");

    let run = |turn: usize| {
        let config = config.clone();
        let state_root = state_root.path().to_path_buf();
        let cwd = cwd.path().to_path_buf();
        async move {
            prompt::run(
                &config,
                &state_root,
                prompt_input(&format!("turn {turn}"), &cwd, "s1"),
            )
            .await
        }
    };

    assert!(run(1).await.is_none());
    let second = run(2).await.expect("roster on turn 2");
    assert!(second.contains("<hindsight_knowledge_refresh>"));
    assert!(second.contains("Component map (kp-1)"));
    assert!(run(3).await.is_none());
    assert!(run(4)
        .await
        .expect("roster on turn 4")
        .contains("Component map"));
    assert_eq!(server.count("GET", "/knowledge-base/tree"), 2);
    assert_eq!(server.count("POST", "/reflect"), 0);
}

// ---------------------------------------------------------------------------
// Stop hook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_first_run_replaces_and_the_second_appends_only_new_turns() {
    let server = TestServer::start(vec![Route::ok(
        "POST",
        "/memories",
        json!({"operation_id": "op"}),
    )])
    .await;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");
    let transcript = cwd.path().join("transcript.jsonl");
    write_rows(
        &transcript,
        &[user_row("first question"), assistant_row("first answer")],
    );

    retain::run(
        &config(&server.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    let first = server.requests();
    assert_eq!(first.len(), 1);
    let item = &first[0].body["items"][0];
    assert!(item.get("update_mode").is_none());
    assert_eq!(first[0].body["async"], true);
    let content = item["content"].as_str().expect("content");
    assert!(content.contains("REF-ID: conversation:s1"));
    assert!(content.contains("first question"));
    assert!(content.contains("first answer"));

    write_rows(
        &transcript,
        &[
            user_row("first question"),
            assistant_row("first answer"),
            user_row("second question"),
        ],
    );
    retain::run(
        &config(&server.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    let second = server.requests();
    assert_eq!(second.len(), 2);
    let item = &second[1].body["items"][0];
    assert_eq!(item["update_mode"], "append");
    let content = item["content"].as_str().expect("append content");
    assert!(content.contains("second question"));
    assert!(!content.contains("first question"));

    retain::run(
        &config(&server.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    assert_eq!(
        server.requests().len(),
        2,
        "unchanged transcript is skipped"
    );
    let cursor = cursor(state_root.path(), "s1").expect("cursor");
    assert_eq!(cursor.turns, 3);
}

#[tokio::test]
async fn a_rewritten_prefix_falls_back_to_replace() {
    let server = TestServer::start(vec![Route::ok("POST", "/memories", json!({}))]).await;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");
    let transcript = cwd.path().join("transcript.jsonl");
    write_rows(&transcript, &[user_row("original")]);
    retain::run(
        &config(&server.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;

    write_rows(&transcript, &[user_row("rewritten")]);
    retain::run(
        &config(&server.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].body["items"][0].get("update_mode").is_none());
    assert!(requests[1].body["items"][0]["content"]
        .as_str()
        .expect("content")
        .contains("rewritten"));
}

#[tokio::test]
async fn a_failed_retain_leaves_the_cursor_for_the_next_run() {
    let failing = TestServer::start(vec![Route::status("POST", "/memories", 500)]).await;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");
    let transcript = cwd.path().join("transcript.jsonl");
    write_rows(&transcript, &[user_row("first")]);
    retain::run(
        &config(&failing.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    assert!(cursor(state_root.path(), "s1").is_none());

    let healthy = TestServer::start(vec![Route::ok("POST", "/memories", json!({}))]).await;
    retain::run(
        &config(&healthy.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    assert_eq!(healthy.count("POST", "/memories"), 1);
    assert_eq!(cursor(state_root.path(), "s1").expect("cursor").turns, 1);
}

#[tokio::test]
async fn injected_memory_and_tool_targets_are_rendered_the_js_way() {
    let server = TestServer::start(vec![Route::ok("POST", "/memories", json!({}))]).await;
    let state_root = tempfile::tempdir().expect("state root");
    let cwd = tempfile::tempdir().expect("cwd");
    let transcript = cwd.path().join("transcript.jsonl");
    write_rows(
        &transcript,
        &[
            user_row("<hindsight_memory>ignore me</hindsight_memory>real question"),
            json!({
                "schema_version": 1,
                "role": "tool",
                "name": "Edit",
                "content": {"file_path": "crates/pa-core/src/lib.rs"},
            }),
        ],
    );
    retain::run(
        &config(&server.url),
        state_root.path(),
        stop_input(&transcript, cwd.path(), "s1"),
    )
    .await;
    let content = server.requests()[0].body["items"][0]["content"]
        .as_str()
        .expect("content")
        .to_string();
    assert!(content.contains("real question"));
    assert!(!content.contains("ignore me"));
    assert!(content.contains(r#""role":"action""#));
    assert!(content.contains("Edit crates/pa-core/src/lib.rs"));
}

// ---------------------------------------------------------------------------
// Bank and config resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_path_map_and_template_resolve_repo_banks() {
    let temp = tempfile::tempdir().expect("temp");
    let mapped = temp.path().join("work").join("mapped");
    std::fs::create_dir_all(&mapped).expect("mapped dir");
    let mut config = config("http://127.0.0.1:1");
    config.map_path_to_bank.insert(
        temp.path().join("work").to_string_lossy().into_owned(),
        "p6".to_string(),
    );
    assert_eq!(bank::resolve(&config, &mapped).await.as_deref(), Some("p6"));

    let repo = temp.path().join("my-repo");
    std::fs::create_dir_all(&repo).expect("repo dir");
    config.dynamic_bank_id = true;
    config.bank_id = None;
    config.map_path_to_bank.clear();
    assert_eq!(
        bank::resolve(&config, &repo).await.as_deref(),
        Some("coding-agent::my-repo")
    );
}

#[tokio::test]
async fn a_fixed_bank_id_skips_the_template() {
    let temp = tempfile::tempdir().expect("temp");
    let mut config = config("http://127.0.0.1:1");
    config.bank_id = Some("coding".to_string());
    config.dynamic_bank_id = false;
    assert_eq!(
        bank::resolve(&config, temp.path()).await.as_deref(),
        Some("coding")
    );
}

struct MapEnv(BTreeMap<String, String>);

impl Env for MapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

#[test]
fn the_config_file_and_environment_overrides_resolve() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("coding-agent.json");
    std::fs::write(
        &path,
        serde_json::json!({
            "apiUrl": "http://127.0.0.1:8888",
            "bankId": "coding",
            "autoReflect": false,
            "reflectTimeoutMs": 1234,
            "pageRefreshEveryTurns": 3,
            "banks": {
                "coding": {"disabled": true, "bank": "renamed"}
            },
        })
        .to_string(),
    )
    .expect("config");
    let env = MapEnv(BTreeMap::from([
        (
            "HINDSIGHT_CONFIG".to_string(),
            path.to_string_lossy().into_owned(),
        ),
        (
            "HINDSIGHT_API_URL".to_string(),
            "http://127.0.0.1:9999/".to_string(),
        ),
    ]));
    let config = Config::load_with(&env);
    assert_eq!(config.api_url, "http://127.0.0.1:9999");
    assert!(!config.auto_reflect);
    assert_eq!(config.reflect_timeout_ms, 1234);
    assert_eq!(config.page_refresh_every_turns, 3);
    let (bank_config, bank_id) = config.for_bank("coding");
    assert_eq!(bank_id, "renamed");
    assert!(bank_config.disabled);
}

#[test]
fn daemon_mode_uses_the_configured_port() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("coding-agent.json");
    std::fs::write(
        &path,
        serde_json::json!({"serverMode": "daemon", "apiPort": 9998}).to_string(),
    )
    .expect("config");
    let env = MapEnv(BTreeMap::from([(
        "HINDSIGHT_CONFIG".to_string(),
        path.to_string_lossy().into_owned(),
    )]));
    let config = Config::load_with(&env);
    assert_eq!(config.api_url, "http://127.0.0.1:9998");
}

#[test]
fn an_absent_config_defaults_to_the_local_server() {
    let env = MapEnv(BTreeMap::new());
    let config = Config::load_with(&env);
    assert_eq!(config.api_url, super::config::DEFAULT_API_URL);
}
