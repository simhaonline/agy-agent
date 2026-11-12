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
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub plan_tier: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub refresh_failure_count: u64,
    #[serde(default)]
    pub last_switched_at: Option<DateTime<Utc>>,
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
            email: None,
            plan_tier: None,
            refresh_token: None,
            token_type: Some("Bearer".to_string()),
            expires_at: None,
            refresh_failure_count: 0,
            last_switched_at: None,
        }
    }

    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn with_plan_tier(mut self, tier: impl Into<String>) -> Self {
        self.plan_tier = Some(tier.into());
        self
    }

    pub fn with_oauth_token(
        mut self,
        token: impl Into<String>,
        refresh: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        self.api_key = token.into();
        self.refresh_token = refresh;
        self.token_type = Some("OAuth".to_string());
        self.expires_at = expires_at;
        self
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

    pub fn is_low_quota(&self, threshold_percent: f64) -> bool {
        if let Some(limit) = self.quota_limit {
            if limit > 0 {
                let ratio = (self.quota_used as f64 / limit as f64) * 100.0;
                return ratio >= threshold_percent;
            }
        }
        false
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() >= expires_at
        } else {
            false
        }
    }

    pub fn expires_in_secs(&self) -> Option<i64> {
        self.expires_at.map(|exp| (exp - Utc::now()).num_seconds())
    }

    pub fn is_usable(&self) -> bool {
        !self.is_cooling() && !self.is_quota_exceeded() && !self.is_expired()
    }

    /// Computes dynamic Health Score (0..=100) inspired by zcop/agy-cli-manager
    pub fn health_score(&self) -> u32 {
        if self.is_cooling() || self.is_quota_exceeded() || self.is_expired() {
            return 0;
        }

        let mut score: i32 = 100;

        // Quota penalty: up to 50 points based on quota used percentage
        if let Some(limit) = self.quota_limit {
            if limit > 0 {
                let usage_ratio = (self.quota_used as f64) / (limit as f64);
                let penalty = (usage_ratio * 50.0).min(50.0) as i32;
                score -= penalty;
            }
        }

        // Error penalty: 5 points per error, capped at 30
        let error_penalty = ((self.error_count as i32) * 5).min(30);
        score -= error_penalty;

        // Refresh failure penalty: 10 points per failure, capped at 30
        let refresh_penalty = ((self.refresh_failure_count as i32) * 10).min(30);
        score -= refresh_penalty;

        // Expiry warning penalty: if expiring in less than 30 minutes, deduct 20 points
        if let Some(secs) = self.expires_in_secs() {
            if secs > 0 && secs < 1800 {
                score -= 20;
            }
        }

        score.max(0) as u32
    }

    pub fn health_status(&self) -> &'static str {
        if self.is_cooling() {
            "Cooling (429)"
        } else if self.is_expired() {
            "Expired"
        } else if self.is_quota_exceeded() {
            "Quota Exceeded"
        } else if self.is_low_quota(90.0) {
            "Low Quota"
        } else {
            let score = self.health_score();
            if score >= 80 {
                "Healthy"
            } else if score >= 50 {
                "Fair"
            } else {
                "Degraded"
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedAccount {
    pub rank: usize,
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub provider: String,
    pub plan_tier: String,
    pub health_score: u32,
    pub status: String,
    pub is_active: bool,
    pub quota_limit: Option<u64>,
    pub quota_used: u64,
    pub quota_percent: Option<f64>,
    pub error_count: u64,
    pub masked_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationResult {
    pub from: String,
    pub to: String,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<AccountProfile>,
}

impl AccountStore {
    pub fn new() -> Self {
        AccountStore {
            accounts: Vec::new(),
        }
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
                a.last_switched_at = Some(Utc::now());
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
            self.accounts[0].last_switched_at = Some(Utc::now());
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
        // Fallback: highest health score usable standby
        self.get_best_standby()
    }

    /// Evaluates and ranks all accounts based on health score descending
    pub fn rank_accounts(&self) -> Vec<RankedAccount> {
        let mut sorted = self.accounts.clone();
        // Sort primarily by health score descending, active account first if score is tied
        sorted.sort_by(|a, b| {
            b.health_score()
                .cmp(&a.health_score())
                .then_with(|| b.is_active.cmp(&a.is_active))
        });

        sorted
            .into_iter()
            .enumerate()
            .map(|(i, acc)| {
                let health_score = acc.health_score();
                let status = acc.health_status().to_string();
                let masked_key = acc.safe_masked_key();
                let q_pct = acc.quota_limit.map(|lim| {
                    if lim > 0 {
                        (acc.quota_used as f64 / lim as f64) * 100.0
                    } else {
                        100.0
                    }
                });
                RankedAccount {
                    rank: i + 1,
                    id: acc.id,
                    name: acc.name,
                    email: acc.email,
                    provider: acc.provider,
                    plan_tier: acc.plan_tier.unwrap_or_else(|| "Standard".to_string()),
                    health_score,
                    status,
                    is_active: acc.is_active,
                    quota_limit: acc.quota_limit,
                    quota_used: acc.quota_used,
                    quota_percent: q_pct,
                    error_count: acc.error_count,
                    masked_key,
                }
            })
            .collect()
    }

    /// Finds the highest-scoring healthy standby account in the pool
    pub fn get_best_standby(&self) -> Option<&AccountProfile> {
        self.accounts
            .iter()
            .filter(|a| !a.is_active && a.is_usable() && a.health_score() > 0)
            .max_by_key(|a| a.health_score())
    }

    /// Checks if the active account needs rotation (rate limit, quota, errors, or expiry)
    pub fn should_rotate(&self, threshold_percent: f64) -> Option<String> {
        if let Some(active) = self.get_active_account() {
            if active.is_cooling() {
                return Some("Active account is cooling down from HTTP 429 rate limit".to_string());
            }
            if active.is_quota_exceeded() {
                return Some("Active account quota ceiling has been reached".to_string());
            }
            if active.is_expired() {
                return Some("Active account credentials have expired".to_string());
            }
            if active.is_low_quota(threshold_percent) {
                return Some(format!(
                    "Active account reached low-quota threshold (>= {:.1}%)",
                    threshold_percent
                ));
            }
            if active.error_count >= 5 {
                return Some("Active account exceeded error tolerance threshold (>= 5 consecutive errors)".to_string());
            }
        } else {
            return Some("No active account is currently designated in the pool".to_string());
        }
        None
    }

    /// Performs automatic quota-aware failover and rotation to the top-ranked standby account
    pub fn auto_rotate(&mut self, threshold_percent: f64) -> Result<Option<RotationResult>, CommonError> {
        let reason = match self.should_rotate(threshold_percent) {
            Some(r) => r,
            None => return Ok(None),
        };

        let current_active_name = self
            .get_active_account()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "none".to_string());

        let best_standby_name = match self.get_best_standby() {
            Some(s) => s.name.clone(),
            None => {
                return Err(CommonError::Generic(format!(
                    "Rotation needed ({}) but no healthy standby account available in pool",
                    reason
                )));
            }
        };

        self.switch_active(&best_standby_name)?;

        Ok(Some(RotationResult {
            from: current_active_name,
            to: best_standby_name,
            reason,
            timestamp: Utc::now(),
        }))
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

    #[test]
    fn test_health_score_and_pool_ranking() {
        let mut store = AccountStore::new();
        let mut acc1 = AccountProfile::new("HealthyStandby", "claude", "sk-ant-test-1", None, None, Some(10000))
            .with_plan_tier("Pro");
        acc1.quota_used = 1000; // 10% used -> score ~95

        let mut acc2 = AccountProfile::new("ActiveDegraded", "openai", "sk-test-2", None, None, Some(1000))
            .with_plan_tier("Free");
        acc2.is_active = true;
        acc2.quota_used = 920; // 92% used -> low quota
        acc2.error_count = 2;

        let mut acc3 = AccountProfile::new("CoolingAccount", "gemini", "sk-test-3", None, None, None);
        acc3.cooldown_until = Some(Utc::now() + chrono::Duration::seconds(60));

        store.accounts.push(acc1);
        store.accounts.push(acc2);
        store.accounts.push(acc3);

        let ranking = store.rank_accounts();
        assert_eq!(ranking.len(), 3);
        assert_eq!(ranking[0].name, "HealthyStandby");
        assert!(ranking[0].health_score >= 90);
        assert_eq!(ranking[2].name, "CoolingAccount");
        assert_eq!(ranking[2].health_score, 0);

        // Test should_rotate: ActiveDegraded is at 92%
        assert!(store.should_rotate(90.0).is_some());

        // Test auto_rotate
        let rot = store.auto_rotate(90.0).unwrap().unwrap();
        assert_eq!(rot.from, "ActiveDegraded");
        assert_eq!(rot.to, "HealthyStandby");
        assert_eq!(store.get_active_account().unwrap().name, "HealthyStandby");
    }
}
