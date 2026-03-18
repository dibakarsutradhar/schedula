use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri::Manager;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;

pub struct HubState(pub Mutex<Option<CommandChild>>);

#[derive(serde::Serialize, Clone)]
pub struct HubStatus {
    pub running: bool,
    pub port: u16,
    pub url: Option<String>,
}

const HUB_PORT: u16 = 7878;

/// License server URL passed to the hub sidecar at startup.
/// Reads `SCHEDULA_LICENSE_URL` env var so `make dev` can point to localhost
/// without rebuilding. Falls back to the production Render URL.
fn license_server_url() -> String {
    std::env::var("SCHEDULA_LICENSE_URL")
        .unwrap_or_else(|_| "https://schedula-license.onrender.com".to_string())
}

fn hub_db_path(app: &AppHandle) -> String {
    app.path()
        .app_data_dir()
        .expect("app data dir")
        .join("schedula-hub.db")
        .to_string_lossy()
        .into_owned()
}

fn local_ip() -> String {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "localhost".to_string())
}

#[tauri::command]
pub fn start_hub_mode(
    app: AppHandle,
    hub: State<'_, HubState>,
) -> Result<HubStatus, String> {
    let mut guard = hub.0.lock().map_err(|e| e.to_string())?;

    if guard.is_some() {
        return Ok(HubStatus {
            running: true,
            port: HUB_PORT,
            url: Some(format!("http://{}:{}", local_ip(), HUB_PORT)),
        });
    }

    let db_path = hub_db_path(&app);
    let port_str = HUB_PORT.to_string();

    let (_rx, child) = app
        .shell()
        .sidecar("schedula-hub")
        .map_err(|e| e.to_string())?
        .args(["--port", &port_str, "--db-path", &db_path,
               "--license-url", &license_server_url()])
        .spawn()
        .map_err(|e| e.to_string())?;

    *guard = Some(child);

    Ok(HubStatus {
        running: true,
        port: HUB_PORT,
        url: Some(format!("http://{}:{}", local_ip(), HUB_PORT)),
    })
}

#[tauri::command]
pub fn stop_hub_mode(hub: State<'_, HubState>) -> Result<(), String> {
    let mut guard = hub.0.lock().map_err(|e| e.to_string())?;
    if let Some(child) = guard.take() {
        child.kill().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_hub_status(hub: State<'_, HubState>) -> HubStatus {
    let guard = hub.0.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        HubStatus {
            running: true,
            port: HUB_PORT,
            url: Some(format!("http://{}:{}", local_ip(), HUB_PORT)),
        }
    } else {
        HubStatus {
            running: false,
            port: HUB_PORT,
            url: None,
        }
    }
}
