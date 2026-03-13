mod util;

use crate::util::dura::Dura;
use crate::util::git_repo::GitRepo;
use dura::database::RuntimeLock;

const START_TIMEOUT: u64 = 8;

#[test]
fn status_when_daemon_not_running() {
    let dura = Dura::new();
    let output = dura.run_output(&["status"]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.is_empty(), "Human mode should not write to stdout");
    assert!(
        stderr.contains("not running"),
        "Expected 'not running' on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Config:"),
        "Expected 'Config:' path on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Cache:"),
        "Expected 'Cache:' path on stderr, got: {stderr}"
    );
}

#[test]
fn status_when_daemon_running_with_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();

    let mut dura = Dura::new();
    dura.run_in_dir(&["watch"], tmp.path());
    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    let output = dura.run_output(&["status"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.is_empty(), "Human mode should not write to stdout");

    let pid = dura.pid(true).unwrap();
    assert!(
        stderr.contains(&format!("PID {pid}")),
        "Expected PID on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Config:"),
        "Expected 'Config:' path on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Cache:"),
        "Expected 'Cache:' path on stderr, got: {stderr}"
    );

    let canonical = tmp.path().canonicalize().unwrap();
    let canonical_str = canonical.to_str().unwrap();
    assert!(
        stderr.contains(canonical_str),
        "Expected repo path '{canonical_str}' on stderr, got: {stderr}"
    );
}

#[test]
fn status_stale_pid() {
    let dura = Dura::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = Some(99999);
    dura.save_runtime_lock(&runtime_lock);

    let output = dura.run_output(&["status"]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.is_empty(), "Human mode should not write to stdout");
    assert!(
        stderr.contains("not running"),
        "Expected 'not running' on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Config:"),
        "Expected 'Config:' path on stderr, got: {stderr}"
    );
}

#[test]
fn status_daemon_running_no_repos() {
    let mut dura = Dura::new();
    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    let output = dura.run_output(&["status"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.is_empty(), "Human mode should not write to stdout");
    assert!(
        stderr.contains("No repositories"),
        "Expected 'No repositories' on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("Config:"),
        "Expected 'Config:' path on stderr, got: {stderr}"
    );
}

#[test]
fn status_json_when_not_running() {
    let dura = Dura::new();
    let output = dura.run_output(&["status", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Expected valid JSON on stdout");

    assert_eq!(json["daemon"]["running"], false);
    assert_eq!(json["daemon"]["pid"], serde_json::Value::Null);
    assert!(json["repositories"].is_array());
    assert!(
        json["config_path"].is_string(),
        "Expected config_path in JSON output"
    );
    assert!(
        json["cache_path"].is_string(),
        "Expected cache_path in JSON output"
    );
}

#[test]
fn status_json_when_running_with_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();

    let mut dura = Dura::new();
    dura.run_in_dir(&["watch"], tmp.path());
    dura.start_async(&["serve"], true);
    dura.primary
        .as_ref()
        .map(|d| d.read_line(START_TIMEOUT).unwrap());

    let output = dura.run_output(&["status", "--json"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Expected valid JSON on stdout");

    assert_eq!(json["daemon"]["running"], true);
    assert!(json["daemon"]["pid"].is_number());

    let repos = json["repositories"].as_array().unwrap();
    assert!(!repos.is_empty());

    let canonical = tmp.path().canonicalize().unwrap();
    let canonical_str = canonical.to_str().unwrap();
    let has_repo = repos
        .iter()
        .any(|r| r["path"].as_str() == Some(canonical_str));
    assert!(
        has_repo,
        "Expected repo path '{canonical_str}' in JSON repositories, got: {repos:?}"
    );

    let first_repo = &repos[0];
    assert!(
        first_repo.as_object().unwrap().len() == 1,
        "Expected repo object to have only 'path' key, got: {first_repo:?}"
    );

    assert!(
        json["config_path"].is_string(),
        "Expected config_path in JSON output"
    );
    assert!(
        json["cache_path"].is_string(),
        "Expected cache_path in JSON output"
    );
}
