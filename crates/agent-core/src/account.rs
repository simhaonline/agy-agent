use chrono::{DateTime, Utc};
use common::error::CommonError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub is_active: bool,
    pub quota_limit: Option<u64>,
    pub quota_used: u64,
    pub request_count: u64,
    pub error_count: u64,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl AccountProfile {
    pub fn new(
        name: impl Into<String>,
        provider: impl Into<String>,
        api_key: impl Into<String>,
        base_url: Option<String>,
        model: Option<String>,
        quota_limit: Option<u64>,
    ) -> Self {
        AccountProfile {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            provider: provider.into(),
            api_key: api_key.into(),
            base_url,
            model,
            is_active: false,
            quota_limit,
            quota_used: 0,
            request_count: 0,
            error_count: 0,
            cooldown_until: None,
            last_used_at: None,
            created_at: Utc::now(),
        }
    }

    pub fn safe_masked_key(&self) -> String {
        if self.api_key.len() <= 8 {
            "********".to_string()
        } else {
            let prefix = &self.api_key[..4];
            let suffix = &self.api_key[self.api_key.len() - 4..];
            format!("{}...{}", prefix, suffix)
        }
    }

    pub fn is_cooling(&self) -> bool {
        if let Some(until) = self.cooldown_until {
            Utc::now() < until
        } else {
            false
        }
    }

    pub fn is_quota_exceeded(&self) -> bool {
        if let Some(limit) = self.quota_limit {
            self.quota_used >= limit
        } else {
            false
        }
    }

    pub fn is_usable(&self) -> bool {
        !self.is_cooling() && !self.is_quota_exceeded()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<AccountProfile>,
}

impl AccountStore {
    pub fn new() -> Self {
        AccountStore { accounts: Vec::new() }
    }

    pub fn default_path(workspace: &Path) -> PathBuf {
        workspace.join(".agy").join("accounts.json")
    }

    pub fn load_from_path(path: &Path) -> Result<Self, CommonError> {
        if !path.exists() {
            return Ok(AccountStore::new());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| CommonError::Generic(format!("Failed to read accounts file {:?}: {}", path, e)))?;
        let store: AccountStore = serde_json::from_str(&content)
            .map_err(|e| CommonError::Generic(format!("Failed to parse accounts JSON: {}", e)))?;
        Ok(store)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), CommonError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CommonError::Generic(format!("Failed to create accounts directory {:?}: {}", parent, e)))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CommonError::Generic(format!("Failed to serialize accounts: {}", e)))?;
        fs::write(path, json)
            .map_err(|e| CommonError::Generic(format!("Failed to write accounts to {:?}: {}", path, e)))?;
        Ok(())
    }

    pub fn add_account(&mut self, mut profile: AccountProfile) -> Result<(), CommonError> {
        if self.accounts.iter().any(|a| a.name == profile.name) {
            return Err(CommonError::Generic(format!("Account with name '{}' already exists", profile.name)));
        }
        // If this is the first account, make it active
        if self.accounts.is_empty() {
            profile.is_active = true;
        }
        self.accounts.push(profile);
        Ok(())
    }

    pub fn list_accounts(&self) -> &[AccountProfile] {
        &self.accounts
    }

    pub fn get_account(&self, id_or_name: &str) -> Option<&AccountProfile> {
        self.accounts
            .iter()
            .find(|a| a.id == id_or_name || a.name.eq_ignore_ascii_case(id_or_name))
    }

    pub fn get_account_mut(&mut self, id_or_name: &str) -> Option<&mut AccountProfile> {
        self.accounts
            .iter_mut()
            .find(|a| a.id == id_or_name || a.name.eq_ignore_ascii_case(id_or_name))
    }

    pub fn get_active_account(&self) -> Option<&AccountProfile> {
        self.accounts.iter().find(|a| a.is_active)
    }

    pub fn switch_active(&mut self, id_or_name: &str) -> Result<(), CommonError> {
        let exists = self.accounts.iter().any(|a| a.id == id_or_name || a.name.eq_ignore_ascii_case(id_or_name));
        if !exists {
            return Err(CommonError::Generic(format!("Account '{}' not found", id_or_name)));
        }
        for a in &mut self.accounts {
            if a.id == id_or_name || a.name.eq_ignore_ascii_case(id_or_name) {
                a.is_active = true;
            } else {
                a.is_active = false;
            }
        }
        Ok(())
    }

    pub fn remove_account(&mut self, id_or_name: &str) -> Result<AccountProfile, CommonError> {
        let index = self
            .accounts
            .iter()
            .position(|a| a.id == id_or_name || a.name.eq_ignore_ascii_case(id_or_name))
            .ok_or_else(|| CommonError::Generic(format!("Account '{}' not found", id_or_name)))?;
        let removed = self.accounts.remove(index);
        // If we removed the active account, make another one active if available
        if removed.is_active && !self.accounts.is_empty() {
            self.accounts[0].is_active = true;
        }
        Ok(removed)
    }

    pub fn mark_cooldown(&mut self, id_or_name: &str, duration_secs: u64) -> Result<(), CommonError> {
        let account = self
            .get_account_mut(id_or_name)
            .ok_or_else(|| CommonError::Generic(format!("Account '{}' not found", id_or_name)))?;
        let until = Utc::now() + chrono::Duration::seconds(duration_secs as i64);
        account.cooldown_until = Some(until);
        account.error_count += 1;
        Ok(())
    }

    pub fn record_usage(&mut self, id_or_name: &str, tokens: u64, is_error: bool) -> Result<(), CommonError> {
        let account = self
            .get_account_mut(id_or_name)
            .ok_or_else(|| CommonError::Generic(format!("Account '{}' not found", id_or_name)))?;
        account.request_count += 1;
        account.quota_used += tokens;
        account.last_used_at = Some(Utc::now());
        if is_error {
            account.error_count += 1;
        }
        Ok(())
    }

    pub fn get_next_available_account(&self) -> Option<&AccountProfile> {
        // First check active account
        if let Some(active) = self.get_active_account() {
            if active.is_usable() {
                return Some(active);
            }
        }
        // Fallback: first non-cooling and non-exhausted account
        self.accounts.iter().find(|a| a.is_usable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_creation_and_switch() {
        let mut store = AccountStore::new();
        let acc1 = AccountProfile::new("Acc1", "openai", "sk-1234567890abcdef", None, Some("gpt-4o".into()), Some(1000));
        let acc2 = AccountProfile::new("Acc2", "claude", "sk-ant-1234567890abcdef", None, Some("claude-3-5-sonnet".into()), None);

        store.add_account(acc1).unwrap();
        store.add_account(acc2).unwrap();

        assert_eq!(store.list_accounts().len(), 2);
        assert_eq!(store.get_active_account().unwrap().name, "Acc1");

        store.switch_active("Acc2").unwrap();
        assert_eq!(store.get_active_account().unwrap().name, "Acc2");
    }

    #[test]
    fn test_account_cooldown_and_failover() {
        let mut store = AccountStore::new();
        let mut acc1 = AccountProfile::new("Primary", "openai", "sk-test-key-1", None, None, None);
        acc1.is_active = true;
        let acc2 = AccountProfile::new("Secondary", "claude", "sk-test-key-2", None, None, None);

        store.accounts.push(acc1);
        store.accounts.push(acc2);

        assert_eq!(store.get_next_available_account().unwrap().name, "Primary");

        // Put Primary into cooldown
        store.mark_cooldown("Primary", 300).unwrap();
        assert!(store.get_account("Primary").unwrap().is_cooling());

        // Failover should pick Secondary
        let next = store.get_next_available_account().unwrap();
        assert_eq!(next.name, "Secondary");
    }

    #[test]
    fn test_quota_tracking_and_usage() {
        let mut store = AccountStore::new();
        let acc = AccountProfile::new("Limited", "openai", "sk-limited-123", None, None, Some(500));
        store.add_account(acc).unwrap();

        store.record_usage("Limited", 200, false).unwrap();
        assert_eq!(store.get_account("Limited").unwrap().quota_used, 200);
        assert!(!store.get_account("Limited").unwrap().is_quota_exceeded());

        store.record_usage("Limited", 350, false).unwrap();
        assert_eq!(store.get_account("Limited").unwrap().quota_used, 550);
        assert!(store.get_account("Limited").unwrap().is_quota_exceeded());
        assert!(!store.get_account("Limited").unwrap().is_usable());
    }
}
