use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.json";
const KEYRING_SERVICE: &str = "lk-lateral";
const KEYRING_USERNAME: &str = "proxy_password";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub use_system_proxy: bool,
    pub custom_host: String,
    pub custom_port: u16,
    pub custom_username: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            use_system_proxy: true,
            custom_host: String::new(),
            custom_port: 7890,
            custom_username: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub lgc_paths: Vec<String>,
    #[serde(default)]
    pub most_paths: Vec<String>,
    #[serde(default)]
    pub most_lang_id: String,
    #[serde(default)]
    pub lgc_lang_id: String,
}

fn default_language() -> String {
    "zh_CN".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            theme: String::new(),
            proxy: ProxyConfig::default(),
            lgc_paths: Vec::new(),
            most_paths: Vec::new(),
            most_lang_id: String::new(),
            lgc_lang_id: String::new(),
        }
    }
}

pub fn get_data_dir() -> PathBuf {
    if let Some(dir) = dirs::data_dir() {
        dir.join("lk-lateral")
    } else {
        PathBuf::from(".").join("data")
    }
}

pub fn config_path() -> PathBuf {
    get_data_dir().join(CONFIG_FILE)
}

pub fn system_locale_to_language() -> String {
    let locale = sys_locale::get_locale().unwrap_or_else(|| String::from("en-US"));
    let lower = locale.to_lowercase().replace('_', "-");
    if lower.starts_with("zh") {
        if lower.contains("hant") || lower.contains("tw") || lower.contains("hk") || lower.contains("mo") {
            return String::from("zh_TW");
        }
        return String::from("zh_CN");
    }
    if lower.starts_with("ru") { return String::from("ru"); }
    if lower.starts_with("ja") { return String::from("ja"); }
    if lower.starts_with("es") { return String::from("es"); }
    if lower.starts_with("de") { return String::from("de"); }
    String::from("en")
}

pub fn load_config() -> Result<AppConfig, String> {
    let path = config_path();
    if !path.exists() {
        let default_config = AppConfig::default();
        save_config(&default_config)?;
        return Ok(default_config);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;
    let config: AppConfig = serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let dir = get_data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create data directory: {}", e))?;
    let path = config_path();
    let content = serde_json::to_string_pretty(config).map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write config: {}", e))?;
    Ok(())
}

pub fn save_proxy_password(password: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    entry
        .set_password(password)
        .map_err(|e| format!("Failed to save password: {}", e))?;
    Ok(())
}

pub fn load_proxy_password() -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    match entry.get_password() {
        Ok(pwd) => Ok(Some(pwd)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to load password: {}", e)),
    }
}

pub fn delete_proxy_password() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    entry
        .set_password("")
        .map_err(|e| format!("Failed to delete password: {}", e))?;
    Ok(())
}

pub fn build_proxy_url(config: &ProxyConfig) -> Option<String> {
    if config.use_system_proxy {
        return None;
    }
    if config.custom_host.is_empty() {
        return None;
    }
    let host = config.custom_host.trim();
    let port = config.custom_port;
    if let Some(ref username) = config.custom_username {
        if !username.is_empty() {
            let password = load_proxy_password().ok().flatten().unwrap_or_default();
            let encoded_user = url_encode(username);
            let encoded_pass = url_encode(&password);
            Some(format!(
                "http://{}:{}@{}:{}",
                encoded_user, encoded_pass, host, port
            ))
        } else {
            Some(format!("http://{}:{}", host, port))
        }
    } else {
        Some(format!("http://{}:{}", host, port))
    }
}

fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            b':' => result.push_str("%3A"),
            b'@' => result.push_str("%40"),
            b'/' => result.push_str("%2F"),
            b'?' => result.push_str("%3F"),
            b'#' => result.push_str("%23"),
            b'[' => result.push_str("%5B"),
            b']' => result.push_str("%5D"),
            b'%' => result.push_str("%25"),
            other => {
                result.push_str(&format!("%{:02X}", other));
            }
        }
    }
    result
}
