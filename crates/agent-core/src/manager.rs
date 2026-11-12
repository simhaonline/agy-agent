use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEnvironmentReport {
    pub installed: bool,
    pub binary_path: Option<PathBuf>,
    pub version: Option<String>,
    pub running_processes: Vec<ProcessInfo>,
    pub is_running: bool,
    pub gateway_connected: bool,
}

pub struct ClientManager;

impl ClientManager {
    /// Detects installed AGY or Antigravity client binaries on the host system
    pub fn detect_installation() -> Option<PathBuf> {
        let candidates = Self::candidate_paths();
        for path in candidates {
            if path.exists() && path.is_file() {
                return Some(path);
            }
        }

        // Try looking up via PATH
        #[cfg(target_os = "windows")]
        let check_cmd = ("where.exe", &["agy.exe", "agy-agent.exe"][..]);
        #[cfg(not(target_os = "windows"))]
        let check_cmd = ("which", &["agy", "agy-agent"][..]);

        for bin in check_cmd.1 {
            if let Ok(output) = Command::new(check_cmd.0).arg(bin).output() {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = path_str.lines().next() {
                        let path = PathBuf::from(first_line.trim());
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }

        None
    }

    pub fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let base = PathBuf::from(local_app_data);
                paths.push(base.join("agy").join("bin").join("agy.exe"));
                paths.push(base.join("Programs").join("agy").join("agy.exe"));
                paths.push(base.join("antigravity").join("bin").join("antigravity.exe"));
            }
            if let Ok(app_data) = std::env::var("APPDATA") {
                let base = PathBuf::from(app_data);
                paths.push(base.join("agy").join("bin").join("agy.exe"));
            }
            if let Ok(prog_files) = std::env::var("ProgramFiles") {
                let base = PathBuf::from(prog_files);
                paths.push(base.join("agy").join("agy.exe"));
                paths.push(base.join("Antigravity").join("antigravity.exe"));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            paths.push(PathBuf::from("/usr/local/bin/agy"));
            paths.push(PathBuf::from("/usr/bin/agy"));
            paths.push(PathBuf::from("/opt/agy/bin/agy"));
            if let Ok(home) = std::env::var("HOME") {
                let base = PathBuf::from(home);
                paths.push(base.join(".local").join("bin").join("agy"));
                paths.push(base.join("bin").join("agy"));
            }
        }

        // Relative paths in current development environment
        paths.push(PathBuf::from("dist/windows-x64/agy-agent.exe"));
        paths.push(PathBuf::from("dist/linux-x64/agy-agent"));
        paths.push(PathBuf::from("target/release/agy-agent.exe"));
        paths.push(PathBuf::from("target/debug/agy-agent.exe"));

        paths
    }

    /// Probes client version by executing `<path> --version`
    pub fn probe_version(binary_path: &Path) -> Option<String> {
        let output = Command::new(binary_path).arg("--version").output().ok()?;
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ver.is_empty() {
                return Some(ver);
            }
        }
        None
    }

    /// Lists running AGY / Antigravity client processes
    pub fn list_running_processes() -> Vec<ProcessInfo> {
        let mut processes = Vec::new();
        let target_names = ["agy", "antigravity", "agy-agent", "agy-patcher"];

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() >= 2 {
                            let raw_name = parts[0].trim_matches('"').to_lowercase();
                            let raw_pid = parts[1].trim_matches('"');
                            for target in &target_names {
                                if raw_name.contains(target) {
                                    if let Ok(pid) = raw_pid.parse::<u32>() {
                                        processes.push(ProcessInfo {
                                            pid,
                                            name: parts[0].trim_matches('"').to_string(),
                                            status: "Running".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(output) = Command::new("ps").args(["-eo", "pid,comm"]).output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines().skip(1) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let pid_str = parts[0];
                            let comm = parts[1].to_lowercase();
                            for target in &target_names {
                                if comm.contains(target) {
                                    if let Ok(pid) = pid_str.parse::<u32>() {
                                        processes.push(ProcessInfo {
                                            pid,
                                            name: parts[1].to_string(),
                                            status: "Running".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        processes
    }

    /// Checks if local gateway is responding on given address
    pub fn check_gateway_connection(host: &str, port: u16) -> bool {
        use std::net::ToSocketAddrs;
        if let Ok(mut addrs) = (host, port).to_socket_addrs() {
            if let Some(first) = addrs.next() {
                std::net::TcpStream::connect_timeout(&first, std::time::Duration::from_millis(300)).is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Generates full diagnostic report of client and host runtime
    pub fn inspect_environment(gateway_port: Option<u16>) -> ClientEnvironmentReport {
        let binary_path = Self::detect_installation();
        let version = binary_path.as_ref().and_then(|p| Self::probe_version(p));
        let running_processes = Self::list_running_processes();
        let is_running = !running_processes.is_empty();
        let port = gateway_port.unwrap_or(8080);
        let gateway_connected = Self::check_gateway_connection("127.0.0.1", port);

        ClientEnvironmentReport {
            installed: binary_path.is_some(),
            binary_path,
            version,
            running_processes,
            is_running,
            gateway_connected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_paths_not_empty() {
        let paths = ClientManager::candidate_paths();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_inspect_environment() {
        let report = ClientManager::inspect_environment(Some(9999));
        // Gateway on port 9999 should not be connected during isolated test
        assert!(!report.gateway_connected);
    }
}
