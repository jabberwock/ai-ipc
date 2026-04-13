pub mod commands;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_shell::process::CommandChild;
use tokio::sync::Mutex;

// ─── State ────────────────────────────────────────────────────────────────────

pub struct AppState {
    pub server_process: Mutex<Option<CommandChild>>,
    pub server_alive: Arc<AtomicBool>,
    /// Last project directory passed to `start_server`. Used by the
    /// window-close handler to run `collab stop all` in the right place so
    /// worker daemons don't leak after the user closes the GUI.
    pub current_project_dir: Mutex<Option<PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            server_process: Mutex::new(None),
            server_alive: Arc::new(AtomicBool::new(false)),
            current_project_dir: Mutex::new(None),
        }
    }
}

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SavedConfig {
    // JS sends camelCase (`serverUrl`, `projectDir`, `setupComplete`, …)
    // and Tauri only auto-converts top-level invoke args, not nested struct
    // fields. Without `rename_all = "camelCase"` here, serde silently dropped
    // every field whose JS name didn't match the Rust field name and reset it
    // to its default on every save — which is why the config file stayed stuck
    // on its defaults no matter what the wizard did.
    #[serde(default)]
    pub token: String,
    #[serde(default = "default_server_url")]
    pub server_url: String,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub project_dir: String,
    #[serde(default)]
    pub setup_complete: bool,
    #[serde(default)]
    pub cli_template: String,
    #[serde(default)]
    pub model: String,
}

fn default_server_url() -> String {
    "http://localhost:8000".into()
}

impl Default for SavedConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            server_url: default_server_url(),
            identity: String::new(),
            project_dir: String::new(),
            setup_complete: false,
            cli_template: "claude -p {prompt} --model {model} --allowedTools Bash,Read,Write,Edit"
                .into(),
            model: "haiku".into(),
        }
    }
}

pub fn gui_config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("hold-my-beer-gui")
            .join("config.json"),
    )
}

pub fn collab_toml_path() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(PathBuf::from(home).join(".collab.toml"))
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Returns `true` if the caller should proceed with closing/exiting, `false`
/// if the user asked to stay open. A `true` return means the caller has
/// already kicked off `shutdown_session` on an async task and will then call
/// `app.exit(0)` — or, for the no-session case, nothing further is needed.
///
/// Both `WindowEvent::CloseRequested` (Cmd+W, red dot) and
/// `RunEvent::ExitRequested` (Cmd+Q on macOS, file menu quit) route through
/// this same helper so the prompt and cleanup behaviour are identical.
fn handle_quit_attempt(app: &tauri::AppHandle, origin: &str) -> bool {
    use tauri::Manager;
    eprintln!("[shutdown] quit attempt from {origin}");
    let state = app.state::<AppState>();

    let has_session = match state.current_project_dir.try_lock() {
        Ok(guard) => guard.is_some(),
        Err(_) => true,
    };
    eprintln!("[shutdown] has_session={has_session}");
    if !has_session {
        eprintln!("[shutdown] no session — allowing quit");
        return true;
    }

    let choice = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("Stop workers before quitting?")
        .set_description(
            "`collab worker` processes are still running and will keep \
             spending tokens until they're stopped.\n\n\
             Yes: stop all workers and quit.\n\
             No: cancel and keep the GUI open."
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    eprintln!("[shutdown] dialog returned: {choice:?}");

    if matches!(choice, rfd::MessageDialogResult::Yes) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            eprintln!("[shutdown] running shutdown_session");
            commands::shutdown_session(&app).await;
            eprintln!("[shutdown] shutdown_session done — calling app.exit");
            app.exit(0);
        });
        // Tell the caller to block the immediate close; our async task will
        // fire `app.exit(0)` once cleanup completes.
        false
    } else {
        eprintln!("[shutdown] user cancelled quit");
        false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::load_config,
            commands::save_config,
            commands::generate_token,
            commands::pick_directory,
            commands::write_file,
            commands::read_file,
            commands::path_exists,
            commands::home_dir,
            commands::start_server,
            commands::mark_session_active,
            commands::stop_server,
            commands::server_running,
            commands::run_command,
        ])
        .on_window_event(|window, event| {
            // Cmd+W / red dot path. On macOS Cmd+Q goes through
            // `RunEvent::ExitRequested` below instead.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle().clone();
                if !handle_quit_attempt(&app, "WindowEvent::CloseRequested") {
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Hold My Beer GUI")
        .run(|app, event| {
            // Cmd+Q / file menu "Quit" path. This is separate from
            // `WindowEvent::CloseRequested` — AppKit sends it through the
            // application delegate, bypassing window events entirely. Before
            // wiring this hook, Cmd+Q would kill the GUI instantly and leave
            // every collab worker daemon running in the background.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // `code == Some(_)` means *we* are initiating the exit (from
                // `app.exit(n)` after cleanup finished). Don't loop.
                if code.is_some() {
                    eprintln!("[shutdown] ExitRequested (self-initiated), allowing");
                    return;
                }
                if !handle_quit_attempt(app, "RunEvent::ExitRequested") {
                    api.prevent_exit();
                }
            }
        });
}
