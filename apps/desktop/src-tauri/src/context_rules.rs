//! Context-aware automatic profile switching rules.
//!
//! [`ContextRules`] is loaded from `context-rules.json` in the app config
//! directory at startup and updated via the `save_context_rules` Tauri command.
//! [`ContextRules::evaluate`] is called by the focus monitor task whenever the
//! focused window changes.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::focus_monitor::FocusedWindow;

// ── Types ─────────────────────────────────────────────────────────────────────

/// A single rule that maps a window identity pattern to a profile `layer_id`.
///
/// At least one of [`match_app`](Self::match_app) or
/// [`match_title`](Self::match_title) must be `Some`. If both are set, **both**
/// must match (AND semantics). Matching is case-insensitive substring search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRule {
    /// Human-readable label shown in the UI (e.g. `"VS Code"`).
    pub name: String,
    /// `layer_id` of the profile to activate when this rule matches.
    pub layer_id: String,
    /// Pattern matched against the application name. `None` = match any app.
    pub match_app: Option<String>,
    /// Pattern matched against the window title. `None` = match any title.
    pub match_title: Option<String>,
}

fn default_version() -> u32 {
    1
}

/// An ordered list of [`ContextRule`]s, serialised to / from `context-rules.json`.
///
/// Rules are evaluated in list order; the first match wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRules {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub rules: Vec<ContextRule>,
}

impl Default for ContextRules {
    fn default() -> Self {
        Self {
            version: 1,
            rules: Vec::new(),
        }
    }
}

// ── Load / save ───────────────────────────────────────────────────────────────

impl ContextRules {
    /// Load rules from `path`.
    ///
    /// - Missing file → returns empty rules (not an error).
    /// - Malformed JSON → logs a warning and returns empty rules.
    pub fn load(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                log::warn!("context_rules: could not read {path:?}: {e}");
                return Self::default();
            }
        };
        match serde_json::from_str(&content) {
            Ok(rules) => rules,
            Err(e) => {
                log::warn!("context_rules: malformed JSON in {path:?}: {e}");
                Self::default()
            }
        }
    }

    /// Serialise and write rules to `path`.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialisation failed: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("write failed: {e}"))
    }

    // ── Validation ────────────────────────────────────────────────────────────

    /// Validate the rule list.
    ///
    /// Returns `Ok(())` if all rules are well-formed, or an `Err` describing
    /// the first violation found.
    pub fn validate(&self) -> Result<(), String> {
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.name.trim().is_empty() {
                return Err(format!("rule {i} has an empty name"));
            }
            if rule.layer_id.trim().is_empty() {
                return Err(format!("rule {i} ('{}') has an empty layer_id", rule.name));
            }
            if rule.match_app.is_none() && rule.match_title.is_none() {
                return Err(format!(
                    "rule {i} ('{}') has no match patterns; \
                     at least one of match_app or match_title is required",
                    rule.name
                ));
            }
        }
        Ok(())
    }

    // ── Evaluation ────────────────────────────────────────────────────────────

    /// Evaluate rules against `window` and return the first matching rule.
    ///
    /// Returns `None` if:
    /// - No rule matches.
    /// - The first matching rule's `layer_id` equals `active_layer_id` (already
    ///   active — no action needed).
    ///
    /// Matching is **case-insensitive substring** search: a pattern matches if
    /// the lowercased target string *contains* the lowercased pattern.
    pub fn evaluate<'a>(
        &'a self,
        window: &FocusedWindow,
        active_layer_id: &str,
    ) -> Option<&'a ContextRule> {
        let app_lower = window.app.to_lowercase();
        let title_lower = window.title.to_lowercase();

        for rule in &self.rules {
            if let Some(pat) = &rule.match_app {
                if !app_lower.contains(pat.to_lowercase().as_str()) {
                    continue;
                }
            }
            if let Some(pat) = &rule.match_title {
                if !title_lower.contains(pat.to_lowercase().as_str()) {
                    continue;
                }
            }
            // First match found.
            return if rule.layer_id == active_layer_id {
                None // already the active profile — no-op
            } else {
                Some(rule)
            };
        }
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn window(app: &str, title: &str) -> FocusedWindow {
        FocusedWindow {
            app: app.into(),
            title: title.into(),
        }
    }

    fn rule(name: &str, layer_id: &str, app: Option<&str>, title: Option<&str>) -> ContextRule {
        ContextRule {
            name: name.into(),
            layer_id: layer_id.into(),
            match_app: app.map(|s| s.into()),
            match_title: title.map(|s| s.into()),
        }
    }

    fn rules(list: Vec<ContextRule>) -> ContextRules {
        ContextRules {
            version: 1,
            rules: list,
        }
    }

    // ── evaluate ──────────────────────────────────────────────────────────────

    #[test]
    fn evaluate_first_match_wins() {
        let cr = rules(vec![
            rule("Firefox", "browsing", Some("firefox"), None),
            rule("Code", "coding", Some("code"), None),
        ]);
        let result = cr.evaluate(&window("firefox", "Mozilla Firefox"), "other");
        assert_eq!(result.unwrap().layer_id, "browsing");
    }

    #[test]
    fn evaluate_skips_non_matching_rule_reaches_second() {
        let cr = rules(vec![
            rule("Code", "coding", Some("code"), None),
            rule("Firefox", "browsing", Some("firefox"), None),
        ]);
        let result = cr.evaluate(&window("firefox", "Mozilla Firefox"), "other");
        assert_eq!(result.unwrap().layer_id, "browsing");
    }

    #[test]
    fn evaluate_app_and_title_both_must_match() {
        let cr = rules(vec![rule(
            "Vim in terminal",
            "vim",
            Some("alacritty"),
            Some("vim"),
        )]);
        // App matches, title doesn't.
        assert!(cr.evaluate(&window("alacritty", "bash"), "other").is_none());
        // Both match.
        let result = cr.evaluate(&window("alacritty", "vim main.rs"), "other");
        assert_eq!(result.unwrap().layer_id, "vim");
    }

    #[test]
    fn evaluate_case_insensitive() {
        let cr = rules(vec![rule("Firefox", "browsing", Some("FIREFOX"), None)]);
        let result = cr.evaluate(&window("firefox", "Mozilla Firefox"), "other");
        assert!(result.is_some());
    }

    #[test]
    fn evaluate_case_insensitive_target() {
        let cr = rules(vec![rule("Firefox", "browsing", Some("firefox"), None)]);
        // Target has mixed case.
        let result = cr.evaluate(&window("Firefox", "Mozilla Firefox"), "other");
        assert!(result.is_some());
    }

    #[test]
    fn evaluate_no_match_returns_none() {
        let cr = rules(vec![rule("Code", "coding", Some("code"), None)]);
        assert!(cr
            .evaluate(&window("firefox", "Mozilla Firefox"), "other")
            .is_none());
    }

    #[test]
    fn evaluate_already_active_returns_none() {
        let cr = rules(vec![rule("Firefox", "browsing", Some("firefox"), None)]);
        // "browsing" is already the active profile — no-op.
        assert!(cr
            .evaluate(&window("firefox", "Mozilla Firefox"), "browsing")
            .is_none());
    }

    #[test]
    fn evaluate_empty_rules_returns_none() {
        let cr = ContextRules::default();
        assert!(cr
            .evaluate(&window("firefox", "Mozilla Firefox"), "other")
            .is_none());
    }

    #[test]
    fn evaluate_only_app_pattern_matches_any_title() {
        let cr = rules(vec![rule("Firefox", "browsing", Some("firefox"), None)]);
        let result = cr.evaluate(
            &window("firefox", "some completely different title"),
            "other",
        );
        assert!(result.is_some());
    }

    #[test]
    fn evaluate_only_title_pattern_matches_any_app() {
        let cr = rules(vec![rule("Vim anywhere", "vim", None, Some("vim"))]);
        let result = cr.evaluate(&window("gnome-terminal", "vim ~/.config"), "other");
        assert!(result.is_some());
    }

    #[test]
    fn evaluate_substring_match_not_exact() {
        // "code" should match "Code - OSS" app name.
        let cr = rules(vec![rule("VSCode", "coding", Some("code"), None)]);
        let result = cr.evaluate(&window("Code - OSS", "main.rs"), "other");
        assert!(result.is_some());
    }

    // ── validate ──────────────────────────────────────────────────────────────

    #[test]
    fn validate_rule_with_no_patterns_is_rejected() {
        let cr = rules(vec![rule("Empty", "coding", None, None)]);
        assert!(cr.validate().is_err());
    }

    #[test]
    fn validate_empty_name_is_rejected() {
        let cr = rules(vec![rule("", "coding", Some("code"), None)]);
        assert!(cr.validate().is_err());
    }

    #[test]
    fn validate_empty_layer_id_is_rejected() {
        let cr = rules(vec![rule("Code", "", Some("code"), None)]);
        assert!(cr.validate().is_err());
    }

    #[test]
    fn validate_valid_rules_pass() {
        let cr = rules(vec![
            rule("Firefox", "browsing", Some("firefox"), None),
            rule("Terminal vim", "vim", Some("alacritty"), Some("vim")),
        ]);
        assert!(cr.validate().is_ok());
    }

    // ── load / save round-trip ────────────────────────────────────────────────

    #[test]
    fn load_missing_file_returns_empty() {
        let path = std::path::Path::new("/tmp/__mapxr_nonexistent_context_rules.json");
        let cr = ContextRules::load(path);
        assert!(cr.rules.is_empty());
    }

    #[test]
    fn load_save_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join("mapxr_test_context_rules.json");

        let original = rules(vec![
            rule("Firefox", "browsing", Some("firefox"), None),
            rule("Code", "coding", Some("code"), None),
        ]);
        original.save(&path).expect("save should succeed");

        let loaded = ContextRules::load(&path);
        assert_eq!(loaded.rules.len(), 2);
        assert_eq!(loaded.rules[0].layer_id, "browsing");
        assert_eq!(loaded.rules[1].layer_id, "coding");
        assert_eq!(loaded.rules[0].match_app.as_deref(), Some("firefox"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_malformed_json_returns_empty() {
        let dir = std::env::temp_dir();
        let path = dir.join("mapxr_test_context_rules_bad.json");
        std::fs::write(&path, b"{ not valid json !!").unwrap();
        let cr = ContextRules::load(&path);
        assert!(cr.rules.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
