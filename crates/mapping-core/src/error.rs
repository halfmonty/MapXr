use thiserror::Error;

/// Errors returned by [`Profile::load`](crate::types::Profile::load) and
/// [`Profile::save`](crate::types::Profile::save).
#[derive(Debug, Error)]
pub enum ProfileError {
    /// The file could not be read from disk.
    #[error("could not read profile file: {0}")]
    Io(#[from] std::io::Error),

    /// The file contents are not valid JSON, or the JSON does not match the
    /// profile schema.
    #[error("profile JSON is malformed: {0}")]
    Json(#[from] serde_json::Error),

    /// The profile declares `"version": N` but this engine only supports
    /// version 1.
    #[error("unsupported profile version {version}; expected 1")]
    UnsupportedVersion {
        /// The version number found in the file.
        version: u32,
    },

    /// A `KeyDef` in an action contains a key name not in the valid key list.
    #[error("unknown key name {key:?} in mapping {label:?}")]
    UnknownKey {
        /// The invalid key name.
        key: String,
        /// Label of the mapping that contains the bad key.
        label: String,
    },

    /// An `Action::Alias` references a name that is not defined in the
    /// profile's `aliases` map.
    #[error("alias {name:?} is not defined in this profile")]
    UndefinedAlias {
        /// The alias name that could not be resolved.
        name: String,
    },

    /// Two aliases reference each other, forming an infinite resolution loop.
    #[error("circular alias reference: {cycle}")]
    CircularAlias {
        /// A description of the cycle, e.g. `"a → b → a"`.
        cycle: String,
    },

    /// A tap code appears in both a `tap` binding and a `double_tap` /
    /// `triple_tap` binding, but no `overload_strategy` is configured.
    #[error(
        "tap code {code:?} is overloaded (appears in tap and double_tap/triple_tap) \
         but no overload_strategy is set in profile settings"
    )]
    OverloadedCodeWithoutStrategy {
        /// The finger-pattern string of the overloaded code.
        code: String,
    },

    /// A `Macro` step contains another `Macro` action (nesting is forbidden).
    #[error("macro nesting is not allowed in mapping {label:?}")]
    NestedMacro {
        /// Label of the mapping whose macro contains a nested macro.
        label: String,
    },

    /// A dual-pattern trigger code appears in a `single` profile, or a
    /// single-pattern code appears in a `dual` profile.
    #[error(
        "trigger code kind mismatch in mapping {label:?}: profile is {profile_kind} but code is {code_kind}"
    )]
    TriggerKindMismatch {
        /// Label of the offending mapping.
        label: String,
        /// `"single"` or `"dual"`.
        profile_kind: String,
        /// `"single"` or `"dual"`.
        code_kind: String,
    },

    /// A `hold_modifier` action has an empty `modifiers` list.
    #[error("hold_modifier action must specify at least one modifier (in mapping {label:?})")]
    HoldModifierEmptyModifiers {
        /// Label of the offending mapping or context.
        label: String,
    },

    /// A `hold_modifier` action has duplicate entries in its `modifiers` list.
    #[error("hold_modifier action contains duplicate modifier {modifier:?} (in mapping {label:?})")]
    HoldModifierDuplicateModifier {
        /// The duplicate modifier name.
        modifier: String,
        /// Label of the offending mapping or context.
        label: String,
    },

    /// A `hold_modifier` action with `mode: count` has `count: 0`.
    #[error("hold_modifier count must be at least 1 (in mapping {label:?})")]
    HoldModifierCountZero {
        /// Label of the offending mapping or context.
        label: String,
    },

    /// A `hold_modifier` action with `mode: timeout` has `timeout_ms: 0`.
    #[error("hold_modifier timeout_ms must be at least 1 (in mapping {label:?})")]
    HoldModifierTimeoutZero {
        /// Label of the offending mapping or context.
        label: String,
    },

    /// A `hold_modifier` action appears as a step inside a `macro`.
    #[error("hold_modifier may not be used inside a macro step (in mapping {label:?})")]
    HoldModifierInsideMacro {
        /// Label of the offending mapping or context.
        label: String,
    },
}
