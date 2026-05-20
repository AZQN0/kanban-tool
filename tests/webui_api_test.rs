#![cfg(feature = "webui")]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn kanban_bin() -> &'static str {
    env!("CARGO_BIN_EXE_kanban")
}

struct TestProject {
    path: PathBuf,
}

impl TestProject {
    fn initialized(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "kanban_webui_api_{}_{}_{}",
            std::process::id(),
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();

        let output = Command::new(kanban_bin())
            .args(["init", path.to_string_lossy().as_ref()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "init failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        Self { path }
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Server {
    child: Child,
    port: u16,
}

impl Server {
    fn start(project: &Path) -> Self {
        let port = free_port();
        let child = Command::new(kanban_bin())
            .args(["web-ui", "--bind", "127.0.0.1", "--port", &port.to_string()])
            .current_dir(project)
            .spawn()
            .unwrap();
        let server = Self { child, port };
        server.wait_until_ready();
        server
    }

    fn wait_until_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut last_error = String::new();

        while Instant::now() < deadline {
            match http_request(self.port, "GET", "/api/cards", None) {
                Ok(response) if response.status == 200 => return,
                Ok(response) => last_error = format!("unexpected status {}", response.status),
                Err(err) => last_error = err,
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        panic!("server did not become ready: {last_error}");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct HttpResponse {
    status: u16,
    body: String,
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn http_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<HttpResponse, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;

    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut raw = String::new();
    stream.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("malformed response: {raw}"))?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| format!("missing status line: {head}"))?;

    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

#[test]
fn webui_api_returns_status_codes_for_bad_input_and_missing_resources() {
    let project = TestProject::initialized("status_codes");
    let server = Server::start(&project.path);

    let bad_create = http_request(
        server.port,
        "POST",
        "/api/cards",
        Some(r#"{"title":"Bad column","column":"missing-column"}"#),
    )
    .unwrap();
    assert_eq!(bad_create.status, 400, "{}", bad_create.body);
    assert!(bad_create
        .body
        .contains("Column 'missing-column' not found"));

    let missing_get = http_request(server.port, "GET", "/api/cards/not-a-card", None).unwrap();
    assert_eq!(missing_get.status, 404, "{}", missing_get.body);
    assert!(missing_get.body.contains("Card not found"));

    let missing_move = http_request(
        server.port,
        "POST",
        "/api/cards/not-a-card/move",
        Some(r#"{"column":"todo"}"#),
    )
    .unwrap();
    assert_eq!(missing_move.status, 404, "{}", missing_move.body);
}

#[test]
fn webui_static_routes_block_traversal_and_report_missing_files() {
    let project = TestProject::initialized("static_security");
    let server = Server::start(&project.path);

    let traversal =
        http_request(server.port, "GET", "/static/%2e%2e/%2e%2e/Cargo.toml", None).unwrap();
    assert!(
        traversal.status == 403 || traversal.status == 404,
        "expected traversal to be blocked, got {} with body {}",
        traversal.status,
        traversal.body
    );
    assert!(!traversal.body.contains("[package]"));

    let missing = http_request(server.port, "GET", "/static/no-such-file.js", None).unwrap();
    assert_eq!(missing.status, 404, "{}", missing.body);
}
