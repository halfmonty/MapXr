use std::io::Write;

use mapping_core::{ProfileError, types::Profile};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Write `content` to a temp file and call `Profile::load` on it.
fn load_str(content: &str) -> Result<Profile, ProfileError> {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    tmp.write_all(content.as_bytes()).expect("write temp file");
    Profile::load(tmp.path())
}

// ── Valid profiles load without error ─────────────────────────────────────────

#[test]
fn profile_load_valid_single_succeeds() {
    let json = include_str!("fixtures/valid_single.json");
    assert!(load_str(json).is_ok());
}

#[test]
fn profile_load_valid_dual_succeeds() {
    let json = include_str!("fixtures/valid_dual.json");
    assert!(load_str(json).is_ok());
}

#[test]
fn profile_load_valid_overloaded_with_strategy_succeeds() {
    let json = include_str!("fixtures/valid_overloaded.json");
    assert!(load_str(json).is_ok());
}

#[test]
fn profile_load_valid_aliases_succeeds() {
    let json = include_str!("fixtures/valid_aliases.json");
    assert!(load_str(json).is_ok());
}

// ── Version check ─────────────────────────────────────────────────────────────

#[test]
fn profile_load_unsupported_version_returns_error() {
    let json = include_str!("fixtures/err_unsupported_version.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::UnsupportedVersion { version: 99 }),
        "expected UnsupportedVersion, got: {err}"
    );
}

// ── Key name validation ───────────────────────────────────────────────────────

#[test]
fn profile_load_unknown_key_returns_error() {
    let json = include_str!("fixtures/err_unknown_key.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::UnknownKey { .. }),
        "expected UnknownKey, got: {err}"
    );
}

#[test]
fn profile_load_key_in_macro_step_is_validated() {
    let json = r#"{
        "version": 1, "kind": "single", "name": "t", "layer_id": "t",
        "mappings": [{
            "label": "mac",
            "trigger": { "type": "tap", "code": "xoooo" },
            "action": {
                "type": "macro",
                "steps": [{ "action": { "type": "key", "key": "bad_key" }, "delay_ms": 0 }]
            }
        }]
    }"#;
    assert!(matches!(
        load_str(json).unwrap_err(),
        ProfileError::UnknownKey { .. }
    ));
}

// ── Alias validation ──────────────────────────────────────────────────────────

#[test]
fn profile_load_undefined_alias_returns_error() {
    let json = include_str!("fixtures/err_undefined_alias.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::UndefinedAlias { .. }),
        "expected UndefinedAlias, got: {err}"
    );
}

#[test]
fn profile_load_circular_alias_returns_error() {
    let json = include_str!("fixtures/err_circular_alias.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::CircularAlias { .. }),
        "expected CircularAlias, got: {err}"
    );
}

// ── Overload strategy ─────────────────────────────────────────────────────────

#[test]
fn profile_load_overloaded_without_strategy_returns_error() {
    let json = include_str!("fixtures/err_overloaded_no_strategy.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::OverloadedCodeWithoutStrategy { .. }),
        "expected OverloadedCodeWithoutStrategy, got: {err}"
    );
}

#[test]
fn profile_load_non_overloaded_codes_do_not_require_strategy() {
    // Different codes in tap vs double_tap — not overloaded.
    let json = r#"{
        "version": 1, "kind": "single", "name": "t", "layer_id": "t",
        "mappings": [
            { "label": "A tap",    "trigger": { "type": "tap",        "code": "oooox" }, "action": { "type": "key", "key": "a" } },
            { "label": "B double", "trigger": { "type": "double_tap", "code": "xoooo" }, "action": { "type": "key", "key": "b" } }
        ]
    }"#;
    assert!(load_str(json).is_ok());
}

// ── Macro nesting ─────────────────────────────────────────────────────────────

#[test]
fn profile_load_nested_macro_returns_error() {
    let json = include_str!("fixtures/err_nested_macro.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::NestedMacro { .. }),
        "expected NestedMacro, got: {err}"
    );
}

#[test]
fn profile_load_flat_macro_succeeds() {
    let json = r#"{
        "version": 1, "kind": "single", "name": "t", "layer_id": "t",
        "mappings": [{
            "label": "flat",
            "trigger": { "type": "tap", "code": "xoooo" },
            "action": {
                "type": "macro",
                "steps": [
                    { "action": { "type": "key", "key": "a" }, "delay_ms": 0 },
                    { "action": { "type": "key", "key": "b" }, "delay_ms": 50 }
                ]
            }
        }]
    }"#;
    assert!(load_str(json).is_ok());
}

// ── Trigger kind mismatch ─────────────────────────────────────────────────────

#[test]
fn profile_load_dual_code_in_single_profile_returns_error() {
    let json = include_str!("fixtures/err_trigger_kind_mismatch.json");
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::TriggerKindMismatch { .. }),
        "expected TriggerKindMismatch, got: {err}"
    );
}

#[test]
fn profile_load_single_code_in_dual_profile_returns_error() {
    let json = r#"{
        "version": 1, "kind": "dual", "name": "t", "layer_id": "t",
        "mappings": [{
            "label": "single code in dual",
            "trigger": { "type": "tap", "code": "xoooo" },
            "action": { "type": "key", "key": "space" }
        }]
    }"#;
    let err = load_str(json).unwrap_err();
    assert!(
        matches!(err, ProfileError::TriggerKindMismatch { .. }),
        "expected TriggerKindMismatch, got: {err}"
    );
}

// ── Profile::save round-trip ──────────────────────────────────────────────────

#[test]
fn profile_save_and_reload_round_trips() {
    let json = include_str!("fixtures/valid_single.json");
    let original = load_str(json).expect("load original");

    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    original.save(tmp.path()).expect("save profile");

    let reloaded = Profile::load(tmp.path()).expect("reload profile");
    assert_eq!(original, reloaded);
}

#[test]
fn profile_save_does_not_leave_tmp_file_on_success() {
    let json = include_str!("fixtures/valid_single.json");
    let profile = load_str(json).expect("load");

    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    let path = tmp.path().to_path_buf();
    profile.save(&path).expect("save");

    let tmp_path = path.with_extension("json.tmp");
    assert!(
        !tmp_path.exists(),
        "tmp file should be cleaned up after rename"
    );
}
