use std::fs;

use mapping_core::LayerRegistry;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn write_profile(dir: &std::path::Path, filename: &str, content: &str) {
    fs::write(dir.join(filename), content).expect("write fixture");
}

const VALID_SINGLE: &str = r#"{
    "version": 1, "kind": "single", "name": "base", "layer_id": "base",
    "mappings": [{ "label": "Thumb", "trigger": { "type": "tap", "code": "xoooo" }, "action": { "type": "key", "key": "space" } }]
}"#;

const VALID_DUAL: &str = r#"{
    "version": 1, "kind": "dual", "name": "dual-base", "layer_id": "dual-base",
    "mappings": []
}"#;

const INVALID_JSON: &str = r#"{ this is not valid json "#;

const INVALID_PROFILE: &str = r#"{
    "version": 99, "kind": "single", "name": "bad", "layer_id": "bad", "mappings": []
}"#;

// ── Basic loading ─────────────────────────────────────────────────────────────

#[test]
fn layer_registry_loads_valid_profiles() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "base.json", VALID_SINGLE);
    write_profile(dir.path(), "dual.json", VALID_DUAL);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");

    assert_eq!(reg.len(), 2);
    assert!(reg.get("base").is_some());
    assert!(reg.get("dual-base").is_some());
    assert!(reg.load_errors().is_empty());
}

#[test]
fn layer_registry_empty_dir_loads_zero_profiles() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");
    assert!(reg.is_empty());
}

#[test]
fn layer_registry_skips_non_json_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();
    fs::write(dir.path().join("base.md"), "also ignore").unwrap();
    write_profile(dir.path(), "base.json", VALID_SINGLE);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");

    assert_eq!(reg.len(), 1);
}

// ── Error handling ────────────────────────────────────────────────────────────

#[test]
fn layer_registry_records_invalid_json_as_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "bad.json", INVALID_JSON);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");

    assert_eq!(reg.len(), 0);
    assert_eq!(reg.load_errors().len(), 1);
}

#[test]
fn layer_registry_records_invalid_profile_as_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "bad.json", INVALID_PROFILE);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");

    assert_eq!(reg.len(), 0);
    assert_eq!(reg.load_errors().len(), 1);
}

#[test]
fn layer_registry_valid_profiles_load_alongside_invalid_ones() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "base.json", VALID_SINGLE);
    write_profile(dir.path(), "bad.json", INVALID_PROFILE);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");

    assert_eq!(reg.len(), 1);
    assert_eq!(reg.load_errors().len(), 1);
    assert!(reg.get("base").is_some());
}

// ── Reload behaviour ──────────────────────────────────────────────────────────

#[test]
fn layer_registry_reload_picks_up_new_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "base.json", VALID_SINGLE);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("first reload");
    assert_eq!(reg.len(), 1);

    write_profile(dir.path(), "dual.json", VALID_DUAL);
    reg.reload().expect("second reload");
    assert_eq!(reg.len(), 2);
}

#[test]
fn layer_registry_reload_drops_removed_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "base.json", VALID_SINGLE);
    write_profile(dir.path(), "dual.json", VALID_DUAL);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("first reload");
    assert_eq!(reg.len(), 2);

    fs::remove_file(dir.path().join("dual.json")).unwrap();
    reg.reload().expect("second reload");
    assert_eq!(reg.len(), 1);
    assert!(reg.get("dual-base").is_none());
}

#[test]
fn layer_registry_reload_clears_previous_errors() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "bad.json", INVALID_JSON);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("first reload");
    assert_eq!(reg.load_errors().len(), 1);

    // Fix the file.
    write_profile(dir.path(), "bad.json", VALID_SINGLE);
    reg.reload().expect("second reload");
    assert_eq!(reg.load_errors().len(), 0);
    assert_eq!(reg.len(), 1);
}

// ── Get / iteration ───────────────────────────────────────────────────────────

#[test]
fn layer_registry_get_unknown_layer_id_returns_none() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");
    assert!(reg.get("does-not-exist").is_none());
}

#[test]
fn layer_registry_profiles_iterator_yields_all_loaded() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_profile(dir.path(), "base.json", VALID_SINGLE);
    write_profile(dir.path(), "dual.json", VALID_DUAL);

    let mut reg = LayerRegistry::new(dir.path());
    reg.reload().expect("reload");

    let ids: Vec<&str> = reg.profiles().map(|p| p.layer_id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"base"));
    assert!(ids.contains(&"dual-base"));
}
