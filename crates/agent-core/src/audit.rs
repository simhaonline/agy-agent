use common::error::CommonError;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: String,
    pub session_id: String,
    pub preset: String,
    pub event_type: String,
    pub details: serde_json::Value,
}

pub struct AuditLogger {
    pub log_path: PathBuf,
}

impl AuditLogger {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let log_path = workspace_root.as_ref().join(".agy").join("audit.jsonl");
        AuditLogger { log_path }
    }

    pub fn log(
        &self,
        session_id: &str,
        preset: &str,
        event_type: &str,
        details: serde_json::Value,
    ) -> Result<(), CommonError> {
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Redact any secrets before writing
        let raw_details = details.to_string();
        let redacted = tools::sandbox::Sandbox::redact_secrets(&raw_details);
        let safe_details: serde_json::Value = serde_json::from_str(&redacted)
            .unwrap_or(serde_json::json!({ "raw": redacted }));

        let event = AuditEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: session_id.to_string(),
            preset: preset.to_string(),
            event_type: event_type.to_string(),
            details: safe_details,
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        let line = serde_json::to_string(&event)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    pub fn read_events(&self) -> Result<Vec<AuditEvent>, CommonError> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.log_path)?;
        let mut events = Vec::new();
        for line in content.lines() {
            if !line.trim().is_empty() {
                if let Ok(ev) = serde_json::from_str::<AuditEvent>(line) {
                    events.push(ev);
                }
            }
        }
        Ok(events)
    }
}
