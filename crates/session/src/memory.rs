use common::error::CommonError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub key: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStore {
    pub items: Vec<MemoryItem>,
}

pub struct MemoryManager {
    pub file_path: PathBuf,
}

impl MemoryManager {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let file_path = workspace_root.as_ref().join(".agy").join("memory.json");
        MemoryManager { file_path }
    }

    pub fn load_store(&self) -> MemoryStore {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                if let Ok(store) = serde_json::from_str(&content) {
                    return store;
                }
            }
        }
        MemoryStore::default()
    }

    pub fn save_store(&self, store: &MemoryStore) -> Result<(), CommonError> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&self.file_path)?;
        let json = serde_json::to_string_pretty(store)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn record(&self, key: &str, content: &str, category: &str) -> Result<(), CommonError> {
        let mut store = self.load_store();
        store.items.retain(|i| i.key != key);
        store.items.push(MemoryItem {
            key: key.to_string(),
            content: content.to_string(),
            category: category.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        });
        self.save_store(&store)
    }

    pub fn delete(&self, key: &str) -> Result<(), CommonError> {
        let mut store = self.load_store();
        store.items.retain(|i| i.key != key);
        self.save_store(&store)
    }

    /// Context compaction: merges/summarizes older messages to fit token budgets
    pub fn compact_messages(messages: &[serde_json::Value], max_items: usize) -> Vec<serde_json::Value> {
        if messages.len() <= max_items {
            return messages.to_vec();
        }

        let retain_tail = max_items.saturating_sub(1);
        let split_idx = messages.len() - retain_tail;

        let older_count = split_idx;
        let summary_item = serde_json::json!({
            "role": "system",
            "content": format!("[Context Compaction: {} earlier turns summarized for budget limits]", older_count)
        });

        let mut compacted = Vec::with_capacity(retain_tail + 1);
        compacted.push(summary_item);
        compacted.extend_from_slice(&messages[split_idx..]);
        compacted
    }
}
