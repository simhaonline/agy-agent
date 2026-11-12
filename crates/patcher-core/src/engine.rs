use crate::rules::PatchRule;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchEvent {
    pub off: usize,
    pub len: usize,
    pub rule_id: String,
    pub r#type: String,
    pub desc: String,
    pub before_bytes: Vec<u8>,
    pub after_bytes: Vec<u8>,
    pub is_changed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchMetrics {
    pub total_events: usize,
    pub changed_events: usize,
    pub noop_events: usize,
    pub zero_match_rules: usize,
    pub distinct_offsets: usize,
    pub changed_bytes_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowConflict {
    pub earlier_rule_id: String,
    pub later_rule_id: String,
    pub substring: String,
    pub container: String,
}

pub struct PatchEngine;

impl PatchEngine {
    /// Detect static shadowing conflicts across rules where an earlier search string
    /// is a substring of a later search string.
    pub fn detect_shadow_conflicts(rules: &[PatchRule]) -> Vec<ShadowConflict> {
        let mut conflicts = Vec::new();
        for i in 0..rules.len() {
            for j in (i + 1)..rules.len() {
                let early = &rules[i];
                let late = &rules[j];
                if late.search_str.contains(&early.search_str) && early.search_str != late.search_str {
                    conflicts.push(ShadowConflict {
                        earlier_rule_id: early.id.clone(),
                        later_rule_id: late.id.clone(),
                        substring: early.search_str.clone(),
                        container: late.search_str.clone(),
                    });
                }
            }
        }
        conflicts
    }

    /// Simulate patching without modifying the original buffer, returning the resulting buffer,
    /// match events, and detailed metrics.
    pub fn simulate(data: &[u8], rules: &[PatchRule]) -> (Vec<u8>, Vec<PatchEvent>, PatchMetrics) {
        let mut buffer = data.to_vec();
        let mut events = Vec::new();
        let mut zero_match_rules = 0;
        let mut changed_offsets = std::collections::BTreeSet::new();

        for rule in rules {
            let mut off = 0;
            let mut matches_for_rule = 0;
            let s_len = rule.search_bytes.len();

            if s_len == 0 {
                continue;
            }

            while off + s_len <= buffer.len() {
                if let Some(pos) = buffer[off..].windows(s_len).position(|w| w == rule.search_bytes.as_slice()) {
                    let absolute_pos = off + pos;
                    let before = buffer[absolute_pos..absolute_pos + s_len].to_vec();
                    let after = rule.effective_bytes.clone();
                    let is_changed = before != after;

                    buffer[absolute_pos..absolute_pos + s_len].copy_from_slice(&after);

                    if is_changed {
                        for byte_idx in absolute_pos..(absolute_pos + s_len) {
                            changed_offsets.insert(byte_idx);
                        }
                    }

                    events.push(PatchEvent {
                        off: absolute_pos,
                        len: s_len,
                        rule_id: rule.id.clone(),
                        r#type: rule.report_type.clone(),
                        desc: rule.description.clone(),
                        before_bytes: before,
                        after_bytes: after,
                        is_changed,
                    });

                    matches_for_rule += 1;
                    off = absolute_pos + s_len;
                } else {
                    break;
                }
            }

            if matches_for_rule == 0 {
                zero_match_rules += 1;
            }
        }

        let total_events = events.len();
        let changed_events = events.iter().filter(|e| e.is_changed).count();
        let noop_events = events.iter().filter(|e| !e.is_changed).count();
        let distinct_offsets = changed_offsets.len();

        let metrics = PatchMetrics {
            total_events,
            changed_events,
            noop_events,
            zero_match_rules,
            distinct_offsets,
            changed_bytes_count: distinct_offsets,
        };

        (buffer, events, metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::get_baseline_104_rules;
    use common::hash::compute_sha256_bytes;

    #[test]
    fn test_synthetic_fixture_oracle_parity() {
        let rules = get_baseline_104_rules();
        let mut searches: Vec<Vec<u8>> = Vec::with_capacity(rules.len());
        for r in &rules {
            searches.push(r.search_bytes.clone());
        }

        // Interleave with NUL byte exactly like Appendix D
        let mut fixture = Vec::new();
        for (i, s) in searches.iter().enumerate() {
            if i > 0 {
                fixture.push(0u8);
            }
            fixture.extend_from_slice(s);
        }

        let input_hash = compute_sha256_bytes(&fixture);
        assert_eq!(
            input_hash,
            "6da67d7db743e5f560a7dc02f2e02cd87b843ce7538bbbbf401c68a58d3f054b",
            "Input synthetic fixture hash must match Appendix D exactly"
        );

        let (output, events, metrics) = PatchEngine::simulate(&fixture, &rules);

        assert_eq!(output.len(), fixture.len(), "Length must be preserved");
        assert_eq!(events.len(), 104, "Total patch events must be 104");
        assert_eq!(metrics.total_events, 104);
        assert_eq!(metrics.noop_events, 8, "Must have exactly 8 no-op events");

        let output_hash = compute_sha256_bytes(&output);
        assert_eq!(
            output_hash,
            "e72031619b9947630c3f9784afa7f638a1426d99e622600997796f12a7b282f1",
            "Output synthetic fixture hash must match Appendix D exactly"
        );
    }

    #[test]
    fn test_shadow_conflicts_detection() {
        let rules = get_baseline_104_rules();
        let conflicts = PatchEngine::detect_shadow_conflicts(&rules);
        assert!(!conflicts.is_empty());

        let has_gitignore_shadow = conflicts.iter().any(|c| {
            c.substring == "allow_agent_access_gitignore_files"
                && c.container == "_allow_agent_access_gitignore_files"
        });
        assert!(has_gitignore_shadow, "Must detect gitignore shadowing");

        let has_restricted_shadow = conflicts.iter().any(|c| {
            c.substring == "RESTRICTED"
                && c.container == "ESSENTIAL_USE_CONSENT_STATE_UNRESTRICTED"
        });
        assert!(has_restricted_shadow, "Must detect RESTRICTED consent shadowing");
    }
}
