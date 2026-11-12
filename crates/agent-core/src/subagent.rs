use common::error::CommonError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentRole {
    Planner,
    Coder,
    Reviewer,
    Researcher,
    BrowserOperator,
}

impl SubagentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubagentRole::Planner => "Planner",
            SubagentRole::Coder => "Coder",
            SubagentRole::Reviewer => "Reviewer",
            SubagentRole::Researcher => "Researcher",
            SubagentRole::BrowserOperator => "BrowserOperator",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentTaskContract {
    pub task_id: String,
    pub role: SubagentRole,
    pub goal: String,
    pub allowed_tools: Vec<String>,
    pub step_budget: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentOutput {
    pub task_id: String,
    pub role: SubagentRole,
    pub success: bool,
    pub findings: String,
    pub steps_taken: usize,
    pub provenance: Value,
}

pub struct SubagentOrchestrator;

impl SubagentOrchestrator {
    pub fn execute_subtask(contract: &SubagentTaskContract) -> Result<SubagentOutput, CommonError> {
        let findings = match contract.role {
            SubagentRole::Planner => format!("Decomposed plan for: {}", contract.goal),
            SubagentRole::Coder => format!("Executed implementation tasks for: {}", contract.goal),
            SubagentRole::Reviewer => format!("Verified diffs and requirements for: {}", contract.goal),
            SubagentRole::Researcher => format!("Gathered facts and context for: {}", contract.goal),
            SubagentRole::BrowserOperator => format!("Automated web interactions for: {}", contract.goal),
        };

        Ok(SubagentOutput {
            task_id: contract.task_id.clone(),
            role: contract.role,
            success: true,
            findings,
            steps_taken: 1,
            provenance: serde_json::json!({
                "budget": contract.step_budget,
                "role": contract.role.as_str()
            }),
        })
    }
}
