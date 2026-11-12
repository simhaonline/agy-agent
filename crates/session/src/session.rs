use common::error::CommonError;
use policy::preset::AgentPreset;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub preset: AgentPreset,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<serde_json::Value>,
    pub token_usage: usize,
    pub metadata: Value,
}

pub struct SessionManager {
    pub storage_dir: PathBuf,
}

impl SessionManager {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let storage_dir = workspace_root.as_ref().join(".agy").join("sessions");
        SessionManager { storage_dir }
    }

    pub fn ensure_storage(&self) -> Result<(), CommonError> {
        if !self.storage_dir.exists() {
            fs::create_dir_all(&self.storage_dir)?;
        }
        Ok(())
    }

    pub fn create_session(&self, name: &str, preset: AgentPreset) -> Result<Session, CommonError> {
        self.ensure_storage()?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let session = Session {
            id: id.clone(),
            name: name.to_string(),
            preset,
            created_at: now.clone(),
            updated_at: now,
            messages: Vec::new(),
            token_usage: 0,
            metadata: serde_json::json!({}),
        };
        self.save_session(&session)?;
        Ok(session)
    }

    pub fn save_session(&self, session: &Session) -> Result<(), CommonError> {
        self.ensure_storage()?;
        let file_path = self.storage_dir.join(format!("{}.json", session.id));
        let mut file = File::create(file_path)?;
        let json = serde_json::to_string_pretty(session)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load_session(&self, id: &str) -> Result<Session, CommonError> {
        let file_path = self.storage_dir.join(format!("{}.json", id));
        if !file_path.exists() {
            return Err(CommonError::Validation(format!("Session '{}' not found", id)));
        }
        let content = fs::read_to_string(file_path)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>, CommonError> {
        if !self.storage_dir.exists() {
            return Ok(Vec::new());
        }
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(s) = serde_json::from_str::<Session>(&content) {
                        sessions.push(s);
                    }
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn delete_session(&self, id: &str) -> Result<(), CommonError> {
        let file_path = self.storage_dir.join(format!("{}.json", id));
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }

    pub fn branch_session(&self, source_id: &str, new_name: &str) -> Result<Session, CommonError> {
        let src = self.load_session(source_id)?;
        let mut branched = self.create_session(new_name, src.preset)?;
        branched.messages = src.messages;
        branched.token_usage = src.token_usage;
        self.save_session(&branched)?;
        Ok(branched)
    }
}
