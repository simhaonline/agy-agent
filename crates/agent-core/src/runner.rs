use crate::audit::AuditLogger;
use crate::background::BackgroundTaskManager;
use common::error::CommonError;
use policy::engine::{PolicyDecision, PolicyEngine};
use policy::preset::AgentPreset;
use providers::{ChatMessage, CompletionRequest, Provider};
use session::memory::MemoryManager;
use session::session::SessionManager;
use std::path::{Path, PathBuf};
use tools::registry::ToolRegistry;
use tools::tool::ToolContext;

pub struct AgentRunner {
    pub workspace_root: PathBuf,
    pub preset: AgentPreset,
    pub policy_engine: PolicyEngine,
    pub tool_registry: ToolRegistry,
    pub audit_logger: AuditLogger,
    pub session_manager: SessionManager,
    pub memory_manager: MemoryManager,
    pub background_manager: BackgroundTaskManager,
}

impl AgentRunner {
    pub fn new<P: AsRef<Path>>(workspace_root: P, preset: AgentPreset) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        let policy_engine = PolicyEngine::new(preset, &root);
        let tool_registry = ToolRegistry::standard();
        let audit_logger = AuditLogger::new(&root);
        let session_manager = SessionManager::new(&root);
        let memory_manager = MemoryManager::new(&root);
        let background_manager = BackgroundTaskManager::new(&root);

        AgentRunner {
            workspace_root: root,
            preset,
            policy_engine,
            tool_registry,
            audit_logger,
            session_manager,
            memory_manager,
            background_manager,
        }
    }

    pub fn run_task(
        &self,
        session_id: &str,
        user_prompt: &str,
        provider: &dyn Provider,
    ) -> Result<String, CommonError> {
        let ep = self.preset.effective_policy();

        self.audit_logger.log(
            session_id,
            self.preset.as_str(),
            "task_start",
            serde_json::json!({ "prompt": user_prompt, "max_steps": ep.max_steps }),
        )?;

        let mut messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: format!(
                    "You are the AGY Standalone Agent running preset {}. Obey user instructions within policy limits.",
                    self.preset.as_str()
                ),
                tool_calls: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
                tool_calls: None,
            },
        ];

        let mut steps = 0;
        let mut final_answer = String::new();

        while steps < ep.max_steps {
            steps += 1;

            let req = CompletionRequest {
                model: "gpt-4o".to_string(),
                messages: messages.clone(),
                temperature: Some(0.2),
                max_tokens: Some(4096),
            };

            let resp = provider.complete(&req)?;
            final_answer = resp.content.clone();

            if resp.tool_calls.is_empty() {
                // Done
                break;
            }

            for tool_call in resp.tool_calls {
                // Policy evaluation
                let path_arg = tool_call.arguments.get("path").and_then(|v| v.as_str()).map(Path::new);
                let cmd_arg = tool_call.arguments.get("command").and_then(|v| v.as_str());

                let decision = self.policy_engine.evaluate_tool_call(
                    &tool_call.name,
                    path_arg,
                    cmd_arg,
                );

                match decision {
                    PolicyDecision::Deny(reason) => {
                        self.audit_logger.log(
                            session_id,
                            self.preset.as_str(),
                            "tool_denied",
                            serde_json::json!({ "tool": tool_call.name, "reason": reason }),
                        )?;
                        return Err(CommonError::Security(reason));
                    }
                    PolicyDecision::RequireApproval(reason) => {
                        self.audit_logger.log(
                            session_id,
                            self.preset.as_str(),
                            "approval_required",
                            serde_json::json!({ "tool": tool_call.name, "reason": reason }),
                        )?;
                        println!("[APPROVAL REQUIRED] {}", reason);
                    }
                    PolicyDecision::Allow => {
                        self.audit_logger.log(
                            session_id,
                            self.preset.as_str(),
                            "tool_allowed",
                            serde_json::json!({ "tool": tool_call.name }),
                        )?;
                    }
                }

                // Execute tool
                if let Some(tool) = self.tool_registry.get_tool(&tool_call.name) {
                    let ctx = ToolContext {
                        workspace_root: self.workspace_root.clone(),
                        session_id: session_id.to_string(),
                        auto_approve: true,
                    };
                    let result = tool.execute(&tool_call.arguments, &ctx)?;

                    self.audit_logger.log(
                        session_id,
                        self.preset.as_str(),
                        "tool_result",
                        serde_json::json!({ "tool": tool_call.name, "success": result.success }),
                    )?;

                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: if result.success { result.output } else { result.error.unwrap_or_default() },
                        tool_calls: None,
                    });
                }
            }
        }

        self.audit_logger.log(
            session_id,
            self.preset.as_str(),
            "task_completed",
            serde_json::json!({ "steps": steps }),
        )?;

        Ok(final_answer)
    }
}
