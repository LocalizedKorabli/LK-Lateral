mod config;
mod localization;
mod process;
mod registry;
mod version;

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use tauri::Emitter;
use tauri::Manager;

#[derive(Serialize, Deserialize)]
struct UpdateInfo {
    version: String,
    path: String,
}

const LATERAL_METADATA_URL: &str = "https://localizedkorabli.org/metadata/lateral/metadata.json";

#[tauri::command]
fn scan_lgc() -> Option<String> {
    registry::scan_lgc().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn scan_most() -> Option<String> {
    registry::scan_most().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn validate_lgc_path(path: String) -> bool {
    registry::validate_lgc(&path)
}

#[tauri::command]
fn validate_most_path(path: String) -> bool {
    registry::validate_most(&path)
}

#[tauri::command]
fn get_exe_version(path: String) -> Option<String> {
    version::get_exe_version(&path)
}

#[tauri::command]
async fn fetch_lgc_metadata() -> Result<localization::LgcMetadata, String> {
    let app_config = config::load_config().unwrap_or_default();
    let proxy_url = config::build_proxy_url(&app_config.proxy);
    localization::get_cached_lgc_metadata(proxy_url.as_deref()).await
}

#[tauri::command]
async fn fetch_most_metadata() -> Result<Vec<localization::MostMetadataItem>, String> {
    let app_config = config::load_config().unwrap_or_default();
    let proxy_url = config::build_proxy_url(&app_config.proxy);
    localization::get_cached_most_metadata(proxy_url.as_deref()).await
}

#[tauri::command]
fn read_cached_lgc_metadata() -> Option<localization::LgcMetadata> {
    localization::read_cached_lgc_metadata()
}

#[tauri::command]
fn read_cached_most_metadata() -> Option<Vec<localization::MostMetadataItem>> {
    localization::read_cached_most_metadata()
}

#[tauri::command]
async fn download_and_install_lgc(app: tauri::AppHandle, lgc_path: String, lang_id: Option<String>) -> Result<String, String> {
    let app_config = config::load_config().unwrap_or_default();
    let proxy_url = config::build_proxy_url(&app_config.proxy);
    localization::install_lgc(Some(&app), &lgc_path, lang_id.as_deref(), proxy_url.as_deref()).await
}

#[tauri::command]
async fn download_and_install_most(app: tauri::AppHandle, most_path: String, lang_id: String) -> Result<String, String> {
    let app_config = config::load_config().unwrap_or_default();
    let proxy_url = config::build_proxy_url(&app_config.proxy);
    localization::install_most(Some(&app), &most_path, &lang_id, proxy_url.as_deref()).await
}

#[tauri::command]
fn set_lgc_language(lgc_path: String, lang_id: String) -> Result<(), String> {
    localization::set_lgc_language(&lgc_path, &lang_id)
}

#[tauri::command]
async fn full_uninstall(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let target_dir = PathBuf::from(&path);
        localization::full_uninstall(&target_dir)
    })
    .await
    .map_err(|e| format!("Uninstall task panicked: {}", e))?
}

#[tauri::command]
fn is_app_running(install_path: String) -> bool {
    process::is_app_running(&install_path)
}

#[tauri::command]
fn force_kill_app(install_path: String) -> Result<(), String> {
    process::force_kill_app(&install_path)
}

#[tauri::command]
fn check_lgc_status(lgc_path: String) -> localization::LgcStatus {
    let exe_version = version::get_exe_version(&lgc_path).unwrap_or_default();
    localization::check_lgc_status(&lgc_path, &exe_version)
}

#[tauri::command]
fn check_most_status(most_path: String) -> localization::MostStatus {
    let exe_version = version::get_exe_version(&most_path).unwrap_or_default();
    localization::check_most_status(&most_path, &exe_version)
}

#[tauri::command]
fn get_app_config() -> Result<config::AppConfig, String> {
    config::load_config()
}

#[tauri::command]
fn save_app_config(app_config: config::AppConfig) -> Result<(), String> {
    config::save_config(&app_config)
}

#[tauri::command]
fn get_data_dir() -> String {
    config::get_data_dir().to_string_lossy().to_string()
}

#[tauri::command]
fn get_cache_size() -> String {
    let bytes = localization::get_cache_size_bytes();
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[tauri::command]
fn clear_cache() -> Result<(), String> {
    localization::clear_all_caches()
}

fn version_newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };
    let r = parse(remote);
    let l = parse(local);
    for i in 0..r.len().max(l.len()) {
        let rn = r.get(i).copied().unwrap_or(0);
        let ln = l.get(i).copied().unwrap_or(0);
        if rn > ln { return true; }
        if rn < ln { return false; }
    }
    false
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn resolve_initial_language() -> String {
    let cfg_path = config::config_path();
    if cfg_path.exists() {
        config::load_config().map(|c| c.language).unwrap_or_else(|_| String::from("zh_CN"))
    } else {
        config::system_locale_to_language()
    }
}

#[tauri::command]
fn launch_app(path: String) -> Result<(), String> {
    let exe_path = PathBuf::from(&path);
    let work_dir = exe_path.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    Command::new(&path)
        .current_dir(&work_dir)
        .spawn()
        .map_err(|e| format!("Failed to launch application: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn check_lateral_update() -> Result<Option<UpdateInfo>, String> {
    let app_config = config::load_config().unwrap_or_default();
    let proxy_url = config::build_proxy_url(&app_config.proxy);
    let client = localization::build_client(proxy_url.as_deref())?;
    let response = client
        .get(LATERAL_METADATA_URL)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch update metadata: {}", e))?;
    let info: UpdateInfo = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse update metadata: {}", e))?;
    let current = env!("CARGO_PKG_VERSION");
    if version_newer(&info.version, current) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

#[derive(Clone, Serialize)]
struct UpdateDownloadProgress {
    percent: u32,
    downloaded_bytes: u64,
    total_bytes: u64,
}

#[tauri::command]
async fn download_and_install_update(app: tauri::AppHandle, download_url: String) -> Result<(), String> {
    let app_config = config::load_config().unwrap_or_default();
    let proxy_url = config::build_proxy_url(&app_config.proxy);
    let client = localization::build_client(proxy_url.as_deref())?;
    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download update: {}", e))?;
    let total_bytes = response.content_length().unwrap_or(0);
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("LK-Lateral_Setup.exe");
    let mut file = std::fs::File::create(&installer_path)
        .map_err(|e| format!("Failed to create installer file: {}", e))?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk).map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        let percent = if total_bytes > 0 {
            ((downloaded as f64 / total_bytes as f64) * 100.0).min(99.0) as u32
        } else {
            0
        };
        let _ = app.emit("update-download-progress", UpdateDownloadProgress {
            percent,
            downloaded_bytes: downloaded,
            total_bytes: total_bytes.max(downloaded),
        });
    }
    let _ = app.emit("update-download-progress", UpdateDownloadProgress {
        percent: 100,
        downloaded_bytes: downloaded,
        total_bytes: downloaded,
    });
    drop(file);
    Command::new(&installer_path)
        .spawn()
        .map_err(|e| format!("Failed to launch installer: {}", e))?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn save_proxy_password(password: String) -> Result<(), String> {
    config::save_proxy_password(&password)
}

#[tauri::command]
fn load_proxy_password() -> Result<Option<String>, String> {
    config::load_proxy_password()
}

#[tauri::command]
fn delete_proxy_password() -> Result<(), String> {
    config::delete_proxy_password()
}

#[tauri::command]
fn set_window_theme(app: tauri::AppHandle, theme: String) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(if theme == "dark" {
            Some(tauri::Theme::Dark)
        } else {
            Some(tauri::Theme::Light)
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            if let Ok(cfg) = config::load_config() {
                if let Some(window) = app.get_webview_window("main") {
                    let theme = if cfg.theme == "dark" { Some(tauri::Theme::Dark) } else { Some(tauri::Theme::Light) };
                    let _ = window.set_theme(theme);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_lgc,
            scan_most,
            validate_lgc_path,
            validate_most_path,
            get_exe_version,
            fetch_lgc_metadata,
            fetch_most_metadata,
            read_cached_lgc_metadata,
            read_cached_most_metadata,
            download_and_install_lgc,
            download_and_install_most,
            set_lgc_language,
            is_app_running,
            force_kill_app,
            full_uninstall,
            check_lgc_status,
            check_most_status,
            get_app_config,
            save_app_config,
            get_data_dir,
            get_cache_size,
            clear_cache,
            get_app_version,
            resolve_initial_language,
            launch_app,
            check_lateral_update,
            download_and_install_update,
            save_proxy_password,
            load_proxy_password,
            delete_proxy_password,
            set_window_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
