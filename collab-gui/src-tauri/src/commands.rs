use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::{gui_config_path, collab_toml_path, AppState, SavedConfig};

// ─── Config commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn load_config() -> SavedConfig {
    gui_config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn save_config(config: SavedConfig) -> Result<(), String> {
    if let Some(path) = gui_config_path() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
    }

    // Also write ~/.collab.toml so the collab CLI picks up token + host
    if let Some(toml_path) = collab_toml_path() {
        let toml = format!(
            "host = \"{}\"\ntoken = \"{}\"\n",
            config.server_url, config.token
        );
        std::fs::write(toml_path, toml).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| format!("{:02x}", rng.gen::<u8>()))
        .collect()
}

// ─── File system commands ─────────────────────────────────────────────────────

#[cfg(not(any(target_os = "ios", target_os = "android")))]
#[tauri::command]
pub async fn pick_directory() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose project directory")
        .pick_folder()
        .await
        .map(|h| h.path().to_string_lossy().to_string())
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    let p = Path::new(&path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn path_exists(path: String) -> bool {
    Path::new(&path).exists()
}

#[tauri::command]
pub fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

// ─── Server lifecycle ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_server(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    server_url: String,
    token: String,
    project_dir: String,
) -> Result<(), String> {
    let port: u16 = server_url
        .trim_end_matches('/')
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8000);

    {
        let mut guard = state.server_process.lock().await;
        if let Some(child) = guard.take() {
            let _ = child.kill();
        }
    }

    let cwd = if project_dir.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&project_dir)
    };

    // Remember where we're running so the window-close handler knows where
    // to run `collab stop all` on shutdown. See also `mark_session_active`,
    // which is called from the launch flow when we probe an existing server
    // and skip the local spawn — the dir still needs to be remembered so
    // Cmd+Q still warns the user about local worker processes.
    {
        let mut dir_guard = state.current_project_dir.lock().await;
        *dir_guard = Some(cwd.clone());
    }

    let sidecar = app
        .shell()
        .sidecar("collab-server")
        .map_err(|e| format!("Could not locate collab-server sidecar: {e}"))?
        .env("COLLAB_TOKEN", &token)
        .env("COLLAB_HOST", "0.0.0.0")
        .args(["--port", &port.to_string()])
        .current_dir(cwd);

    let (mut rx, child) = sidecar
        .spawn()
        .map_err(|e| format!("Could not start collab-server sidecar: {e}"))?;

    state.server_alive.store(true, Ordering::SeqCst);
    let alive = state.server_alive.clone();
    let app2 = app.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) | CommandEvent::Stderr(bytes) => {
                    let line = String::from_utf8_lossy(&bytes).to_string();
                    let _ = app2.emit("server-log", &line);
                }
                CommandEvent::Terminated(_) => {
                    alive.store(false, Ordering::SeqCst);
                    break;
                }
                _ => {}
            }
        }
    });

    let mut guard = state.server_process.lock().await;
    *guard = Some(child);
    Ok(())
}

/// Register an active session without spawning a local collab-server.
///
/// Called from the GUI launch flow when we probe an existing server at
/// `cfg.serverUrl` and skip the local spawn (e.g. a mac joiner connecting
/// to a Windows host). The project dir still needs to be remembered so
/// `handle_quit_attempt` can show the "stop workers before quitting?"
/// warning and `shutdown_session` can run `collab stop all` on the
/// local worker daemons this GUI manages.
#[tauri::command]
pub async fn mark_session_active(
    state: tauri::State<'_, AppState>,
    project_dir: String,
) -> Result<(), String> {
    let cwd = if project_dir.is_empty() {
        std::env::current_dir().map_err(|e| e.to_string())?
    } else {
        PathBuf::from(&project_dir)
    };
    let mut guard = state.current_project_dir.lock().await;
    *guard = Some(cwd);
    Ok(())
}

#[tauri::command]
pub async fn stop_server(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.server_process.lock().await;
    if let Some(child) = guard.take() {
        child.kill().map_err(|e| e.to_string())?;
    }
    state.server_alive.store(false, Ordering::SeqCst);
    Ok(())
}

/// Best-effort cleanup called from the window-close handler.
///
/// Steps:
///   1. If we know the project dir, run `collab stop all` via sidecar so
///      every worker daemon tracked in the lifecycle manifest receives
///      SIGTERM. This is the same command a user would type to clean up.
///   2. Kill the embedded `collab-server` child we spawned.
///
/// Both steps are best-effort — a failure in step 1 must not block step 2,
/// and a failure in step 2 must not block process exit.
pub async fn shutdown_session(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();

    // Step 1: stop worker daemons via the CLI.
    let project_dir = {
        let guard = state.current_project_dir.lock().await;
        guard.clone()
    };
    if let Some(dir) = project_dir {
        if let Ok(cmd) = app.shell().sidecar("collab") {
            let spawn_result = cmd.args(["stop", "all"]).current_dir(dir).spawn();
            if let Ok((mut rx, _child)) = spawn_result {
                // Wait up to 5s for Terminated so we don't hang exit forever.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(remaining, rx.recv()).await {
                        Ok(Some(CommandEvent::Terminated(_))) => break,
                        Ok(Some(_)) => continue,
                        Ok(None) => break,
                        Err(_) => break, // timeout
                    }
                }
            }
        }
    }

    // Step 2: kill the embedded server.
    let mut guard = state.server_process.lock().await;
    if let Some(child) = guard.take() {
        let _ = child.kill();
    }
    state.server_alive.store(false, Ordering::SeqCst);
}

#[tauri::command]
pub async fn server_running(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    if !state.server_alive.load(Ordering::SeqCst) {
        let mut guard = state.server_process.lock().await;
        *guard = None;
        return Ok(false);
    }
    Ok(true)
}

// ─── Generic command runner ───────────────────────────────────────────────────

#[tauri::command]
pub async fn run_command(
    app: tauri::AppHandle,
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    envs: Vec<(String, String)>,
) -> Result<i32, String> {
    // Route bundled binaries through the sidecar API; everything else is rejected
    // (the GUI should only ever invoke `collab` / `collab-server` from here).
    let sidecar_name = match program.as_str() {
        "collab" | "collab-server" => program.as_str(),
        other => return Err(format!("run_command: `{other}` is not an allowed sidecar")),
    };

    let mut cmd = app
        .shell()
        .sidecar(sidecar_name)
        .map_err(|e| format!("Could not locate `{sidecar_name}` sidecar: {e}"))?
        .args(&args);
    if let Some(dir) = &cwd {
        cmd = cmd.current_dir(PathBuf::from(dir));
    }
    for (k, v) in &envs {
        cmd = cmd.env(k, v);
    }

    let (mut rx, _child) = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn `{sidecar_name}`: {e}"))?;

    let mut exit_code: i32 = -1;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let line = String::from_utf8_lossy(&bytes).to_string();
                let _ = app.emit(
                    "cmd-output",
                    serde_json::json!({ "stream": "out", "line": line }),
                );
            }
            CommandEvent::Stderr(bytes) => {
                let line = String::from_utf8_lossy(&bytes).to_string();
                let _ = app.emit(
                    "cmd-output",
                    serde_json::json!({ "stream": "err", "line": line }),
                );
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code.unwrap_or(-1);
                break;
            }
            _ => {}
        }
    }
    Ok(exit_code)
}
