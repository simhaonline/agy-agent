use common::encoding::{encode_latin1, normalize_replacement};
use common::hash::{compute_sha256_bytes, compute_sha256_file};
use patcher_core::backup::BackupManager;
use patcher_core::download::ReleaseAcquisition;
use patcher_core::engine::PatchEngine;
use patcher_core::pe::PeInspection;
use patcher_core::profile::PatcherPreset;
use patcher_core::report::LegacyReport;
use patcher_core::restore::RestoreManager;
use patcher_core::rules::{derive_telemetry_replacement, get_baseline_104_rules, RuleCategory};
use patcher_core::transaction::TransactionalApplier;
use patcher_core::verify::Verifier;
use std::fs;

#[test]
fn t01_baseline_inventory_and_hash_verification() {
    let rules = get_baseline_104_rules();
    assert_eq!(rules.len(), 104, "T01: Must account for all 104 rules");

    // Check category counts matching Section 1.2
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::HarmCategories).count(), 7);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::PhishFilters).count(), 4);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::BrainFilters).count(), 2);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::SupercompleteFilters).count(), 4);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::InstructionOverrides).count(), 13);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::SemanticBlocks).count(), 40);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::TelemetryFunctions).count(), 22);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::AnalyticsEndpoints).count(), 8);
    assert_eq!(rules.iter().filter(|r| r.category == RuleCategory::ConsentGates).count(), 4);
}

#[test]
fn t02_padding_and_truncation_exact_semantics() {
    assert_eq!(normalize_replacement(5, b"A"), b"A    ");
    assert_eq!(normalize_replacement(5, b"ABCDE"), b"ABCDE");
    assert_eq!(normalize_replacement(5, b"ABCDEFG"), b"ABCDE");
}

#[test]
fn t03_latin1_encoding_and_rejection() {
    assert!(encode_latin1("valid latin1 ©").is_ok());
    assert!(encode_latin1("invalid unicode 🚀").is_err());
}

#[test]
fn t06_telemetry_precedence_and_no_ops() {
    // Record -> Void
    assert_eq!(derive_telemetry_replacement("RecordCommandUsage"), "VoidCommandUsage");
    // Trajectory override
    assert_eq!(derive_telemetry_replacement("RecordFullTrajectoryAnalytics"), "RecordFullDisabledAnalytics");
    // Sentry override
    assert_eq!(derive_telemetry_replacement("log_timeout_errors_instead_of_sentry"), "log_timeout_errors_instead_of_local");

    let rules = get_baseline_104_rules();
    let noops: Vec<_> = rules.iter().filter(|r| r.is_noop).collect();
    assert_eq!(noops.len(), 8, "T06: Exactly 8 telemetry no-op rules required");
}

#[test]
fn t08_shadowing_cases() {
    let rules = get_baseline_104_rules();
    let conflicts = PatchEngine::detect_shadow_conflicts(&rules);
    assert!(conflicts.iter().any(|c| c.substring == "allow_agent_access_gitignore_files"));
    assert!(conflicts.iter().any(|c| c.substring == "RESTRICTED"));
    assert!(conflicts.iter().any(|c| c.substring == "RecordFullTrajectoryAnalytics"));
    assert!(conflicts.iter().any(|c| c.substring == "RecordGitTelemetry"));
}

#[test]
fn t09_synthetic_all_rule_oracle_parity() {
    let rules = get_baseline_104_rules();
    let searches: Vec<Vec<u8>> = rules.iter().map(|r| r.search_bytes.clone()).collect();
    let mut fixture = Vec::new();
    for (i, s) in searches.iter().enumerate() {
        if i > 0 { fixture.push(0u8); }
        fixture.extend_from_slice(s);
    }

    assert_eq!(
        compute_sha256_bytes(&fixture),
        "6da67d7db743e5f560a7dc02f2e02cd87b843ce7538bbbbf401c68a58d3f054b"
    );

    let (output, events, metrics) = PatchEngine::simulate(&fixture, &rules);
    assert_eq!(output.len(), fixture.len());
    assert_eq!(events.len(), 104);
    assert_eq!(metrics.noop_events, 8);
    assert_eq!(
        compute_sha256_bytes(&output),
        "e72031619b9947630c3f9784afa7f638a1426d99e622600997796f12a7b282f1"
    );
}

#[test]
fn t10_legacy_report_schema() {
    let rules = get_baseline_104_rules();
    let (_output, events, _) = PatchEngine::simulate(b"test buffer", &rules);
    let rep = LegacyReport::from_events(&events, "orig_hash", "new_hash");
    let json = serde_json::to_value(&rep).unwrap();
    assert!(json.get("patches").is_some());
    assert!(json.get("count").is_some());
    assert!(json.get("orig_hash").is_some());
    assert!(json.get("new_hash").is_some());
}

#[test]
fn t14_backup_and_stale_prevention() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("sample.exe");
    fs::write(&target, b"initial executable bytes").unwrap();

    let backup_rec = BackupManager::create_verified_backup(&target, None).unwrap();
    assert!(backup_rec.backup_path.exists());
    assert_eq!(backup_rec.original_sha256, compute_sha256_file(&target).unwrap());

    // Mutate source
    fs::write(&target, b"modified executable bytes").unwrap();

    // Re-creating backup should fail to prevent stale baseline reuse
    let stale_attempt = BackupManager::create_verified_backup(&target, None);
    assert!(stale_attempt.is_err(), "T14: Must prevent stale backup reuse");
}

#[test]
fn t20_restore_fidelity() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("app.exe");
    fs::write(&target, b"original clean file").unwrap();

    let backup_rec = BackupManager::create_verified_backup(&target, None).unwrap();

    // Damage target
    fs::write(&target, b"corrupted damaged file").unwrap();

    let restored = RestoreManager::restore(&target, None).unwrap();
    assert_eq!(
        compute_sha256_file(&restored).unwrap(),
        backup_rec.original_sha256
    );
}

#[test]
fn t23_zip_traversal_protection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut zip_buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("../../../evil.exe", options).unwrap();
        use std::io::Write;
        writer.write_all(b"malicious").unwrap();
        writer.finish().unwrap();
    }

    let extract_res = ReleaseAcquisition::extract_member_safe(
        &zip_buf,
        "evil.exe",
        temp_dir.path(),
        10000,
    );
    assert!(extract_res.is_err(), "T23: Must block zip traversal attempt");
}

#[test]
fn t12_patcher_preset_filtering() {
    let rules = get_baseline_104_rules();
    let unleashed_rules = PatcherPreset::Unleashed.filter_rules(&rules);
    assert_eq!(unleashed_rules.len(), 104);

    let heretic_rules = PatcherPreset::Heretic.filter_rules(&rules);
    assert!(heretic_rules.len() < 104);
}

#[test]
fn t13_pe_rejection_on_invalid_data() {
    let invalid_dos = b"NOT_A_PE_HEADER";
    let res = PeInspection::parse(invalid_dos);
    assert!(res.is_err(), "T13: Non-PE header must be rejected");
}

#[test]
fn t18_transactional_apply() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("out.exe");
    let content = b"transactional content payload";
    let hash = compute_sha256_bytes(content);

    let res = TransactionalApplier::apply(&target, content, &hash);
    assert!(res.is_ok(), "T18: Transactional apply must succeed");
    assert_eq!(compute_sha256_file(&target).unwrap(), hash);
}

#[test]
fn t24_verifier_validation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target = temp_dir.path().join("verified.exe");
    let content = b"verified byte stream";
    fs::write(&target, content).unwrap();

    let hash = compute_sha256_bytes(content);
    let ok_res = Verifier::verify_file(&target, &hash, Some(content.len() as u64)).unwrap();
    assert!(ok_res.is_valid);

    let bad_res = Verifier::verify_file(&target, "0000000000000000000000000000000000000000000000000000000000000000", None).unwrap();
    assert!(!bad_res.is_valid);
}
