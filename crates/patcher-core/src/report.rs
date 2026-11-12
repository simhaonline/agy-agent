use crate::engine::{PatchEvent, PatchMetrics};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyPatchEntry {
    pub off: usize,
    pub len: usize,
    pub r#type: String,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyReport {
    pub patches: Vec<LegacyPatchEntry>,
    pub count: usize,
    pub orig_hash: String,
    pub new_hash: String,
}

impl LegacyReport {
    pub fn from_events(events: &[PatchEvent], orig_hash: &str, new_hash: &str) -> Self {
        let patches = events
            .iter()
            .map(|e| LegacyPatchEntry {
                off: e.off,
                len: e.len,
                r#type: e.r#type.clone(),
                desc: e.desc.clone(),
            })
            .collect();

        LegacyReport {
            count: events.len(),
            patches,
            orig_hash: orig_hash.to_string(),
            new_hash: new_hash.to_string(),
        }
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut f = File::create(path)?;
        let json = serde_json::to_string_pretty(self)?;
        f.write_all(json.as_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichReport {
    pub tool_version: String,
    pub timestamp: String,
    pub input_path: String,
    pub output_path: String,
    pub orig_hash: String,
    pub new_hash: String,
    pub preset: Option<String>,
    pub metrics: PatchMetrics,
    pub events: Vec<PatchEvent>,
    pub warnings: Vec<String>,
}

impl RichReport {
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut f = File::create(path)?;
        let json = serde_json::to_string_pretty(self)?;
        f.write_all(json.as_bytes())
    }
}
