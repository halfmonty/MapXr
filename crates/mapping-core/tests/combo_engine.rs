use std::collections::HashMap;
use std::time::{Duration, Instant};

use mapping_core::engine::DebugEvent;
use mapping_core::engine::{ComboEngine, RawTapEvent};
use mapping_core::types::{
    Action, Hand, HoldModifierMode, KeyDef, MacroStep, Mapping, Modifier, Profile, ProfileKind,
    ProfileSettings, PushLayerMode, TapCode, TapStep, Trigger, TriggerPattern,
};

// ── Profile builders ──────────────────────────────────────────────────────────

fn single_profile_with_mappings(mappings: Vec<Mapping>) -> Profile {
    Profile {
        version: 1,
        kind: ProfileKind::Single,
        name: "test".into(),
        layer_id: "test".into(),
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

fn dual_profile_with_mappings(mappings: Vec<Mapping>, combo_window_ms: u64) -> Profile {
    Profile {
        version: 1,
        kind: ProfileKind::Dual,
        name: "test".into(),
        layer_id: "test".into(),
        hand: None,
        description: None,
        passthrough: false,
        settings: ProfileSettings {
            combo_window_ms: Some(combo_window_ms),
            ..Default::default()
        },
        aliases: HashMap::new(),
        variables: HashMap::new(),
        on_enter: None,
        on_exit: None,
        mappings,
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
        condition: None,
    }
}

fn double_tap_mapping(label: &str, code: u8, key: &str) -> Mapping {
    Mapping {
        label: label.into(),
        trigger: Trigger::DoubleTap {
            code: TriggerPattern::Single(TapCode::from_u8(code).unwrap()),
        },
        action: Action::Key {
            key: KeyDef::new_unchecked(key),
            modifiers: vec![],
        },
        enabled: true,
        condition: None,
    }
}

fn triple_tap_mapping(label: &str, code: u8, key: &str) -> Mapping {
    Mapping {
        label: label.into(),
        trigger: Trigger::TripleTap {
            code: TriggerPattern::Single(TapCode::from_u8(code).unwrap()),
        },
        action: Action::Key {
            key: KeyDef::new_unchecked(key),
            modifiers: vec![],
        },
        enabled: true,
        condition: None,
    }
}

fn dual_tap_mapping(label: &str, left_code: u8, right_code: u8, key: &str) -> Mapping {
    Mapping {
        label: label.into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Dual {
                left: TapCode::from_u8(left_code).unwrap(),
                right: TapCode::from_u8(right_code).unwrap(),
            },
        },
        action: Action::Key {
            key: KeyDef::new_unchecked(key),
            modifiers: vec![],
        },
        enabled: true,
        condition: None,
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

// ── Single tap ────────────────────────────────────────────────────────────────

#[test]
fn single_tap_only_profile_dispatches_immediately() {
    // Opt A: a profile with only Tap bindings (no DoubleTap/TripleTap) must
    // dispatch the event on the same push_event call — no 250 ms buffer.
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Code 1 fires on first push — no waiting.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(
        key_in_output(&out1, "a"),
        "expected 'a' on first push (immediate dispatch), got {out1:?}"
    );
}

#[test]
fn single_tap_with_no_matching_binding_produces_no_output() {
    // Code 2 has no binding — should resolve silently.
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 0)), t(base, 0));
    let out = engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 300)), t(base, 300));
    // Code 2 resolves with no output (no binding); code 3 is now buffered.
    assert!(out.iter().all(|o| o.actions.is_empty()));
}

// ── Hardware-bounce debounce ──────────────────────────────────────────────────

#[test]
fn hardware_bounce_duplicate_within_debounce_window_is_discarded() {
    // Regression: TAP Strap sometimes emits a duplicate notification within
    // ~10–30 ms of a genuine tap. Without debounce, two events within the
    // double-tap window advance tap_pending to TapPending::Two and the single
    // tap fires as a double-tap action — the inversion bug.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            triple_tap_window_ms: Some(400),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Genuine tap at T=0.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "first tap should buffer");

    // Hardware bounce at T=10 (within 50ms debounce window) — must be discarded.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 10)), t(base, 10));
    assert!(
        out2.is_empty(),
        "bounce within debounce window must be silently discarded"
    );

    // After the double-tap window, only ONE tap should have fired (key 'a'), not
    // the double-tap action (key 'b').
    let out3 = engine.check_timeout(t(base, 300));
    assert!(
        key_in_output(&out3, "a"),
        "single tap should fire 'a' after window, got {out3:?}"
    );
    assert!(
        !key_in_output(&out3, "b"),
        "double-tap action must NOT fire for a bounced single tap"
    );
}

#[test]
fn hardware_bounce_different_code_is_not_debounced() {
    // Debounce only suppresses the same tap_code. A different code within
    // the window must still be processed.
    let profile =
        single_profile_with_mappings(vec![tap_mapping("A", 1, "a"), tap_mapping("B", 2, "b")]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(key_in_output(&out1, "a"), "code 1 should fire immediately");

    // Different code (2) within debounce window — should NOT be discarded.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 10)), t(base, 10));
    assert!(
        key_in_output(&out2, "b"),
        "different code within debounce window should still fire, got {out2:?}"
    );
}

#[test]
fn hardware_bounce_outside_debounce_window_is_treated_as_second_tap() {
    // A second event with the same code arriving AFTER the debounce window
    // (>= 50ms) is a genuine second tap and must be processed normally.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            triple_tap_window_ms: Some(400),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // First tap.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "first tap should buffer");

    // Second tap at T=100ms — outside debounce window, within double-tap window.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 100)), t(base, 100));
    assert!(
        out2.is_empty(),
        "second tap within double-tap window should buffer"
    );

    // Triple-tap window expires → should fire as double-tap.
    let out3 = engine.check_timeout(t(base, 450));
    assert!(
        key_in_output(&out3, "b"),
        "intentional double-tap should fire 'b', got {out3:?}"
    );
}

// ── check_timeout flushing ────────────────────────────────────────────────────

#[test]
fn check_timeout_flushes_single_tap_after_double_tap_window_expires() {
    // Regression test: when a code HAS a DoubleTap binding (and is therefore
    // buffered), the single tap must fire via check_timeout once the window
    // expires — not wait forever for the next push_event.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // First tap — buffered for double-tap detection (code is in needs_wait).
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "should buffer, not fire immediately");

    // Simulate timer ticks before the window expires — nothing should fire yet.
    let mid = engine.check_timeout(t(base, 100));
    assert!(
        mid.is_empty(),
        "100ms < 250ms window, should still be buffered"
    );

    // Simulate timer tick after the double-tap window has expired.
    let out2 = engine.check_timeout(t(base, 300));
    assert!(
        key_in_output(&out2, "a"),
        "expected 'a' to fire via check_timeout after window expired, got {out2:?}"
    );
}

#[test]
fn check_timeout_flushes_double_tap_after_triple_tap_window_expires() {
    // Two taps of the same code → buffered as TapPending::Two.
    // check_timeout must fire a double-tap action when the triple-tap window expires.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A tap", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            triple_tap_window_ms: Some(400),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 100)), t(base, 100));

    // Still within triple-tap window — nothing fires yet.
    let mid = engine.check_timeout(t(base, 200));
    assert!(mid.is_empty(), "should still be in TapPending::Two");

    // After triple-tap window expires, dispatch as double-tap.
    let out = engine.check_timeout(t(base, 450));
    assert!(
        key_in_output(&out, "b"),
        "expected double-tap action 'b' via check_timeout, got {out:?}"
    );
}

// ── Double tap ────────────────────────────────────────────────────────────────

#[test]
fn double_tap_within_window_fires_double_tap_action() {
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A tap", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // First tap.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty());

    // Second tap within 250ms — should be recognised as double-tap.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 100)), t(base, 100));
    assert!(
        out2.is_empty(),
        "double-tap still buffered waiting for triple window"
    );

    // Third different tap to flush — should resolve as double-tap of code 1.
    let out3 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 500)), t(base, 500));
    assert!(
        key_in_output(&out3, "b"),
        "expected 'b' (double-tap), got {out3:?}"
    );
}

#[test]
fn double_tap_outside_window_resolves_as_two_single_taps() {
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A tap", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    // Second tap after 300ms — outside 250ms double-tap window.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 300)), t(base, 300));
    // First tap resolves as single 'a'.
    assert!(
        key_in_output(&out2, "a"),
        "expected 'a' (single tap flush), got {out2:?}"
    );
}

// ── Triple tap ────────────────────────────────────────────────────────────────

#[test]
fn triple_tap_within_window_fires_triple_tap_action() {
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A tap", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
        triple_tap_mapping("A triple", 1, "c"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            triple_tap_window_ms: Some(400),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 100)), t(base, 100));
    // Third tap fires the triple-tap immediately.
    let out = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 200)), t(base, 200));
    assert!(
        key_in_output(&out, "c"),
        "expected 'c' (triple-tap), got {out:?}"
    );
}

// ── Cross-device combo ────────────────────────────────────────────────────────

#[test]
fn cross_device_combo_within_window_fires_combo_action() {
    // left=1 (thumb), right=1 (thumb) → Dual { left: 1, right: 1 } → "space"
    let profile = dual_profile_with_mappings(
        vec![dual_tap_mapping("Both thumbs", 1, 1, "space")],
        150, // combo_window_ms
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Left device taps at t=0.
    let out1 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "left tap buffered");

    // Right device taps at t=100 (within 150ms window).
    let out2 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 100)), t(base, 100));
    assert!(
        key_in_output(&out2, "space"),
        "expected 'space' combo, got {out2:?}"
    );
}

#[test]
fn cross_device_combo_outside_window_resolves_as_two_singles() {
    // left tap has a binding; right tap has a binding; they are too far apart.
    let profile = dual_profile_with_mappings(
        vec![
            Mapping {
                label: "left thumb".into(),
                trigger: Trigger::Tap {
                    code: TriggerPattern::Dual {
                        left: TapCode::from_u8(1).unwrap(),
                        right: TapCode::from_u8(0).unwrap(),
                    },
                },
                action: Action::Key {
                    key: KeyDef::new_unchecked("a"),
                    modifiers: vec![],
                },
                enabled: true,
                condition: None,
            },
            Mapping {
                label: "right thumb".into(),
                trigger: Trigger::Tap {
                    code: TriggerPattern::Dual {
                        left: TapCode::from_u8(0).unwrap(),
                        right: TapCode::from_u8(1).unwrap(),
                    },
                },
                action: Action::Key {
                    key: KeyDef::new_unchecked("b"),
                    modifiers: vec![],
                },
                enabled: true,
                condition: None,
            },
        ],
        150,
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Left taps at t=0.
    engine.push_event(RawTapEvent::new_at("left", 1, t(base, 0)), t(base, 0));

    // Right taps at t=200 — OUTSIDE 150ms window.
    // When the right tap arrives, flush_expired_combo should fire the left tap.
    let out = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 200)), t(base, 200));
    // Left tap (Dual{left:1, right:0}) should match "a", then right tap buffered.
    assert!(
        key_in_output(&out, "a"),
        "expected 'a' from flushed left tap, got {out:?}"
    );
}

// ── Sequence engine ───────────────────────────────────────────────────────────

fn seq_code(raw: u8) -> TriggerPattern {
    TriggerPattern::Single(TapCode::from_u8(raw).unwrap())
}

fn sequence_mapping(label: &str, codes: &[u8], key: &str, window_ms: Option<u64>) -> Mapping {
    Mapping {
        label: label.into(),
        trigger: Trigger::Sequence {
            steps: codes
                .iter()
                .map(|&c| TapStep { code: seq_code(c) })
                .collect(),
            window_ms,
        },
        action: Action::Key {
            key: KeyDef::new_unchecked(key),
            modifiers: vec![],
        },
        enabled: true,
        condition: None,
    }
}

#[test]
fn sequence_full_match_fires_action() {
    // Two-step sequence [code 1, code 2] → "s".
    let profile = single_profile_with_mappings(vec![sequence_mapping("seq", &[1, 2], "s", None)]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Step 1 — buffered.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "step 1 should be buffered, got {out1:?}");

    // Step 2 within default 500ms window — sequence fires.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 200)), t(base, 200));
    assert!(
        key_in_output(&out2, "s"),
        "expected 's' from full sequence match, got {out2:?}"
    );
}

#[test]
fn sequence_step_timeout_flushes_buffered_steps_as_singles() {
    // Sequence [code 1, code 2] with a tight 300ms window.
    // Code 1 also has a tap binding.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A", 1, "a"),
        sequence_mapping("seq", &[1, 2], "s", Some(300)),
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Step 1 — buffered by sequence.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // Code 3 arrives at t=400ms — outside the 300ms sequence window.
    // flush_expired_sequence should fire first, flushing step 1 as single "a".
    let out = engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 400)), t(base, 400));
    assert!(
        key_in_output(&out, "a"),
        "expected 'a' from flushed sequence step, got {out:?}"
    );
    // The sequence "s" must NOT have fired.
    assert!(
        !key_in_output(&out, "s"),
        "sequence 's' must not fire on timeout, got {out:?}"
    );
}

#[test]
fn sequence_mismatch_mid_way_flushes_buffered_steps_as_singles() {
    // Sequence [code 1, code 2] → "s"; code 1 alone → "a"; code 3 → "c".
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A", 1, "a"),
        tap_mapping("C", 3, "c"),
        sequence_mapping("seq", &[1, 2], "s", None),
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Step 1 — sequence started.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // Code 3 — wrong step: sequence aborts; step 1 flushed as "a", then code 3 as "c".
    let out = engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 100)), t(base, 100));
    assert!(
        key_in_output(&out, "a"),
        "expected 'a' from aborted sequence flush, got {out:?}"
    );
    // Code 3 is processed normally after the abort — it gets buffered for potential
    // double-tap detection but no binding matches it immediately here.
    // We just verify the sequence action did not fire.
    assert!(
        !key_in_output(&out, "s"),
        "sequence 's' must not fire on mismatch, got {out:?}"
    );
}

#[test]
fn sequence_interleaved_with_non_matching_tap_does_not_fire() {
    // Profile: sequence [1, 2] → "s".
    // Send: 1, 3 (mismatch), 2.
    // The sequence should abort on "3" and the subsequent "2" should not continue it.
    let profile = single_profile_with_mappings(vec![sequence_mapping("seq", &[1, 2], "s", None)]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    // Mismatch: sequence aborted.
    engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 100)), t(base, 100));
    // Code 2 arrives — sequence is no longer in progress.
    let out = engine.push_event(RawTapEvent::new_at("solo", 4, t(base, 200)), t(base, 200));
    assert!(
        !key_in_output(&out, "s"),
        "sequence 's' must not fire after mismatch, got {out:?}"
    );
}

#[test]
fn sequence_per_trigger_window_overrides_profile_default() {
    // Profile has sequence_window_ms = 500 (default), but the trigger sets 200ms.
    // Sending steps 400ms apart should time out.
    let profile = {
        let mappings = vec![sequence_mapping("seq", &[1, 2], "s", Some(200))];
        Profile {
            settings: ProfileSettings {
                sequence_window_ms: Some(500),
                ..Default::default()
            },
            ..single_profile_with_mappings(mappings)
        }
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    // Second step at 400ms — within profile default (500ms) but outside trigger window (200ms).
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 400)), t(base, 400));
    // Sequence must NOT have fired (trigger window took priority).
    assert!(
        !key_in_output(&out, "s"),
        "sequence must not fire when per-trigger window exceeded, got {out:?}"
    );
}

// ── Debug mode ────────────────────────────────────────────────────────────────

#[test]
fn debug_mode_off_produces_no_debug_events() {
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 300)), t(base, 300));
    assert!(out.iter().all(|o| o.debug.is_none()));
}

#[test]
fn debug_mode_on_attaches_resolved_event_on_match() {
    // Opt A: single-tap-only profile dispatches immediately, so the Resolved
    // debug event is in the first push_event output, not the second.
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    let out = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    let has_debug = out.iter().any(|o| {
        matches!(
            &o.debug,
            Some(mapping_core::engine::DebugEvent::Resolved { .. })
        )
    });
    assert!(has_debug, "expected Resolved debug event, got {out:?}");
}

#[test]
fn debug_mode_on_attaches_unmatched_event_on_miss() {
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    // Code 2 has no binding.
    engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 0)), t(base, 0));
    let out = engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 300)), t(base, 300));
    let has_unmatched = out
        .iter()
        .any(|o| matches!(&o.debug, Some(DebugEvent::Unmatched { .. })));
    assert!(has_unmatched, "expected Unmatched debug event, got {out:?}");
}

// ── Debug event timing metadata (task 2.35) ───────────────────────────────────

#[test]
fn debug_resolved_waited_ms_is_zero_for_immediate_dispatch() {
    // Opt A: single-tap-only profile dispatches immediately (waited_ms = 0).
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    let out = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let waited = out.iter().find_map(|o| {
        if let Some(DebugEvent::Resolved { waited_ms, .. }) = &o.debug {
            Some(*waited_ms)
        } else {
            None
        }
    });
    assert_eq!(
        waited,
        Some(0),
        "waited_ms should be 0 for immediate dispatch, got {out:?}"
    );
}

#[test]
fn debug_resolved_waited_ms_reflects_buffering_duration() {
    // When a code HAS a DoubleTap binding it is buffered; waited_ms must
    // reflect the actual gap between tap and flush.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    // Code 1 tapped at t=0, flushed by code 2 at t=200.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 200)), t(base, 200));

    let waited = out.iter().find_map(|o| {
        if let Some(DebugEvent::Resolved { waited_ms, .. }) = &o.debug {
            Some(*waited_ms)
        } else {
            None
        }
    });
    assert_eq!(
        waited,
        Some(200),
        "waited_ms should be 200ms (flush gap), got {out:?}"
    );
}

#[test]
fn debug_resolved_pattern_matches_tapped_code() {
    // Code 1 = thumb only on right hand → pattern string "xoooo".
    // Opt A: single-tap-only profile dispatches immediately on first push.
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    let out = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let pattern = out.iter().find_map(|o| {
        if let Some(DebugEvent::Resolved { pattern, .. }) = &o.debug {
            Some(pattern.clone())
        } else {
            None
        }
    });
    assert_eq!(
        pattern.as_deref(),
        Some("xoooo"),
        "pattern string wrong, got {out:?}"
    );
}

#[test]
fn debug_resolved_layer_stack_and_matched_layer_are_correct() {
    // Push an overlay. The overlay has a binding for code 1.
    // layer_stack should be ["overlay", "base"]; matched_layer should be "overlay".
    // Opt A: single-tap-only stack dispatches immediately on first push.
    let base_profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let overlay_profile = {
        let mut p = single_profile_with_mappings(vec![tap_mapping("B", 1, "b")]);
        p.layer_id = "overlay".into();
        p.name = "overlay".into();
        p
    };
    let mut engine = ComboEngine::new(base_profile);
    engine.set_debug(true);
    let base = Instant::now();

    engine.push_layer(overlay_profile, PushLayerMode::Permanent, base);

    let out = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let debug_info = out.iter().find_map(|o| {
        if let Some(DebugEvent::Resolved {
            layer_stack,
            matched_layer,
            ..
        }) = &o.debug
        {
            Some((layer_stack.clone(), matched_layer.clone()))
        } else {
            None
        }
    });
    let (stack, matched) = debug_info.expect("expected Resolved debug event");
    assert_eq!(
        stack,
        vec!["overlay", "test"],
        "layer_stack wrong: {stack:?}"
    );
    assert_eq!(matched, "overlay", "matched_layer wrong");
}

#[test]
fn debug_resolved_matched_mapping_label_is_correct() {
    // Opt A: single-tap-only profile dispatches immediately on first push.
    let profile = single_profile_with_mappings(vec![tap_mapping("My A Binding", 1, "a")]);
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    let out = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let label = out.iter().find_map(|o| {
        if let Some(DebugEvent::Resolved {
            matched_mapping, ..
        }) = &o.debug
        {
            Some(matched_mapping.clone())
        } else {
            None
        }
    });
    assert_eq!(
        label.as_deref(),
        Some("My A Binding"),
        "matched_mapping label wrong, got {out:?}"
    );
}

#[test]
fn debug_unmatched_passthrough_layers_checked_lists_all_walked_layers() {
    // Two-layer stack: overlay (passthrough: true, no binding for code 3) +
    // base (no binding for code 3). Both should appear in passthrough_layers_checked.
    let base_profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let overlay_profile = {
        let mut p = Profile {
            passthrough: true,
            ..single_profile_with_mappings(vec![tap_mapping("B", 2, "b")])
        };
        p.layer_id = "overlay".into();
        p.name = "overlay".into();
        p
    };
    let mut engine = ComboEngine::new(base_profile);
    engine.set_debug(true);
    let base = Instant::now();

    engine.push_layer(overlay_profile, PushLayerMode::Permanent, base);

    // Code 3 has no binding in either layer.
    engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 0)), t(base, 0));
    let out = engine.push_event(RawTapEvent::new_at("solo", 4, t(base, 300)), t(base, 300));

    let layers = out.iter().find_map(|o| {
        if let Some(DebugEvent::Unmatched {
            passthrough_layers_checked,
            ..
        }) = &o.debug
        {
            Some(passthrough_layers_checked.clone())
        } else {
            None
        }
    });
    let layers = layers.expect("expected Unmatched debug event");
    assert!(
        layers.contains(&"overlay".to_string()),
        "overlay should be listed, got {layers:?}"
    );
    assert!(
        layers.contains(&"test".to_string()),
        "base layer 'test' should be listed, got {layers:?}"
    );
}

#[test]
fn debug_combo_timeout_reports_correct_gap_and_window() {
    // Dual profile with combo_window_ms=150. Left at t=0, right at t=200.
    // Gap = 200ms > window = 150ms → ComboTimeout with actual_gap_ms=200.
    let profile =
        dual_profile_with_mappings(vec![dual_tap_mapping("Both thumbs", 1, 1, "space")], 150);
    let mut engine = ComboEngine::new(profile);
    engine.set_debug(true);
    let base = Instant::now();

    // Left device taps at t=0 (buffered).
    engine.push_event(RawTapEvent::new_at("left", 1, t(base, 0)), t(base, 0));
    // Right device taps at t=200 — outside 150ms window; triggers a ComboTimeout.
    let out = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 200)), t(base, 200));

    let timeout_info = out.iter().find_map(|o| {
        if let Some(DebugEvent::ComboTimeout {
            combo_window_ms,
            actual_gap_ms,
            ..
        }) = &o.debug
        {
            Some((*combo_window_ms, *actual_gap_ms))
        } else {
            None
        }
    });
    let (window, gap) = timeout_info.expect("expected ComboTimeout debug event");
    assert_eq!(window, 150, "combo_window_ms should be 150");
    assert_eq!(gap, 200, "actual_gap_ms should be 200");
}

// ── Optimization A — immediate dispatch for codes with no multi-tap binding ───

#[test]
fn single_tap_with_double_tap_binding_still_waits() {
    // Opt A: a code that appears in a DoubleTap trigger IS in needs_wait and
    // must still be buffered to allow double-tap detection.
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A single", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // First tap — must buffer, not fire immediately.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(
        out1.is_empty(),
        "code with DoubleTap binding must buffer on first tap, got {out1:?}"
    );
}

#[test]
fn single_tap_triple_tap_binding_still_waits() {
    // Opt A: a code that appears in a TripleTap trigger IS in needs_wait.
    let profile = single_profile_with_mappings(vec![triple_tap_mapping("A triple", 1, "c")]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // First tap — must buffer (TripleTap binding in profile).
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(
        out1.is_empty(),
        "code with TripleTap binding must buffer on first tap, got {out1:?}"
    );
}

// ── Rapid alternating dual taps ───────────────────────────────────────────────

#[test]
fn rapid_alternating_dual_taps_all_combos_fire() {
    // Simulates the user-reported scenario: rapid R→L→R→L alternation.
    // All pairs should form combos; none should be missed or delayed.
    let profile =
        dual_profile_with_mappings(vec![dual_tap_mapping("Both thumbs", 1, 1, "space")], 80);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Pair 1: R@0, L@20 — 20ms gap, well within 80ms window.
    let out1 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "R@0 should buffer");
    let out2 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 20)), t(base, 20));
    assert!(
        key_in_output(&out2, "space"),
        "pair 1 combo should fire, got {out2:?}"
    );

    // Pair 2: R@40, L@60 — 20ms gap.
    let out3 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 40)), t(base, 40));
    assert!(out3.is_empty(), "R@40 should buffer");
    let out4 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 60)), t(base, 60));
    assert!(
        key_in_output(&out4, "space"),
        "pair 2 combo should fire, got {out4:?}"
    );

    // Pair 3: R@80, L@100 — 20ms gap.
    let out5 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 80)), t(base, 80));
    assert!(out5.is_empty(), "R@80 should buffer");
    let out6 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 100)), t(base, 100));
    assert!(
        key_in_output(&out6, "space"),
        "pair 3 combo should fire, got {out6:?}"
    );

    // Pair 4: R@120, L@140 — 20ms gap.
    let out7 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 120)), t(base, 120));
    assert!(out7.is_empty(), "R@120 should buffer");
    let out8 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 140)), t(base, 140));
    assert!(
        key_in_output(&out8, "space"),
        "pair 4 combo should fire, got {out8:?}"
    );
}

#[test]
fn rapid_alternating_dual_taps_left_first_all_combos_fire() {
    // Same as above but left device always taps first.
    let profile =
        dual_profile_with_mappings(vec![dual_tap_mapping("Both thumbs", 1, 1, "space")], 80);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    for i in 0u64..4 {
        let l_time = i * 40;
        let r_time = l_time + 20;
        let out_l = engine.push_event(
            RawTapEvent::new_at("left", 1, t(base, l_time)),
            t(base, l_time),
        );
        assert!(out_l.is_empty(), "L@{l_time} should buffer");
        let out_r = engine.push_event(
            RawTapEvent::new_at("right", 1, t(base, r_time)),
            t(base, r_time),
        );
        assert!(
            key_in_output(&out_r, "space"),
            "pair {} combo (L first) should fire, got {out_r:?}",
            i + 1
        );
    }
}

#[test]
fn rapid_alternating_dual_same_device_stacks_then_all_resolve() {
    // Both right-device taps arrive before either left-device tap.
    // combo_pending accumulates [R@0, R@5] before any left event.
    // Both should eventually find left partners in order.
    let profile = dual_profile_with_mappings(
        vec![
            dual_tap_mapping("Both thumbs", 1, 1, "space"),
            // solo bindings so flushed entries also produce output
            Mapping {
                label: "right solo".into(),
                trigger: Trigger::Tap {
                    code: TriggerPattern::Dual {
                        left: TapCode::from_u8(0).unwrap(),
                        right: TapCode::from_u8(1).unwrap(),
                    },
                },
                action: Action::Key {
                    key: KeyDef::new_unchecked("r"),
                    modifiers: vec![],
                },
                enabled: true,
                condition: None,
            },
            Mapping {
                label: "left solo".into(),
                trigger: Trigger::Tap {
                    code: TriggerPattern::Dual {
                        left: TapCode::from_u8(1).unwrap(),
                        right: TapCode::from_u8(0).unwrap(),
                    },
                },
                action: Action::Key {
                    key: KeyDef::new_unchecked("l"),
                    modifiers: vec![],
                },
                enabled: true,
                condition: None,
            },
        ],
        80,
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Two right-device taps arrive together (same-device burst).
    let out1 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "R@0 should buffer");
    let out2 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 5)), t(base, 5));
    assert!(
        out2.is_empty(),
        "R@5 (same device) should buffer alongside R@0"
    );

    // Left taps arrive, each should match the oldest pending right entry.
    let out3 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 20)), t(base, 20));
    assert!(
        key_in_output(&out3, "space"),
        "L@20 should combo with R@0 (oldest right entry), got {out3:?}"
    );

    let out4 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 30)), t(base, 30));
    assert!(
        key_in_output(&out4, "space"),
        "L@30 should combo with R@5 (next right entry), got {out4:?}"
    );
}

// ── Optimization B — next_deadline precision ──────────────────────────────────

#[test]
fn next_deadline_none_when_nothing_pending() {
    let profile = single_profile_with_mappings(vec![tap_mapping("A", 1, "a")]);
    let engine = ComboEngine::new(profile);
    assert!(
        engine.next_deadline().is_none(),
        "no pending state → next_deadline must be None"
    );
}

#[test]
fn next_deadline_set_after_tap_buffered() {
    // When a code is in needs_wait, the first tap is buffered and
    // next_deadline should return Some(received_at + double_tap_window_ms).
    let profile = single_profile_with_mappings(vec![
        tap_mapping("A single", 1, "a"),
        double_tap_mapping("A double", 1, "b"),
    ]);
    let profile = Profile {
        settings: ProfileSettings {
            double_tap_window_ms: Some(250),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    assert!(engine.next_deadline().is_none(), "nothing pending yet");

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let deadline = engine
        .next_deadline()
        .expect("deadline must be set after buffering");
    let expected = t(base, 0) + Duration::from_millis(250);
    assert_eq!(
        deadline, expected,
        "deadline should be received_at + double_tap_window_ms"
    );
}

// ── hold_modifier ─────────────────────────────────────────────────────────────

/// Build a single-hand profile with two mappings:
///   code 1 → `hold_modifier { modifiers, mode }`
///   code 2 → `Key { key, modifiers: [] }`
fn hold_modifier_profile(modifiers: Vec<Modifier>, mode: HoldModifierMode, key: &str) -> Profile {
    single_profile_with_mappings(vec![
        Mapping {
            label: "hold".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier { modifiers, mode },
            enabled: true,
            condition: None,
        },
        tap_mapping("key", 2, key),
    ])
}

/// Collect all actions from a flat list of EngineOutputs.
fn collect_actions(outputs: &[mapping_core::engine::EngineOutput]) -> Vec<&Action> {
    outputs.iter().flat_map(|o| &o.actions).collect()
}

#[test]
fn hold_modifier_toggle_activates_on_first_dispatch() {
    let profile = hold_modifier_profile(vec![Modifier::Shift], HoldModifierMode::Toggle, "a");
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Tap code 1 → hold_modifier; should produce no actions.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    assert!(
        collect_actions(&out1).is_empty(),
        "hold_modifier fires no action: {out1:?}"
    );

    // Tap code 2 → Key "a"; should have Shift injected.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 100)), t(base, 100));
    let actions = collect_actions(&out2);
    assert_eq!(actions.len(), 1, "expected one action: {actions:?}");
    match actions[0] {
        Action::Key { key, modifiers } => {
            assert_eq!(key.as_str(), "a");
            assert!(
                modifiers.contains(&Modifier::Shift),
                "expected Shift in modifiers, got {modifiers:?}"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_toggle_deactivates_on_second_dispatch_same_set() {
    let profile = hold_modifier_profile(vec![Modifier::Shift], HoldModifierMode::Toggle, "a");
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Activate: tap code 1 twice → second should deactivate.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 100)), t(base, 100));

    // Tap code 2 → Key "a"; modifier should be gone.
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 200)), t(base, 200));
    let actions = collect_actions(&out);
    assert_eq!(actions.len(), 1, "expected one action: {actions:?}");
    match actions[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                !modifiers.contains(&Modifier::Shift),
                "Shift should be deactivated, got {modifiers:?}"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_toggle_two_different_sets_independent() {
    // Code 1 → hold Shift (toggle), Code 3 → hold Ctrl (toggle), Code 2 → Key "a"
    let profile = single_profile_with_mappings(vec![
        Mapping {
            label: "hold shift".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: HoldModifierMode::Toggle,
            },
            enabled: true,
            condition: None,
        },
        Mapping {
            label: "hold ctrl".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(3).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Ctrl],
                mode: HoldModifierMode::Toggle,
            },
            enabled: true,
            condition: None,
        },
        tap_mapping("key", 2, "a"),
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));
    engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 100)), t(base, 100));

    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 200)), t(base, 200));
    let actions = collect_actions(&out);
    assert_eq!(actions.len(), 1);
    match actions[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                modifiers.contains(&Modifier::Shift),
                "expected Shift: {modifiers:?}"
            );
            assert!(
                modifiers.contains(&Modifier::Ctrl),
                "expected Ctrl: {modifiers:?}"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_count_one_applies_modifier_to_first_key_only() {
    let profile = hold_modifier_profile(
        vec![Modifier::Shift],
        HoldModifierMode::Count { count: 1 },
        "a",
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // First key: Shift applied.
    let out1 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 100)), t(base, 100));
    match collect_actions(&out1)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                modifiers.contains(&Modifier::Shift),
                "first key should have Shift"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }

    // Second key: count exhausted; no Shift.
    let out2 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 200)), t(base, 200));
    match collect_actions(&out2)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                !modifiers.contains(&Modifier::Shift),
                "second key should not have Shift"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_count_two_applies_modifier_to_two_keys() {
    let profile = hold_modifier_profile(
        vec![Modifier::Shift],
        HoldModifierMode::Count { count: 2 },
        "a",
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    for (i, offset) in [100u64, 200].iter().enumerate() {
        let out = engine.push_event(
            RawTapEvent::new_at("solo", 2, t(base, *offset)),
            t(base, *offset),
        );
        match collect_actions(&out)[0] {
            Action::Key { modifiers, .. } => {
                assert!(
                    modifiers.contains(&Modifier::Shift),
                    "key {i} should have Shift"
                );
            }
            other => panic!("expected Key, got {other:?}"),
        }
    }

    // Third key: count exhausted.
    let out3 = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 300)), t(base, 300));
    match collect_actions(&out3)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                !modifiers.contains(&Modifier::Shift),
                "third key should not have Shift"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_count_type_string_decrements_count_without_applying_modifier() {
    // Code 1 → hold_modifier count=1, Code 2 → TypeString, Code 3 → Key "a"
    let profile = single_profile_with_mappings(vec![
        Mapping {
            label: "hold".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: HoldModifierMode::Count { count: 1 },
            },
            enabled: true,
            condition: None,
        },
        Mapping {
            label: "type".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(2).unwrap()),
            },
            action: Action::TypeString {
                text: "hello".into(),
            },
            enabled: true,
            condition: None,
        },
        tap_mapping("key", 3, "a"),
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // TypeString: modifier not applied, but count decremented.
    let out_ts = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 100)), t(base, 100));
    let ts_actions = collect_actions(&out_ts);
    assert!(
        matches!(ts_actions[0], Action::TypeString { text } if text == "hello"),
        "TypeString should be unmodified: {ts_actions:?}"
    );

    // Key "a": count was exhausted by TypeString dispatch; no modifier.
    let out_key = engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 200)), t(base, 200));
    match collect_actions(&out_key)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                !modifiers.contains(&Modifier::Shift),
                "count exhausted; key should have no Shift"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_timeout_applies_within_window() {
    let profile = hold_modifier_profile(
        vec![Modifier::Shift],
        HoldModifierMode::Timeout { timeout_ms: 1000 },
        "a",
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Activate hold_modifier.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // Key dispatched at 500 ms — well within the 1000 ms timeout.
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 500)), t(base, 500));
    match collect_actions(&out)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                modifiers.contains(&Modifier::Shift),
                "Shift should be active within window"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_timeout_does_not_apply_after_expiry() {
    let profile = hold_modifier_profile(
        vec![Modifier::Shift],
        HoldModifierMode::Timeout { timeout_ms: 100 },
        "a",
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Activate hold_modifier.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // Advance time past the deadline via check_timeout.
    engine.check_timeout(t(base, 200));

    // Key dispatched after expiry — Shift should be gone.
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 300)), t(base, 300));
    match collect_actions(&out)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                !modifiers.contains(&Modifier::Shift),
                "Shift should be expired: {modifiers:?}"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_combines_with_action_own_modifiers() {
    // Key binding already has Ctrl; hold_modifier adds Shift.
    let profile = single_profile_with_mappings(vec![
        Mapping {
            label: "hold shift".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: HoldModifierMode::Toggle,
            },
            enabled: true,
            condition: None,
        },
        Mapping {
            label: "ctrl+a".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(2).unwrap()),
            },
            action: Action::Key {
                key: KeyDef::new_unchecked("a"),
                modifiers: vec![Modifier::Ctrl],
            },
            enabled: true,
            condition: None,
        },
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 100)), t(base, 100));
    match collect_actions(&out)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                modifiers.contains(&Modifier::Ctrl),
                "expected Ctrl: {modifiers:?}"
            );
            assert!(
                modifiers.contains(&Modifier::Shift),
                "expected Shift: {modifiers:?}"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_survives_push_layer() {
    let base_profile = single_profile_with_mappings(vec![
        Mapping {
            label: "hold shift".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: HoldModifierMode::Toggle,
            },
            enabled: true,
            condition: None,
        },
        tap_mapping("key", 2, "a"),
    ]);
    let overlay_profile = single_profile_with_mappings(vec![tap_mapping("key2", 2, "b")]);

    let mut engine = ComboEngine::new(base_profile);
    let base = Instant::now();

    // Activate modifier on base layer.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // Push an overlay layer — held_modifiers must survive.
    engine.push_layer(overlay_profile, PushLayerMode::Permanent, t(base, 100));

    // Key on overlay layer should have Shift.
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 200)), t(base, 200));
    match collect_actions(&out)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                modifiers.contains(&Modifier::Shift),
                "Shift should survive push_layer"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_survives_pop_layer() {
    let base_profile = single_profile_with_mappings(vec![tap_mapping("key", 2, "a")]);
    let overlay_profile = single_profile_with_mappings(vec![
        Mapping {
            label: "hold shift".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: HoldModifierMode::Toggle,
            },
            enabled: true,
            condition: None,
        },
        Mapping {
            label: "pop".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(3).unwrap()),
            },
            action: Action::PopLayer,
            enabled: true,
            condition: None,
        },
    ]);

    let mut engine = ComboEngine::new(base_profile);
    let base = Instant::now();

    engine.push_layer(overlay_profile, PushLayerMode::Permanent, t(base, 0));

    // Activate modifier on overlay.
    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 100)), t(base, 100));

    // Pop layer.
    engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 200)), t(base, 200));

    // Key on base layer should still have Shift.
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 300)), t(base, 300));
    match collect_actions(&out)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                modifiers.contains(&Modifier::Shift),
                "Shift should survive pop_layer"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

#[test]
fn hold_modifier_macro_counts_as_single_decrement() {
    // count=1; a macro with two Key steps should count as one decrement.
    let profile = single_profile_with_mappings(vec![
        Mapping {
            label: "hold shift".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            },
            action: Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: HoldModifierMode::Count { count: 1 },
            },
            enabled: true,
            condition: None,
        },
        Mapping {
            label: "macro".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(TapCode::from_u8(2).unwrap()),
            },
            action: Action::Macro {
                steps: vec![
                    MacroStep {
                        action: Action::Key {
                            key: KeyDef::new_unchecked("a"),
                            modifiers: vec![],
                        },
                        delay_ms: 0,
                    },
                    MacroStep {
                        action: Action::Key {
                            key: KeyDef::new_unchecked("b"),
                            modifiers: vec![],
                        },
                        delay_ms: 0,
                    },
                ],
            },
            enabled: true,
            condition: None,
        },
        tap_mapping("key", 3, "c"),
    ]);
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    // Macro dispatch: Shift applied to both steps; count decremented once.
    let out = engine.push_event(RawTapEvent::new_at("solo", 2, t(base, 100)), t(base, 100));
    let actions = collect_actions(&out);
    assert_eq!(actions.len(), 1, "one Macro action returned");
    match actions[0] {
        Action::Macro { steps } => {
            for step in steps {
                match &step.action {
                    Action::Key { modifiers, .. } => {
                        assert!(
                            modifiers.contains(&Modifier::Shift),
                            "macro step should have Shift: {modifiers:?}"
                        );
                    }
                    other => panic!("expected Key step, got {other:?}"),
                }
            }
        }
        other => panic!("expected Macro, got {other:?}"),
    }

    // Key "c": count was exhausted by the macro dispatch (one decrement total).
    let out3 = engine.push_event(RawTapEvent::new_at("solo", 3, t(base, 200)), t(base, 200));
    match collect_actions(&out3)[0] {
        Action::Key { modifiers, .. } => {
            assert!(
                !modifiers.contains(&Modifier::Shift),
                "count exhausted after macro"
            );
        }
        other => panic!("expected Key, got {other:?}"),
    }
}

// ── pop_layer bug fixes (task 8.1) ────────────────────────────────────────────

/// `pop_layer()` must return `None` when the stack is already at the base layer.
#[test]
fn pop_layer_at_base_returns_none() {
    let profile = single_profile_with_mappings(vec![]);
    let mut engine = ComboEngine::new(profile);
    assert!(
        engine.pop_layer().is_none(),
        "pop_layer at base should return None"
    );
}

/// `pop_layer()` must return `Some([])` when the popped layer has no on_exit
/// action — previously this returned an empty vec which was misread as the
/// at-base sentinel.
#[test]
fn pop_layer_no_on_exit_returns_some_empty_outputs() {
    let base = single_profile_with_mappings(vec![]);
    let overlay = Profile {
        layer_id: "overlay".into(),
        name: "overlay".into(),
        on_exit: None, // no on_exit action
        ..base.clone()
    };
    let mut engine = ComboEngine::new(base);
    engine.push_layer(overlay, PushLayerMode::Permanent, Instant::now());

    let result = engine.pop_layer();
    assert!(
        result.is_some(),
        "pop_layer should succeed when not at base"
    );
    assert!(
        result.unwrap().is_empty(),
        "outputs should be empty when no on_exit action"
    );
}

/// `pop_layer()` must return `Some(outputs)` containing the on_exit action
/// when the popped layer has one.
#[test]
fn pop_layer_with_on_exit_returns_some_with_outputs() {
    let base = single_profile_with_mappings(vec![]);
    let overlay = Profile {
        layer_id: "overlay".into(),
        name: "overlay".into(),
        on_exit: Some(Action::Key {
            key: KeyDef::new_unchecked("f14"),
            modifiers: vec![],
        }),
        ..base.clone()
    };
    let mut engine = ComboEngine::new(base);
    engine.push_layer(overlay, PushLayerMode::Permanent, Instant::now());

    let result = engine.pop_layer();
    assert!(result.is_some(), "pop_layer should succeed");
    assert!(
        key_in_output(&result.unwrap(), "f14"),
        "on_exit action should be in outputs"
    );
}

/// When `Action::PopLayer` fires from a mapping, the engine output must carry
/// `layer_changed = true` so the pump knows to emit a layer-changed event —
/// regardless of whether the popped layer had an on_exit action.
#[test]
fn pop_layer_action_in_mapping_sets_layer_changed_flag() {
    use mapping_core::types::Trigger;

    let base_profile = single_profile_with_mappings(vec![]);
    let pop_mapping = Mapping {
        label: "pop".into(),
        trigger: Trigger::Tap {
            code: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
        },
        action: Action::PopLayer,
        enabled: true,
        condition: None,
    };
    // Push an overlay with no on_exit that has a PopLayer mapping.
    let overlay = Profile {
        layer_id: "overlay".into(),
        name: "overlay".into(),
        on_exit: None,
        mappings: vec![pop_mapping],
        ..base_profile.clone()
    };
    let mut engine = ComboEngine::new(base_profile);
    engine.push_layer(overlay, PushLayerMode::Permanent, Instant::now());

    let base = Instant::now();
    let outputs = engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    assert!(
        outputs.iter().any(|o| o.layer_changed),
        "at least one output should have layer_changed = true after PopLayer fires"
    );
    // Stack should be back at base.
    assert_eq!(engine.layer_ids(), vec!["test"]);
}

#[test]
fn hold_modifier_timeout_included_in_next_deadline() {
    let profile = hold_modifier_profile(
        vec![Modifier::Shift],
        HoldModifierMode::Timeout { timeout_ms: 500 },
        "a",
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    assert!(
        engine.next_deadline().is_none(),
        "no deadline before activation"
    );

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let deadline = engine
        .next_deadline()
        .expect("deadline set after hold_modifier activation");
    // Deadline should be approximately base + 500 ms (within a small tolerance).
    let lower = t(base, 490);
    let upper = t(base, 510);
    assert!(
        deadline >= lower && deadline <= upper,
        "deadline {deadline:?} should be near base+500ms"
    );
}
