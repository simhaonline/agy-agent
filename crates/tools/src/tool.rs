use common::error::CommonError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub workspace_root: PathBuf,
    pub session_id: String,
    pub auto_approve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn ok(msg: impl Into<String>) -> Self {
        ToolResult {
            success: true,
            output: msg.into(),
            error: None,
        }
    }

    pub fn err(err: impl Into<String>) -> Self {
        ToolResult {
            success: false,
            output: String::new(),
            error: Some(err.into()),
        }
    }
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;
    fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError>;
}
