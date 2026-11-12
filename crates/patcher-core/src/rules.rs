use common::encoding::{encode_latin1, normalize_replacement};
use common::error::CommonError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleCategory {
    HarmCategories,
    PhishFilters,
    BrainFilters,
    SupercompleteFilters,
    InstructionOverrides,
    SemanticBlocks,
    TelemetryFunctions,
    AnalyticsEndpoints,
    ConsentGates,
}

impl RuleCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleCategory::HarmCategories => "harm_categories",
            RuleCategory::PhishFilters => "phish_filters",
            RuleCategory::BrainFilters => "brain_filters",
            RuleCategory::SupercompleteFilters => "supercomplete_filters",
            RuleCategory::InstructionOverrides => "instruction_overrides",
            RuleCategory::SemanticBlocks => "semantic_blocks",
            RuleCategory::TelemetryFunctions => "telemetry_functions",
            RuleCategory::AnalyticsEndpoints => "analytics_endpoints",
            RuleCategory::ConsentGates => "consent_gates",
        }
    }

    pub fn default_report_type(&self) -> &'static str {
        match self {
            RuleCategory::HarmCategories
            | RuleCategory::PhishFilters
            | RuleCategory::BrainFilters
            | RuleCategory::SupercompleteFilters => "enum",
            RuleCategory::InstructionOverrides => "override",
            RuleCategory::SemanticBlocks => "semantic",
            RuleCategory::TelemetryFunctions => "telemetry",
            RuleCategory::AnalyticsEndpoints => "analytics",
            RuleCategory::ConsentGates => "consent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchRule {
    pub id: String,
    pub category: RuleCategory,
    pub report_type: String,
    pub search_str: String,
    pub search_bytes: Vec<u8>,
    pub requested_repl: String,
    pub effective_bytes: Vec<u8>,
    pub is_noop: bool,
    pub description: String,
}

pub fn derive_telemetry_replacement(orig: &str) -> String {
    let mut repl = orig.replace("Record", "Void").replace("record", "void");
    if orig.to_lowercase().contains("trajectory") {
        repl = orig
            .replace("trajectory", "disabled")
            .replace("Trajectory", "Disabled");
    }
    if orig.to_lowercase().contains("sentry") {
        repl = orig.replace("sentry", "local");
    }
    repl
}

pub fn build_rule(
    category: RuleCategory,
    ordinal: usize,
    search: &str,
    requested: &str,
) -> Result<PatchRule, CommonError> {
    let id = format!("{}:{:02}", category.as_str(), ordinal);
    let s_bytes = encode_latin1(search)?;
    let r_bytes = encode_latin1(requested)?;
    let effective = normalize_replacement(s_bytes.len(), &r_bytes);
    let is_noop = s_bytes == effective;
    let report_type = category.default_report_type().to_string();

    let desc = if category == RuleCategory::SemanticBlocks {
        let s_preview: String = search.chars().take(50).collect();
        let r_preview: String = requested.chars().take(50).collect();
        format!("{}->{}", s_preview, r_preview)
    } else {
        format!("{}->{}", search, requested)
    };

    Ok(PatchRule {
        id,
        category,
        report_type,
        search_str: search.to_string(),
        search_bytes: s_bytes,
        requested_repl: requested.to_string(),
        effective_bytes: effective,
        is_noop,
        description: desc,
    })
}

/// The 104 rules compiled in exact execution order.
pub fn get_baseline_104_rules() -> Vec<PatchRule> {
    let raw_json = include_str!("targets.json");
    parse_targets_json(raw_json).expect("Baseline targets.json must parse into 104 rules")
}

pub fn parse_targets_json(json_str: &str) -> Result<Vec<PatchRule>, CommonError> {
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| CommonError::Serialization(e))?;

    let mut rules = Vec::with_capacity(104);

    // 1. harm_categories (dict)
    if let Some(obj) = value.get("harm_categories").and_then(|v| v.as_object()) {
        for (idx, (k, v)) in obj.iter().enumerate() {
            let r_str = v.as_str().unwrap_or("");
            rules.push(build_rule(RuleCategory::HarmCategories, idx + 1, k, r_str)?);
        }
    }

    // 2. phish_filters (dict)
    if let Some(obj) = value.get("phish_filters").and_then(|v| v.as_object()) {
        for (idx, (k, v)) in obj.iter().enumerate() {
            let r_str = v.as_str().unwrap_or("");
            rules.push(build_rule(RuleCategory::PhishFilters, idx + 1, k, r_str)?);
        }
    }

    // 3. brain_filters (dict)
    if let Some(obj) = value.get("brain_filters").and_then(|v| v.as_object()) {
        for (idx, (k, v)) in obj.iter().enumerate() {
            let r_str = v.as_str().unwrap_or("");
            rules.push(build_rule(RuleCategory::BrainFilters, idx + 1, k, r_str)?);
        }
    }

    // 4. supercomplete_filters (dict)
    if let Some(obj) = value.get("supercomplete_filters").and_then(|v| v.as_object()) {
        for (idx, (k, v)) in obj.iter().enumerate() {
            let r_str = v.as_str().unwrap_or("");
            rules.push(build_rule(RuleCategory::SupercompleteFilters, idx + 1, k, r_str)?);
        }
    }

    // 5. instruction_overrides (dict)
    if let Some(obj) = value.get("instruction_overrides").and_then(|v| v.as_object()) {
        for (idx, (k, v)) in obj.iter().enumerate() {
            let r_str = v.as_str().unwrap_or("");
            rules.push(build_rule(RuleCategory::InstructionOverrides, idx + 1, k, r_str)?);
        }
    }

    // 6. semantic_blocks (list of 2-element lists)
    if let Some(arr) = value.get("semantic_blocks").and_then(|v| v.as_array()) {
        for (idx, item) in arr.iter().enumerate() {
            if let Some(pair) = item.as_array() {
                if pair.len() >= 2 {
                    let s_str = pair[0].as_str().unwrap_or("");
                    let r_str = pair[1].as_str().unwrap_or("");
                    rules.push(build_rule(RuleCategory::SemanticBlocks, idx + 1, s_str, r_str)?);
                }
            }
        }
    }

    // 7. telemetry_functions (list of strings)
    if let Some(arr) = value.get("telemetry_functions").and_then(|v| v.as_array()) {
        for (idx, item) in arr.iter().enumerate() {
            if let Some(orig) = item.as_str() {
                let repl = derive_telemetry_replacement(orig);
                rules.push(build_rule(RuleCategory::TelemetryFunctions, idx + 1, orig, &repl)?);
            }
        }
    }

    // 8. analytics_endpoints (list of strings, repl = "void.local")
    if let Some(arr) = value.get("analytics_endpoints").and_then(|v| v.as_array()) {
        for (idx, item) in arr.iter().enumerate() {
            if let Some(orig) = item.as_str() {
                rules.push(build_rule(RuleCategory::AnalyticsEndpoints, idx + 1, orig, "void.local")?);
            }
        }
    }

    // 9. consent_gates (dict)
    if let Some(obj) = value.get("consent_gates").and_then(|v| v.as_object()) {
        for (idx, (k, v)) in obj.iter().enumerate() {
            let r_str = v.as_str().unwrap_or("");
            rules.push(build_rule(RuleCategory::ConsentGates, idx + 1, k, r_str)?);
        }
    }

    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_104_rules_count() {
        let rules = get_baseline_104_rules();
        assert_eq!(rules.len(), 104, "Must have exactly 104 baseline rules");

        let noops: Vec<_> = rules.iter().filter(|r| r.is_noop).collect();
        assert_eq!(noops.len(), 8, "Must have exactly 8 telemetry no-op rules");

        let noop_names = [
            "GetImageGeneration",
            "StepScopedSubtrajectoryUpdatesEntry",
            "send_command_input",
            "list_browser_pages",
            "browser_move_mouse",
            "browser_scroll_dom",
            "browser_mouse_down",
            "Custom action name",
        ];
        for name in noop_names {
            assert!(
                noops.iter().any(|r| r.search_str == name),
                "Missing expected no-op rule: {}",
                name
            );
        }
    }

    #[test]
    fn test_telemetry_derivation() {
        assert_eq!(derive_telemetry_replacement("RecordCommandUsage"), "VoidCommandUsage");
        assert_eq!(
            derive_telemetry_replacement("RecordFullTrajectoryAnalytics"),
            "RecordFullDisabledAnalytics"
        );
        assert_eq!(
            derive_telemetry_replacement("log_timeout_errors_instead_of_sentry"),
            "log_timeout_errors_instead_of_local"
        );
        assert_eq!(derive_telemetry_replacement("GetImageGeneration"), "GetImageGeneration");
    }
}
