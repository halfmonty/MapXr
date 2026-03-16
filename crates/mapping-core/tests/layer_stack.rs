use std::collections::HashMap;
use std::time::{Duration, Instant};

use mapping_core::engine::{ComboEngine, RawTapEvent};
use mapping_core::types::{
    Action, Hand, KeyDef, Mapping, Profile, ProfileKind, ProfileSettings, PushLayerMode, TapCode,
    Trigger, TriggerPattern, VariableValue,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_profile(layer_id: &str, mappings: Vec<Mapping>) -> Profile {
    Profile {
        version: 1,
        kind: ProfileKind::Single,
        name: layer_id.into(),
        layer_id: layer_id.into(),
        hand: Some(Hand::Right),
        description: None,
        passthrough: false,
        settings: ProfileSettings::default(),
        aliases: HashMap::new(),
        variables: HashMap::new(),
        on_enter: None,
        on_exit: None,
        mappings,
    }
}

fn make_passthrough_profile(layer_id: &str, mappings: Vec<Mapping>) -> Profile {
    Profile {
        passthrough: true,
        ..make_profile(layer_id, mappings)
    }
}

fn tap_mapping(label: &str, code: u8, key: &str) -> Mapping {
    Mapping {
        label: label.into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Single(TapCode::from_u8(code).unwrap()),
        },
        action: Action::Key {
            key: KeyDef::new_unchecked(key),
            modifiers: vec![],
        },
        enabled: true,
    }
}

fn key_action(key: &str) -> Action {
    Action::Key {
        key: KeyDef::new_unchecked(key),
        modifiers: vec![],
    }
}

fn key_in_output(outputs: &[mapping_core::engine::EngineOutput], key: &str) -> bool {
    outputs.iter().any(|o| {
        o.actions
            .iter()
            .any(|a| matches!(a, Action::Key { key: k, .. } if k.as_str() == key))
    })
}

fn t(base: Instant, offset_ms: u64) -> Instant {
    base + Duration::from_millis(offset_ms)
}

// ── Push / pop / switch ───────────────────────────────────────────────────────

#[test]
fn push_layer_makes_new_layer_active() {
    // Base has code 1 → "a"; pushed layer has code 1 → "b".
    // After push, code 1 should produce "b".
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_profile("overlay", vec![tap_mapping("B", 1, "b")]);
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Permanent, now);

    // Trigger the binding — overlay wins. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "b"),
        "expected overlay 'b', got {out1:?}"
    );
    assert!(!key_in_output(&out1, "a"), "base 'a' must not fire");
}

#[test]
fn pop_layer_restores_previous_layer() {
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_profile("overlay", vec![tap_mapping("B", 1, "b")]);
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Permanent, now);
    engine.pop_layer();

    // After pop, base should be active again. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "a"),
        "expected base 'a' after pop, got {out1:?}"
    );
    assert!(!key_in_output(&out1, "b"));
}

#[test]
fn pop_layer_at_base_is_noop() {
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(base);

    // Popping the base layer must produce no output and not panic.
    let out = engine.pop_layer();
    assert!(
        out.is_empty(),
        "pop at base should produce no output, got {out:?}"
    );

    // Engine still works. Opt A: fires immediately.
    let out1 = engine.push_event(
        RawTapEvent::new_at("solo", 1, Instant::now()),
        Instant::now(),
    );
    assert!(
        key_in_output(&out1, "a"),
        "engine still works, got {out1:?}"
    );
}

#[test]
fn switch_layer_replaces_entire_stack() {
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_profile("overlay", vec![tap_mapping("B", 1, "b")]);
    let gaming = make_profile("gaming", vec![tap_mapping("G", 1, "g")]);
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Permanent, now);
    engine.switch_layer(gaming);

    // After switch_layer, only the gaming layer should be active. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "g"),
        "expected 'g' after switch, got {out1:?}"
    );
    assert!(!key_in_output(&out1, "a") && !key_in_output(&out1, "b"));
}

// ── Passthrough walk (task 2.21) ──────────────────────────────────────────────

#[test]
fn passthrough_walks_to_lower_layer_on_unmatched() {
    // Base has code 1 → "a". Overlay has passthrough: true but no binding for code 1.
    // Code 1 tap should fall through to base and produce "a".
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_passthrough_profile("overlay", vec![tap_mapping("B", 2, "b")]);
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Permanent, now);

    // Opt A: fires immediately (no buffering).
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "a"),
        "expected base 'a' via passthrough, got {out1:?}"
    );
}

#[test]
fn no_passthrough_blocks_walk_on_unmatched() {
    // Base has code 1 → "a". Overlay has passthrough: false but no binding for code 1.
    // Code 1 should be silently consumed.
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_profile("overlay", vec![tap_mapping("B", 2, "b")]); // passthrough: false
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Permanent, now);

    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 3, t(now, 300)), t(now, 300));
    assert!(
        !key_in_output(&out2, "a"),
        "base 'a' must not fire when overlay has no passthrough"
    );
    let _ = out1;
}

// ── Block action (task 2.22) ──────────────────────────────────────────────────

#[test]
fn block_action_prevents_passthrough_to_lower_layer() {
    // Base has code 1 → "a". Overlay has passthrough: true AND code 1 → block.
    // Block should consume code 1 without passing through.
    let block_mapping = Mapping {
        label: "block code1".into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
        },
        action: Action::Block,
        enabled: true,
    };
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_passthrough_profile("overlay", vec![block_mapping]);
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Permanent, now);

    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 3, t(now, 300)), t(now, 300));
    assert!(
        !key_in_output(&out2, "a"),
        "block must prevent 'a' from base, got {out2:?}"
    );
    assert!(out2.iter().all(|o| o.actions.is_empty()));
    let _ = out1;
}

// ── Count expiry (task 2.27) ──────────────────────────────────────────────────

#[test]
fn count_mode_layer_pops_after_n_firings() {
    // Overlay with count: 2. After 2 successful trigger firings it should pop.
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_profile("overlay", vec![tap_mapping("B", 2, "b")]);
    let mut engine = ComboEngine::new(base);
    let now = Instant::now();

    engine.push_layer(overlay, PushLayerMode::Count { count: 2 }, now);

    // First firing (code 2 → "b"). Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 2, t(now, 0)), t(now, 0));
    assert!(key_in_output(&out1, "b"), "first firing: {out1:?}");

    // Second firing (code 2 → "b" again), which should trigger the pop.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 2, t(now, 100)), t(now, 100));
    assert!(key_in_output(&out2, "b"), "second firing: {out2:?}");

    // Now overlay should be popped — code 1 → "a" should be active.
    let out3 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 200)), t(now, 200));
    assert!(
        key_in_output(&out3, "a"),
        "base active after count expiry: {out3:?}"
    );
}

// ── Timeout expiry (task 2.28) ────────────────────────────────────────────────

#[test]
fn timeout_mode_layer_pops_after_deadline() {
    let base = make_profile("base", vec![tap_mapping("A", 1, "a")]);
    let overlay = make_profile("overlay", vec![tap_mapping("B", 1, "b")]);
    let now = Instant::now();
    let mut engine = ComboEngine::new(base);

    engine.push_layer(overlay, PushLayerMode::Timeout { timeout_ms: 500 }, now);

    // Before timeout: overlay is active.
    let before = t(now, 400);
    let out_before = engine.check_timeout(before);
    assert!(out_before.is_empty(), "no pop before deadline");

    // After timeout: overlay pops.
    let after = t(now, 600);
    let _pop_out = engine.check_timeout(after);

    // Base should now be active. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 700)), t(now, 700));
    assert!(
        key_in_output(&out1, "a"),
        "base active after timeout pop: {out1:?}"
    );
}

// ── on_enter / on_exit (task 2.30) ───────────────────────────────────────────

#[test]
fn push_layer_returns_on_enter_action() {
    let base = make_profile("base", vec![]);
    let mut overlay = make_profile("overlay", vec![]);
    overlay.on_enter = Some(key_action("f13"));

    let mut engine = ComboEngine::new(base);
    let now = Instant::now();
    let enter_out = engine.push_layer(overlay, PushLayerMode::Permanent, now);

    assert!(
        key_in_output(&enter_out, "f13"),
        "expected on_enter 'f13', got {enter_out:?}"
    );
}

#[test]
fn pop_layer_returns_on_exit_action() {
    let base = make_profile("base", vec![]);
    let mut overlay = make_profile("overlay", vec![]);
    overlay.on_exit = Some(key_action("f14"));

    let mut engine = ComboEngine::new(base);
    let now = Instant::now();
    engine.push_layer(overlay, PushLayerMode::Permanent, now);

    let exit_out = engine.pop_layer();
    assert!(
        key_in_output(&exit_out, "f14"),
        "expected on_exit 'f14', got {exit_out:?}"
    );
}

// ── Variable toggle (task 2.25) ───────────────────────────────────────────────

#[test]
fn toggle_variable_fires_on_false_when_variable_is_false() {
    let mut profile = make_profile("base", vec![]);
    profile
        .variables
        .insert("muted".into(), VariableValue::Bool(false));
    profile.mappings.push(Mapping {
        label: "toggle muted".into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
        },
        action: Action::ToggleVariable {
            variable: "muted".into(),
            on_true: Box::new(key_action("f13")),
            on_false: Box::new(key_action("f14")),
        },
        enabled: true,
    });

    let mut engine = ComboEngine::new(profile);
    let now = Instant::now();

    // muted=false → fires on_false → "f14", then flips to true. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "f14"),
        "expected on_false 'f14' when muted=false: {out1:?}"
    );
}

#[test]
fn toggle_variable_fires_on_true_when_variable_is_true() {
    let mut profile = make_profile("base", vec![]);
    profile
        .variables
        .insert("muted".into(), VariableValue::Bool(true));
    profile.mappings.push(Mapping {
        label: "toggle muted".into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
        },
        action: Action::ToggleVariable {
            variable: "muted".into(),
            on_true: Box::new(key_action("f13")),
            on_false: Box::new(key_action("f14")),
        },
        enabled: true,
    });

    let mut engine = ComboEngine::new(profile);
    let now = Instant::now();

    // muted=true → fires on_true → "f13", then flips to false. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "f13"),
        "expected on_true 'f13' when muted=true: {out1:?}"
    );
}

#[test]
fn toggle_variable_flips_on_each_successive_call() {
    // Each tap should alternate between on_false and on_true.
    let mut profile = make_profile("base", vec![]);
    profile
        .variables
        .insert("muted".into(), VariableValue::Bool(false));
    profile.mappings.push(Mapping {
        label: "toggle muted".into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
        },
        action: Action::ToggleVariable {
            variable: "muted".into(),
            on_true: Box::new(key_action("f13")),
            on_false: Box::new(key_action("f14")),
        },
        enabled: true,
    });

    let mut engine = ComboEngine::new(profile);
    let now = Instant::now();

    // Tap 1: muted=false → f14, now muted=true. Opt A: fires immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 0)), t(now, 0));
    assert!(
        key_in_output(&out1, "f14"),
        "first toggle (false→true): {out1:?}"
    );

    // Tap 2: muted=true → f13, now muted=false.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 1, t(now, 400)), t(now, 400));
    assert!(
        key_in_output(&out2, "f13"),
        "second toggle (true→false): {out2:?}"
    );
}
