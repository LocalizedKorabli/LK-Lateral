use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const LGC_DEFAULT_PATH: &str = r"C:\ProgramData\Lesta\GameCenter";
const MOST_DEFAULT_PATH: &str = r"C:\Program Files\Lesta\Most Korabli";

fn parse_default_icon_value(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() {
        return None;
    }

    let cleaned = raw.trim_matches('"');
    let path_str = if let Some(comma_pos) = cleaned.rfind(',') {
        let candidate = &cleaned[..comma_pos];
        let suffix_check = candidate.to_lowercase();
        if suffix_check.ends_with(".exe") || suffix_check.ends_with(".dll") || suffix_check.ends_with(".ico") {
            candidate.to_string()
        } else {
            cleaned.to_string()
        }
    } else {
        cleaned.to_string()
    };

    let path = PathBuf::from(&path_str);

    if path.is_file() {
        if let Some(parent) = path.parent() {
            let lgc_exe = parent.join("lgc.exe");
            if lgc_exe.exists() {
                return Some(parent.to_path_buf());
            }
        }
        return None;
    }

    if path.is_dir() {
        let lgc_exe = path.join("lgc.exe");
        if lgc_exe.exists() {
            return Some(path);
        }
    }

    if let Some(parent) = path.parent() {
        let lgc_exe = parent.join("lgc.exe");
        if lgc_exe.exists() {
            return Some(parent.to_path_buf());
        }
    }

    None
}

pub fn scan_lgc() -> Option<PathBuf> {
    if let Some(path) = scan_lgc_registry() {
        return Some(path);
    }

    let default = PathBuf::from(LGC_DEFAULT_PATH);
    let lgc_exe = default.join("lgc.exe");
    if lgc_exe.exists() {
        return Some(default);
    }

    None
}

fn scan_lgc_registry() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(r"Software\Classes\lgc\DefaultIcon").ok()?;
    let value: String = key.get_value("").unwrap_or_default();
    parse_default_icon_value(&value)
}

pub fn scan_most() -> Option<PathBuf> {
    if let Some(path) = scan_most_registry() {
        return Some(path);
    }

    let default = PathBuf::from(MOST_DEFAULT_PATH);
    let most_exe = default.join("Korabli.Most.exe");
    if most_exe.exists() {
        return Some(default);
    }

    None
}

fn scan_most_registry() -> Option<PathBuf> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey(r"SOFTWARE\Lesta\Most Korabli\Data").ok()?;
    let install_path: String = key.get_value("InstallPath").unwrap_or_default();

    if install_path.is_empty() {
        return None;
    }

    let path = PathBuf::from(&install_path);
    let most_exe = path.join("Korabli.Most.exe");
    if most_exe.exists() {
        return Some(path);
    }

    None
}

pub fn validate_lgc(path: &str) -> bool {
    let p = PathBuf::from(path);
    if !p.is_dir() {
        return false;
    }
    p.join("lgc.exe").exists()
}

pub fn validate_most(path: &str) -> bool {
    let p = PathBuf::from(path);
    if !p.is_dir() {
        return false;
    }
    p.join("Korabli.Most.exe").exists()
}
