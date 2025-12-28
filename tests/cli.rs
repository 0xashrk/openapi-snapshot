use assert_cmd::Command;
use httpmock::prelude::*;
use predicates::str::contains;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

fn mock_server_with_body(body: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/openapi.json");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    server
}

#[test]
fn writes_minified_output() {
    let server = mock_server_with_body(
        r#"{"openapi":"3.0.3","paths":{"/health":{}},"components":{}}"#,
    );
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(&out_path);
    cmd.assert().success();

    let contents = fs::read_to_string(&out_path).unwrap();
    assert!(!contents.contains('\n'));
    let parsed: Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed.get("paths").is_some());
}

#[test]
fn reduces_output_to_paths_and_components() {
    let server = mock_server_with_body(
        r#"{"openapi":"3.0.3","paths":{"/health":{}},"components":{"schemas":{}},"info":{"title":"x"}}"#,
    );
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(&out_path)
        .arg("--reduce")
        .arg("paths,components");
    cmd.assert().success();

    let contents = fs::read_to_string(&out_path).unwrap();
    let parsed: Value = serde_json::from_str(&contents).unwrap();
    assert!(parsed.get("paths").is_some());
    assert!(parsed.get("components").is_some());
    assert!(parsed.get("info").is_none());
}

#[test]
fn non_200_returns_exit_code_1() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/openapi.json");
        then.status(500).body("nope");
    });
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(&out_path);
    cmd.assert().failure().code(1);
}

#[test]
fn invalid_json_returns_exit_code_2() {
    let server = mock_server_with_body("not-json");
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(&out_path);
    cmd.assert().failure().code(2);
}

#[test]
fn reduce_missing_key_returns_exit_code_3() {
    let server = mock_server_with_body(r#"{"openapi":"3.0.3","paths":{}}"#);
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(&out_path)
        .arg("--reduce")
        .arg("components");
    cmd.assert().failure().code(3);
}

#[test]
fn stdout_writes_output_without_file() {
    let server = mock_server_with_body(r#"{"openapi":"3.0.3","paths":{}}"#);
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--stdout");
    cmd.assert().success().stdout(contains("openapi"));
    assert!(!out_path.exists());
}

#[test]
fn directory_as_output_returns_exit_code_4() {
    let server = mock_server_with_body(r#"{"openapi":"3.0.3","paths":{}}"#);
    let temp = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(temp.path());
    cmd.assert().failure().code(4);
}

#[test]
fn creates_output_directory_if_missing() {
    let server = mock_server_with_body(r#"{"openapi":"3.0.3","paths":{}}"#);
    let temp = tempdir().unwrap();
    let out_path = temp.path().join("nested/dir/openapi.min.json");
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--url")
        .arg(server.url("/openapi.json"))
        .arg("--out")
        .arg(&out_path);
    cmd.assert().success();
    assert!(out_path.exists());
}


#[test]
fn help_includes_example() {
    let mut cmd = Command::cargo_bin("openapi-snapshot").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(contains("Examples:"))
        .stdout(contains("openapi-snapshot watch"));
}
