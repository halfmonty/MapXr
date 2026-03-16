use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;
use crate::types::{
    Action, Hand, HoldModifierMode, Mapping, Modifier, ProfileKind, ProfileSettings,
    TriggerPattern, VariableValue,
};

fn default_passthrough() -> bool {
    false
}

fn is_false(v: &bool) -> bool {
    !v
}

fn default_hand() -> Option<Hand> {
    Some(Hand::Right)
}

/// A complete layer profile loaded from a `.json` file.
///
/// Required fields: `version`, `kind`, `name`, `layer_id`.
/// All other fields are optional and default to sensible values.
///
/// Unknown fields (e.g. `"_pattern_guide"`) are silently ignored on
/// deserialisation; they are not preserved on serialisation.
///
/// # Example (minimal single-hand profile)
///
/// ```json
/// {
///   "version": 1,
///   "kind": "single",
///   "name": "base",
///   "layer_id": "base",
///   "mappings": []
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// JSON schema version. Currently always `1`.
    pub version: u32,

    /// Whether this profile targets one or two Tap devices.
    pub kind: ProfileKind,

    /// Human-readable profile name shown in the UI.
    pub name: String,

    /// Unique identifier used by `push_layer` / `pop_layer` actions.
    pub layer_id: String,

    /// Which hand the device is worn on. Required for `single` profiles;
    /// omitted for `dual` profiles. Defaults to `Right` when absent.
    #[serde(default = "default_hand", skip_serializing_if = "Option::is_none")]
    pub hand: Option<Hand>,

    /// Free-text description shown in the UI. Omitted when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// If `true`, unmatched codes fall through to the next layer down the
    /// stack. Omitted from JSON when `false` (the default).
    #[serde(default = "default_passthrough", skip_serializing_if = "is_false")]
    pub passthrough: bool,

    /// Per-profile timing and behaviour overrides. Serialised as `{}` when
    /// all fields are at their defaults.
    #[serde(default)]
    pub settings: ProfileSettings,

    /// Named reusable actions, resolved by `Action::Alias`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub aliases: HashMap<String, Action>,

    /// Named boolean/integer state variables, initialised from this map on
    /// layer push and reset to these values on reload.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variables: HashMap<String, VariableValue>,

    /// Action fired when this layer becomes active (pushed onto the stack).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_enter: Option<Action>,

    /// Action fired when this layer is popped off the stack.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_exit: Option<Action>,

    /// Ordered list of input bindings.
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

impl Profile {
    /// Load and validate a profile from a JSON file.
    ///
    /// Returns an error if:
    /// - The file cannot be read
    /// - The JSON is malformed or does not match the schema
    /// - The profile version is not `1`
    /// - Any `KeyDef` contains an unknown key name
    /// - Any `Action::Alias` references an undefined alias
    /// - Alias definitions form a circular reference
    /// - A tap code is overloaded without `overload_strategy` set
    /// - A `Macro` step contains another `Macro`
    /// - A trigger code kind does not match the profile kind
    pub fn load(path: &Path) -> Result<Profile, ProfileError> {
        let content = std::fs::read_to_string(path)?;
        let profile: Profile = serde_json::from_str(&content)?;
        profile.validate()?;
        Ok(profile)
    }

    /// Serialise and write the profile to a JSON file.
    ///
    /// The file is written atomically by first writing a temporary file in the
    /// same directory and then renaming it, so a crash mid-write cannot corrupt
    /// an existing file.
    pub fn save(&self, path: &Path) -> Result<(), ProfileError> {
        let json = serde_json::to_string_pretty(self)?;
        // Write to a sibling temp file then rename for atomicity.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Run all validation rules.
    ///
    /// Called automatically by [`Profile::load`]. Also exposed publicly so
    /// callers (e.g. `save_profile` Tauri command) can reject invalid profiles
    /// before writing them to disk.
    pub fn validate(&self) -> Result<(), ProfileError> {
        self.check_version()?;
        self.check_trigger_kinds()?;
        self.check_key_names()?;
        self.check_macro_nesting()?;
        self.check_hold_modifier_rules()?;
        self.check_aliases()?;
        self.check_overloaded_codes()?;
        Ok(())
    }

    /// Rule: version must be 1.
    fn check_version(&self) -> Result<(), ProfileError> {
        if self.version != 1 {
            return Err(ProfileError::UnsupportedVersion {
                version: self.version,
            });
        }
        Ok(())
    }

    /// Rule: trigger code kinds must match the profile kind.
    fn check_trigger_kinds(&self) -> Result<(), ProfileError> {
        use crate::types::Trigger;
        let profile_kind = match self.kind {
            ProfileKind::Single => "single",
            ProfileKind::Dual => "dual",
        };
        for mapping in &self.mappings {
            let patterns: Vec<&TriggerPattern> = match &mapping.trigger {
                Trigger::Tap { code }
                | Trigger::DoubleTap { code }
                | Trigger::TripleTap { code } => vec![code],
                Trigger::Sequence { steps, .. } => steps.iter().map(|s| &s.code).collect(),
            };
            for pattern in patterns {
                let code_kind = match pattern {
                    TriggerPattern::Single(_) => "single",
                    TriggerPattern::Dual { .. } => "dual",
                };
                if profile_kind != code_kind {
                    return Err(ProfileError::TriggerKindMismatch {
                        label: mapping.label.clone(),
                        profile_kind: profile_kind.into(),
                        code_kind: code_kind.into(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Rule: all KeyDef values must be in the valid key list.
    fn check_key_names(&self) -> Result<(), ProfileError> {
        for mapping in &self.mappings {
            check_action_keys(&mapping.action, &mapping.label)?;
        }
        for action in self.aliases.values() {
            check_action_keys(action, "<alias>")?;
        }
        if let Some(action) = &self.on_enter {
            check_action_keys(action, "<on_enter>")?;
        }
        if let Some(action) = &self.on_exit {
            check_action_keys(action, "<on_exit>")?;
        }
        Ok(())
    }

    /// Rule: macro steps may not contain another macro.
    fn check_macro_nesting(&self) -> Result<(), ProfileError> {
        for mapping in &self.mappings {
            check_action_macro_nesting(&mapping.action, &mapping.label)?;
        }
        Ok(())
    }

    /// Rules: hold_modifier actions must be valid and may not appear inside macros.
    fn check_hold_modifier_rules(&self) -> Result<(), ProfileError> {
        for mapping in &self.mappings {
            check_action_hold_modifier(&mapping.action, &mapping.label, false)?;
        }
        for action in self.aliases.values() {
            check_action_hold_modifier(action, "<alias>", false)?;
        }
        if let Some(action) = &self.on_enter {
            check_action_hold_modifier(action, "<on_enter>", false)?;
        }
        if let Some(action) = &self.on_exit {
            check_action_hold_modifier(action, "<on_exit>", false)?;
        }
        Ok(())
    }

    /// Rules: all Alias actions must resolve to a defined name; no cycles.
    fn check_aliases(&self) -> Result<(), ProfileError> {
        // Collect all alias names referenced in mappings and alias values.
        for mapping in &self.mappings {
            check_action_aliases(&mapping.action, &self.aliases)?;
        }
        for action in self.aliases.values() {
            check_action_aliases(action, &self.aliases)?;
        }
        // Check for cycles: a → b → a etc. using DFS.
        for start in self.aliases.keys() {
            let mut visited: Vec<&str> = vec![start.as_str()];
            detect_alias_cycle(start, &self.aliases, &mut visited)?;
        }
        Ok(())
    }

    /// Rule: overloaded codes require overload_strategy to be set.
    fn check_overloaded_codes(&self) -> Result<(), ProfileError> {
        use crate::types::{Hand, Trigger};
        if self.settings.overload_strategy.is_some() {
            return Ok(());
        }
        // Use the profile's hand (or Right for dual) for canonical string form.
        let hand = self.hand.unwrap_or(Hand::Right);
        let mut tap_codes: HashSet<String> = HashSet::new();
        for mapping in &self.mappings {
            match &mapping.trigger {
                Trigger::Tap { code } => {
                    tap_codes.insert((*code).to_pattern_string(hand));
                }
                Trigger::DoubleTap { code } | Trigger::TripleTap { code } => {
                    let key = (*code).to_pattern_string(hand);
                    if tap_codes.contains(&key) {
                        return Err(ProfileError::OverloadedCodeWithoutStrategy { code: key });
                    }
                }
                Trigger::Sequence { .. } => {}
            }
        }
        Ok(())
    }
}

/// Walk an action tree and validate all `KeyDef` names.
fn check_action_keys(action: &Action, label: &str) -> Result<(), ProfileError> {
    match action {
        Action::Key { key, .. } => {
            key.validate().map_err(|_| ProfileError::UnknownKey {
                key: key.as_str().into(),
                label: label.into(),
            })?;
        }
        Action::Macro { steps } => {
            for step in steps {
                check_action_keys(&step.action, label)?;
            }
        }
        Action::ToggleVariable {
            on_true, on_false, ..
        } => {
            check_action_keys(on_true, label)?;
            check_action_keys(on_false, label)?;
        }
        _ => {}
    }
    Ok(())
}

/// Walk an action tree and reject macro-in-macro nesting.
fn check_action_macro_nesting(action: &Action, label: &str) -> Result<(), ProfileError> {
    if let Action::Macro { steps } = action {
        for step in steps {
            if matches!(step.action, Action::Macro { .. }) {
                return Err(ProfileError::NestedMacro {
                    label: label.into(),
                });
            }
        }
    }
    Ok(())
}

/// Walk an action tree and verify all `Alias` names exist in `aliases`.
fn check_action_aliases(
    action: &Action,
    aliases: &HashMap<String, Action>,
) -> Result<(), ProfileError> {
    match action {
        Action::Alias { name } => {
            if !aliases.contains_key(name) {
                return Err(ProfileError::UndefinedAlias { name: name.clone() });
            }
        }
        Action::Macro { steps } => {
            for step in steps {
                check_action_aliases(&step.action, aliases)?;
            }
        }
        Action::ToggleVariable {
            on_true, on_false, ..
        } => {
            check_action_aliases(on_true, aliases)?;
            check_action_aliases(on_false, aliases)?;
        }
        _ => {}
    }
    Ok(())
}

/// Walk an action tree and enforce hold_modifier rules.
///
/// - At the top level (`in_macro = false`): validates non-empty modifiers, no
///   duplicates, and valid count/timeout values.
/// - Inside a macro step (`in_macro = true`): rejects any `HoldModifier` action.
fn check_action_hold_modifier(
    action: &Action,
    label: &str,
    in_macro: bool,
) -> Result<(), ProfileError> {
    match action {
        Action::HoldModifier { modifiers, mode } => {
            if in_macro {
                return Err(ProfileError::HoldModifierInsideMacro {
                    label: label.into(),
                });
            }
            if modifiers.is_empty() {
                return Err(ProfileError::HoldModifierEmptyModifiers {
                    label: label.into(),
                });
            }
            let mut seen = std::collections::HashSet::new();
            for m in modifiers {
                if !seen.insert(*m) {
                    return Err(ProfileError::HoldModifierDuplicateModifier {
                        modifier: modifier_name(*m).into(),
                        label: label.into(),
                    });
                }
            }
            match mode {
                HoldModifierMode::Count { count } if *count == 0 => {
                    return Err(ProfileError::HoldModifierCountZero {
                        label: label.into(),
                    });
                }
                HoldModifierMode::Timeout { timeout_ms } if *timeout_ms == 0 => {
                    return Err(ProfileError::HoldModifierTimeoutZero {
                        label: label.into(),
                    });
                }
                _ => {}
            }
            Ok(())
        }
        Action::Macro { steps } => {
            for step in steps {
                check_action_hold_modifier(&step.action, label, true)?;
            }
            Ok(())
        }
        Action::ToggleVariable {
            on_true, on_false, ..
        } => {
            check_action_hold_modifier(on_true, label, in_macro)?;
            check_action_hold_modifier(on_false, label, in_macro)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Return the canonical lowercase name of a modifier key.
fn modifier_name(m: Modifier) -> &'static str {
    match m {
        Modifier::Ctrl => "ctrl",
        Modifier::Shift => "shift",
        Modifier::Alt => "alt",
        Modifier::Meta => "meta",
    }
}

/// DFS cycle detection for alias chains.
fn detect_alias_cycle<'a>(
    name: &'a str,
    aliases: &'a HashMap<String, Action>,
    visited: &mut Vec<&'a str>,
) -> Result<(), ProfileError> {
    if let Some(Action::Alias { name: next }) = aliases.get(name) {
        if visited.contains(&next.as_str()) {
            visited.push(next.as_str());
            return Err(ProfileError::CircularAlias {
                cycle: visited.join(" → "),
            });
        }
        visited.push(next.as_str());
        detect_alias_cycle(next, aliases, visited)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Action, HoldModifierMode, KeyDef, MacroStep, Modifier, ProfileKind, TapCode, Trigger,
        TriggerPattern,
    };

    fn minimal_profile() -> Profile {
        Profile {
            version: 1,
            kind: ProfileKind::Single,
            name: "base".into(),
            layer_id: "base".into(),
            hand: Some(Hand::Right),
            description: None,
            passthrough: false,
            settings: ProfileSettings::default(),
            aliases: HashMap::new(),
            variables: HashMap::new(),
            on_enter: None,
            on_exit: None,
            mappings: vec![],
        }
    }

    fn round_trip(p: &Profile) -> Profile {
        let json = serde_json::to_string(p).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn profile_minimal_round_trips() {
        let p = minimal_profile();
        assert_eq!(round_trip(&p), p);
    }

    #[test]
    fn profile_passthrough_false_omitted_from_json() {
        let p = minimal_profile();
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("passthrough"), "got: {json}");
    }

    #[test]
    fn profile_passthrough_true_present_in_json() {
        let p = Profile {
            passthrough: true,
            ..minimal_profile()
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"passthrough\":true"), "got: {json}");
    }

    #[test]
    fn profile_aliases_omitted_when_empty() {
        let p = minimal_profile();
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("aliases"), "got: {json}");
    }

    #[test]
    fn profile_variables_omitted_when_empty() {
        let p = minimal_profile();
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("variables"), "got: {json}");
    }

    #[test]
    fn profile_hand_omitted_for_dual_kind() {
        let p = Profile {
            kind: ProfileKind::Dual,
            hand: None,
            ..minimal_profile()
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("\"hand\""), "got: {json}");
    }

    #[test]
    fn profile_on_enter_on_exit_round_trip() {
        let p = Profile {
            on_enter: Some(Action::Key {
                key: KeyDef::new_unchecked("f15"),
                modifiers: vec![],
            }),
            on_exit: Some(Action::Key {
                key: KeyDef::new_unchecked("f15"),
                modifiers: vec![],
            }),
            ..minimal_profile()
        };
        assert_eq!(round_trip(&p), p);
    }

    #[test]
    fn profile_with_mapping_round_trips() {
        let p = Profile {
            mappings: vec![Mapping {
                label: "Thumb → Space".into(),
                trigger: Trigger::Tap {
                    code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
                },
                action: Action::Key {
                    key: KeyDef::new_unchecked("space"),
                    modifiers: vec![],
                },
                enabled: true,
            }],
            ..minimal_profile()
        };
        assert_eq!(round_trip(&p), p);
    }

    #[test]
    fn profile_deserialises_from_spec_json() {
        let json = r#"{
            "version": 1,
            "kind": "single",
            "name": "paused",
            "layer_id": "paused",
            "passthrough": false,
            "on_enter": { "type": "key", "key": "f15" },
            "on_exit":  { "type": "key", "key": "f15" },
            "_pattern_guide": "ignored comment field",
            "mappings": [
                {
                    "label": "Unpause",
                    "trigger": { "type": "tap", "code": "xxxxx" },
                    "action": { "type": "pop_layer" }
                }
            ]
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.version, 1);
        assert_eq!(p.kind, ProfileKind::Single);
        assert_eq!(p.layer_id, "paused");
        assert!(p.on_enter.is_some());
        assert_eq!(p.mappings.len(), 1);
        assert_eq!(p.mappings[0].label, "Unpause");
    }

    #[test]
    fn profile_settings_embedded_deserialises_correctly() {
        let json = r#"{
            "version": 1,
            "kind": "dual",
            "name": "base",
            "layer_id": "base",
            "settings": {
                "combo_window_ms": 150,
                "overload_strategy": "eager"
            },
            "mappings": []
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.settings.combo_window_ms, Some(150));
        assert_eq!(
            p.settings.overload_strategy,
            Some(crate::types::OverloadStrategy::Eager)
        );
    }

    // ── hold_modifier validation ──────────────────────────────────────────────

    fn hold_modifier_mapping(label: &str, action: Action) -> Mapping {
        Mapping {
            label: label.into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action,
            enabled: true,
        }
    }

    #[test]
    fn hold_modifier_valid_toggle_passes_validation() {
        let p = Profile {
            mappings: vec![hold_modifier_mapping(
                "hold shift",
                Action::HoldModifier {
                    modifiers: vec![Modifier::Shift],
                    mode: HoldModifierMode::Toggle,
                },
            )],
            ..minimal_profile()
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn hold_modifier_empty_modifiers_fails_validation() {
        let p = Profile {
            mappings: vec![hold_modifier_mapping(
                "bad",
                Action::HoldModifier {
                    modifiers: vec![],
                    mode: HoldModifierMode::Toggle,
                },
            )],
            ..minimal_profile()
        };
        assert!(matches!(
            p.validate(),
            Err(ProfileError::HoldModifierEmptyModifiers { .. })
        ));
    }

    #[test]
    fn hold_modifier_duplicate_modifiers_fails_validation() {
        let p = Profile {
            mappings: vec![hold_modifier_mapping(
                "bad",
                Action::HoldModifier {
                    modifiers: vec![Modifier::Shift, Modifier::Shift],
                    mode: HoldModifierMode::Toggle,
                },
            )],
            ..minimal_profile()
        };
        assert!(matches!(
            p.validate(),
            Err(ProfileError::HoldModifierDuplicateModifier { .. })
        ));
    }

    #[test]
    fn hold_modifier_count_zero_fails_validation() {
        let p = Profile {
            mappings: vec![hold_modifier_mapping(
                "bad",
                Action::HoldModifier {
                    modifiers: vec![Modifier::Ctrl],
                    mode: HoldModifierMode::Count { count: 0 },
                },
            )],
            ..minimal_profile()
        };
        assert!(matches!(
            p.validate(),
            Err(ProfileError::HoldModifierCountZero { .. })
        ));
    }

    #[test]
    fn hold_modifier_timeout_ms_zero_fails_validation() {
        let p = Profile {
            mappings: vec![hold_modifier_mapping(
                "bad",
                Action::HoldModifier {
                    modifiers: vec![Modifier::Alt],
                    mode: HoldModifierMode::Timeout { timeout_ms: 0 },
                },
            )],
            ..minimal_profile()
        };
        assert!(matches!(
            p.validate(),
            Err(ProfileError::HoldModifierTimeoutZero { .. })
        ));
    }

    #[test]
    fn hold_modifier_inside_macro_fails_validation() {
        let p = Profile {
            mappings: vec![Mapping {
                label: "bad macro".into(),
                trigger: Trigger::Tap {
                    code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
                },
                action: Action::Macro {
                    steps: vec![MacroStep {
                        action: Action::HoldModifier {
                            modifiers: vec![Modifier::Shift],
                            mode: HoldModifierMode::Toggle,
                        },
                        delay_ms: 0,
                    }],
                },
                enabled: true,
            }],
            ..minimal_profile()
        };
        assert!(matches!(
            p.validate(),
            Err(ProfileError::HoldModifierInsideMacro { .. })
        ));
    }
}
