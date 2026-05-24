use std::path::Path;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub fn is_app_running(install_path: &str) -> bool {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );

    let target_dir = canonical_lower(install_path);

    for process in system.processes().values() {
        if let Some(exe_path) = process.exe() {
            if let Some(parent) = exe_path.parent() {
                if canonical_lower(&parent.to_string_lossy()) == target_dir {
                    return true;
                }
            }
        }
    }

    false
}

pub fn force_kill_app(install_path: &str) -> Result<(), String> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );

    let target_dir = canonical_lower(install_path);
    let mut killed = false;

    for (pid, process) in system.processes() {
        if let Some(exe_path) = process.exe() {
            if let Some(parent) = exe_path.parent() {
                if canonical_lower(&parent.to_string_lossy()) == target_dir {
                    let result = std::process::Command::new("taskkill")
                        .args(["/F", "/PID", &pid.as_u32().to_string()])
                        .output();

                    match result {
                        Ok(output) if output.status.success() => {
                            killed = true;
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            return Err(format!("Failed to kill process {}: {}", pid, stderr.trim()));
                        }
                        Err(e) => {
                            return Err(format!("Failed to run taskkill: {}", e));
                        }
                    }
                }
            }
        }
    }

    if killed {
        Ok(())
    } else {
        Err("No running processes found in the app directory".to_string())
    }
}

fn canonical_lower(path: &str) -> String {
    Path::new(path)
        .to_string_lossy()
        .to_lowercase()
        .trim_end_matches(&['\\', '/'] as &[_])
        .to_string()
}
