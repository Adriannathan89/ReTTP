use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    thread::{self, JoinHandle},
    time::Duration,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("rettp-cli-run-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct TestServer {
    origin: String,
    worker: JoinHandle<Vec<Vec<u8>>>,
}

impl TestServer {
    fn serve(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            responses
                .into_iter()
                .map(|response| {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let request = read_request(&mut stream);
                    if !response.delay.is_zero() {
                        thread::sleep(response.delay);
                    }
                    let _ = stream.write_all(&response.bytes);
                    let _ = stream.flush();
                    request
                })
                .collect()
        });
        Self {
            origin: format!("http://{address}"),
            worker,
        }
    }

    fn finish(self) -> Vec<Vec<u8>> {
        self.worker.join().unwrap()
    }
}

struct Response {
    bytes: Vec<u8>,
    delay: Duration,
}

impl Response {
    fn new(status: u16, headers: &[(&str, &str)], body: &str) -> Self {
        let reason = if status == 200 { "OK" } else { "Test" };
        let mut text = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in headers {
            text.push_str(name);
            text.push_str(": ");
            text.push_str(value);
            text.push_str("\r\n");
        }
        text.push_str("Connection: close\r\n\r\n");
        text.push_str(body);
        Self {
            bytes: text.into_bytes(),
            delay: Duration::ZERO,
        }
    }

    fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
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
                        .then(|| value.trim().parse::<usize>().unwrap())
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
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn basic_source(expected_status: u16) -> String {
    format!(
        r#"test "health" {{
            request GET "/health"
            expect {{ status = {expected_status} body empty }}
        }}"#
    )
}

#[test]
fn run_requires_base_url_and_rejects_invalid_cli_or_configuration_with_four() {
    let directory = TemporaryDirectory::new();
    let source = directory.write("suite.rttp", basic_source(200));

    let missing = command().arg("run").arg(&source).output().unwrap();
    assert_eq!(missing.status.code(), Some(4));
    assert!(stderr(&missing).contains("--base-url"));

    for duration in [
        "0s",
        "10",
        "ms",
        "1.5s",
        "18446744073709551616ms",
        "18446744073709551615m",
    ] {
        let invalid_duration = run(&source, "http://127.0.0.1:9", &["--timeout", duration]);
        assert_eq!(invalid_duration.status.code(), Some(4), "{duration}");
    }

    let invalid_url = run(&source, "ftp://example.test", &[]);
    assert_eq!(invalid_url.status.code(), Some(4));
    assert!(stderr(&invalid_url).contains("error[configuration]"));

    let same = directory.join("same.report");
    let same_text = same.to_str().unwrap();
    let conflicting = run(
        &source,
        "http://127.0.0.1:9",
        &["--json-file", same_text, "--junit-file", same_text],
    );
    assert_eq!(conflicting.status.code(), Some(4));
    assert!(stderr(&conflicting).contains("must use different paths"));
}

#[test]
fn checker_runs_before_http_configuration_or_network() {
    let directory = TemporaryDirectory::new();
    let invalid = directory.write("invalid.rttp", "@");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());

    let output = run(&invalid, &base_url, &[]);
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("error[lexical]"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );

    let invalid_base = run(&invalid, "ftp://not-checked-yet", &[]);
    assert_eq!(invalid_base.status.code(), Some(3));
    assert!(stderr(&invalid_base).contains("error[lexical]"));
}

#[test]
fn passed_failed_and_core_abort_use_exit_codes_zero_one_and_two() {
    let directory = TemporaryDirectory::new();

    let passed_source = directory.write("passed.rttp", basic_source(200));
    let passed_server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let passed = run(&passed_source, &passed_server.origin, &[]);
    assert_eq!(passed.status.code(), Some(0));
    assert!(stderr(&passed).is_empty());
    assert!(stdout(&passed).contains("[PASS]"));
    assert!(!stdout(&passed).contains('\x1b'));
    assert_eq!(passed_server.finish().len(), 1);

    let failed_source = directory.write("failed.rttp", basic_source(201));
    let failed_server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let failed = run(&failed_source, &failed_server.origin, &[]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(stdout(&failed).contains("[FAIL]"));
    assert!(
        stdout(&failed).contains("status did not match"),
        "{}",
        stdout(&failed)
    );
    assert_eq!(failed_server.finish().len(), 1);

    let core_source = directory.write(
        "core.rttp",
        r#"core {
            test "setup" { request GET "/core" expect { status = 201 } }
        }
        test "never" { request GET "/never" expect { status = 200 } }"#,
    );
    let core_server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let aborted = run(&core_source, &core_server.origin, &[]);
    assert_eq!(aborted.status.code(), Some(2));
    assert!(stdout(&aborted).contains("[ABORT]"));
    assert!(stdout(&aborted).contains("[SKIP]"));
    assert_eq!(core_server.finish().len(), 1);
}

#[test]
fn env_file_overrides_process_environment_and_cli_overrides_env_file() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "precedence.rttp",
        r#"test "precedence" {
            request GET "/${RETTP_CLI_PRECEDENCE_VALUE}"
            expect { status = 200 }
        }"#,
    );
    let env_file = directory.write("values.env", "RETTP_CLI_PRECEDENCE_VALUE=env-file\n");
    let env_path = env_file.to_str().unwrap();

    let env_server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let env_output = command()
        .arg("run")
        .arg(&source)
        .arg("--base-url")
        .arg(&env_server.origin)
        .arg("--env-file")
        .arg(env_path)
        .env("RETTP_CLI_PRECEDENCE_VALUE", "process")
        .output()
        .unwrap();
    assert_eq!(env_output.status.code(), Some(0));
    let env_request = env_server.finish().pop().unwrap();
    assert!(String::from_utf8_lossy(&env_request).starts_with("GET /env-file HTTP/1.1"));

    let cli_server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let cli_output = command()
        .arg("run")
        .arg(&source)
        .arg("--base-url")
        .arg(&cli_server.origin)
        .arg("--env-file")
        .arg(env_path)
        .arg("--var")
        .arg("RETTP_CLI_PRECEDENCE_VALUE=first")
        .arg("--var")
        .arg("RETTP_CLI_PRECEDENCE_VALUE=cli")
        .env("RETTP_CLI_PRECEDENCE_VALUE", "process")
        .output()
        .unwrap();
    assert_eq!(cli_output.status.code(), Some(0));
    let cli_request = cli_server.finish().pop().unwrap();
    assert!(String::from_utf8_lossy(&cli_request).starts_with("GET /cli HTTP/1.1"));
}

#[test]
fn sends_interpolated_path_header_query_and_json_body_under_base_path() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "request.rttp",
        r#"test "request" {
            request POST "/items/${ITEM}" {
                headers { "X-Token" = "Bearer ${TOKEN}" }
                query { enabled = true }
                body { name = "${ITEM}", count = 2 }
            }
            expect { status = 200 body { ok: boolean = true } }
        }"#,
    );
    let server = TestServer::serve(vec![Response::new(
        200,
        &[("Content-Type", "application/json")],
        r#"{"ok":true}"#,
    )]);
    let output = run(
        &source,
        &format!("{}/api", server.origin),
        &["--var", "ITEM=widget", "--var", "TOKEN=private"],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let request = server.finish().pop().unwrap();
    let request = String::from_utf8(request).unwrap();
    assert!(request.starts_with("POST /api/items/widget?enabled=true HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("x-token: bearer private\r\n")
    );
    assert!(
        request
            .to_ascii_lowercase()
            .contains("content-type: application/json\r\n")
    );
    assert!(
        request.ends_with(r#"{"count":2,"name":"widget"}"#),
        "{request:?}"
    );
}

#[test]
fn json_and_junit_coexist_create_parents_and_never_publish_secrets() {
    let directory = TemporaryDirectory::new();
    let source = directory.write(
        "redaction.rttp",
        r#"test "redaction" {
            request GET "/" { headers { "Authorization" = "${SECRET}" } }
            expect {
                status = 200
                headers { "X-Secret" = "${SECRET}" }
                body = "${SECRET}"
            }
        }"#,
    );
    let secret = "super-private-token-7391";
    let server = TestServer::serve(vec![Response::new(
        200,
        &[("X-Secret", secret), ("Content-Type", "text/plain")],
        "different-secret-response",
    )]);
    let json = directory.join("nested/json/report.json");
    let junit = directory.join("nested/xml/report.xml");
    let output = run(
        &source,
        &server.origin,
        &[
            "--var",
            &format!("SECRET={secret}"),
            "--json-file",
            json.to_str().unwrap(),
            "--junit-file",
            junit.to_str().unwrap(),
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(server.finish().len(), 1);

    let terminal = stdout(&output);
    let json_text = fs::read_to_string(json).unwrap();
    let junit_text = fs::read_to_string(junit).unwrap();
    for published in [&terminal, &json_text, &junit_text, &stderr(&output)] {
        assert!(!published.contains(secret));
        assert!(!published.contains("different-secret-response"));
    }
    assert!(json_text.ends_with('\n'));
    assert!(json_text.contains("\"schema_version\": 1"));
    assert!(json_text.contains("\"status\": \"failed\""));
    assert!(junit_text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(junit_text.contains("<failure type=\"assertion\""));
    assert!(terminal.contains("<redacted>"), "{terminal}");
    assert!(!terminal.contains('\x1b'));
}

#[test]
fn timeout_is_a_normal_test_failure_and_output_failure_overrides_result_with_five() {
    let directory = TemporaryDirectory::new();
    let source = directory.write("timeout.rttp", basic_source(200));
    let timeout_server = TestServer::serve(vec![
        Response::new(200, &[], "").delayed(Duration::from_millis(100)),
    ]);
    let timed_out = run(&source, &timeout_server.origin, &["--timeout", "1ms"]);
    assert_eq!(timed_out.status.code(), Some(1));
    assert!(stdout(&timed_out).contains("request timed out"));
    assert_eq!(timeout_server.finish().len(), 1);

    let output_server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let destination_directory = directory.join("cannot-replace-directory");
    fs::create_dir(&destination_directory).unwrap();
    let failed_output = run(
        &source,
        &output_server.origin,
        &["--json-file", destination_directory.to_str().unwrap()],
    );
    assert_eq!(failed_output.status.code(), Some(5));
    assert!(stderr(&failed_output).contains("error[report]"));
    assert_eq!(output_server.finish().len(), 1);
}

#[test]
fn invalid_env_files_and_cli_assignments_are_code_four_and_value_free() {
    let directory = TemporaryDirectory::new();
    let source = directory.write("valid.rttp", basic_source(200));
    let secret = "do-not-print-this-secret";
    let invalid_env = directory.write("invalid.env", format!("VALUE=\"{secret}\\q\"\n"));

    let env_output = run(
        &source,
        "http://127.0.0.1:9",
        &["--env-file", invalid_env.to_str().unwrap()],
    );
    assert_eq!(env_output.status.code(), Some(4));
    assert!(stderr(&env_output).contains("unsupported escape"));
    assert!(!stderr(&env_output).contains(secret));

    for raw in [format!("NO_EQUALS_{secret}"), format!("9BAD={secret}")] {
        let output = run(&source, "http://127.0.0.1:9", &["--var", raw.as_str()]);
        assert_eq!(output.status.code(), Some(4));
        assert!(stderr(&output).contains("invalid --var assignment 1"));
        assert!(!stderr(&output).contains(secret));
    }
}

#[test]
fn relative_report_path_uses_the_process_working_directory() {
    let directory = TemporaryDirectory::new();
    let source = directory.write("relative.rttp", basic_source(200));
    let server = TestServer::serve(vec![Response::new(200, &[], "")]);
    let output = command()
        .current_dir(&directory.0)
        .arg("run")
        .arg(&source)
        .arg("--base-url")
        .arg(&server.origin)
        .arg("--json-file")
        .arg("relative.json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(server.finish().len(), 1);
    assert!(directory.join("relative.json").is_file());
}
