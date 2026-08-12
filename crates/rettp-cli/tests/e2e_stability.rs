use std::{
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;
const TEST_DEADLINE: Duration = Duration::from_secs(10);
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("rettp-stability-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).expect("create temporary directory");
        Self(path)
    }

    fn write(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(&path, content).expect("write fixture");
        path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

enum ServerAction {
    Respond(Vec<u8>),
    Delay(Duration, Vec<u8>),
    Close,
    HoldUntilClientCloses,
}

struct FaultServer {
    origin: String,
    accepted: Receiver<usize>,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    worker: JoinHandle<()>,
}

impl FaultServer {
    fn serve(actions: Vec<ServerAction>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback server");
        listener
            .set_nonblocking(true)
            .expect("make loopback server bounded");
        let address = listener.local_addr().expect("server address");
        let (accepted_tx, accepted) = mpsc::channel();
        let requests = Arc::new(Mutex::new(Vec::with_capacity(actions.len())));
        let recorded = Arc::clone(&requests);
        let worker = thread::spawn(move || {
            for (index, action) in actions.into_iter().enumerate() {
                let mut stream = accept_before(&listener, Instant::now() + TEST_DEADLINE);
                stream
                    .set_read_timeout(Some(TEST_DEADLINE))
                    .expect("set read timeout");
                stream
                    .set_write_timeout(Some(TEST_DEADLINE))
                    .expect("set write timeout");
                let request = read_request(&mut stream);
                recorded.lock().expect("request lock").push(request);
                accepted_tx.send(index).expect("notify accepted request");

                match action {
                    ServerAction::Respond(bytes) => write_response(&mut stream, &bytes),
                    ServerAction::Delay(duration, bytes) => {
                        thread::sleep(duration);
                        write_response(&mut stream, &bytes);
                    }
                    ServerAction::Close => {}
                    ServerAction::HoldUntilClientCloses => {
                        let mut byte = [0_u8; 1];
                        loop {
                            match stream.read(&mut byte) {
                                Ok(0) | Err(_) => break,
                                Ok(_) => {}
                            }
                        }
                    }
                }
            }
        });
        Self {
            origin: format!("http://{address}"),
            accepted,
            requests,
            worker,
        }
    }

    fn wait_for_request(&self, expected_index: usize) {
        assert_eq!(
            self.accepted
                .recv_timeout(TEST_DEADLINE)
                .expect("request acceptance deadline"),
            expected_index
        );
    }

    fn finish(self) -> Vec<Vec<u8>> {
        self.worker.join().expect("server worker");
        Arc::try_unwrap(self.requests)
            .expect("all request owners dropped")
            .into_inner()
            .expect("request lock")
    }
}

fn accept_before(listener: &TcpListener, deadline: Instant) -> TcpStream {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for expected request"
                );
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("accept expected request: {error}"),
        }
    }
}

fn write_response(stream: &mut TcpStream, bytes: &[u8]) {
    let _ = stream.write_all(bytes);
    let _ = stream.flush();
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut expected = None;
    loop {
        let count = stream.read(&mut buffer).unwrap_or(0);
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if expected.is_none()
            && let Some(header_end) = find_bytes(&request, b"\r\n\r\n")
        {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            expected = Some(header_end + 4 + content_length);
        }
        if expected.is_some_and(|length| request.len() >= length) {
            break;
        }
    }
    request
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn response(status: u16, content_type: Option<&str>, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n",
        body.len()
    )
    .into_bytes();
    if let Some(content_type) = content_type {
        bytes.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
    }
    bytes.extend_from_slice(b"Connection: close\r\n\r\n");
    bytes.extend_from_slice(body);
    bytes
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rettp"))
}

fn run(source: &Path, base_url: &str, extra: &[&str]) -> Output {
    command()
        .arg("run")
        .arg(source)
        .arg("--base-url")
        .arg(base_url)
        .args(extra)
        .output()
        .expect("run CLI")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 stderr")
}

#[test]
fn complete_core_pipeline_capture_and_standalone_journey_passes() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "full_pipeline.rttp",
        include_bytes!("fixtures/valid/full_pipeline.rttp"),
    );
    let server = FaultServer::serve(vec![
        ServerAction::Respond(response(
            200,
            Some("application/json"),
            br#"{"token":"session-token"}"#,
        )),
        ServerAction::Respond(response(201, Some("application/json"), br#"{"id":42}"#)),
        ServerAction::Respond(response(
            200,
            Some("application/json"),
            br#"{"id":42,"name":"sample"}"#,
        )),
        ServerAction::Respond(response(204, None, b"")),
    ]);

    let output = run(&source, &server.origin, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).is_empty());
    assert!(stdout(&output).contains("[PASS]"));
    assert!(!stdout(&output).contains("session-token"));
    assert!(!stderr(&output).contains("session-token"));

    let requests = server.finish();
    assert_eq!(requests.len(), 4);
    assert!(
        String::from_utf8_lossy(&requests[1])
            .to_ascii_lowercase()
            .contains("authorization: bearer session-token")
    );
    assert!(String::from_utf8_lossy(&requests[2]).starts_with("GET /data/42 "));
    assert!(String::from_utf8_lossy(&requests[3]).starts_with("GET /health/session-token "));
}

#[test]
fn malformed_and_bounded_http_responses_are_normal_test_failures() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "response.rttp",
        r#"test "response" { request GET "/" expect { status = 200 body empty } }"#,
    );
    let declared_too_large =
        b"HTTP/1.1 200 OK\r\nContent-Length: 10485761\r\nConnection: close\r\n\r\n".to_vec();
    let cases = [
        b"not-http\r\n\r\n".to_vec(),
        response(200, Some("application/json"), b"{"),
        response(200, Some("text/plain; charset=utf-8"), &[0xff]),
        response(200, Some("application/octet-stream"), b"binary"),
        response(200, None, b"binary"),
        b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nxy".to_vec(),
        declared_too_large,
    ];

    for bytes in cases {
        let server = FaultServer::serve(vec![ServerAction::Respond(bytes)]);
        let output = run(&source, &server.origin, &[]);
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert!(stdout(&output).contains("[FAIL]"));
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn timeout_connection_refusal_and_early_close_are_classified_without_panics() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "transport.rttp",
        r#"test "transport" { request GET "/" expect { status = 200 } }"#,
    );

    let timeout_server = FaultServer::serve(vec![ServerAction::Delay(
        Duration::from_millis(100),
        response(200, None, b""),
    )]);
    let timed_out = run(&source, &timeout_server.origin, &["--timeout", "1ms"]);
    assert_eq!(timed_out.status.code(), Some(1));
    assert!(stdout(&timed_out).contains("request timed out"));
    assert_eq!(timeout_server.finish().len(), 1);

    let closed_server = FaultServer::serve(vec![ServerAction::Close]);
    let closed = run(&source, &closed_server.origin, &[]);
    assert_eq!(closed.status.code(), Some(1));
    assert_eq!(closed_server.finish().len(), 1);

    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve address");
    let address = listener.local_addr().expect("reserved address");
    drop(listener);
    let refused = run(&source, &format!("http://{address}"), &[]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(stdout(&refused).contains("connection"));
}

#[test]
fn capture_mismatch_skips_pipeline_tail_but_later_block_runs() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "capture.rttp",
        r#"pipeline "capture" {
            test "capture token" {
                request GET "/first"
                expect { status = 200 body { token: string -> TOKEN } }
            }
            test "never" {
                request GET "/never/${TOKEN}"
                expect { status = 200 }
            }
        }
        test "later" { request GET "/later" expect { status = 200 } }"#,
    );
    let server = FaultServer::serve(vec![
        ServerAction::Respond(response(200, Some("application/json"), br#"{"token":42}"#)),
        ServerAction::Respond(response(200, None, b"")),
    ]);

    let output = run(&source, &server.origin, &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("[FAIL]"));
    assert!(stdout(&output).contains("[SKIP]"));
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(String::from_utf8_lossy(&requests[1]).starts_with("GET /later "));
}

#[test]
fn core_failure_aborts_without_sending_later_requests() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "core.rttp",
        r#"core {
            test "dependency" { request GET "/core" expect { status = 201 } }
        }
        test "never" { request GET "/never" expect { status = 200 } }"#,
    );
    let server = FaultServer::serve(vec![ServerAction::Respond(response(200, None, b""))]);

    let output = run(&source, &server.origin, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).contains("[ABORT]"));
    assert!(stdout(&output).contains("[SKIP]"));
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn source_reader_enforces_utf8_and_the_five_mib_boundary_for_both_commands() {
    let directory = TemporaryDirectory::new();
    let prefix = b"test \"limit\" { request GET \"/\" expect { status = 200 } }";
    let mut exact = Vec::with_capacity(MAX_SOURCE_BYTES);
    exact.extend_from_slice(prefix);
    exact.resize(MAX_SOURCE_BYTES, b' ');
    let exact_path = directory.write("exact.rttp", exact);

    let checked = command()
        .arg("check")
        .arg(&exact_path)
        .output()
        .expect("check exact source");
    assert_eq!(checked.status.code(), Some(0), "{}", stderr(&checked));

    let oversized_path = directory.write("oversized.rttp", vec![b' '; MAX_SOURCE_BYTES + 1]);
    for subcommand in ["check", "run"] {
        let mut process = command();
        process.arg(subcommand).arg(&oversized_path);
        if subcommand == "run" {
            process.arg("--base-url").arg("http://127.0.0.1:9");
        }
        let output = process.output().expect("run oversized source command");
        assert_eq!(output.status.code(), Some(4));
        assert!(stderr(&output).contains("5242880-byte limit"));
    }

    let invalid_path = directory.write(
        "invalid-utf8.rttp",
        [0xff, b's', b'e', b'c', b'r', b'e', b't'],
    );
    let invalid = command()
        .arg("check")
        .arg(invalid_path)
        .output()
        .expect("check invalid UTF-8");
    assert_eq!(invalid.status.code(), Some(4));
    assert!(stderr(&invalid).contains("must contain valid UTF-8"));
    assert!(!stderr(&invalid).contains("secret"));
}

#[test]
fn permanent_invalid_fixtures_stop_in_the_expected_checker_phase() {
    for (fixture, phase) in [
        ("fixtures/invalid/lexical.rttp", "error[lexical]"),
        ("fixtures/invalid/syntax.rttp", "error[syntax]"),
        ("fixtures/invalid/semantic.rttp", "error[semantic]"),
    ] {
        let output = command()
            .arg("check")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(fixture),
            )
            .output()
            .expect("check regression fixture");
        assert_eq!(output.status.code(), Some(3), "{fixture}");
        assert!(stderr(&output).contains(phase), "{fixture}");
    }
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_in_flight_execution_and_preserves_artifacts() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "interrupt.rttp",
        r#"test "waiting" { request GET "/waiting" expect { status = 200 } }
        test "never" { request GET "/never" expect { status = 200 } }"#,
    );
    let json = directory.write("report.json", "existing-json");
    let junit = directory.write("report.xml", "existing-junit");
    let server = FaultServer::serve(vec![ServerAction::HoldUntilClientCloses]);

    let mut child = command()
        .arg("run")
        .arg(&source)
        .arg("--base-url")
        .arg(&server.origin)
        .arg("--json-file")
        .arg(&json)
        .arg("--junit-file")
        .arg(&junit)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interrupted CLI");
    server.wait_for_request(0);

    let signal = Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()
        .expect("send SIGINT");
    assert!(signal.success());

    let deadline = Instant::now() + TEST_DEADLINE;
    loop {
        if child.try_wait().expect("poll child").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "interrupt deadline");
        thread::sleep(Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .expect("collect interrupted output");
    assert_eq!(output.status.code(), Some(130));
    assert!(stdout(&output).is_empty());
    assert_eq!(
        stderr(&output),
        "error[interrupted]: execution interrupted by Ctrl+C\n"
    );
    assert_eq!(
        fs::read_to_string(json).expect("JSON artifact"),
        "existing-json"
    );
    assert_eq!(
        fs::read_to_string(junit).expect("JUnit artifact"),
        "existing-junit"
    );
    assert_eq!(server.finish().len(), 1);
}
