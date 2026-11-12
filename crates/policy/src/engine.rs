use crate::preset::{AgentPreset, EffectivePolicy};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    RequireApproval(String),
    Deny(String),
}

pub struct PolicyEngine {
    pub preset: AgentPreset,
    pub effective_policy: EffectivePolicy,
    pub workspace_root: std::path::PathBuf,
}

impl PolicyEngine {
    pub fn new<P: AsRef<Path>>(preset: AgentPreset, workspace_root: P) -> Self {
        let effective_policy = preset.effective_policy();
        PolicyEngine {
            preset,
            effective_policy,
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Check a tool invocation against non-bypassable boundaries, deny rules, and preset policy
    pub fn evaluate_tool_call(
        &self,
        tool_name: &str,
        path_arg: Option<&Path>,
        command_arg: Option<&str>,
    ) -> PolicyDecision {
        // 1. NON-BYPASSABLE BOUNDARY: Deny dangerous system commands
        if let Some(cmd) = command_arg {
            let lower = cmd.to_lowercase();
            if lower.contains("format ")
                || lower.contains("rmdir /s /q c:\\")
                || lower.contains("del /f /s /q c:\\")
                || lower.contains("mkfs")
                || lower.contains(":(){ :|:& };:")
            {
                return PolicyDecision::Deny(format!(
                    "Command '{}' violates non-bypassable system protection boundary",
                    cmd
                ));
            }
        }

        // 2. NON-BYPASSABLE BOUNDARY: Deny access to sensitive credentials
        if let Some(path) = path_arg {
            let path_str = path.to_string_lossy().to_lowercase();
            if path_str.contains("id_rsa")
                || path_str.contains("id_ed25519")
                || path_str.contains(".ssh")
                || path_str.contains(".aws/credentials")
                || path_str.contains(".bash_history")
            {
                return PolicyDecision::Deny(format!(
                    "Path {:?} violates non-bypassable secrets protection boundary",
                    path
                ));
            }

            // Path containment check
            if let Ok(safe) = common::fs::ensure_within_root(&self.workspace_root, path) {
                // contained
                let _ = safe;
            } else {
                return PolicyDecision::Deny(format!(
                    "Path {:?} escapes configured workspace boundary {:?}",
                    path, self.workspace_root
                ));
            }
        }

        // 3. Preset-specific approval requirements
        match self.preset {
            AgentPreset::Unleashed => {
                // Minimal approvals in sandbox
                PolicyDecision::Allow
            }
            AgentPreset::Unrestricted | AgentPreset::Aggressive | AgentPreset::Abliterated => {
                if tool_name == "shell_exec" {
                    // Shell commands in broad allowlist require approval unless read-only
                    if let Some(cmd) = command_arg {
                        let trimmed = cmd.trim().to_lowercase();
                        if trimmed.starts_with("git status")
                            || trimmed.starts_with("git diff")
                            || trimmed.starts_with("cargo check")
                            || trimmed.starts_with("cargo test")
                            || trimmed.starts_with("dir")
                            || trimmed.starts_with("ls")
                        {
                            PolicyDecision::Allow
                        } else {
                            PolicyDecision::RequireApproval(format!("Execute shell command: {}", cmd))
                        }
                    } else {
                        PolicyDecision::Allow
                    }
                } else {
                    PolicyDecision::Allow
                }
            }
            AgentPreset::Heretic | AgentPreset::Uncensored => {
                if tool_name == "shell_exec" || tool_name == "file_write" || tool_name == "file_edit" {
                    PolicyDecision::RequireApproval(format!("Tool '{}' requires standard approval", tool_name))
                } else {
                    PolicyDecision::Allow
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_bypassable_boundaries_win_over_unleashed() {
        let engine = PolicyEngine::new(AgentPreset::Unleashed, ".");

        // Destructive command blocked even on Unleashed
        let decision = engine.evaluate_tool_call("shell_exec", None, Some("rmdir /s /q C:\\"));
        assert!(matches!(decision, PolicyDecision::Deny(_)));

        // Secrets access blocked even on Unleashed
        let decision = engine.evaluate_tool_call("file_read", Some(Path::new("C:/Users/test/.ssh/id_rsa")), None);
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn test_effective_policy_display() {
        for preset in [
            AgentPreset::Heretic,
            AgentPreset::Uncensored,
            AgentPreset::Unrestricted,
            AgentPreset::Aggressive,
            AgentPreset::Abliterated,
            AgentPreset::Unleashed,
        ] {
            let ep = preset.effective_policy();
            assert_eq!(ep.preset_name, preset.as_str());
            assert_eq!(ep.audit, "Always on");
            assert!(!ep.non_bypassable_boundaries.is_empty());
        }
    }
}
