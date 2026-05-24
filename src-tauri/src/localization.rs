use chrono::Local;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use uuid::Uuid;

use crate::version;

const METADATA_BASE_URL: &str = "https://localizedkorabli.org";
const LGC_METADATA_URL: &str = "https://localizedkorabli.org/metadata/lgc/l10n.json";
const MOST_METADATA_URL: &str = "https://localizedkorabli.org/metadata/most/l10n.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LgcLanguage {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LgcMetadata {
    pub path: String,
    pub version: String,
    pub supported_lgc_version: String,
    #[serde(default)]
    pub supported_languages: Vec<LgcLanguage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MostMetadataItem {
    pub id: String,
    pub name: String,
    pub l10n_app: L10nPackage,
    pub l10n_mods: L10nPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L10nPackage {
    pub path: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_most_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstInfo {
    pub language: String,
    pub date: String,
    pub loc_version: String,
    pub loc_mods_version: String,
    pub supported_version: String,
    pub app_version: String,
    pub files: Vec<FileEntry>,
    pub backup_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload {
    pub step: String,
    pub percent: u32,
    pub message: String,
    pub message_key: String,
    pub message_params: HashMap<String, String>,
    pub instance: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LgcStatus {
    pub path: String,
    pub version: String,
    pub loc_installed: bool,
    pub loc_version: String,
    pub loc_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MostStatus {
    pub path: String,
    pub version: String,
    pub loc_installed: bool,
    pub loc_app_version: String,
    pub loc_mods_version: String,
    pub loc_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EtagEntry {
    etag: String,
    cached_file: String,
}

type EtagCache = HashMap<String, EtagEntry>;

fn etag_cache_path() -> PathBuf {
    get_app_data_dir().join("etag_cache.json")
}

fn load_etag_cache() -> EtagCache {
    let path = etag_cache_path();
    if !path.exists() {
        return HashMap::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_etag_cache(cache: &EtagCache) {
    let path = etag_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(&path, content);
    }
}

fn lgc_metadata_cache_path() -> PathBuf {
    get_app_data_dir().join("metadata_cache").join("lgc_metadata.json")
}

fn most_metadata_cache_path() -> PathBuf {
    get_app_data_dir().join("metadata_cache").join("most_metadata.json")
}

fn load_cached_lgc_metadata() -> Option<LgcMetadata> {
    let path = lgc_metadata_cache_path();
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save_lgc_metadata(metadata: &LgcMetadata) {
    let path = lgc_metadata_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(metadata) {
        let _ = std::fs::write(&path, content);
    }
}

fn load_cached_most_metadata() -> Option<Vec<MostMetadataItem>> {
    let path = most_metadata_cache_path();
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save_most_metadata(metadata: &Vec<MostMetadataItem>) {
    let path = most_metadata_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(metadata) {
        let _ = std::fs::write(&path, content);
    }
}

fn is_in_download_cache(path: &Path) -> bool {
    let cache_dir = get_app_data_dir().join("download_cache");
    path.starts_with(&cache_dir)
}

fn compute_sha256(file_path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(file_path).map_err(|e| format!("Failed to open file for hash: {}", e))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| format!("Failed to hash file: {}", e))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn resolve_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}/{}", METADATA_BASE_URL, path.trim_start_matches('/'))
    }
}

pub fn build_client(proxy_url: Option<&str>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder();
    if let Some(url) = proxy_url {
        let proxy = reqwest::Proxy::all(url).map_err(|e| format!("Invalid proxy URL: {}", e))?;
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|e| format!("Failed to build HTTP client: {}", e))
}

pub async fn fetch_lgc_metadata(proxy_url: Option<&str>) -> Result<LgcMetadata, String> {
    let client = build_client(proxy_url)?;
    let response = client
        .get(LGC_METADATA_URL)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch LGC metadata: {}", e))?;
    let metadata: LgcMetadata = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LGC metadata: {}", e))?;
    save_lgc_metadata(&metadata);
    Ok(metadata)
}

pub async fn fetch_most_metadata(
    proxy_url: Option<&str>,
) -> Result<Vec<MostMetadataItem>, String> {
    let client = build_client(proxy_url)?;
    let response = client
        .get(MOST_METADATA_URL)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Most metadata: {}", e))?;
    let metadata: Vec<MostMetadataItem> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Most metadata: {}", e))?;
    save_most_metadata(&metadata);
    Ok(metadata)
}

pub async fn get_cached_lgc_metadata(proxy_url: Option<&str>) -> Result<LgcMetadata, String> {
    match fetch_lgc_metadata(proxy_url).await {
        Ok(m) => return Ok(m),
        Err(e) => {
            if let Some(cached) = load_cached_lgc_metadata() {
                return Ok(cached);
            }
            return Err(e);
        }
    }
}

pub async fn get_cached_most_metadata(
    proxy_url: Option<&str>,
) -> Result<Vec<MostMetadataItem>, String> {
    match fetch_most_metadata(proxy_url).await {
        Ok(m) => return Ok(m),
        Err(e) => {
            if let Some(cached) = load_cached_most_metadata() {
                return Ok(cached);
            }
            return Err(e);
        }
    }
}

pub fn read_cached_lgc_metadata() -> Option<LgcMetadata> {
    load_cached_lgc_metadata()
}

pub fn read_cached_most_metadata() -> Option<Vec<MostMetadataItem>> {
    load_cached_most_metadata()
}

struct ProgressEmitter<'a> {
    app_handle: Option<&'a tauri::AppHandle>,
    instance: &'a str,
}

impl<'a> ProgressEmitter<'a> {
    fn new(app_handle: Option<&'a tauri::AppHandle>, instance: &'a str) -> Self {
        Self { app_handle, instance }
    }

    fn emit(
        &self,
        step: &str,
        percent: u32,
        message: &str,
        message_key: &str,
        message_params: &[(&str, &str)],
        downloaded_bytes: u64,
        total_bytes: u64,
    ) {
        if let Some(handle) = self.app_handle {
            let params: HashMap<String, String> = message_params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let _ = handle.emit("install-progress", ProgressPayload {
                step: step.to_string(),
                percent,
                message: message.to_string(),
                message_key: message_key.to_string(),
                message_params: params,
                instance: self.instance.to_string(),
                downloaded_bytes,
                total_bytes,
            });
        }
    }
}

async fn download_file_with_etag(
    url: &str,
    proxy_url: Option<&str>,
    emitter: &ProgressEmitter<'_>,
) -> Result<PathBuf, String> {
    let resolved = resolve_url(url);
    let mut cache = load_etag_cache();

    if let Some(entry) = cache.get(&resolved) {
        let cached_path = PathBuf::from(&entry.cached_file);
        if cached_path.exists() {
            if entry.etag.is_empty() {
                emitter.emit("downloading", 80, "Using cached file", "progress.cached", &[], 0, 0);
                return Ok(cached_path);
            }
            let client = match build_client(proxy_url) {
                Ok(c) => c,
                Err(_) => {
                    emitter.emit("downloading", 80, "Using cached file (offline)", "progress.cached", &[], 0, 0);
                    return Ok(cached_path);
                }
            };
            emitter.emit("downloading", 0, "Connecting...", "progress.connecting", &[], 0, 0);
            let response = match client
                .get(&resolved)
                .header("If-None-Match", &entry.etag)
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => {
                    emitter.emit("downloading", 80, "Using cached file (offline)", "progress.cached", &[], 0, 0);
                    return Ok(cached_path);
                }
            };
            if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                emitter.emit("downloading", 80, "Using cached file (304 Not Modified)", "progress.cached", &[], 0, 0);
                return Ok(cached_path);
            }
        }
    }

    let client = build_client(proxy_url)?;
    emitter.emit("downloading", 0, "Connecting...", "progress.connecting", &[], 0, 0);

    let response = client
        .get(&resolved)
        .send()
        .await
        .map_err(|e| format!("Failed to download from {}: {}", resolved, e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let etag = response.headers().get("etag").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let total_size = response.content_length().unwrap_or(0);

    let mut downloaded: u64 = 0;
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        bytes.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;
        if total_size > 0 {
            let pct = 5 + ((downloaded as f64 / total_size as f64) * 75.0) as u32;
            let dl = format!("{:.1} MB", downloaded as f64 / 1_048_576.0);
            let tl = format!("{:.1} MB", total_size as f64 / 1_048_576.0);
            emitter.emit("downloading", pct, &format!("Downloading... {} / {}", dl, tl), "progress.downloading", &[("downloaded", dl.as_str()), ("total", tl.as_str())], downloaded, total_size);
        } else {
            let dl = format!("{:.1} MB", downloaded as f64 / 1_048_576.0);
            emitter.emit("downloading", 10, &format!("Downloading... {}", dl), "progress.downloading_nosize", &[("downloaded", dl.as_str())], downloaded, 0);
        }
    }

    let ext = if resolved.ends_with(".7z") { "7z" } else { "7z" };
    let temp_path = std::env::temp_dir().join(format!("lk_lateral_{}.{}", Uuid::new_v4(), ext));
    std::fs::write(&temp_path, &bytes)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    let cache_dir = get_app_data_dir().join("download_cache");
    let _ = std::fs::create_dir_all(&cache_dir);
    let cached = cache_dir.join(format!("{}.7z", Uuid::new_v4()));
    let _ = std::fs::copy(&temp_path, &cached);
    cache.insert(resolved.clone(), EtagEntry {
        etag: etag.unwrap_or_default(),
        cached_file: cached.to_string_lossy().to_string(),
    });
    save_etag_cache(&cache);

    Ok(temp_path)
}

pub fn extract_7z(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create extract directory: {}", e))?;
    sevenz_rust::decompress_file(src, dst)
        .map_err(|e| format!("Failed to extract 7z archive: {}", e))
}

fn walk_files(base: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    walk_files_recursive(base, base, &mut files)?;
    Ok(files)
}

fn walk_files_recursive(
    base: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(current)
        .map_err(|e| format!("Failed to read directory {:?}: {}", current, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            walk_files_recursive(base, &path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn key_for_dir(target_dir: &Path) -> String {
    if let Ok(canonical) = target_dir.canonicalize() {
        canonical.to_string_lossy().to_lowercase()
    } else {
        target_dir.to_string_lossy().to_lowercase().trim_end_matches(&['\\', '/'][..]).to_string()
    }
}

fn inst_info_path(target_dir: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(key_for_dir(target_dir).as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    get_app_data_dir().join("instances").join(&hash[..16]).join("inst_info.json")
}

fn get_app_data_dir() -> PathBuf {
    if let Some(dir) = dirs::data_dir() {
        dir.join("lk-lateral")
    } else {
        PathBuf::from(".").join("data")
    }
}

pub fn clear_all_caches() -> Result<(), String> {
    let data_dir = get_app_data_dir();

    let download_cache = data_dir.join("download_cache");
    if download_cache.exists() {
        std::fs::remove_dir_all(&download_cache)
            .map_err(|e| format!("Failed to clear download cache: {}", e))?;
    }

    let metadata_cache = data_dir.join("metadata_cache");
    if metadata_cache.exists() {
        std::fs::remove_dir_all(&metadata_cache)
            .map_err(|e| format!("Failed to clear metadata cache: {}", e))?;
    }

    let etag_path = etag_cache_path();
    if etag_path.exists() {
        std::fs::remove_file(&etag_path)
            .map_err(|e| format!("Failed to clear etag cache: {}", e))?;
    }

    Ok(())
}

pub fn get_cache_size_bytes() -> u64 {
    let data_dir = get_app_data_dir();
    let mut total = 0u64;

    let download_cache = data_dir.join("download_cache");
    if download_cache.exists() {
        total += dir_size(&download_cache);
    }

    let metadata_cache = data_dir.join("metadata_cache");
    if metadata_cache.exists() {
        total += dir_size(&metadata_cache);
    }

    total
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn backups_base_dir(target_dir: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(key_for_dir(target_dir).as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    get_app_data_dir().join("backups").join(&hash[..16])
}

fn backups_files_dir(target_dir: &Path) -> PathBuf {
    backups_base_dir(target_dir).join("original").join("files")
}

fn read_current_localization(lgc_path: &str) -> Option<String> {
    let prefs_path = PathBuf::from(lgc_path).join("preferences.xml");
    let content = std::fs::read_to_string(&prefs_path).ok()?;

    let l10n_marker = "<localization_manager>";
    let l10n_pos = content.find(l10n_marker)?;
    let after_l10n = &content[l10n_pos + l10n_marker.len()..];

    let tag = "<current_localization>";
    let close = "</current_localization>";
    let tag_start = after_l10n.find(tag)?;
    let tag_end = tag_start + tag.len();
    let close_start = after_l10n[tag_end..].find(close)?;

    let abs_start = l10n_pos + l10n_marker.len() + tag_end;
    let abs_end = abs_start + close_start;

    Some(content[abs_start..abs_end].to_string())
}

fn write_current_localization(lgc_path: &str, lang_id: &str) -> Result<(), String> {
    let prefs_path = PathBuf::from(lgc_path).join("preferences.xml");
    if !prefs_path.exists() {
        return Err("preferences.xml not found".to_string());
    }
    let content = std::fs::read_to_string(&prefs_path)
        .map_err(|e| format!("Failed to read preferences.xml: {}", e))?;

    let l10n_marker = "<localization_manager>";
    let Some(l10n_pos) = content.find(l10n_marker) else {
        return Err("localization_manager section not found".to_string());
    };
    let after_l10n = &content[l10n_pos + l10n_marker.len()..];

    let tag = "<current_localization>";
    let close = "</current_localization>";
    let Some(tag_start) = after_l10n.find(tag) else {
        return Err("current_localization tag not found".to_string());
    };
    let tag_end = tag_start + tag.len();
    let Some(close_start) = after_l10n[tag_end..].find(close) else {
        return Err("current_localization closing tag not found".to_string());
    };

    let abs_start = l10n_pos + l10n_marker.len() + tag_end;
    let abs_end = abs_start + close_start;

    let new_content = format!(
        "{}{}{}",
        &content[..abs_start],
        lang_id.to_lowercase(),
        &content[abs_end..]
    );

    std::fs::write(&prefs_path, &new_content)
        .map_err(|e| format!("Failed to write preferences.xml: {}", e))
}

pub fn set_lgc_language(lgc_path: &str, lang_id: &str) -> Result<(), String> {
    write_current_localization(lgc_path, lang_id)
}

pub fn check_installed(target_dir: &Path) -> Option<InstInfo> {
    let path = inst_info_path(target_dir);
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let info: InstInfo = serde_json::from_str(&content).ok()?;
    if info.files.is_empty() || info.backup_dir.is_empty() {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    if !target_dir.exists() {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let all_files_valid = info.files.iter().all(|f| {
        if f.path.is_empty() || f.sha256.is_empty() {
            return false;
        }
        let target = target_dir.join(&f.path);
        target.exists() && compute_sha256(&target).is_ok_and(|h| h == f.sha256)
    });
    if !all_files_valid {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(info)
}

pub async fn install_lgc(
    app_handle: Option<&tauri::AppHandle>,
    lgc_path: &str,
    lang_id: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<String, String> {
    let emitter = ProgressEmitter::new(app_handle, "lgc");
    let target_dir = PathBuf::from(lgc_path);
    if !target_dir.exists() {
        return Err("LGC directory does not exist".to_string());
    }

    let metadata = fetch_lgc_metadata(proxy_url).await?;
    let resolved_url = resolve_url(&metadata.path);

    let temp_7z = download_file_with_etag(&resolved_url, proxy_url, &emitter).await?;
    let temp_extract = std::env::temp_dir().join(format!("lk_lateral_extract_{}", Uuid::new_v4()));

    let language = if let Some(lang) = lang_id {
        if lang.is_empty() { "zh_CN" } else { lang }
    } else {
        "zh_CN"
    };

    let app_version = version::get_exe_version(lgc_path).unwrap_or_default();

    let result = create_node_and_install(
        &emitter,
        &target_dir,
        &temp_7z,
        &temp_extract,
        language,
        &metadata.version,
        &metadata.supported_lgc_version,
        &app_version,
    );

    if let Some(lang) = lang_id {
        if !lang.is_empty() {
            let _ = set_lgc_language(lgc_path, lang);
        }
    }

    result
}

pub async fn install_most(
    app_handle: Option<&tauri::AppHandle>,
    most_path: &str,
    lang_id: &str,
    proxy_url: Option<&str>,
) -> Result<String, String> {
    let emitter = ProgressEmitter::new(app_handle, "most");
    let target_dir = PathBuf::from(most_path);
    if !target_dir.exists() {
        return Err("Most directory does not exist".to_string());
    }

    let metadata = fetch_most_metadata(proxy_url).await?;
    let lang_entry = metadata
        .iter()
        .find(|m| m.id == lang_id)
        .ok_or_else(|| format!("Language '{}' not found in metadata", lang_id))?;

    let supported_version = lang_entry
        .l10n_app
        .supported_most_version
        .clone()
        .unwrap_or_default();

    let app_url = resolve_url(&lang_entry.l10n_app.path);
    let mods_url = resolve_url(&lang_entry.l10n_mods.path);

    emitter.emit("downloading", 0, "Downloading localization app package...", "progress.downloading_app", &[], 0, 0);
    let temp_app_7z = download_file_with_etag(&app_url, proxy_url, &emitter).await?;
    emitter.emit("downloading", 0, "Downloading mods package...", "progress.downloading_mods", &[], 0, 0);
    let temp_mods_7z = download_file_with_etag(&mods_url, proxy_url, &emitter).await?;

    let temp_extract = std::env::temp_dir().join(format!("lk_lateral_extract_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_extract)
        .map_err(|e| format!("Failed to create temp extract dir: {}", e))?;

    emitter.emit("extracting", 82, "Extracting packages...", "progress.extracting", &[], 0, 0);
    extract_7z(&temp_app_7z, &temp_extract)?;
    extract_7z(&temp_mods_7z, &temp_extract)?;

    if !is_in_download_cache(&temp_app_7z) {
        let _ = std::fs::remove_file(&temp_app_7z);
    }
    if !is_in_download_cache(&temp_mods_7z) {
        let _ = std::fs::remove_file(&temp_mods_7z);
    }

    let app_version = version::get_exe_version(most_path).unwrap_or_default();

    let result = finalize_install(&emitter, &target_dir, &temp_extract, lang_id, &lang_entry.l10n_app.version, &lang_entry.l10n_mods.version, &supported_version, &app_version);

    let _ = std::fs::remove_dir_all(&temp_extract);

    if result.is_ok() {
        let _ = patch_most_language_config();
    }

    result
}

fn create_node_and_install(
    emitter: &ProgressEmitter,
    target_dir: &Path,
    temp_7z: &Path,
    temp_extract: &Path,
    language: &str,
    version: &str,
    supported_version: &str,
    app_version: &str,
) -> Result<String, String> {
    std::fs::create_dir_all(temp_extract)
        .map_err(|e| format!("Failed to create temp extract dir: {}", e))?;
    emitter.emit("extracting", 82, "Extracting localization package...", "progress.extracting", &[], 0, 0);
    extract_7z(temp_7z, temp_extract)?;

    let result = finalize_install(emitter, target_dir, temp_extract, language, version, "", supported_version, app_version);

    if !is_in_download_cache(temp_7z) {
        let _ = std::fs::remove_file(temp_7z);
    }
    let _ = std::fs::remove_dir_all(temp_extract);

    result
}

fn finalize_install(
    emitter: &ProgressEmitter,
    target_dir: &Path,
    extract_dir: &Path,
    language: &str,
    version: &str,
    mods_version: &str,
    supported_version: &str,
    app_version: &str,
) -> Result<String, String> {
    emitter.emit("installing", 85, "Checking existing files...", "progress.checking_files", &[], 0, 0);
    let extracted_files = walk_files(extract_dir)?;

    let inst_path = inst_info_path(target_dir);
    if let Some(parent) = inst_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create inst info directory: {}", e))?;
    }

    let existing_info: Option<InstInfo> = if inst_path.exists() {
        std::fs::read_to_string(&inst_path).ok()
            .and_then(|s| serde_json::from_str::<InstInfo>(&s).ok())
    } else {
        None
    };

    let is_first_install = existing_info.as_ref().map_or(true, |info| info.backup_dir.is_empty());

    let backup_dir_abs = if is_first_install {
        let backup_dir_path = backups_files_dir(target_dir);
        std::fs::create_dir_all(&backup_dir_path)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;

        let mut backed_up_files: Vec<FileEntry> = Vec::new();
        for src in &extracted_files {
            let rel_path = src.strip_prefix(extract_dir)
                .map_err(|e| format!("Failed to compute relative path: {}", e))?;
            let rel_str = rel_path.to_string_lossy().replace('\\', "/");
            let dst = target_dir.join(&rel_path);
            if dst.exists() {
                let original_hash = compute_sha256(&dst)?;
                let backup_dst = backup_dir_path.join(&rel_path);
                if let Some(parent) = backup_dst.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create backup parent dir: {}", e))?;
                }
                std::fs::copy(&dst, &backup_dst)
                    .map_err(|e| format!("Failed to backup file {}: {}", rel_str, e))?;
                backed_up_files.push(FileEntry { path: rel_str, sha256: original_hash });
            }
        }

        let backup_dir_root = backups_base_dir(target_dir).join("original");
        let backup_info_path = backup_dir_root.join("backup_info.json");
        let backup_info = BackupInfo { files: backed_up_files };
        let backup_content = serde_json::to_string_pretty(&backup_info)
            .map_err(|e| format!("Failed to serialize backup info: {}", e))?;
        std::fs::write(&backup_info_path, backup_content)
            .map_err(|e| format!("Failed to write backup info: {}", e))?;

        backup_dir_root.to_string_lossy().to_string()
    } else {
        existing_info.as_ref().map_or(String::new(), |info| info.backup_dir.clone())
    };

    let total = extracted_files.len();
    let mut installed_files: Vec<FileEntry> = Vec::new();
    for (i, src) in extracted_files.iter().enumerate() {
        let rel_path = src.strip_prefix(extract_dir)
            .map_err(|e| format!("Failed to compute relative path: {}", e))?;
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let dst = target_dir.join(&rel_path);

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        std::fs::copy(src, &dst).map_err(|e| {
            format!("Failed to copy file {} to {}: {}", src.display(), dst.display(), e)
        })?;
        let new_hash = compute_sha256(&dst)?;
        installed_files.push(FileEntry { path: rel_str, sha256: new_hash });

        if total > 0 {
            let pct = 85 + ((i + 1) as u32 * 10 / total as u32);
            let cur = (i + 1).to_string();
            let cnt = total.to_string();
            emitter.emit("installing", pct, &format!("Installing files... {}/{}", cur, cnt), "progress.installing_files", &[("current", cur.as_str()), ("count", cnt.as_str())], 0, 0);
        }
    }

    emitter.emit("installing", 98, "Saving installation record...", "progress.saving", &[], 0, 0);

    let display_date = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut inst_info = existing_info.unwrap_or_else(|| InstInfo {
        language: language.to_string(),
        date: String::new(),
        loc_version: String::new(),
        loc_mods_version: String::new(),
        supported_version: String::new(),
        app_version: String::new(),
        files: Vec::new(),
        backup_dir: String::new(),
    });

    inst_info.language = language.to_string();
    inst_info.date = display_date;
    inst_info.loc_version = version.to_string();
    inst_info.loc_mods_version = mods_version.to_string();
    inst_info.supported_version = supported_version.to_string();
    inst_info.app_version = app_version.to_string();
    inst_info.files = installed_files;
    inst_info.backup_dir = backup_dir_abs;

    let content = serde_json::to_string_pretty(&inst_info)
        .map_err(|e| format!("Failed to serialize inst info: {}", e))?;
    std::fs::write(&inst_path, content)
        .map_err(|e| format!("Failed to write inst info: {}", e))?;

    emitter.emit("installing", 100, "Installation complete!", "progress.complete", &[], 0, 0);

    Ok(format!("Localization {} installed successfully", version))
}

pub fn full_uninstall(target_dir: &Path) -> Result<String, String> {
    let inst_path = inst_info_path(target_dir);
    if !inst_path.exists() {
        return Err("No localization installation found".to_string());
    }

    let content = std::fs::read_to_string(&inst_path)
        .map_err(|e| format!("Failed to read inst_info.json: {}", e))?;
    let inst_info: InstInfo = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse inst_info.json: {}", e))?;

    if inst_info.backup_dir.is_empty() {
        let _ = std::fs::remove_file(&inst_path);
        return Ok("Uninstallation successful".to_string());
    }

    let backup_files_dir = PathBuf::from(&inst_info.backup_dir).join("files");

    for entry in &inst_info.files {
        let target_file = target_dir.join(&entry.path);
        let backup_file = backup_files_dir.join(&entry.path);

        if backup_file.exists() {
            if let Some(parent) = target_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::copy(&backup_file, &target_file).map_err(|e| {
                format!("Failed to restore file {}: {}", entry.path, e)
            })?;
        } else {
            if target_file.exists() {
                std::fs::remove_file(&target_file).map_err(|e| {
                    format!("Failed to remove file {}: {}", entry.path, e)
                })?;
            }
        }
    }

    let backup_root = PathBuf::from(&inst_info.backup_dir).parent().map(|p| p.to_path_buf());
    let _ = std::fs::remove_file(&inst_path);
    if let Some(root) = backup_root {
        let _ = std::fs::remove_dir_all(&root);
    }

    Ok("Uninstallation successful".to_string())
}



pub fn check_lgc_status(lgc_path: &str, exe_version: &str) -> LgcStatus {
    let target_dir = PathBuf::from(lgc_path);
    let prefs_lang = read_current_localization(lgc_path);
    match check_installed(&target_dir) {
        Some(info) => {
            let loc_language = prefs_lang.unwrap_or_else(|| info.language.clone());
            LgcStatus {
                path: lgc_path.to_string(),
                version: exe_version.to_string(),
                loc_installed: true,
                loc_version: info.loc_version,
                loc_language,
            }
        }
        None => LgcStatus {
            path: lgc_path.to_string(),
            version: exe_version.to_string(),
            loc_installed: false,
            loc_version: String::new(),
            loc_language: prefs_lang.unwrap_or_default(),
        },
    }
}

fn patch_most_language_config() -> Result<(), String> {
    let config_path = std::path::Path::new(r"C:\ProgramData\Lesta\Most Korabli\Korabli.Most.config");
    if !config_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read most config: {}", e))?;
    let re = regex::Regex::new(r#"(<Model\s+Language=")ru-RU(")"#)
        .map_err(|e| format!("Failed to compile regex: {}", e))?;
    let patched = re.replace(&content, r#"${1}be-BY${2}"#).to_string();
    if patched == content {
        return Ok(());
    }
    std::fs::write(config_path, &patched)
        .map_err(|e| format!("Failed to write most config: {}", e))
}

pub fn check_most_status(most_path: &str, exe_version: &str) -> MostStatus {
    let target_dir = PathBuf::from(most_path);
    match check_installed(&target_dir) {
        Some(info) => {
            MostStatus {
                path: most_path.to_string(),
                version: exe_version.to_string(),
                loc_installed: true,
                loc_app_version: info.loc_version,
                loc_mods_version: info.loc_mods_version,
                loc_language: info.language,
            }
        }
        None => MostStatus {
            path: most_path.to_string(),
            version: exe_version.to_string(),
            loc_installed: false,
            loc_app_version: String::new(),
            loc_mods_version: String::new(),
            loc_language: String::new(),
        },
    }
}
