use agent_core::runner::AgentRunner;
use agent_core::subagent::{SubagentOrchestrator, SubagentRole, SubagentTaskContract};
use policy::engine::{PolicyDecision, PolicyEngine};
use policy::preset::AgentPreset;
use providers::MockProvider;
use session::memory::MemoryManager;
use tools::sandbox::Sandbox;

#[test]
fn t33_preset_schema_and_deny_precedence() {
    let engine = PolicyEngine::new(AgentPreset::Unleashed, ".");
    let res = engine.evaluate_tool_call("shell_exec", None, Some("format C: /fs:NTFS"));
    assert!(matches!(res, PolicyDecision::Deny(_)), "T33: Deny rule must override Unleashed preset");
}

#[test]
fn t34_six_preset_effective_policies() {
    let presets = [
        AgentPreset::Heretic,
        AgentPreset::Uncensored,
        AgentPreset::Unrestricted,
        AgentPreset::Aggressive,
        AgentPreset::Abliterated,
        AgentPreset::Unleashed,
    ];
    for p in presets {
        let ep = p.effective_policy();
        assert_eq!(ep.preset_name, p.as_str());
        assert_eq!(ep.audit, "Always on");
        assert_eq!(ep.telemetry, "Opt-in only");
        assert!(ep.max_steps >= 25);
    }
}

#[test]
fn t36_uncensored_client_policy_boundary() {
    let ep = AgentPreset::Uncensored.effective_policy();
    assert!(ep.client_friction.contains("None"));
    assert!(ep.non_bypassable_boundaries.iter().any(|b| b.contains("Provider policy")));
}

#[test]
fn t37_unrestricted_sandbox_and_approvals() {
    let temp_dir = tempfile::tempdir().unwrap();
    let engine = PolicyEngine::new(AgentPreset::Unrestricted, temp_dir.path());
    // Safe command allows
    let res = engine.evaluate_tool_call("shell_exec", None, Some("git status"));
    assert_eq!(res, PolicyDecision::Allow);
}

#[test]
fn t41_tool_sandbox_security_traversal() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(temp_dir.path());
    let escape = sb.resolve_safe_path("../../../windows/system32");
    assert!(escape.is_err(), "T41: Traversal out of workspace must fail");
}

#[test]
fn t42_prompt_and_secret_redaction() {
    let secret_prompt = "User key is Bearer sk-998877665544332211aabbcc";
    let redacted = Sandbox::redact_secrets(secret_prompt);
    assert!(!redacted.contains("sk-998877665544332211aabbcc"));
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn t43_session_and_memory_compaction() {
    let messages = (0..20)
        .map(|i| serde_json::json!({ "role": "user", "content": format!("Turn {}", i) }))
        .collect::<Vec<_>>();
    let compacted = MemoryManager::compact_messages(&messages, 5);
    assert_eq!(compacted.len(), 5);
    assert!(compacted[0]["content"].as_str().unwrap().contains("Context Compaction"));
}

#[test]
fn t44_subagent_task_contract_isolation() {
    let contract = SubagentTaskContract {
        task_id: "task-01".to_string(),
        role: SubagentRole::Planner,
        goal: "Deconstruct multi-step refactor".to_string(),
        allowed_tools: vec!["file_read".to_string()],
        step_budget: 10,
    };
    let out = SubagentOrchestrator::execute_subtask(&contract).unwrap();
    assert!(out.success);
    assert_eq!(out.role, SubagentRole::Planner);
}

#[test]
fn t46_audit_logging_completeness() {
    let temp_dir = tempfile::tempdir().unwrap();
    let runner = AgentRunner::new(temp_dir.path(), AgentPreset::Aggressive);
    let mock_provider = MockProvider::new("Task done successfully.");

    let ans = runner.run_task("session-test-01", "Create file test.txt", &mock_provider).unwrap();
    assert_eq!(ans, "Task done successfully.");

    let events = runner.audit_logger.read_events().unwrap();
    assert!(events.iter().any(|e| e.event_type == "task_start"));
    assert!(events.iter().any(|e| e.event_type == "task_completed"));
}
