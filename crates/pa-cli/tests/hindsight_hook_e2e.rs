//! End-to-end coverage for the hidden `hindsight-hook` entry: the real
//! binary, a local one-shot HTTP server, and a temp agent directory. The
//! tests pin the stdout contract (JSON only) and the fail-open exit code.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

fn read_request(stream: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        let read = stream.read(&mut chunk).expect("read request");
        assert!(read > 0, "client closed before headers");
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
        })
        .unwrap_or(0);
    while buffer.len() < header_end + content_length {
        let mut chunk = [0u8; 4096];
        let read = stream.read(&mut chunk).expect("read body");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    headers
}

fn respond(stream: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
    let _ = stream.flush();
}

/// Serve up to `expected` requests without ever blocking the test on an
/// accept that never comes (the hook may fail open before sending).
fn serve(
    listener: TcpListener,
    expected: usize,
    responder: fn(&str) -> &'static str,
    stop: Arc<AtomicBool>,
) -> (JoinHandle<()>, Arc<Mutex<Vec<String>>>) {
    let paths = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&paths);
    listener.set_nonblocking(true).expect("nonblocking");
    let handle = std::thread::spawn(move || {
        let mut served = 0;
        while served < expected && !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).expect("blocking");
                    let headers = read_request(&mut stream);
                    recorded.lock().expect("paths").push(headers.clone());
                    respond(&mut stream, responder(&headers));
                    served += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return,
            }
        }
    });
    (handle, paths)
}

fn config_file(dir: &Path, url: &str, bank: &str) -> std::path::PathBuf {
    let path = dir.join("coding-agent.json");
    std::fs::write(
        &path,
        serde_json::json!({
            "apiUrl": url,
            "bankId": bank,
            "reflectTimeoutMs": 2000,
        })
        .to_string(),
    )
    .expect("config");
    path
}

fn run_hook(kind: &str, payload: &str, agent_dir: &Path, config: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prime-agent"))
        .args(["hindsight-hook", kind])
        .env("PRIME_AGENT_CODING_AGENT_DIR", agent_dir)
        .env("HINDSIGHT_CONFIG", config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn prime-agent");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("stdin write");
    child.wait_with_output().expect("wait")
}

fn prompt_responder(headers: &str) -> &'static str {
    if headers.contains("/reflect") {
        r#"{"text":"E2E MEMORY"}"#
    } else {
        r#"{"roots":[]}"#
    }
}

#[test]
fn the_prompt_entry_prints_one_json_object_and_nothing_else() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let stop = Arc::new(AtomicBool::new(false));
    let (server, _) = serve(listener, 2, prompt_responder, Arc::clone(&stop));

    let temp = tempfile::tempdir().expect("temp");
    let agent_dir = temp.path().join("agent");
    std::fs::create_dir_all(&agent_dir).expect("agent dir");
    let config = config_file(
        temp.path(),
        &format!("http://{address}"),
        "coding-agent::e2e",
    );
    let payload = serde_json::json!({
        "prompt": "what did we decide?",
        "cwd": temp.path(),
        "session_id": "e2e-session",
    })
    .to_string();
    let output = run_hook("prompt", &payload, &agent_dir, &config);
    stop.store(true, Ordering::Relaxed);
    server.join().expect("server");

    assert!(output.status.success(), "hook exits 0");
    assert!(output.stderr.is_empty(), "hook keeps stderr clean");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("one JSON object");
    let context = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additionalContext");
    assert!(context.contains("<hindsight_memory>"));
    assert!(context.contains("E2E MEMORY"));
}

#[test]
fn the_stop_entry_is_silent_and_fail_open() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let stop = Arc::new(AtomicBool::new(false));
    let (server, paths) = serve(listener, 1, |_| "{}", Arc::clone(&stop));

    let temp = tempfile::tempdir().expect("temp");
    let agent_dir = temp.path().join("agent");
    std::fs::create_dir_all(&agent_dir).expect("agent dir");
    let config = config_file(
        temp.path(),
        &format!("http://{address}"),
        "coding-agent::e2e",
    );
    let transcript = temp.path().join("transcript.jsonl");
    std::fs::write(
        &transcript,
        format!(
            "{}\n",
            serde_json::json!({
                "schema_version": 1,
                "role": "user",
                "content": [{"type": "text", "text": "remember this"}],
            })
        ),
    )
    .expect("transcript");
    let payload = serde_json::json!({
        "session_id": "e2e-session",
        "transcript_path": transcript,
        "cwd": temp.path(),
    })
    .to_string();
    let output = run_hook("stop", &payload, &agent_dir, &config);
    stop.store(true, Ordering::Relaxed);
    server.join().expect("server");

    assert!(output.status.success(), "stop hook exits 0");
    assert!(output.stdout.is_empty(), "stop hook writes no stdout");
    let paths = paths.lock().expect("paths");
    assert!(
        paths.iter().any(|headers| headers.contains("/memories")),
        "the write-back request was sent: {paths:?}"
    );
}
