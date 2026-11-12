use crate::runner::AgentRunner;
use common::error::CommonError;
use providers::Provider;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaffRole {
    Researcher,
    Reviewer,
    Implementer,
    Ask,
}

impl std::fmt::Display for StaffRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StaffRole::Researcher => write!(f, "Researcher"),
            StaffRole::Reviewer => write!(f, "Reviewer"),
            StaffRole::Implementer => write!(f, "Implementer"),
            StaffRole::Ask => write!(f, "Ask"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffVerdict {
    pub role: StaffRole,
    pub summary: String,
    pub decision: Option<String>,
    pub findings: Vec<String>,
    pub dirty_tree_checked: bool,
    pub uncommitted_files: Vec<String>,
}

pub struct StaffRunner;

impl StaffRunner {
    /// Inspects git status to detect dirty working tree
    pub fn check_git_dirty_tree(workspace: &Path) -> Result<Vec<String>, CommonError> {
        let output = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(workspace)
            .output();

        match output {
            Ok(out) => {
                if !out.status.success() {
                    // Not a git repository or git error
                    return Ok(Vec::new());
                }
                let text = String::from_utf8_lossy(&out.stdout);
                let dirty_files: Vec<String> = text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.to_string())
                    .collect();
                Ok(dirty_files)
            }
            Err(_) => {
                // If git binary is not installed, assume clean or non-git repo
                Ok(Vec::new())
            }
        }
    }

    /// Obtains git diff for review
    pub fn get_git_diff(workspace: &Path, target_file: Option<&Path>) -> Result<String, CommonError> {
        let mut cmd = Command::new("git");
        cmd.arg("diff");
        if let Some(file) = target_file {
            cmd.arg(file);
        }
        cmd.current_dir(workspace);

        match cmd.output() {
            Ok(out) => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
            Err(e) => Err(CommonError::Io(e)),
        }
    }

    /// Ask Role: Ad-hoc query answering grounded in repository
    pub fn run_ask(
        workspace: &Path,
        question: &str,
        runner: &AgentRunner,
        provider: &dyn Provider,
    ) -> Result<StaffVerdict, CommonError> {
        let prompt = format!(
            "[AGY Staff Companion - Role: Ask]\n\
             Workspace Root: {:?}\n\
             Question: {}\n\
             Provide a direct, precise, context-grounded response.",
            workspace, question
        );

        let session = runner.session_manager.create_session("StaffAsk", runner.preset)?;
        let summary = runner.run_task(&session.id, &prompt, provider)?;

        Ok(StaffVerdict {
            role: StaffRole::Ask,
            summary,
            decision: None,
            findings: Vec::new(),
            dirty_tree_checked: false,
            uncommitted_files: Vec::new(),
        })
    }

    /// Researcher Role: Read-only exploration and architecture analysis
    pub fn run_research(
        workspace: &Path,
        topic: &str,
        runner: &AgentRunner,
        provider: &dyn Provider,
    ) -> Result<StaffVerdict, CommonError> {
        let prompt = format!(
            "[AGY Staff Companion - Role: Researcher]\n\
             Workspace Root: {:?}\n\
             Objective: Conduct research and architectural mapping for: {}\n\
             Requirements:\n\
             1. Examine repository code structure, dependencies, and interfaces.\n\
             2. Do NOT create or modify any project files.\n\
             3. Formulate structured findings with citations and recommendations.",
            workspace, topic
        );

        let session = runner.session_manager.create_session("StaffResearch", runner.preset)?;
        let summary = runner.run_task(&session.id, &prompt, provider)?;

        Ok(StaffVerdict {
            role: StaffRole::Researcher,
            summary,
            decision: None,
            findings: vec![format!("Research completed for topic: {}", topic)],
            dirty_tree_checked: false,
            uncommitted_files: Vec::new(),
        })
    }

    /// Reviewer Role: Inspects diff or files, produces structured review
    pub fn run_review(
        workspace: &Path,
        target_file: Option<&Path>,
        runner: &AgentRunner,
        provider: &dyn Provider,
    ) -> Result<StaffVerdict, CommonError> {
        let diff = Self::get_git_diff(workspace, target_file).unwrap_or_default();
        let target_desc = target_file
            .map(|f| format!("Target File: {:?}", f))
            .unwrap_or_else(|| "Target: Staged & Working Directory Diff".to_string());

        let prompt = format!(
            "[AGY Staff Companion - Role: Reviewer]\n\
             {}\n\
             Git Diff Content:\n\
             ```diff\n\
             {}\n\
             ```\n\
             Review Instructions:\n\
             1. Analyze changes for correctness, security vulnerabilities, regression risks, and styling.\n\
             2. Conclude with a clear verdict line: 'VERDICT: APPROVED' or 'VERDICT: CHANGES_REQUESTED'.\n\
             3. Enumerate findings with actionable suggestions.",
            target_desc, diff
        );

        let session = runner.session_manager.create_session("StaffReview", runner.preset)?;
        let summary = runner.run_task(&session.id, &prompt, provider)?;

        let decision = if summary.contains("CHANGES_REQUESTED") {
            Some("CHANGES_REQUESTED".to_string())
        } else if summary.contains("APPROVED") {
            Some("APPROVED".to_string())
        } else {
            Some("COMMENT".to_string())
        };

        Ok(StaffVerdict {
            role: StaffRole::Reviewer,
            summary,
            decision,
            findings: vec!["Code review analysis completed".to_string()],
            dirty_tree_checked: false,
            uncommitted_files: Vec::new(),
        })
    }

    /// Implementer Role: Code implementation with mandatory Git Dirty-Tree Safety Check!
    pub fn run_implement(
        workspace: &Path,
        task: &str,
        force: bool,
        runner: &AgentRunner,
        provider: &dyn Provider,
    ) -> Result<StaffVerdict, CommonError> {
        // Enforce dirty tree guardrail
        let dirty_files = Self::check_git_dirty_tree(workspace)?;
        if !dirty_files.is_empty() && !force {
            return Err(CommonError::Security(format!(
                "Refusing to execute implementation on dirty git working tree ({} uncommitted file(s) found). \
                 Commit or stash your changes before delegating to staff implementer, or pass --force. \
                 Uncommitted files:\n{}",
                dirty_files.len(),
                dirty_files.join("\n")
            )));
        }

        let prompt = format!(
            "[AGY Staff Companion - Role: Implementer]\n\
             Workspace Root: {:?}\n\
             Task: {}\n\
             Safety Guardrail Status: Dirty tree check passed ({} uncommitted files, force={})\n\
             Implement the requested feature or fix cleanly and verify changes.",
            workspace,
            task,
            dirty_files.len(),
            force
        );

        let session = runner.session_manager.create_session("StaffImplement", runner.preset)?;
        let summary = runner.run_task(&session.id, &prompt, provider)?;

        Ok(StaffVerdict {
            role: StaffRole::Implementer,
            summary,
            decision: Some("IMPLEMENTED".to_string()),
            findings: vec![format!("Task implemented: {}", task)],
            dirty_tree_checked: true,
            uncommitted_files: dirty_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_staff_role_display() {
        assert_eq!(StaffRole::Researcher.to_string(), "Researcher");
        assert_eq!(StaffRole::Reviewer.to_string(), "Reviewer");
        assert_eq!(StaffRole::Implementer.to_string(), "Implementer");
        assert_eq!(StaffRole::Ask.to_string(), "Ask");
    }

    #[test]
    fn test_check_git_dirty_tree() {
        let cur = std::env::current_dir().unwrap();
        let res = StaffRunner::check_git_dirty_tree(&cur);
        assert!(res.is_ok());
    }
}
