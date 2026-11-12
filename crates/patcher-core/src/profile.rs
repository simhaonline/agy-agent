use crate::rules::{PatchRule, RuleCategory};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatcherPreset {
    Heretic,
    Uncensored,
    Unrestricted,
    Aggressive,
    Abliterated,
    Unleashed,
}

impl PatcherPreset {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "heretic" => Some(PatcherPreset::Heretic),
            "uncensored" => Some(PatcherPreset::Uncensored),
            "unrestricted" => Some(PatcherPreset::Unrestricted),
            "aggressive" => Some(PatcherPreset::Aggressive),
            "abliterated" => Some(PatcherPreset::Abliterated),
            "unleashed" => Some(PatcherPreset::Unleashed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PatcherPreset::Heretic => "Heretic",
            PatcherPreset::Uncensored => "Uncensored",
            PatcherPreset::Unrestricted => "Unrestricted",
            PatcherPreset::Aggressive => "Aggressive",
            PatcherPreset::Abliterated => "Abliterated",
            PatcherPreset::Unleashed => "Unleashed",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PatcherPreset::Heretic => "Instruction-override and semantic-text profile (reduced stylistic friction).",
            PatcherPreset::Uncensored => "Harm, phish, brain, supercomplete, and semantic safety string filter profile.",
            PatcherPreset::Unrestricted => "Consent and instruction-label override profile.",
            PatcherPreset::Aggressive => "Subagent and autonomy label profile.",
            PatcherPreset::Abliterated => "Broadest legacy rule set (excluding telemetry/analytics). Note: NOT model-weight modification.",
            PatcherPreset::Unleashed => "All 104 legacy rules active.",
        }
    }

    pub fn filter_rules(&self, all_rules: &[PatchRule]) -> Vec<PatchRule> {
        match self {
            PatcherPreset::Heretic => all_rules
                .iter()
                .filter(|r| {
                    r.category == RuleCategory::InstructionOverrides
                        || (r.category == RuleCategory::SemanticBlocks
                            && (r.search_str.contains("Prohibited")
                                || r.search_str.contains("adhere to safety")
                                || r.search_str.contains("compromise safety")))
                })
                .cloned()
                .collect(),

            PatcherPreset::Uncensored => all_rules
                .iter()
                .filter(|r| {
                    r.category == RuleCategory::HarmCategories
                        || r.category == RuleCategory::PhishFilters
                        || r.category == RuleCategory::BrainFilters
                        || r.category == RuleCategory::SupercompleteFilters
                        || r.category == RuleCategory::SemanticBlocks
                })
                .cloned()
                .collect(),

            PatcherPreset::Unrestricted => all_rules
                .iter()
                .filter(|r| {
                    r.category == RuleCategory::InstructionOverrides
                        || r.category == RuleCategory::ConsentGates
                })
                .cloned()
                .collect(),

            PatcherPreset::Aggressive => all_rules
                .iter()
                .filter(|r| {
                    r.category == RuleCategory::InstructionOverrides
                        || (r.category == RuleCategory::SemanticBlocks
                            && (r.search_str.contains("subagent")
                                || r.search_str.contains("best-of-N")
                                || r.search_str.contains("AgentState")))
                })
                .cloned()
                .collect(),

            PatcherPreset::Abliterated => all_rules
                .iter()
                .filter(|r| {
                    r.category != RuleCategory::TelemetryFunctions
                        && r.category != RuleCategory::AnalyticsEndpoints
                })
                .cloned()
                .collect(),

            PatcherPreset::Unleashed => all_rules.to_vec(),
        }
    }
}
