//! Integration tests for config resolution.
//!
//! Tests the priority chain:
//!   CLI flag > env var > .env file > .collab.toml (local) > ~/.collab.toml (global) > default
//!
//! Each test creates a temp directory structure, writes config files as needed,
//! and runs `collab config-path` or `collab list` (which will fail to connect but
//! shows us what config was resolved via error messages).
//!
//! We use `collab roster` with --server pointed at a dead port to test resolution
//! without needing a running server — the connection error includes the resolved URL.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper: path to the built binary
fn collab_bin() -> Command {
    Command::cargo_bin("collab").expect("binary not found")
}

/// Helper: create a .env file
fn write_env(dir: &std::path::Path, contents: &str) {
    fs::write(dir.join(".env"), contents).unwrap();
}

/// Helper: create a .collab.toml file
fn write_collab_toml(dir: &std::path::Path, contents: &str) {
    fs::write(dir.join(".collab.toml"), contents).unwrap();
}

// ─── .env loading ───────────────────────────────────────────────────────────

#[test]
fn env_file_sets_collab_instance() {
    let tmp = TempDir::new().unwrap();
    write_env(tmp.path(), "COLLAB_INSTANCE=from-dotenv\nCOLLAB_SERVER=http://127.0.0.1:19999\n");

    // `collab list` should pick up COLLAB_INSTANCE from .env
    // It will fail to connect, but the error output includes the instance name
    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())  // prevent reading real ~/.collab.toml
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should NOT say "Instance ID required" — .env provided it
    assert!(
        !combined.contains("Instance ID required"),
        "Expected .env to provide COLLAB_INSTANCE, but got: {}",
        combined
    );
}

#[test]
fn env_file_does_not_override_real_env() {
    let tmp = TempDir::new().unwrap();
    write_env(tmp.path(), "COLLAB_INSTANCE=from-dotenv\nCOLLAB_SERVER=http://127.0.0.1:19999\n");

    // Real env var should take precedence over .env
    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env("COLLAB_INSTANCE", "from-real-env")
        .env("COLLAB_SERVER", "http://127.0.0.1:19998")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Connection error should reference port 19998 (from real env), not 19999 (from .env)
    assert!(
        combined.contains("19998") || combined.contains("from-real-env"),
        "Expected real env var to override .env, got: {}",
        combined
    );
}

#[test]
fn env_file_walks_up_from_cwd() {
    let tmp = TempDir::new().unwrap();
    let subdir = tmp.path().join("project").join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    // .env is in the parent, cwd is in the subdir
    write_env(tmp.path().join("project").as_ref(), "COLLAB_INSTANCE=from-parent-env\nCOLLAB_SERVER=http://127.0.0.1:19997\n");

    let out = collab_bin()
        .current_dir(&subdir)
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        !combined.contains("Instance ID required"),
        "Expected walk-up .env to provide COLLAB_INSTANCE, but got: {}",
        combined
    );
}

#[test]
fn env_file_in_home_is_loaded() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // .env in $HOME IS loaded (walk-up reaches home and finds it)
    // This differs from .collab.toml which skips $HOME to avoid double-loading
    write_env(tmp.path(), "COLLAB_INSTANCE=from-home-env\nCOLLAB_SERVER=http://127.0.0.1:19997\n");

    let out = collab_bin()
        .current_dir(&project)
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // .env in $HOME should be found and loaded
    assert!(
        !combined.contains("Instance ID required"),
        "Expected .env in $HOME to be loaded, got: {}",
        combined
    );
}

// ─── .collab.toml loading ───────────────────────────────────────────────────

#[test]
fn collab_toml_provides_instance() {
    let tmp = TempDir::new().unwrap();
    write_collab_toml(
        tmp.path(),
        "host = \"http://127.0.0.1:19996\"\ninstance = \"from-toml\"\n",
    );

    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))  // no global toml
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        !combined.contains("Instance ID required"),
        "Expected .collab.toml to provide instance, got: {}",
        combined
    );
}

#[test]
fn global_collab_toml_provides_instance() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Write global config in fake $HOME
    write_collab_toml(
        tmp.path(),
        "host = \"http://127.0.0.1:19995\"\ninstance = \"from-global-toml\"\n",
    );

    let out = collab_bin()
        .current_dir(&project)
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        !combined.contains("Instance ID required"),
        "Expected global ~/.collab.toml to provide instance, got: {}",
        combined
    );
}

#[test]
fn local_toml_overrides_global_toml() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Global: instance = global-worker, host = port 19994
    write_collab_toml(
        tmp.path(),
        "host = \"http://127.0.0.1:19994\"\ninstance = \"global-worker\"\n",
    );
    // Local: instance = local-worker, host = port 19993
    write_collab_toml(
        &project,
        "host = \"http://127.0.0.1:19993\"\ninstance = \"local-worker\"\n",
    );

    let out = collab_bin()
        .current_dir(&project)
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should use local config's port, not global
    assert!(
        !combined.contains("Instance ID required"),
        "Config resolution failed: {}",
        combined
    );
    // The connection error should show the local port
    assert!(
        combined.contains("19993"),
        "Expected local .collab.toml (port 19993) to override global (19994), got: {}",
        combined
    );
}

#[test]
fn local_toml_partial_merge_with_global() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Global has instance
    write_collab_toml(
        tmp.path(),
        "instance = \"global-worker\"\n",
    );
    // Local has only host (no instance)
    write_collab_toml(
        &project,
        "host = \"http://127.0.0.1:19992\"\n",
    );

    let out = collab_bin()
        .current_dir(&project)
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Instance from global, host from local — both should be resolved
    assert!(
        !combined.contains("Instance ID required"),
        "Expected global instance to merge with local host, got: {}",
        combined
    );
    assert!(
        combined.contains("19992"),
        "Expected local host (port 19992) to be used, got: {}",
        combined
    );
}

// ─── Priority chain: CLI > env > .env > .collab.toml ────────────────────────

#[test]
fn cli_flag_overrides_env_var() {
    let tmp = TempDir::new().unwrap();

    let out = collab_bin()
        .current_dir(tmp.path())
        .args(["--server", "http://127.0.0.1:19991", "--instance", "cli-worker", "list"])
        .env("COLLAB_INSTANCE", "env-worker")
        .env("COLLAB_SERVER", "http://127.0.0.1:19990")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should use CLI port 19991, not env port 19990
    assert!(
        combined.contains("19991"),
        "Expected CLI flag (port 19991) to override env var (19990), got: {}",
        combined
    );
}

#[test]
fn env_var_overrides_dotenv() {
    let tmp = TempDir::new().unwrap();
    write_env(tmp.path(), "COLLAB_SERVER=http://127.0.0.1:19989\nCOLLAB_INSTANCE=dotenv-worker\n");

    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env("COLLAB_INSTANCE", "env-worker")
        .env("COLLAB_SERVER", "http://127.0.0.1:19988")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should use env port 19988, not .env port 19989
    assert!(
        combined.contains("19988"),
        "Expected env var (port 19988) to override .env (19989), got: {}",
        combined
    );
}

#[test]
fn dotenv_overrides_collab_toml() {
    let tmp = TempDir::new().unwrap();
    write_env(tmp.path(), "COLLAB_SERVER=http://127.0.0.1:19987\nCOLLAB_INSTANCE=dotenv-worker\n");
    write_collab_toml(
        tmp.path(),
        "host = \"http://127.0.0.1:19986\"\ninstance = \"toml-worker\"\n",
    );

    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // .env sets env vars, which are checked before .collab.toml in the priority chain
    // So port 19987 (.env) should win over 19986 (.collab.toml)
    assert!(
        combined.contains("19987"),
        "Expected .env (port 19987) to override .collab.toml (19986), got: {}",
        combined
    );
}

#[test]
fn no_config_uses_defaults() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    let out = collab_bin()
        .current_dir(&project)
        .args(["--instance", "test-worker", "list"])
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Default server is localhost:8000
    assert!(
        combined.contains("8000") || combined.contains("localhost"),
        "Expected default server (localhost:8000) when no config present, got: {}",
        combined
    );
}

// ─── Token resolution ───────────────────────────────────────────────────────

#[test]
fn token_from_env_file() {
    let tmp = TempDir::new().unwrap();
    write_env(
        tmp.path(),
        "COLLAB_TOKEN=secret-from-dotenv\nCOLLAB_INSTANCE=test\nCOLLAB_SERVER=http://127.0.0.1:19985\n",
    );

    // We can't easily check what token was used without a server,
    // but we can verify the command runs without error about missing config
    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should not complain about missing instance (it's in .env)
    assert!(
        !combined.contains("Instance ID required"),
        "Expected .env to provide full config, got: {}",
        combined
    );
}

#[test]
fn token_from_collab_toml() {
    let tmp = TempDir::new().unwrap();
    write_collab_toml(
        tmp.path(),
        "host = \"http://127.0.0.1:19984\"\ninstance = \"test\"\ntoken = \"secret-from-toml\"\n",
    );

    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        !combined.contains("Instance ID required"),
        "Expected .collab.toml to provide instance + token, got: {}",
        combined
    );
}

// ─── Missing instance ───────────────────────────────────────────────────────

#[test]
fn missing_instance_shows_error() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = collab_bin();
    cmd.current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"));

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Instance ID required"));
}

// ─── Init manifest placement ────────────────────────────────────────────────

#[test]
fn init_creates_manifest_at_project_root() {
    let tmp = TempDir::new().unwrap();

    let yaml = r#"
server: http://127.0.0.1:19983
output_dir: ./workers
workers:
  - name: alpha
    role: "test worker"
"#;
    fs::write(tmp.path().join("workers.yaml"), yaml).unwrap();

    let out = collab_bin()
        .current_dir(tmp.path())
        .args(["init", "workers.yaml"])
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "collab init failed: {}", stdout);

    // Manifest should be at .collab/workers.json (project root), NOT workers/.collab/workers.json
    let root_manifest = tmp.path().join(".collab/workers.json");
    let wrong_manifest = tmp.path().join("workers/.collab/workers.json");

    assert!(
        root_manifest.exists(),
        "Expected manifest at {}, but it doesn't exist. Contents:\n{:?}",
        root_manifest.display(),
        fs::read_dir(tmp.path()).unwrap().map(|e| e.unwrap().path()).collect::<Vec<_>>()
    );
    assert!(
        !wrong_manifest.exists(),
        "Manifest should NOT be at {} (inside output_dir)",
        wrong_manifest.display()
    );
}

#[test]
fn init_without_output_dir_creates_manifest_at_cwd() {
    let tmp = TempDir::new().unwrap();

    let yaml = r#"
server: http://127.0.0.1:19982
workers:
  - name: beta
    role: "test worker"
"#;
    fs::write(tmp.path().join("workers.yaml"), yaml).unwrap();

    let out = collab_bin()
        .current_dir(tmp.path())
        .args(["init", "workers.yaml"])
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "collab init failed: {}", stdout);

    // When no output_dir, base is "." — manifest should be at .collab/workers.json
    let manifest = tmp.path().join(".collab/workers.json");
    assert!(
        manifest.exists(),
        "Expected manifest at {}", manifest.display()
    );
}

#[test]
fn init_manifest_contains_correct_paths() {
    let tmp = TempDir::new().unwrap();

    let yaml = r#"
server: http://127.0.0.1:19981
output_dir: ./workers
codebase_path: /tmp/myproject
workers:
  - name: gamma
    role: "gamma worker"
  - name: delta
    role: "delta worker"
"#;
    fs::write(tmp.path().join("workers.yaml"), yaml).unwrap();

    collab_bin()
        .current_dir(tmp.path())
        .args(["init", "workers.yaml"])
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .assert()
        .success();

    let manifest = fs::read_to_string(tmp.path().join(".collab/workers.json")).unwrap();
    let entries: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let arr = entries.as_array().unwrap();

    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "gamma");
    assert_eq!(arr[1]["name"], "delta");
    // output_dir should reference workers subdir, not project root
    assert!(
        arr[0]["output_dir"].as_str().unwrap().contains("workers/gamma"),
        "output_dir should reference workers/gamma, got: {}",
        arr[0]["output_dir"]
    );
}

// ─── .env quoting and comment handling ──────────────────────────────────────

#[test]
fn env_file_handles_quotes_and_comments() {
    let tmp = TempDir::new().unwrap();
    write_env(
        tmp.path(),
        "# This is a comment\n\
         COLLAB_INSTANCE=\"quoted-instance\"\n\
         COLLAB_SERVER='http://127.0.0.1:19980'\n\
         \n\
         # Another comment\n\
         COLLAB_TOKEN=unquoted-token\n",
    );

    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should resolve without "Instance ID required" — quotes are stripped
    assert!(
        !combined.contains("Instance ID required"),
        "Expected .env with quotes to work, got: {}",
        combined
    );
    // Should use the quoted server value (port 19980)
    assert!(
        combined.contains("19980"),
        "Expected quoted server from .env, got: {}",
        combined
    );
}

// ─── collab.toml ignores unknown fields gracefully ──────────────────────────

#[test]
fn collab_toml_ignores_unknown_fields() {
    let tmp = TempDir::new().unwrap();
    // Include a field that doesn't exist in the Config struct
    write_collab_toml(
        tmp.path(),
        "host = \"http://127.0.0.1:19979\"\ninstance = \"test\"\nfuture_field = \"should be ignored\"\n",
    );

    let out = collab_bin()
        .current_dir(tmp.path())
        .arg("list")
        .env_remove("COLLAB_INSTANCE")
        .env_remove("COLLAB_SERVER")
        .env_remove("COLLAB_TOKEN")
        .env("HOME", tmp.path().join("fake-home"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}{}", stdout, stderr);

    // Should not crash on unknown field
    assert!(
        !combined.contains("Instance ID required"),
        "Expected .collab.toml with unknown fields to still work, got: {}",
        combined
    );
}
