use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

struct TemporaryFile {
    path: PathBuf,
}

impl TemporaryFile {
    fn text(name: &str, content: &str) -> Self {
        Self::bytes(name, content.as_bytes())
    }

    fn bytes(name: &str, content: &[u8]) -> Self {
        let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("utest-cli-integration-{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let path = directory.join(name);
        fs::write(&path, content).expect("temporary fixture should be written");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

fn utest() -> Command {
    Command::new(env!("CARGO_BIN_EXE_utest"))
}

fn run(arguments: &[&str]) -> Output {
    utest()
        .args(arguments)
        .output()
        .expect("utest binary should run")
}

fn run_check(path: &Path, arguments: &[&str]) -> Output {
    utest()
        .arg("check")
        .arg(path)
        .args(arguments)
        .output()
        .expect("utest check should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

#[test]
fn top_level_help_and_version_are_successful() {
    let help = run(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(stdout(&help).contains("Usage: utest <COMMAND>"));
    assert!(stdout(&help).contains("check"));
    assert!(stderr(&help).is_empty());

    let version = run(&["--version"]);
    assert_eq!(version.status.code(), Some(0));
    assert_eq!(stdout(&version), "utest 0.1.0\n");
    assert!(stderr(&version).is_empty());
}

#[test]
fn clap_usage_errors_exit_with_code_two() {
    for arguments in [Vec::<&str>::new(), vec!["unknown"]] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains("Usage:"));
    }
}

#[test]
fn bundled_basic_example_is_valid_and_reports_its_path() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/basic.utest");
    let output = run_check(&path, &[]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), format!("{}: valid\n", path.display()));
    assert!(stderr(&output).is_empty());
}

#[test]
fn bundled_interpolation_example_accepts_repeated_var_options() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/interpolation.utest");
    let output = run_check(
        &path,
        &["--var", "id=42", "--var", "interpolated_string=ready"],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), format!("{}: valid\n", path.display()));
    assert!(stderr(&output).is_empty());
}

#[test]
fn duplicate_var_names_are_accepted_as_one_predefined_name() {
    let source = TemporaryFile::text(
        "duplicate-var.utest",
        r#"test "duplicate" { request GET "/${DUPLICATE_CLI_VAR}" expect {} }"#,
    );
    let output = run_check(
        source.path(),
        &[
            "--var",
            "DUPLICATE_CLI_VAR=first",
            "--var",
            "DUPLICATE_CLI_VAR=second",
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(stderr(&output).is_empty());
}

#[test]
fn predefined_cli_variable_cannot_be_redeclared_as_a_capture() {
    let source = TemporaryFile::text(
        "capture-collision.utest",
        r#"test "capture" {
            request GET "/"
            expect { body { id: integer -> PREDEFINED } }
        }"#,
    );
    let secret = "must-not-appear-in-diagnostics";
    let output = run_check(source.path(), &["--var", &format!("PREDEFINED={secret}")]);

    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("variable `PREDEFINED` is already defined"));
    assert!(!stderr(&output).contains(secret));
}

#[test]
fn missing_and_invalid_var_arguments_are_clap_errors() {
    let source = TemporaryFile::text(
        "variable-errors.utest",
        r#"test "valid" { request GET "/" expect {} }"#,
    );

    for arguments in [
        vec!["--var"],
        vec!["--var", "NO_EQUALS"],
        vec!["--var", "9BAD=value"],
    ] {
        let output = run_check(source.path(), &arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains("error:"));
    }
}

#[test]
fn an_undefined_cli_variable_is_a_semantic_error() {
    let variable = format!(
        "UTEST_CLI_MISSING_{}_{}",
        std::process::id(),
        NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
    );
    let source = TemporaryFile::text(
        "undefined.utest",
        &format!(r#"test "missing" {{ request GET "/${{{variable}}}" expect {{}} }}"#),
    );
    let output = run_check(source.path(), &[]);

    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains(&format!("error[semantic]: undefined variable `{variable}`")));
}

#[test]
fn environment_variable_names_are_available_to_interpolation() {
    let variable = format!(
        "UTEST_CLI_ENV_{}_{}",
        std::process::id(),
        NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
    );
    let source = TemporaryFile::text(
        "environment.utest",
        &format!(r#"test "environment" {{ request GET "/${{{variable}}}" expect {{}} }}"#),
    );
    let output = utest()
        .arg("check")
        .arg(source.path())
        .env(&variable, "present")
        .output()
        .expect("utest check should run");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        format!("{}: valid\n", source.path().display())
    );
    assert!(stderr(&output).is_empty());
}

#[test]
fn lexical_diagnostic_has_phase_path_and_exact_location() {
    let source = TemporaryFile::text("lexical.utest", "@");
    let output = run_check(source.path(), &[]);

    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert_eq!(
        stderr(&output),
        format!(
            "{}:1:1: error[lexical]: unexpected character `@`\n",
            source.path().display()
        )
    );
}

#[test]
fn syntax_diagnostics_have_phase_path_and_exact_location() {
    let source = TemporaryFile::text("syntax.utest", r#"test "bad" {}"#);
    let output = run_check(source.path(), &[]);

    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert_eq!(
        stderr(&output),
        format!(
            concat!(
                "{}:1:13: error[syntax]: a test must contain at least one request\n",
                "{}:1:13: error[syntax]: a test must contain at least one expectation\n"
            ),
            source.path().display(),
            source.path().display()
        )
    );
}

#[test]
fn semantic_diagnostic_has_phase_path_and_exact_location() {
    let source = TemporaryFile::text(
        "semantic.utest",
        r#"test "bad" { request GET "/" expect { status = 99 } }"#,
    );
    let output = run_check(source.path(), &[]);

    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert_eq!(
        stderr(&output),
        format!(
            "{}:1:48: error[semantic]: HTTP status code 99 is outside 100..=599\n",
            source.path().display()
        )
    );
}

#[test]
fn missing_file_and_invalid_utf8_are_io_errors() {
    let missing_path = std::env::temp_dir().join(format!(
        "utest-cli-definitely-missing-{}-{}.utest",
        std::process::id(),
        NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let missing = run_check(&missing_path, &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stdout(&missing).is_empty());
    assert!(stderr(&missing).starts_with(&format!("{}: error[io]:", missing_path.display())));

    let invalid = TemporaryFile::bytes("invalid-utf8.utest", &[0x66, 0x80, 0x6f]);
    let invalid_output = run_check(invalid.path(), &[]);
    assert_eq!(invalid_output.status.code(), Some(1));
    assert!(stdout(&invalid_output).is_empty());
    assert!(
        stderr(&invalid_output).starts_with(&format!("{}: error[io]:", invalid.path().display()))
    );
    assert!(stderr(&invalid_output).contains("UTF-8"));
}
