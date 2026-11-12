use crate::engine::{PatchEngine, PatchEvent, PatchMetrics, ShadowConflict};
use crate::pe::PeInspection;
use crate::profile::PatcherPreset;
use crate::rules::PatchRule;
use common::hash::compute_sha256_bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchPlan {
    pub input_hash: String,
    pub input_size: usize,
    pub config_hash: String,
    pub preset: Option<String>,
    pub is_compat_mode: bool,
    pub total_rules: usize,
    pub events: Vec<PatchEvent>,
    pub metrics: PatchMetrics,
    pub expected_output_hash: String,
    pub shadow_conflicts: Vec<ShadowConflict>,
    pub code_section_violations: Vec<usize>,
    pub warnings: Vec<String>,
}

impl PatchPlan {
    pub fn build(
        data: &[u8],
        rules: &[PatchRule],
        preset: Option<PatcherPreset>,
        config_hash: &str,
        is_compat_mode: bool,
        enforce_pe_protection: bool,
    ) -> Self {
        let input_hash = compute_sha256_bytes(data);
        let input_size = data.len();

        let active_rules = if let Some(p) = preset {
            p.filter_rules(rules)
        } else {
            rules.to_vec()
        };

        let shadow_conflicts = PatchEngine::detect_shadow_conflicts(&active_rules);
        let (output_bytes, events, metrics) = PatchEngine::simulate(data, &active_rules);
        let expected_output_hash = compute_sha256_bytes(&output_bytes);

        let mut code_section_violations = Vec::new();
        let mut warnings = Vec::new();

        if let Ok(pe) = PeInspection::parse(data) {
            for event in &events {
                if event.is_changed && pe.is_in_code_section(event.off) {
                    code_section_violations.push(event.off);
                }
            }
            if !code_section_violations.is_empty() {
                warnings.push(format!(
                    "Plan proposes {} modifications inside executable code (.text) sections",
                    code_section_violations.len()
                ));
            }
            if pe.has_authenticode_table {
                warnings.push(
                    "Binary contains an Authenticode digital signature directory; in-place patching invalidates signature trust"
                        .to_string(),
                );
            }
        }

        if !is_compat_mode && enforce_pe_protection && !code_section_violations.is_empty() {
            warnings.push(
                "Strict profile mode: code section modifications are blocked without explicit override"
                    .to_string(),
            );
        }

        PatchPlan {
            input_hash,
            input_size,
            config_hash: config_hash.to_string(),
            preset: preset.map(|p| p.as_str().to_string()),
            is_compat_mode,
            total_rules: active_rules.len(),
            events,
            metrics,
            expected_output_hash,
            shadow_conflicts,
            code_section_violations,
            warnings,
        }
    }

    pub fn to_summary(&self) -> String {
        format!(
            "Patch Plan Summary:\n  Input SHA-256: {} ({} bytes)\n  Expected Output SHA-256: {}\n  Active Rules: {}\n  Total Events: {} (Changed: {}, No-ops: {})\n  Distinct Byte Offsets Changed: {}\n  Zero-match Rules: {}\n  Shadow Conflicts: {}\n  Code Section Hits: {}\n  Warnings: {}",
            self.input_hash,
            self.input_size,
            self.expected_output_hash,
            self.total_rules,
            self.metrics.total_events,
            self.metrics.changed_events,
            self.metrics.noop_events,
            self.metrics.distinct_offsets,
            self.metrics.zero_match_rules,
            self.shadow_conflicts.len(),
            self.code_section_violations.len(),
            self.warnings.len()
        )
    }
}
