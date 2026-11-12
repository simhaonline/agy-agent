use crate::sandbox::Sandbox;
use crate::tool::{Tool, ToolContext, ToolResult};
use common::error::CommonError;
use serde_json::{json, Value};
use std::fs;
use std::process::Command;

pub struct FileReadTool;
impl Tool for FileReadTool {
    fn name(&self) -> &'static str { "file_read" }
    fn description(&self) -> &'static str { "Read text from a file inside workspace" }
    fn schema(&self) -> Value { json!({ "path": "string" }) }
    fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let path_str = args["path"].as_str().ok_or_else(|| CommonError::Validation("Missing path".into()))?;
        let sb = Sandbox::new(&ctx.workspace_root);
        let safe_path = sb.resolve_safe_path(path_str)?;
        let content = fs::read_to_string(&safe_path)?;
        Ok(ToolResult::ok(Sandbox::redact_secrets(&content)))
    }
}

pub struct FileWriteTool;
impl Tool for FileWriteTool {
    fn name(&self) -> &'static str { "file_write" }
    fn description(&self) -> &'static str { "Write text to a file inside workspace" }
    fn schema(&self) -> Value { json!({ "path": "string", "content": "string" }) }
    fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let path_str = args["path"].as_str().ok_or_else(|| CommonError::Validation("Missing path".into()))?;
        let content = args["content"].as_str().ok_or_else(|| CommonError::Validation("Missing content".into()))?;
        let sb = Sandbox::new(&ctx.workspace_root);
        let safe_path = sb.resolve_safe_path(path_str)?;
        if let Some(parent) = safe_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&safe_path, content)?;
        Ok(ToolResult::ok(format!("Successfully wrote {} bytes to {}", content.len(), path_str)))
    }
}

pub struct FileEditTool;
impl Tool for FileEditTool {
    fn name(&self) -> &'static str { "file_edit" }
    fn description(&self) -> &'static str { "Replace exact target string with replacement in a file" }
    fn schema(&self) -> Value { json!({ "path": "string", "target": "string", "replacement": "string" }) }
    fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let path_str = args["path"].as_str().ok_or_else(|| CommonError::Validation("Missing path".into()))?;
        let target = args["target"].as_str().ok_or_else(|| CommonError::Validation("Missing target".into()))?;
        let repl = args["replacement"].as_str().ok_or_else(|| CommonError::Validation("Missing replacement".into()))?;
        let sb = Sandbox::new(&ctx.workspace_root);
        let safe_path = sb.resolve_safe_path(path_str)?;
        let content = fs::read_to_string(&safe_path)?;
        if !content.contains(target) {
            return Ok(ToolResult::err(format!("Target string not found in {}", path_str)));
        }
        let updated = content.replace(target, repl);
        fs::write(&safe_path, updated)?;
        Ok(ToolResult::ok(format!("Successfully edited {}", path_str)))
    }
}

pub struct DirListTool;
impl Tool for DirListTool {
    fn name(&self) -> &'static str { "dir_list" }
    fn description(&self) -> &'static str { "List directory entries" }
    fn schema(&self) -> Value { json!({ "path": "string" }) }
    fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let path_str = args["path"].as_str().unwrap_or(".");
        let sb = Sandbox::new(&ctx.workspace_root);
        let safe_path = sb.resolve_safe_path(path_str)?;
        let mut list = Vec::new();
        for entry in fs::read_dir(safe_path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type()?.is_dir();
            list.push(format!("{}{}", name, if is_dir { "/" } else { "" }));
        }
        Ok(ToolResult::ok(list.join("\n")))
    }
}

pub struct ShellExecTool;
impl Tool for ShellExecTool {
    fn name(&self) -> &'static str { "shell_exec" }
    fn description(&self) -> &'static str { "Execute a shell command inside workspace" }
    fn schema(&self) -> Value { json!({ "command": "string" }) }
    fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let cmd = args["command"].as_str().ok_or_else(|| CommonError::Validation("Missing command".into()))?;
        let output = if cfg!(target_os = "windows") {
            Command::new("powershell")
                .args(["-NoProfile", "-Command", cmd])
                .current_dir(&ctx.workspace_root)
                .output()?
        } else {
            Command::new("sh")
                .args(["-c", cmd])
                .current_dir(&ctx.workspace_root)
                .output()?
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);
        let redacted = Sandbox::redact_secrets(&combined);

        if output.status.success() {
            Ok(ToolResult::ok(redacted))
        } else {
            Ok(ToolResult::err(format!("Exit code {}: {}", output.status, redacted)))
        }
    }
}

pub struct GitStatusTool;
impl Tool for GitStatusTool {
    fn name(&self) -> &'static str { "git_status" }
    fn description(&self) -> &'static str { "Check git status in workspace" }
    fn schema(&self) -> Value { json!({}) }
    fn execute(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let output = Command::new("git")
            .args(["status", "--short"])
            .current_dir(&ctx.workspace_root)
            .output()?;
        Ok(ToolResult::ok(String::from_utf8_lossy(&output.stdout).to_string()))
    }
}

pub struct GitDiffTool;
impl Tool for GitDiffTool {
    fn name(&self) -> &'static str { "git_diff" }
    fn description(&self) -> &'static str { "Check git diff in workspace" }
    fn schema(&self) -> Value { json!({}) }
    fn execute(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolResult, CommonError> {
        let output = Command::new("git")
            .args(["diff"])
            .current_dir(&ctx.workspace_root)
            .output()?;
        Ok(ToolResult::ok(String::from_utf8_lossy(&output.stdout).to_string()))
    }
}

pub struct ToolRegistry {
    pub tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn standard() -> Self {
        ToolRegistry {
            tools: vec![
                Box::new(FileReadTool),
                Box::new(FileWriteTool),
                Box::new(FileEditTool),
                Box::new(DirListTool),
                Box::new(ShellExecTool),
                Box::new(GitStatusTool),
                Box::new(GitDiffTool),
            ],
        }
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|b| b.as_ref())
    }
}
