use crate::account::{AccountProfile, AccountStore};
use chrono::{DateTime, Utc};
use common::error::CommonError;
use common::hash::compute_sha256_bytes;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetMode {
    Cli,
    Desktop,
    Both,
}

impl Default for TargetMode {
    fn default() -> Self {
        TargetMode::Both
    }
}

impl std::str::FromStr for TargetMode {
    type Err = CommonError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cli" => Ok(TargetMode::Cli),
            "desktop" => Ok(TargetMode::Desktop),
            "both" | "all" => Ok(TargetMode::Both),
            other => Err(CommonError::Generic(format!(
                "Invalid target mode '{}': expected 'cli', 'desktop', or 'both'",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub filename: String,
    pub path: PathBuf,
    pub alias: String,
    pub target: String,
    pub file_size: u64,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchReport {
    pub account_id: String,
    pub account_name: String,
    pub provider: String,
    pub target_mode: TargetMode,
    pub updated_paths: Vec<PathBuf>,
    pub backups_created: Vec<BackupInfo>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub live_token_found: bool,
    pub live_token_path: Option<PathBuf>,
    pub action_taken: String,
    pub active_account_name: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub restored_from: PathBuf,
    pub target_mode: TargetMode,
    pub updated_paths: Vec<PathBuf>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileBundle {
    pub version: String,
    pub exported_at: DateTime<Utc>,
    pub total_accounts: usize,
    pub accounts: Vec<AccountProfile>,
    pub signature: String,
}

pub struct AuthCredentialManager;

impl AuthCredentialManager {
    /// Candidate search paths for the active Google Antigravity CLI token
    pub fn candidate_cli_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let p = PathBuf::from(&user_profile);
                paths.push(p.join(".gemini").join("antigravity-cli").join("antigravity-oauth-token"));
                paths.push(p.join(".gemini").join("antigravity-cli").join("token.json"));
                paths.push(p.join(".antigravity").join("token.json"));
            }
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let p = PathBuf::from(&local_app_data);
                paths.push(p.join("antigravity").join("token.json"));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                let p = PathBuf::from(&home);
                paths.push(p.join(".gemini").join("antigravity-cli").join("antigravity-oauth-token"));
                paths.push(p.join(".gemini").join("antigravity-cli").join("token.json"));
                paths.push(p.join(".config").join("antigravity").join("token.json"));
            }
        }

        paths
    }

    /// Candidate search paths for the Antigravity Desktop app token
    pub fn candidate_desktop_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(app_data) = std::env::var("APPDATA") {
                let p = PathBuf::from(&app_data);
                paths.push(p.join("antigravity").join("token.json"));
                paths.push(p.join("antigravity").join("credentials.json"));
            }
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let p = PathBuf::from(&local_app_data);
                paths.push(p.join("antigravity").join("auth.json"));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                let p = PathBuf::from(&home);
                paths.push(p.join(".config").join("antigravity").join("token.json"));
                paths.push(p.join(".config").join("antigravity").join("credentials.json"));
            }
        }

        paths
    }

    /// Finds first existing CLI token file
    pub fn detect_cli_token_path() -> Option<PathBuf> {
        Self::candidate_cli_paths().into_iter().find(|p| p.exists() && p.is_file())
    }

    /// Finds first existing Desktop token file
    pub fn detect_desktop_token_path() -> Option<PathBuf> {
        Self::candidate_desktop_paths().into_iter().find(|p| p.exists() && p.is_file())
    }

    /// Returns standard default CLI token target path, creating parent directory if needed
    pub fn ensure_cli_token_target_path() -> PathBuf {
        if let Some(existing) = Self::detect_cli_token_path() {
            return existing;
        }
        let default_path = {
            #[cfg(target_os = "windows")]
            {
                let base = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".to_string());
                PathBuf::from(base).join(".gemini").join("antigravity-cli").join("antigravity-oauth-token")
            }
            #[cfg(not(target_os = "windows"))]
            {
                let base = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                PathBuf::from(base).join(".gemini").join("antigravity-cli").join("antigravity-oauth-token")
            }
        };
        if let Some(parent) = default_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        default_path
    }

    /// Returns standard default Desktop token target path, creating parent directory if needed
    pub fn ensure_desktop_token_target_path() -> PathBuf {
        if let Some(existing) = Self::detect_desktop_token_path() {
            return existing;
        }
        let default_path = {
            #[cfg(target_os = "windows")]
            {
                let base = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Roaming".to_string());
                PathBuf::from(base).join("antigravity").join("token.json")
            }
            #[cfg(not(target_os = "windows"))]
            {
                let base = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                PathBuf::from(base).join(".config").join("antigravity").join("token.json")
            }
        };
        if let Some(parent) = default_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        default_path
    }

    /// Creates an automatic, timestamped backup of the current live credentials before switching (inspired by fikrilal/agy-auth)
    pub fn backup_token_file(source: &Path, backup_dir: &Path, alias: &str) -> Result<BackupInfo, CommonError> {
        fs::create_dir_all(backup_dir)
            .map_err(|e| CommonError::Generic(format!("Failed to create auth backups directory: {}", e)))?;

        if !source.exists() {
            return Err(CommonError::Generic(format!(
                "Source token file does not exist: {:?}",
                source
            )));
        }

        let bytes = fs::read(source)
            .map_err(|e| CommonError::Generic(format!("Failed to read source token for backup: {}", e)))?;
        let sha256_full = compute_sha256_bytes(&bytes);
        let sha256_short = if sha256_full.len() >= 8 { &sha256_full[..8] } else { &sha256_full };

        let now = Utc::now();
        let filename = format!(
            "{}_{}_{}.bak",
            alias.replace([' ', '/', '\\', ':'], "_"),
            now.format("%Y%m%d_%H%M%S"),
            sha256_short
        );
        let backup_path = backup_dir.join(&filename);

        fs::write(&backup_path, &bytes)
            .map_err(|e| CommonError::Generic(format!("Failed to write backup file {:?}: {}", backup_path, e)))?;

        Ok(BackupInfo {
            id: Uuid::new_v4().to_string(),
            filename,
            path: backup_path,
            alias: alias.to_string(),
            target: source.to_string_lossy().to_string(),
            file_size: bytes.len() as u64,
            sha256: sha256_full,
            created_at: now,
        })
    }

    /// Atomically writes content to the target token path using a staged temporary file and atomic rename
    pub fn write_token_atomically(target_path: &Path, content: &str) -> Result<(), CommonError> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CommonError::Generic(format!("Failed to create parent directory for token: {}", e)))?;
        }

        let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
        let stage_name = format!(".agy_token_stage_{}", Uuid::new_v4());
        let stage_path = parent.join(stage_name);

        fs::write(&stage_path, content)
            .map_err(|e| CommonError::Generic(format!("Failed to write staging token file: {}", e)))?;

        // Atomically replace target
        fs::rename(&stage_path, target_path).map_err(|e| {
            let _ = fs::remove_file(&stage_path);
            CommonError::Generic(format!(
                "Failed to atomically rename staged token file to {:?}: {}",
                target_path, e
            ))
        })?;

        Ok(())
    }

    /// Generates token file content payload based on profile configuration (OAuth JSON or bearer string)
    pub fn format_token_payload(profile: &AccountProfile) -> String {
        if profile.token_type.as_deref() == Some("OAuth") || profile.refresh_token.is_some() {
            // Format as standard Google Antigravity OAuth JSON token
            let json_obj = serde_json::json!({
                "access_token": profile.api_key,
                "refresh_token": profile.refresh_token,
                "token_type": "Bearer",
                "expiry": profile.expires_at.map(|e| e.to_rfc3339()),
                "email": profile.email,
                "plan_tier": profile.plan_tier,
                "updated_at": Utc::now().to_rfc3339(),
                "managed_by": "agy-agent"
            });
            serde_json::to_string_pretty(&json_obj).unwrap_or_else(|_| profile.api_key.clone())
        } else {
            // Standard raw bearer token / API key
            profile.api_key.clone()
        }
    }

    /// Applies seamless account switch to live CLI credentials and/or desktop profiles with automatic pre-switch backup
    pub fn apply_switch(
        store: &mut AccountStore,
        id_or_name: &str,
        target: TargetMode,
        workspace: &Path,
    ) -> Result<SwitchReport, CommonError> {
        // Validate account exists
        let profile = store
            .get_account(id_or_name)
            .cloned()
            .ok_or_else(|| CommonError::Generic(format!("Account profile '{}' not found", id_or_name)))?;

        let backup_dir = workspace.join(".agy").join("auth_backups");
        let mut backups_created = Vec::new();
        let mut updated_paths = Vec::new();
        let payload = Self::format_token_payload(&profile);

        // 1. Process CLI target
        if target == TargetMode::Cli || target == TargetMode::Both {
            let cli_path = Self::ensure_cli_token_target_path();
            if cli_path.exists() {
                if let Ok(backup) = Self::backup_token_file(&cli_path, &backup_dir, &format!("cli_{}", profile.name)) {
                    backups_created.push(backup);
                }
            }
            Self::write_token_atomically(&cli_path, &payload)?;
            updated_paths.push(cli_path);
        }

        // 2. Process Desktop target
        if target == TargetMode::Desktop || target == TargetMode::Both {
            let desktop_path = Self::ensure_desktop_token_target_path();
            if desktop_path.exists() {
                if let Ok(backup) = Self::backup_token_file(&desktop_path, &backup_dir, &format!("desktop_{}", profile.name)) {
                    backups_created.push(backup);
                }
            }
            Self::write_token_atomically(&desktop_path, &payload)?;
            updated_paths.push(desktop_path);
        }

        // 3. Update store active state
        store.switch_active(&profile.name)?;
        let store_path = AccountStore::default_path(workspace);
        store.save_to_path(&store_path)?;

        Ok(SwitchReport {
            account_id: profile.id,
            account_name: profile.name,
            provider: profile.provider,
            target_mode: target,
            updated_paths,
            backups_created,
            timestamp: Utc::now(),
        })
    }

    /// Captures the current live CLI credentials into a managed AccountProfile snapshot
    pub fn capture_live_cli_token(
        alias: &str,
        provider: Option<&str>,
        plan_tier: Option<&str>,
    ) -> Result<AccountProfile, CommonError> {
        let cli_path = Self::detect_cli_token_path().ok_or_else(|| {
            CommonError::Generic("No active live Antigravity CLI token file found on host".to_string())
        })?;

        let raw = fs::read_to_string(&cli_path)
            .map_err(|e| CommonError::Generic(format!("Failed to read live token file {:?}: {}", cli_path, e)))?;
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(CommonError::Generic("Live token file is empty".to_string()));
        }

        // Try parsing as JSON first
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let access_token = val
                .get("access_token")
                .and_then(|v| v.as_str())
                .unwrap_or(trimmed)
                .to_string();
            let refresh_token = val.get("refresh_token").and_then(|v| v.as_str()).map(String::from);
            let email = val.get("email").and_then(|v| v.as_str()).map(String::from);
            let tier = plan_tier
                .map(String::from)
                .or_else(|| val.get("plan_tier").and_then(|v| v.as_str()).map(String::from));
            let expires_at = val
                .get("expiry")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            let mut profile = AccountProfile::new(
                alias,
                provider.unwrap_or("antigravity"),
                access_token,
                None,
                Some("gemini-2.0-flash".to_string()),
                None,
            );
            profile.email = email;
            profile.plan_tier = tier.or_else(|| Some("Pro".to_string()));
            profile.refresh_token = refresh_token;
            profile.token_type = Some("OAuth".to_string());
            profile.expires_at = expires_at;
            Ok(profile)
        } else {
            // Raw token string
            let mut profile = AccountProfile::new(
                alias,
                provider.unwrap_or("antigravity"),
                trimmed,
                None,
                Some("gemini-2.0-flash".to_string()),
                None,
            );
            profile.plan_tier = plan_tier.map(String::from).or_else(|| Some("Standard".to_string()));
            Ok(profile)
        }
    }

    /// Synchronizes the managed account pool with the host's live CLI credentials
    pub fn sync_with_cli(store: &mut AccountStore, workspace: &Path) -> Result<SyncReport, CommonError> {
        let cli_path = match Self::detect_cli_token_path() {
            Some(p) => p,
            None => {
                return Ok(SyncReport {
                    live_token_found: false,
                    live_token_path: None,
                    action_taken: "No live CLI token found on system. No synchronization needed.".to_string(),
                    active_account_name: store.get_active_account().map(|a| a.name.clone()).unwrap_or_default(),
                    timestamp: Utc::now(),
                });
            }
        };

        let raw = fs::read_to_string(&cli_path)
            .map_err(|e| CommonError::Generic(format!("Failed to read live token: {}", e)))?;
        let trimmed = raw.trim();

        // Check if token matches any existing account profile
        let matched = store.accounts.iter().find(|a| {
            if a.api_key == trimmed {
                return true;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(tok) = val.get("access_token").and_then(|v| v.as_str()) {
                    if a.api_key == tok {
                        return true;
                    }
                }
                if let Some(em) = val.get("email").and_then(|v| v.as_str()) {
                    if a.email.as_deref() == Some(em) {
                        return true;
                    }
                }
            }
            false
        });

        if let Some(m) = matched {
            let matched_name = m.name.clone();
            store.switch_active(&matched_name)?;
            let store_path = AccountStore::default_path(workspace);
            store.save_to_path(&store_path)?;

            Ok(SyncReport {
                live_token_found: true,
                live_token_path: Some(cli_path),
                action_taken: format!("Matched existing profile '{}' and set as active", matched_name),
                active_account_name: matched_name,
                timestamp: Utc::now(),
            })
        } else {
            // New active token not in store: capture it as a new profile
            let alias = format!("cli-synced-{}", &Uuid::new_v4().to_string()[..6]);
            let captured = Self::capture_live_cli_token(&alias, Some("antigravity"), Some("Pro"))?;
            let captured_name = captured.name.clone();
            store.add_account(captured)?;
            store.switch_active(&captured_name)?;
            let store_path = AccountStore::default_path(workspace);
            store.save_to_path(&store_path)?;

            Ok(SyncReport {
                live_token_found: true,
                live_token_path: Some(cli_path),
                action_taken: format!("Captured new live CLI token as profile '{}'", captured_name),
                active_account_name: captured_name,
                timestamp: Utc::now(),
            })
        }
    }

    /// Lists existing credential backups stored in `.agy/auth_backups/`
    pub fn list_backups(workspace: &Path) -> Vec<BackupInfo> {
        let backup_dir = workspace.join(".agy").join("auth_backups");
        if !backup_dir.exists() {
            return Vec::new();
        }

        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(&backup_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("bak") {
                    if let Ok(meta) = entry.metadata() {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let bytes = fs::read(&path).unwrap_or_default();
                        let sha = compute_sha256_bytes(&bytes);
                        results.push(BackupInfo {
                            id: Uuid::new_v4().to_string(),
                            filename,
                            path,
                            alias: "credential_backup".to_string(),
                            target: "live_credentials".to_string(),
                            file_size: meta.len(),
                            sha256: sha,
                            created_at: meta.created().map(DateTime::<Utc>::from).unwrap_or_else(|_| Utc::now()),
                        });
                    }
                }
            }
        }
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        results
    }

    /// Restores credentials from a backup file to live targets
    pub fn restore_backup(
        backup_file: &Path,
        target: TargetMode,
    ) -> Result<RestoreReport, CommonError> {
        if !backup_file.exists() {
            return Err(CommonError::Generic(format!("Backup file does not exist: {:?}", backup_file)));
        }

        let content = fs::read_to_string(backup_file)
            .map_err(|e| CommonError::Generic(format!("Failed to read backup file: {}", e)))?;

        let mut updated_paths = Vec::new();

        if target == TargetMode::Cli || target == TargetMode::Both {
            let cli_path = Self::ensure_cli_token_target_path();
            Self::write_token_atomically(&cli_path, &content)?;
            updated_paths.push(cli_path);
        }

        if target == TargetMode::Desktop || target == TargetMode::Both {
            let desktop_path = Self::ensure_desktop_token_target_path();
            Self::write_token_atomically(&desktop_path, &content)?;
            updated_paths.push(desktop_path);
        }

        Ok(RestoreReport {
            restored_from: backup_file.to_path_buf(),
            target_mode: target,
            updated_paths,
            timestamp: Utc::now(),
        })
    }

    /// Exports all managed account profiles into a portable JSON snapshot bundle
    pub fn export_bundle(store: &AccountStore, output_path: &Path) -> Result<(), CommonError> {
        let accounts = store.list_accounts().to_vec();
        let total = accounts.len();
        let json_bytes = serde_json::to_vec(&accounts)
            .map_err(|e| CommonError::Generic(format!("Failed to serialize bundle accounts: {}", e)))?;
        let signature = compute_sha256_bytes(&json_bytes);

        let bundle = ProfileBundle {
            version: "1.0.0".to_string(),
            exported_at: Utc::now(),
            total_accounts: total,
            accounts,
            signature,
        };

        let output_json = serde_json::to_string_pretty(&bundle)
            .map_err(|e| CommonError::Generic(format!("Failed to serialize bundle: {}", e)))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CommonError::Generic(format!("Failed to create export directory: {}", e)))?;
        }

        fs::write(output_path, output_json)
            .map_err(|e| CommonError::Generic(format!("Failed to write export bundle: {}", e)))?;

        Ok(())
    }

    /// Imports account profiles from a JSON snapshot bundle into the store
    pub fn import_bundle(
        store: &mut AccountStore,
        input_path: &Path,
        overwrite: bool,
    ) -> Result<usize, CommonError> {
        if !input_path.exists() {
            return Err(CommonError::Generic(format!("Bundle file does not exist: {:?}", input_path)));
        }

        let content = fs::read_to_string(input_path)
            .map_err(|e| CommonError::Generic(format!("Failed to read bundle file: {}", e)))?;

        let bundle: ProfileBundle = serde_json::from_str(&content)
            .map_err(|e| CommonError::Generic(format!("Failed to parse bundle JSON: {}", e)))?;

        let mut imported_count = 0;
        for acc in bundle.accounts {
            if let Some(existing) = store.get_account_mut(&acc.name) {
                if overwrite {
                    *existing = acc;
                    imported_count += 1;
                }
            } else {
                store.accounts.push(acc);
                imported_count += 1;
            }
        }

        Ok(imported_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_mode_parse() {
        assert_eq!("cli".parse::<TargetMode>().unwrap(), TargetMode::Cli);
        assert_eq!("desktop".parse::<TargetMode>().unwrap(), TargetMode::Desktop);
        assert_eq!("both".parse::<TargetMode>().unwrap(), TargetMode::Both);
        assert_eq!("all".parse::<TargetMode>().unwrap(), TargetMode::Both);
        assert!("invalid".parse::<TargetMode>().is_err());
    }

    #[test]
    fn test_format_token_payload() {
        let p_api = AccountProfile::new("APIKeyAcc", "openai", "sk-proj-test1234", None, None, None);
        assert_eq!(AuthCredentialManager::format_token_payload(&p_api), "sk-proj-test1234");

        let p_oauth = AccountProfile::new("OAuthAcc", "antigravity", "ya29.test1234", None, None, None)
            .with_email("developer@google.com")
            .with_plan_tier("Ultra")
            .with_oauth_token("ya29.test1234", Some("1//refresh-token".into()), None);

        let formatted = AuthCredentialManager::format_token_payload(&p_oauth);
        assert!(formatted.contains("ya29.test1234"));
        assert!(formatted.contains("developer@google.com"));
        assert!(formatted.contains("Ultra"));
        assert!(formatted.contains("1//refresh-token"));
    }

    #[test]
    fn test_export_and_import_bundle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let export_file = temp_dir.path().join("profiles.bundle.json");

        let mut store = AccountStore::new();
        store
            .add_account(
                AccountProfile::new("Work", "claude", "sk-ant-test", None, None, Some(50000))
                    .with_email("work@corp.com")
                    .with_plan_tier("Enterprise"),
            )
            .unwrap();
        store
            .add_account(
                AccountProfile::new("Personal", "gemini", "ya29.personal", None, None, None)
                    .with_email("personal@gmail.com")
                    .with_plan_tier("Pro"),
            )
            .unwrap();

        AuthCredentialManager::export_bundle(&store, &export_file).unwrap();
        assert!(export_file.exists());

        let mut new_store = AccountStore::new();
        let count = AuthCredentialManager::import_bundle(&mut new_store, &export_file, false).unwrap();
        assert_eq!(count, 2);
        assert_eq!(new_store.list_accounts().len(), 2);
        assert_eq!(new_store.get_account("Work").unwrap().email.as_deref(), Some("work@corp.com"));
        assert_eq!(new_store.get_account("Personal").unwrap().plan_tier.as_deref(), Some("Pro"));
    }

    #[test]
    fn test_atomic_write_and_backup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("live_token.json");
        let backup_dir = temp_dir.path().join("backups");

        // 1. Initial write
        AuthCredentialManager::write_token_atomically(&target_file, "initial-token-123").unwrap();
        assert_eq!(fs::read_to_string(&target_file).unwrap(), "initial-token-123");

        // 2. Backup
        let backup = AuthCredentialManager::backup_token_file(&target_file, &backup_dir, "test_acc").unwrap();
        assert!(backup.path.exists());
        assert_eq!(fs::read_to_string(&backup.path).unwrap(), "initial-token-123");

        // 3. Update atomically
        AuthCredentialManager::write_token_atomically(&target_file, "updated-token-456").unwrap();
        assert_eq!(fs::read_to_string(&target_file).unwrap(), "updated-token-456");

        // 4. Restore
        let restore = AuthCredentialManager::restore_backup(&backup.path, TargetMode::Cli).unwrap();
        assert_eq!(restore.restored_from, backup.path);
    }
}
