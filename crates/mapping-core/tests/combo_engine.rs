use std::collections::HashMap;
use std::time::{Duration, Instant};

use mapping_core::engine::DebugEvent;
use mapping_core::engine::{ComboEngine, RawTapEvent};
use mapping_core::types::{
    Action, Hand, KeyDef, Mapping, OverloadStrategy, Profile, ProfileKind, ProfileSettings,
    PushLayerMode, TapCode, TapStep, Trigger, TriggerPattern,
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
            overload_strategy: Some(OverloadStrategy::Patient),
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
    assert!(mid.is_empty(), "100ms < 250ms window, should still be buffered");

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
            overload_strategy: Some(OverloadStrategy::Patient),
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
            overload_strategy: Some(OverloadStrategy::Patient),
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
            overload_strategy: Some(OverloadStrategy::Patient),
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
            overload_strategy: Some(OverloadStrategy::Patient),
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
            overload_strategy: Some(OverloadStrategy::Patient),
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
    let profile = Profile {
        settings: ProfileSettings {
            overload_strategy: Some(OverloadStrategy::Patient),
            ..Default::default()
        },
        ..profile
    };
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
    let profile =
        single_profile_with_mappings(vec![triple_tap_mapping("A triple", 1, "c")]);
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
    let profile = dual_profile_with_mappings(
        vec![dual_tap_mapping("Both thumbs", 1, 1, "space")],
        80,
    );
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    // Pair 1: R@0, L@20 — 20ms gap, well within 80ms window.
    let out1 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 0)), t(base, 0));
    assert!(out1.is_empty(), "R@0 should buffer");
    let out2 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 20)), t(base, 20));
    assert!(key_in_output(&out2, "space"), "pair 1 combo should fire, got {out2:?}");

    // Pair 2: R@40, L@60 — 20ms gap.
    let out3 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 40)), t(base, 40));
    assert!(out3.is_empty(), "R@40 should buffer");
    let out4 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 60)), t(base, 60));
    assert!(key_in_output(&out4, "space"), "pair 2 combo should fire, got {out4:?}");

    // Pair 3: R@80, L@100 — 20ms gap.
    let out5 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 80)), t(base, 80));
    assert!(out5.is_empty(), "R@80 should buffer");
    let out6 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 100)), t(base, 100));
    assert!(key_in_output(&out6, "space"), "pair 3 combo should fire, got {out6:?}");

    // Pair 4: R@120, L@140 — 20ms gap.
    let out7 = engine.push_event(RawTapEvent::new_at("right", 1, t(base, 120)), t(base, 120));
    assert!(out7.is_empty(), "R@120 should buffer");
    let out8 = engine.push_event(RawTapEvent::new_at("left", 1, t(base, 140)), t(base, 140));
    assert!(key_in_output(&out8, "space"), "pair 4 combo should fire, got {out8:?}");
}

#[test]
fn rapid_alternating_dual_taps_left_first_all_combos_fire() {
    // Same as above but left device always taps first.
    let profile = dual_profile_with_mappings(
        vec![dual_tap_mapping("Both thumbs", 1, 1, "space")],
        80,
    );
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
    assert!(out2.is_empty(), "R@5 (same device) should buffer alongside R@0");

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
            overload_strategy: Some(OverloadStrategy::Patient),
            ..Default::default()
        },
        ..profile
    };
    let mut engine = ComboEngine::new(profile);
    let base = Instant::now();

    assert!(engine.next_deadline().is_none(), "nothing pending yet");

    engine.push_event(RawTapEvent::new_at("solo", 1, t(base, 0)), t(base, 0));

    let deadline = engine.next_deadline().expect("deadline must be set after buffering");
    let expected = t(base, 0) + Duration::from_millis(250);
    assert_eq!(deadline, expected, "deadline should be received_at + double_tap_window_ms");
}
