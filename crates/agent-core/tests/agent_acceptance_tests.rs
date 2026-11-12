use agent_core::account::{AccountProfile, AccountStore};
use agent_core::auth::AuthCredentialManager;
use agent_core::gateway::{
    format_claude_response, format_gemini_response, format_openai_response,
    translate_claude_request, translate_gemini_request, translate_openai_request,
};
use agent_core::manager::ClientManager;
use agent_core::runner::AgentRunner;
use agent_core::staff::{StaffRole, StaffRunner};
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

#[test]
fn t47_account_switch_and_cooldown_failover() {
    let mut store = AccountStore::new();
    let acc_a = AccountProfile::new("Claude-Primary", "claude", "sk-ant-key-1", None, Some("claude-3-5-sonnet".into()), None);
    let acc_b = AccountProfile::new("OpenAI-Backup", "openai", "sk-openai-key-2", None, Some("gpt-4o".into()), None);

    store.add_account(acc_a).unwrap();
    store.add_account(acc_b).unwrap();

    // Acc A is active by default
    assert_eq!(store.get_active_account().unwrap().name, "Claude-Primary");
    assert_eq!(store.get_next_available_account().unwrap().name, "Claude-Primary");

    // Put Acc A into cooldown (simulating 429 cooling)
    store.mark_cooldown("Claude-Primary", 120).unwrap();
    assert!(store.get_account("Claude-Primary").unwrap().is_cooling());

    // Failover should immediately select OpenAI-Backup
    let fallback = store.get_next_available_account().unwrap();
    assert_eq!(fallback.name, "OpenAI-Backup");

    // Switch active manually
    store.switch_active("OpenAI-Backup").unwrap();
    assert_eq!(store.get_active_account().unwrap().name, "OpenAI-Backup");
}

#[test]
fn t48_gateway_multi_format_translation() {
    // 1. OpenAI format
    let openai_req = serde_json::json!({
        "model": "gpt-4o",
        "messages": [
            { "role": "system", "content": "You are a pure Rust agent." },
            { "role": "user", "content": "How do you handle 429?" }
        ]
    });
    let (model, msgs) = translate_openai_request(&openai_req).unwrap();
    assert_eq!(model, "gpt-4o");
    assert_eq!(msgs.len(), 2);
    let openai_resp = format_openai_response("chatcmpl-test", "gpt-4o", "With cooldown rotation", 24);
    assert_eq!(openai_resp["object"], "chat.completion");
    assert_eq!(openai_resp["choices"][0]["message"]["content"], "With cooldown rotation");

    // 2. Claude format
    let claude_req = serde_json::json!({
        "model": "claude-3-5-sonnet",
        "system": "System instructions",
        "messages": [
            { "role": "user", "content": "Review this PR" }
        ]
    });
    let (c_model, c_msgs) = translate_claude_request(&claude_req).unwrap();
    assert_eq!(c_model, "claude-3-5-sonnet");
    assert_eq!(c_msgs.len(), 2);
    let claude_resp = format_claude_response("msg_123", "claude-3-5-sonnet", "Looks solid", 18);
    assert_eq!(claude_resp["type"], "message");
    assert_eq!(claude_resp["content"][0]["text"], "Looks solid");

    // 3. Gemini format
    let gemini_req = serde_json::json!({
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": "Explain quantum computing" }]
            }
        ]
    });
    let (g_model, g_msgs) = translate_gemini_request(&gemini_req).unwrap();
    assert_eq!(g_model, "gemini-1.5-pro");
    assert_eq!(g_msgs[0].content, "Explain quantum computing");
    let gemini_resp = format_gemini_response("Qubits and superposition", 32);
    assert_eq!(gemini_resp["candidates"][0]["content"]["parts"][0]["text"], "Qubits and superposition");
}

#[test]
fn t49_staff_git_dirty_tree_safety_guardrail() {
    let temp_dir = tempfile::tempdir().unwrap();
    let runner = AgentRunner::new(temp_dir.path(), AgentPreset::Aggressive);
    let mock = MockProvider::new("Staff response completed.");

    // Staff Ask
    let ask_verdict = StaffRunner::run_ask(temp_dir.path(), "What is the project architecture?", &runner, &mock).unwrap();
    assert_eq!(ask_verdict.role, StaffRole::Ask);

    // Staff Research
    let res_verdict = StaffRunner::run_research(temp_dir.path(), "Authentication modules", &runner, &mock).unwrap();
    assert_eq!(res_verdict.role, StaffRole::Researcher);

    // Staff Implement in clean/empty non-git directory passes
    let imp_verdict = StaffRunner::run_implement(temp_dir.path(), "Implement logger", false, &runner, &mock).unwrap();
    assert_eq!(imp_verdict.role, StaffRole::Implementer);
    assert!(imp_verdict.dirty_tree_checked);
}

#[test]
fn t50_client_manager_diagnostics() {
    let report = ClientManager::inspect_environment(Some(8080));
    // Candidate paths should exist and list running processes should return safely
    assert!(!ClientManager::candidate_paths().is_empty());
    let _procs = ClientManager::list_running_processes();
    assert_eq!(report.installed, ClientManager::detect_installation().is_some());
}

#[test]
fn t51_account_quota_and_token_tracking() {
    let mut store = AccountStore::new();
    let acc = AccountProfile::new("DeepSeek-Tier1", "deepseek", "sk-ds-key", None, Some("deepseek-coder".into()), Some(5000));
    store.add_account(acc).unwrap();

    store.record_usage("DeepSeek-Tier1", 1500, false).unwrap();
    assert_eq!(store.get_account("DeepSeek-Tier1").unwrap().quota_used, 1500);
    assert_eq!(store.get_account("DeepSeek-Tier1").unwrap().request_count, 1);
    assert_eq!(store.get_account("DeepSeek-Tier1").unwrap().error_count, 0);

    store.record_usage("DeepSeek-Tier1", 4000, true).unwrap();
    let updated = store.get_account("DeepSeek-Tier1").unwrap();
    assert_eq!(updated.quota_used, 5500);
    assert_eq!(updated.request_count, 2);
    assert_eq!(updated.error_count, 1);
    assert!(updated.is_quota_exceeded());
    assert!(!updated.is_usable());
}

#[test]
fn t52_auth_pool_ranking_and_health_scoring() {
    let mut store = AccountStore::new();
    let acc1 = AccountProfile::new("Claude-Enterprise", "claude", "sk-ant-ent", None, None, Some(500000))
        .with_email("team@corp.com")
        .with_plan_tier("Enterprise");
    let acc2 = AccountProfile::new("OpenAI-Pro", "openai", "sk-proj-pro", None, None, Some(100000))
        .with_email("dev@corp.com")
        .with_plan_tier("Pro");

    store.add_account(acc1).unwrap();
    store.add_account(acc2).unwrap();

    // Record some usage and errors on OpenAI
    store.record_usage("OpenAI-Pro", 85000, true).unwrap();

    let ranking = store.rank_accounts();
    assert_eq!(ranking.len(), 2);
    assert_eq!(ranking[0].name, "Claude-Enterprise");
    assert!(ranking[0].health_score >= 95);
    assert!(ranking[1].health_score < ranking[0].health_score);
    assert_eq!(ranking[0].plan_tier, "Enterprise");
}

#[test]
fn t53_auth_atomic_switch_pre_backup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let backup_dir = temp_dir.path().join(".agy").join("auth_backups");
    let dummy_cli_token = temp_dir.path().join("dummy_token.json");

    // Seed dummy live credentials
    std::fs::write(&dummy_cli_token, "original-live-token-secret").unwrap();

    // Verify backup creation
    let backup = AuthCredentialManager::backup_token_file(&dummy_cli_token, &backup_dir, "test-backup").unwrap();
    assert!(backup.path.exists());
    assert_eq!(backup.file_size, "original-live-token-secret".len() as u64);

    // Atomically overwrite dummy token
    AuthCredentialManager::write_token_atomically(&dummy_cli_token, "switched-token-new").unwrap();
    assert_eq!(std::fs::read_to_string(&dummy_cli_token).unwrap(), "switched-token-new");

    // Verify restore works cleanly
    AuthCredentialManager::write_token_atomically(&dummy_cli_token, &std::fs::read_to_string(&backup.path).unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(&dummy_cli_token).unwrap(), "original-live-token-secret");
}

#[test]
fn t54_auth_quota_aware_auto_rotation() {
    let mut store = AccountStore::new();
    let mut primary = AccountProfile::new("ActiveNearExhaustion", "openai", "sk-p1", None, None, Some(1000));
    primary.is_active = true;
    primary.quota_used = 950; // 95% used (> 90% threshold)

    let standby = AccountProfile::new("HealthyStandby", "claude", "sk-s1", None, None, Some(100000))
        .with_plan_tier("Pro");

    store.accounts.push(primary);
    store.accounts.push(standby);

    // Check should_rotate at 90% threshold
    assert!(store.should_rotate(90.0).is_some());

    // Trigger auto-rotation
    let rot = store.auto_rotate(90.0).unwrap().unwrap();
    assert_eq!(rot.from, "ActiveNearExhaustion");
    assert_eq!(rot.to, "HealthyStandby");
    assert_eq!(store.get_active_account().unwrap().name, "HealthyStandby");
}
