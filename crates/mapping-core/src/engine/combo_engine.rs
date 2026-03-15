use std::time::{Duration, Instant};

use crate::engine::{
    DebugEvent, DeviceId, EngineOutput, LayerStack, RawTapEvent, ResolvedEvent, ResolvedTriggerKind,
};
use crate::types::{
    Action, Hand, OverloadStrategy, Profile, PushLayerMode, TapCode, TriggerPattern, VariableValue,
};

// ── Default timing constants ───────────────────────────────────────────────

const DEFAULT_COMBO_WINDOW_MS: u64 = 80;
const DEFAULT_DOUBLE_TAP_WINDOW_MS: u64 = 250;
const DEFAULT_TRIPLE_TAP_WINDOW_MS: u64 = 400;
const DEFAULT_SEQUENCE_WINDOW_MS: u64 = 500;

// ── Pending event buffer entry ─────────────────────────────────────────────

/// A raw event held in the pending buffer while the engine waits to determine
/// whether it is part of a combo, double-tap, triple-tap, or sequence.
#[derive(Debug, Clone)]
struct PendingEntry {
    device_id: DeviceId,
    tap_code: TapCode,
    received_at: Instant,
}

// ── Overload table ─────────────────────────────────────────────────────────

/// Tap codes that appear in both a `tap` and a `double_tap`/`triple_tap`
/// binding in the active profile.
#[derive(Debug, Default)]
struct OverloadTable {
    /// Canonical right-hand pattern strings of overloaded codes.
    overloaded: std::collections::HashSet<String>,
}

impl OverloadTable {
    fn build(profile: &Profile) -> Self {
        use crate::types::Trigger;
        let hand = profile.hand.unwrap_or(Hand::Right);
        let mut tap_codes = std::collections::HashSet::new();
        let mut overloaded = std::collections::HashSet::new();
        for mapping in &profile.mappings {
            if !mapping.enabled {
                continue;
            }
            match &mapping.trigger {
                Trigger::Tap { code } => {
                    tap_codes.insert((*code).to_pattern_string(hand));
                }
                Trigger::DoubleTap { code } | Trigger::TripleTap { code } => {
                    let key = (*code).to_pattern_string(hand);
                    if tap_codes.contains(&key) {
                        overloaded.insert(key);
                    }
                }
                Trigger::Sequence { .. } => {}
            }
        }
        Self { overloaded }
    }

    fn is_overloaded(&self, pattern: &str) -> bool {
        self.overloaded.contains(pattern)
    }
}

// ── Pending double/triple tap state ───────────────────────────────────────

/// State held while the engine waits to see if a tap becomes a double- or
/// triple-tap.
#[derive(Debug)]
enum TapPending {
    /// Waiting for a potential second tap of the same code.
    One { entry: PendingEntry },
    /// Got two identical taps; waiting for a potential third.
    Two {
        first: PendingEntry,
        second: PendingEntry,
    },
}

// ── Sequence progress tracking ─────────────────────────────────────────────

/// Tracks partial progress through a [`crate::types::Trigger::Sequence`] match.
///
/// Created when the first step of a sequence mapping is observed. Cleared when
/// the sequence either completes (all steps matched) or is aborted (timeout or
/// step mismatch).
#[derive(Debug)]
struct SequenceProgress {
    /// Index into `profile.mappings` of the sequence being tracked.
    mapping_idx: usize,
    /// Number of steps successfully matched so far (always ≥ 1 once created).
    steps_matched: usize,
    /// Per-step timeout window in milliseconds (from per-trigger `window_ms` or
    /// the profile-level `sequence_window_ms` default).
    window_ms: u64,
    /// Timestamp of the last successfully matched step.
    last_step_at: Instant,
    /// Raw events for all matched steps, buffered so they can be dispatched as
    /// individual single-taps if the sequence is later aborted.
    buffered: Vec<PendingEntry>,
}

// ── ComboEngine ────────────────────────────────────────────────────────────

/// The main event-processing engine.
///
/// Receives [`RawTapEvent`] values from the BLE layer, resolves them through
/// combo detection, overload strategy, and double/triple-tap detection, and
/// returns [`EngineOutput`] values for the platform layer to execute.
///
/// # Timing
///
/// All timing comparisons use `Instant` values from the events themselves.
/// `push_event` accepts a `now: Instant` parameter so that callers (including
/// tests) can provide controlled timestamps.
///
/// # Usage
///
/// ```no_run
/// use mapping_core::engine::{ComboEngine, RawTapEvent, DeviceId};
/// use mapping_core::types::Profile;
/// use std::time::Instant;
///
/// # fn load_profile() -> Profile { unimplemented!() }
/// let mut engine = ComboEngine::new(load_profile());
/// let event = RawTapEvent {
///     device_id: DeviceId::new("solo"),
///     tap_code: 1,
///     received_at: Instant::now(),
/// };
/// let outputs = engine.push_event(event, Instant::now());
/// ```
pub struct ComboEngine {
    /// Active layer stack. The top layer drives all resolution decisions.
    layer_stack: LayerStack,
    /// Overloaded codes pre-computed from the top layer at engine init / stack
    /// change. Rebuilt whenever the top layer changes.
    overloads: OverloadTable,
    /// Tap codes that appear in any `DoubleTap` or `TripleTap` trigger across
    /// all layers of the active stack. Rebuilt alongside `overloads` whenever
    /// the stack changes.
    ///
    /// Events for codes **not** in this set have no multi-tap binding and can
    /// be dispatched immediately without waiting for the double-tap window.
    needs_wait: std::collections::HashSet<TapCode>,
    /// Pending events waiting for a cross-device combo partner.
    /// At most one per device (later events of the same device replace
    /// earlier ones after the earlier one is flushed).
    combo_pending: Vec<PendingEntry>,
    /// State for double/triple-tap detection on the current code.
    tap_pending: Option<TapPending>,
    /// In-progress partial sequence match, if any.
    seq_progress: Option<SequenceProgress>,
    /// Whether debug events are emitted alongside action outputs.
    debug_mode: bool,
}

impl ComboEngine {
    /// Create a new `ComboEngine` with the given base profile.
    ///
    /// The profile's `on_enter` action (if any) is NOT dispatched here — it
    /// is the caller's responsibility to inspect `profile.on_enter` before
    /// constructing the engine and fire it if needed.
    pub fn new(profile: Profile) -> Self {
        let layer_stack = LayerStack::new(profile);
        let overloads = OverloadTable::build(layer_stack.top());
        let needs_wait = build_needs_wait(&layer_stack);
        Self {
            layer_stack,
            overloads,
            needs_wait,
            combo_pending: Vec::new(),
            tap_pending: None,
            seq_progress: None,
            debug_mode: false,
        }
    }

    /// Replace the base profile, discarding the entire stack. Clears all
    /// pending state.
    pub fn set_profile(&mut self, profile: Profile) {
        self.layer_stack.switch_to(profile);
        self.rebuild_overloads();
        self.combo_pending.clear();
        self.tap_pending = None;
        self.seq_progress = None;
    }

    /// Push a new layer onto the stack, returning any `on_enter` action.
    pub fn push_layer(
        &mut self,
        profile: Profile,
        mode: PushLayerMode,
        now: std::time::Instant,
    ) -> Vec<EngineOutput> {
        let on_enter = self.layer_stack.push(profile, mode, now);
        self.rebuild_overloads();
        self.clear_pending();
        on_enter
            .into_iter()
            .map(|a| EngineOutput::actions(vec![a]))
            .collect()
    }

    /// Pop the top layer, returning any `on_exit` action.
    ///
    /// If the stack is already at the base layer this is a no-op (returns
    /// empty) — stack underflow guard per spec rule 5.
    pub fn pop_layer(&mut self) -> Vec<EngineOutput> {
        let on_exit = self.layer_stack.pop();
        self.rebuild_overloads();
        self.clear_pending();
        on_exit
            .into_iter()
            .map(|a| EngineOutput::actions(vec![a]))
            .collect()
    }

    /// Replace the entire stack with a single layer.
    pub fn switch_layer(&mut self, profile: Profile) -> Vec<EngineOutput> {
        self.layer_stack.switch_to(profile);
        self.rebuild_overloads();
        self.clear_pending();
        vec![]
    }

    /// Flush any buffered pending events whose detection windows have expired,
    /// and check whether the top layer's timeout has elapsed.
    ///
    /// Must be called periodically (the Tauri event pump calls it every 50 ms).
    /// Without this, single taps buffered in `tap_pending` for double/triple-tap
    /// detection would never fire unless a subsequent tap arrived to flush them.
    pub fn check_timeout(&mut self, now: std::time::Instant) -> Vec<EngineOutput> {
        let mut outputs = Vec::new();

        // Flush combo-pending entries whose window expired between events.
        outputs.extend(self.flush_expired_combo(now));

        // Flush expired sequence progress.
        outputs.extend(self.flush_expired_sequence(now));

        // Flush tap-pending state whose double/triple-tap window has expired.
        outputs.extend(self.flush_expired_tap_pending(now));

        // Check whether the top layer has timed out (Timeout push-layer mode).
        if let Some(on_exit) = self.layer_stack.check_timeout(now) {
            self.rebuild_overloads();
            self.clear_pending();
            outputs.push(EngineOutput::actions(vec![on_exit]));
        }

        outputs
    }

    /// Enable or disable debug event emission.
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    /// Whether debug mode is currently enabled.
    pub fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// Layer IDs from top (active) to bottom (base).
    pub fn layer_ids(&self) -> Vec<String> {
        self.layer_stack.layer_ids()
    }

    /// All variable values on the currently active (top) layer.
    pub fn top_variables(&self) -> &std::collections::HashMap<String, crate::types::VariableValue> {
        self.layer_stack.top_variables()
    }

    /// Toggle a boolean variable on the top layer.
    ///
    /// Returns the new value, or `None` if the variable is absent or not a `Bool`.
    pub fn layer_stack_toggle_variable(
        &mut self,
        name: &str,
    ) -> Option<crate::types::VariableValue> {
        self.layer_stack.toggle_variable(name)
    }

    /// Set a variable on the top layer.
    pub fn layer_stack_set_variable(&mut self, name: &str, value: crate::types::VariableValue) {
        self.layer_stack.set_variable(name, value);
    }

    /// Look up an alias by name in the current top-layer profile.
    ///
    /// Returns a clone of the resolved action, or `None` if the alias is not defined.
    pub fn top_profile_alias(&self, name: &str) -> Option<crate::types::Action> {
        self.layer_stack.top().aliases.get(name).cloned()
    }

    /// Process a raw tap event and return zero or more engine outputs.
    ///
    /// May return nothing if the event is buffered waiting for a combo
    /// partner or a double/triple-tap. Pending events are flushed when a
    /// subsequent event arrives outside the relevant window.
    pub fn push_event(&mut self, event: RawTapEvent, now: Instant) -> Vec<EngineOutput> {
        let tap_code = match TapCode::from_u8(event.tap_code) {
            Some(c) => c,
            None => return vec![],
        };

        // Ignore all-open events (tap_code=0). The TAP hardware may emit a
        // zero-code notification when all fingers are released. Treating that
        // as a real tap event would place a phantom entry in combo_pending and
        // steal a partner slot from the next genuine tap.
        if tap_code.as_u8() == 0 {
            return vec![];
        }

        // Before flushing, detect combo timeouts for debug reporting.
        // The flush below will remove the expired entry, so we check here first.
        let mut outputs: Vec<EngineOutput> = Vec::new();
        if self.debug_mode && self.layer_stack.top().kind == crate::types::ProfileKind::Dual {
            let window = self.combo_window_ms();
            for partner in &self.combo_pending {
                if partner.device_id != event.device_id {
                    let gap_ms = event
                        .received_at
                        .saturating_duration_since(partner.received_at)
                        .as_millis() as u64;
                    if gap_ms > window {
                        let debug = DebugEvent::ComboTimeout {
                            first_pattern: partner.tap_code.to_single_pattern(Hand::Right),
                            first_device: partner.device_id.to_string(),
                            second_pattern: tap_code.to_single_pattern(Hand::Right),
                            second_device: event.device_id.to_string(),
                            combo_window_ms: window,
                            actual_gap_ms: gap_ms,
                        };
                        outputs.push(EngineOutput::with_debug(vec![], debug));
                    }
                }
            }
        }

        // Flush combo-pending events that have timed out.
        outputs.extend(self.flush_expired_combo(event.received_at));

        // Flush in-progress sequence if the step timeout has elapsed.
        outputs.extend(self.flush_expired_sequence(event.received_at));

        // Determine if this event forms a cross-device combo.
        if self.layer_stack.top().kind == crate::types::ProfileKind::Dual
            && !self.combo_pending.is_empty()
        {
            let partner_idx = self
                .combo_pending
                .iter()
                .position(|p| p.device_id != event.device_id);

            if let Some(idx) = partner_idx {
                let gap_ms = event
                    .received_at
                    .saturating_duration_since(self.combo_pending[idx].received_at)
                    .as_millis() as u64;
                let window = self.combo_window_ms();

                if gap_ms <= window {
                    // Combo match!
                    let partner = self.combo_pending.remove(idx);
                    let (left, right) = order_by_device(
                        partner,
                        PendingEntry {
                            device_id: event.device_id.clone(),
                            tap_code,
                            received_at: event.received_at,
                        },
                    );
                    let pattern = TriggerPattern::Dual {
                        left: left.tap_code,
                        right: right.tap_code,
                    };
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: left.device_id,
                        received_at: left.received_at,
                        kind: ResolvedTriggerKind::Tap,
                        waited_ms: gap_ms,
                        window_ms: window,
                    };
                    outputs.extend(self.dispatch(resolved, now));
                    return outputs;
                }
                // Outside window: entry was already flushed by flush_expired_combo above;
                // debug event was emitted before the flush if debug_mode is on.
            }
        }

        // Check whether this event advances or starts a sequence match.
        let entry = PendingEntry {
            device_id: event.device_id.clone(),
            tap_code,
            received_at: event.received_at,
        };
        let (seq_outputs, seq_consumed) = self.handle_sequence_step(entry, now);
        outputs.extend(seq_outputs);
        if seq_consumed {
            return outputs;
        }

        // Buffer or immediately resolve the event.
        let top = self.layer_stack.top();
        let hand = top.hand.unwrap_or(Hand::Right);
        let pattern_str = TriggerPattern::Single(tap_code).to_pattern_string(hand);
        let is_overloaded = self.overloads.is_overloaded(&pattern_str);
        let strategy = top.settings.overload_strategy;
        let is_dual = top.kind == crate::types::ProfileKind::Dual;
        // End the shared borrow on layer_stack before any mutable calls.
        let _ = top;

        match strategy {
            Some(OverloadStrategy::Patient) if is_overloaded => {
                // Patient strategy: buffer and wait.
                outputs.extend(self.handle_patient(
                    PendingEntry {
                        device_id: event.device_id,
                        tap_code,
                        received_at: event.received_at,
                    },
                    now,
                ));
            }
            _ => {
                // Dual profiles: buffer for combo window; single profiles with
                // non-overloaded codes or eager strategy: handle double/triple-tap
                // detection (or resolve immediately if no overloads).
                if is_dual {
                    self.combo_pending.push(PendingEntry {
                        device_id: event.device_id,
                        tap_code,
                        received_at: event.received_at,
                    });
                } else if is_overloaded {
                    // Eager strategy: fire immediately, record for potential undo.
                    outputs.extend(self.handle_eager(
                        PendingEntry {
                            device_id: event.device_id,
                            tap_code,
                            received_at: event.received_at,
                        },
                        now,
                    ));
                } else if self.needs_wait.contains(&tap_code) {
                    // Code has a multi-tap binding — must wait for the window.
                    outputs.extend(self.handle_tap(
                        PendingEntry {
                            device_id: event.device_id,
                            tap_code,
                            received_at: event.received_at,
                        },
                        now,
                    ));
                } else {
                    // No multi-tap binding anywhere in the stack — dispatch
                    // immediately without buffering.
                    //
                    // A new unrelated code confirms any pending tap (which
                    // must be a different code, since codes in needs_wait take
                    // the handle_tap path above). Flush it before dispatching
                    // so the ordering is correct: pending tap fires first,
                    // then the new code.
                    outputs.extend(self.flush_tap_pending_now(now));
                    let pattern = TriggerPattern::Single(tap_code);
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: event.device_id,
                        received_at: event.received_at,
                        kind: ResolvedTriggerKind::Tap,
                        waited_ms: 0,
                        window_ms: 0,
                    };
                    outputs.extend(self.dispatch(resolved, now));
                }
            }
        }

        outputs
    }

    /// Flush any combo-pending entries whose combo window has expired.
    ///
    /// For dual profiles, a timed-out entry from one device is dispatched as a
    /// `Dual` pattern with the partner side set to all-open (`TapCode(0)`), so
    /// that dual-only bindings can still match.
    fn flush_expired_combo(&mut self, now: Instant) -> Vec<EngineOutput> {
        let window = Duration::from_millis(self.combo_window_ms());
        let is_dual = self.layer_stack.top().kind == crate::types::ProfileKind::Dual;
        let zero = TapCode::from_u8(0).expect("0 is always valid");
        let mut expired: Vec<ResolvedEvent> = Vec::new();
        let mut remaining = Vec::new();
        for entry in self.combo_pending.drain(..) {
            let waited = now.saturating_duration_since(entry.received_at);
            if waited > window {
                let pattern = if is_dual {
                    if entry.device_id.as_str() == "left" {
                        TriggerPattern::Dual {
                            left: entry.tap_code,
                            right: zero,
                        }
                    } else {
                        TriggerPattern::Dual {
                            left: zero,
                            right: entry.tap_code,
                        }
                    }
                } else {
                    TriggerPattern::Single(entry.tap_code)
                };
                expired.push(ResolvedEvent {
                    pattern,
                    device_id: entry.device_id,
                    received_at: entry.received_at,
                    kind: ResolvedTriggerKind::Tap,
                    waited_ms: waited.as_millis() as u64,
                    window_ms: window.as_millis() as u64,
                });
            } else {
                remaining.push(entry);
            }
        }
        self.combo_pending = remaining;
        expired
            .into_iter()
            .flat_map(|r| self.dispatch(r, now))
            .collect()
    }

    /// Flush `tap_pending` state once the relevant detection window has expired.
    ///
    /// Called by [`check_timeout`] so that single taps fire promptly even when
    /// no follow-up event arrives to trigger the lazy flush in `push_event`.
    ///
    /// | State         | Fires when …                               |
    /// |---------------|--------------------------------------------|
    /// | `One { .. }`  | `double_tap_window_ms` elapsed since tap   |
    /// | `Two { .. }`  | `triple_tap_window_ms` elapsed since first |
    fn flush_expired_tap_pending(&mut self, now: Instant) -> Vec<EngineOutput> {
        let double_window = Duration::from_millis(self.double_tap_window_ms());
        let triple_window = Duration::from_millis(self.triple_tap_window_ms());

        match self.tap_pending.take() {
            None => vec![],
            Some(TapPending::One { entry }) => {
                let waited = now.saturating_duration_since(entry.received_at);
                if waited >= double_window {
                    let pattern = TriggerPattern::Single(entry.tap_code);
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: entry.device_id,
                        received_at: entry.received_at,
                        kind: ResolvedTriggerKind::Tap,
                        waited_ms: waited.as_millis() as u64,
                        window_ms: double_window.as_millis() as u64,
                    };
                    self.dispatch(resolved, now)
                } else {
                    // Window not yet expired — put the entry back.
                    self.tap_pending = Some(TapPending::One { entry });
                    vec![]
                }
            }
            Some(TapPending::Two { first, second }) => {
                let waited = now.saturating_duration_since(first.received_at);
                if waited >= triple_window {
                    let pattern = TriggerPattern::Single(first.tap_code);
                    let gap_ms = second
                        .received_at
                        .saturating_duration_since(first.received_at)
                        .as_millis() as u64;
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: first.device_id,
                        received_at: first.received_at,
                        kind: ResolvedTriggerKind::DoubleTap,
                        waited_ms: gap_ms,
                        window_ms: double_window.as_millis() as u64,
                    };
                    self.dispatch(resolved, now)
                } else {
                    self.tap_pending = Some(TapPending::Two { first, second });
                    vec![]
                }
            }
        }
    }

    /// Flush `tap_pending` immediately regardless of window expiry.
    ///
    /// Called when a code with no multi-tap binding arrives, confirming that
    /// any currently pending tap (which must be for a different code, since
    /// codes with multi-tap bindings take the [`handle_tap`] path) is a
    /// complete single or double tap.
    ///
    /// - `TapPending::One`  → fires as `Tap`.
    /// - `TapPending::Two`  → fires as `DoubleTap` (waited the inter-tap gap).
    /// - `None`             → no-op.
    fn flush_tap_pending_now(&mut self, now: Instant) -> Vec<EngineOutput> {
        let double_window = Duration::from_millis(self.double_tap_window_ms());
        match self.tap_pending.take() {
            None => vec![],
            Some(TapPending::One { entry }) => {
                let waited = now.saturating_duration_since(entry.received_at);
                let resolved = ResolvedEvent {
                    pattern: TriggerPattern::Single(entry.tap_code),
                    device_id: entry.device_id,
                    received_at: entry.received_at,
                    kind: ResolvedTriggerKind::Tap,
                    waited_ms: waited.as_millis() as u64,
                    window_ms: double_window.as_millis() as u64,
                };
                self.dispatch(resolved, now)
            }
            Some(TapPending::Two { first, second }) => {
                let gap_ms = second
                    .received_at
                    .saturating_duration_since(first.received_at)
                    .as_millis() as u64;
                let resolved = ResolvedEvent {
                    pattern: TriggerPattern::Single(first.tap_code),
                    device_id: first.device_id,
                    received_at: first.received_at,
                    kind: ResolvedTriggerKind::DoubleTap,
                    waited_ms: gap_ms,
                    window_ms: double_window.as_millis() as u64,
                };
                self.dispatch(resolved, now)
            }
        }
    }

    /// Handle a tap event under the `patient` overload strategy.
    fn handle_patient(&mut self, entry: PendingEntry, now: Instant) -> Vec<EngineOutput> {
        self.handle_tap(entry, now)
    }

    /// Handle a tap event under the `eager` overload strategy.
    fn handle_eager(&mut self, entry: PendingEntry, now: Instant) -> Vec<EngineOutput> {
        // Eager: fire the tap action immediately, then also start double-tap watch.
        let pattern = TriggerPattern::Single(entry.tap_code);
        let resolved = ResolvedEvent {
            pattern,
            device_id: entry.device_id.clone(),
            received_at: entry.received_at,
            kind: ResolvedTriggerKind::Tap,
            waited_ms: 0,
            window_ms: 0,
        };
        let outputs = self.dispatch(resolved, now);
        // Also buffer for potential double-tap undo (tracked via tap_pending One).
        self.tap_pending = Some(TapPending::One { entry });
        outputs
    }

    /// Handle double/triple-tap detection for an event.
    fn handle_tap(&mut self, entry: PendingEntry, now: Instant) -> Vec<EngineOutput> {
        let double_window = Duration::from_millis(self.double_tap_window_ms());
        let triple_window = Duration::from_millis(self.triple_tap_window_ms());

        match self.tap_pending.take() {
            None => {
                // First tap — buffer it.
                self.tap_pending = Some(TapPending::One { entry });
                vec![]
            }
            Some(TapPending::One { entry: first }) => {
                let gap = entry
                    .received_at
                    .saturating_duration_since(first.received_at);
                if entry.tap_code == first.tap_code && gap <= double_window {
                    // Second tap of same code within window — buffer for triple check.
                    self.tap_pending = Some(TapPending::Two {
                        first,
                        second: entry,
                    });
                    vec![]
                } else {
                    // Different code or outside window — flush first as single tap.
                    let pattern = TriggerPattern::Single(first.tap_code);
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: first.device_id,
                        received_at: first.received_at,
                        kind: ResolvedTriggerKind::Tap,
                        waited_ms: gap.as_millis() as u64,
                        window_ms: double_window.as_millis() as u64,
                    };
                    let outputs = self.dispatch(resolved, now);
                    // Re-buffer the new event.
                    self.tap_pending = Some(TapPending::One { entry });
                    outputs
                }
            }
            Some(TapPending::Two { first, second }) => {
                let gap = entry
                    .received_at
                    .saturating_duration_since(first.received_at);
                if entry.tap_code == first.tap_code && gap <= triple_window {
                    // Third tap — resolve as triple-tap.
                    let pattern = TriggerPattern::Single(first.tap_code);
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: first.device_id,
                        received_at: first.received_at,
                        kind: ResolvedTriggerKind::TripleTap,
                        waited_ms: gap.as_millis() as u64,
                        window_ms: triple_window.as_millis() as u64,
                    };
                    self.dispatch(resolved, now)
                } else {
                    // Different code or outside triple window — flush first two as double-tap.
                    let pattern = TriggerPattern::Single(first.tap_code);
                    let resolved = ResolvedEvent {
                        pattern,
                        device_id: first.device_id,
                        received_at: first.received_at,
                        kind: ResolvedTriggerKind::DoubleTap,
                        waited_ms: second
                            .received_at
                            .saturating_duration_since(first.received_at)
                            .as_millis() as u64,
                        window_ms: double_window.as_millis() as u64,
                    };
                    let outputs = self.dispatch(resolved, now);
                    // Re-buffer the new event.
                    self.tap_pending = Some(TapPending::One { entry });
                    outputs
                }
            }
        }
    }

    /// Flush the in-progress sequence if the step timeout has expired.
    ///
    /// Uses lazy evaluation: the timeout is detected the moment the *next* event
    /// arrives, matching the same pattern used for combo-window expiry.
    fn flush_expired_sequence(&mut self, now: Instant) -> Vec<EngineOutput> {
        let timed_out = self.seq_progress.as_ref().is_some_and(|p| {
            now.saturating_duration_since(p.last_step_at).as_millis() as u64 > p.window_ms
        });
        if timed_out {
            let progress = self.seq_progress.take().expect("timed_out implies Some");
            self.flush_sequence_as_singles(progress.buffered, now)
        } else {
            vec![]
        }
    }

    /// Attempt to advance or start a sequence match for `entry`.
    ///
    /// Returns `(outputs, consumed)`:
    /// - `consumed = true`  — the event was absorbed by sequence logic.
    /// - `consumed = false` — not a sequence event; caller should handle normally.
    fn handle_sequence_step(
        &mut self,
        entry: PendingEntry,
        now: Instant,
    ) -> (Vec<EngineOutput>, bool) {
        use crate::types::Trigger;

        if let Some(mut progress) = self.seq_progress.take() {
            // Clone what we need before any mutable borrow of `self`.
            let (total_steps, next_code) = {
                let mapping = &self.layer_stack.top().mappings[progress.mapping_idx];
                if let Trigger::Sequence { steps, .. } = &mapping.trigger {
                    let code = steps[progress.steps_matched].code;
                    (steps.len(), Some(code))
                } else {
                    (0, None)
                }
            };

            if next_code == Some(TriggerPattern::Single(entry.tap_code)) {
                progress.steps_matched += 1;
                progress.last_step_at = entry.received_at;
                progress.buffered.push(entry);

                if progress.steps_matched == total_steps {
                    let outputs =
                        self.dispatch_sequence(progress.mapping_idx, &progress.buffered, now);
                    return (outputs, true);
                }
                self.seq_progress = Some(progress);
                return (vec![], true);
            }

            // Mismatch — abort and flush buffered steps as individual singles.
            let flush = self.flush_sequence_as_singles(progress.buffered, now);
            return (flush, false);
        }

        // No active sequence — check whether this tap starts one.
        // Collect (idx, wms, steps_len) to avoid borrow-after-move.
        let found =
            self.layer_stack
                .top()
                .mappings
                .iter()
                .enumerate()
                .find_map(|(idx, mapping)| {
                    if !mapping.enabled {
                        return None;
                    }
                    if let Trigger::Sequence { steps, window_ms } = &mapping.trigger {
                        if !steps.is_empty()
                            && steps[0].code == TriggerPattern::Single(entry.tap_code)
                        {
                            let wms = window_ms.unwrap_or_else(|| self.sequence_window_ms());
                            return Some((idx, wms, steps.len()));
                        }
                    }
                    None
                });

        if let Some((idx, wms, steps_len)) = found {
            if steps_len == 1 {
                let outputs = self.dispatch_sequence(idx, &[entry], now);
                return (outputs, true);
            }
            self.seq_progress = Some(SequenceProgress {
                mapping_idx: idx,
                steps_matched: 1,
                window_ms: wms,
                last_step_at: entry.received_at,
                buffered: vec![entry],
            });
            return (vec![], true);
        }

        (vec![], false)
    }

    /// Dispatch all buffered sequence steps as individual single-tap actions.
    fn flush_sequence_as_singles(
        &mut self,
        buffered: Vec<PendingEntry>,
        now: Instant,
    ) -> Vec<EngineOutput> {
        buffered
            .into_iter()
            .flat_map(|entry| {
                let pattern = TriggerPattern::Single(entry.tap_code);
                let resolved = ResolvedEvent {
                    pattern,
                    device_id: entry.device_id,
                    received_at: entry.received_at,
                    kind: ResolvedTriggerKind::Tap,
                    waited_ms: 0,
                    window_ms: 0,
                };
                self.dispatch(resolved, now)
            })
            .collect()
    }

    /// Emit `EngineOutput` for a fully-matched sequence (mapping index known).
    fn dispatch_sequence(
        &mut self,
        mapping_idx: usize,
        buffered: &[PendingEntry],
        now: Instant,
    ) -> Vec<EngineOutput> {
        // Clone what we need before the mutable borrow in `execute_action`.
        let (raw_action, label, layer_id, hand, seq_window_ms) = {
            use crate::types::Trigger;
            let top = self.layer_stack.top();
            let mapping = &top.mappings[mapping_idx];
            let seq_window = match &mapping.trigger {
                Trigger::Sequence { window_ms, .. } => {
                    window_ms.unwrap_or_else(|| self.sequence_window_ms())
                }
                _ => self.sequence_window_ms(),
            };
            (
                mapping.action.clone(),
                mapping.label.clone(),
                top.layer_id.clone(),
                top.hand.unwrap_or(Hand::Right),
                seq_window,
            )
        };
        let action = Self::resolve_action_in(raw_action, self.layer_stack.top());
        let layer_stack_ids = self.layer_stack.layer_ids();

        let mut outputs = self.execute_action(action.clone());

        if self.debug_mode && !buffered.is_empty() {
            let first = &buffered[0];
            let debug = DebugEvent::Resolved {
                pattern: TriggerPattern::Single(first.tap_code).to_pattern_string(hand),
                device: first.device_id.to_string(),
                layer_stack: layer_stack_ids,
                matched_layer: layer_id,
                matched_mapping: label,
                action_fired: action,
                waited_ms: now.saturating_duration_since(first.received_at).as_millis() as u64,
                window_ms: seq_window_ms,
            };
            // Attach debug to first output or create a standalone one.
            if let Some(first_out) = outputs.first_mut() {
                first_out.debug = Some(debug);
            } else {
                outputs.push(EngineOutput::with_debug(vec![], debug));
            }
        }

        // Count decrement after sequence fires.
        if let Some(on_exit) = self.layer_stack.on_trigger_fired() {
            self.rebuild_overloads();
            self.clear_pending();
            outputs.push(EngineOutput::actions(vec![on_exit]));
        }

        outputs
    }

    /// Walk the layer stack from top to bottom and find a matching trigger.
    ///
    /// Implements the passthrough walk (spec §7):
    /// 1. Check the active (top) layer for a matching trigger.
    /// 2. If found, fire the action.
    /// 3. If not found and `passthrough: true`, check the next layer down.
    /// 4. If not found and `passthrough: false`, stop (event consumed silently).
    ///
    /// `Action::Block` stops the walk at that layer regardless of passthrough.
    fn dispatch(&mut self, resolved: ResolvedEvent, _now: Instant) -> Vec<EngineOutput> {
        use crate::types::Trigger;

        // Phase 1 — find the match (immutable walk).
        struct MatchResult {
            action: Action,
            matched_layer_id: String,
            matched_mapping_label: String,
        }

        // Capture the full stack before walking (for the Resolved debug event).
        let full_layer_ids = self.layer_stack.layer_ids();

        let mut layers_checked: Vec<String> = Vec::new();
        let found: Option<MatchResult> = {
            let mut result = None;

            'walk: for profile in self.layer_stack.walk() {
                layers_checked.push(profile.layer_id.clone());

                for mapping in &profile.mappings {
                    if !mapping.enabled {
                        continue;
                    }
                    let trigger_matches = match (&mapping.trigger, resolved.kind) {
                        (Trigger::Tap { code }, ResolvedTriggerKind::Tap) => {
                            *code == resolved.pattern
                        }
                        (Trigger::DoubleTap { code }, ResolvedTriggerKind::DoubleTap) => {
                            *code == resolved.pattern
                        }
                        (Trigger::TripleTap { code }, ResolvedTriggerKind::TripleTap) => {
                            *code == resolved.pattern
                        }
                        _ => false,
                    };

                    if trigger_matches {
                        let action = Self::resolve_action_in(mapping.action.clone(), profile);
                        result = Some(MatchResult {
                            action,
                            matched_layer_id: profile.layer_id.clone(),
                            matched_mapping_label: mapping.label.clone(),
                        });
                        break 'walk;
                    }
                }

                if !profile.passthrough {
                    break;
                }
            }

            result
        };

        // Phase 2 — execute the matched action.
        let Some(MatchResult {
            action,
            matched_layer_id,
            matched_mapping_label,
        }) = found
        else {
            // No match.
            if self.debug_mode {
                let hand = self.layer_stack.top().hand.unwrap_or(Hand::Right);
                let debug = DebugEvent::Unmatched {
                    pattern: resolved.pattern.to_pattern_string(hand),
                    device: resolved.device_id.to_string(),
                    passthrough_layers_checked: layers_checked,
                };
                return vec![EngineOutput::with_debug(vec![], debug)];
            }
            return vec![];
        };

        let mut outputs = self.execute_action(action.clone());

        if self.debug_mode {
            let hand = self.layer_stack.top().hand.unwrap_or(Hand::Right);
            let debug = DebugEvent::Resolved {
                pattern: resolved.pattern.to_pattern_string(hand),
                device: resolved.device_id.to_string(),
                layer_stack: full_layer_ids,
                matched_layer: matched_layer_id,
                matched_mapping: matched_mapping_label,
                action_fired: action,
                waited_ms: resolved.waited_ms,
                window_ms: resolved.window_ms,
            };
            if let Some(first_out) = outputs.first_mut() {
                first_out.debug = Some(debug);
            } else {
                outputs.push(EngineOutput::with_debug(vec![], debug));
            }
        }

        // Count decrement after any successful trigger (including block).
        if let Some(on_exit) = self.layer_stack.on_trigger_fired() {
            self.rebuild_overloads();
            self.clear_pending();
            outputs.push(EngineOutput::actions(vec![on_exit]));
        }

        outputs
    }

    /// Execute a resolved action, handling special cases inline.
    ///
    /// - `Block` — consumed silently; returns empty.
    /// - `PopLayer` — pops the top layer; returns `on_exit` actions.
    /// - `ToggleVariable` — mutates top-layer variable; returns child action.
    /// - `SetVariable` — mutates top-layer variable; returns empty.
    /// - Everything else — returned as-is for the caller to dispatch.
    fn execute_action(&mut self, action: Action) -> Vec<EngineOutput> {
        match action {
            Action::Block => vec![],
            Action::PopLayer => {
                let on_exit = self.layer_stack.pop();
                self.rebuild_overloads();
                self.clear_pending();
                on_exit
                    .into_iter()
                    .map(|a| EngineOutput::actions(vec![a]))
                    .collect()
            }
            Action::ToggleVariable {
                variable,
                on_true,
                on_false,
            } => {
                // Read CURRENT value, fire matching child, THEN flip.
                let current = self.layer_stack.get_variable(&variable).cloned();
                let child = match &current {
                    Some(VariableValue::Bool(true)) => *on_true,
                    _ => *on_false,
                };
                self.layer_stack.toggle_variable(&variable);
                self.execute_action(child)
            }
            Action::SetVariable { variable, value } => {
                self.layer_stack.set_variable(&variable, value);
                vec![]
            }
            other => vec![EngineOutput::actions(vec![other])],
        }
    }

    /// Resolve an `Action::Alias` one level deep via `profile`'s alias map.
    fn resolve_action_in(action: Action, profile: &Profile) -> Action {
        if let Action::Alias { name } = &action {
            if let Some(resolved) = profile.aliases.get(name) {
                return resolved.clone();
            }
        }
        action
    }

    // ── Housekeeping ───────────────────────────────────────────────────────

    /// Rebuild the overload table and needs-wait set from the current stack.
    fn rebuild_overloads(&mut self) {
        self.overloads = OverloadTable::build(self.layer_stack.top());
        self.needs_wait = build_needs_wait(&self.layer_stack);
    }

    /// Clear all pending event state (combo, double/triple-tap, sequence).
    fn clear_pending(&mut self) {
        self.combo_pending.clear();
        self.tap_pending = None;
        self.seq_progress = None;
    }

    // ── Timing helpers ─────────────────────────────────────────────────────

    fn combo_window_ms(&self) -> u64 {
        self.layer_stack
            .top()
            .settings
            .combo_window_ms
            .unwrap_or(DEFAULT_COMBO_WINDOW_MS)
    }

    fn double_tap_window_ms(&self) -> u64 {
        self.layer_stack
            .top()
            .settings
            .double_tap_window_ms
            .unwrap_or(DEFAULT_DOUBLE_TAP_WINDOW_MS)
    }

    fn triple_tap_window_ms(&self) -> u64 {
        self.layer_stack
            .top()
            .settings
            .triple_tap_window_ms
            .unwrap_or(DEFAULT_TRIPLE_TAP_WINDOW_MS)
    }

    fn sequence_window_ms(&self) -> u64 {
        self.layer_stack
            .top()
            .settings
            .sequence_window_ms
            .unwrap_or(DEFAULT_SEQUENCE_WINDOW_MS)
    }

    /// Return the earliest pending deadline across all buffered state, or
    /// `None` if no timeouts are currently active.
    ///
    /// The event pump uses this to sleep precisely until the next expiry
    /// instead of waking on a fixed polling interval, eliminating the up-to-
    /// 50 ms extra latency that the previous fixed-tick approach introduced.
    pub fn next_deadline(&self) -> Option<Instant> {
        let double_ms = self.double_tap_window_ms();
        let triple_ms = self.triple_tap_window_ms();
        let combo_ms = self.combo_window_ms();

        let mut earliest: Option<Instant> = None;

        // Helper: update `earliest` with `t` if `t` is sooner.
        macro_rules! bump {
            ($t:expr) => {
                let t = $t;
                earliest = Some(match earliest {
                    None => t,
                    Some(e) => e.min(t),
                });
            };
        }

        match &self.tap_pending {
            Some(TapPending::One { entry }) => {
                bump!(entry.received_at + Duration::from_millis(double_ms));
            }
            Some(TapPending::Two { first, .. }) => {
                bump!(first.received_at + Duration::from_millis(triple_ms));
            }
            None => {}
        }

        for entry in &self.combo_pending {
            bump!(entry.received_at + Duration::from_millis(combo_ms));
        }

        if let Some(seq) = &self.seq_progress {
            bump!(seq.last_step_at + Duration::from_millis(seq.window_ms));
        }

        if let Some(t) = self.layer_stack.next_timeout() {
            bump!(t);
        }

        earliest
    }
}

/// Assign left/right roles to a pair of pending entries based on `device_id`.
/// Entries whose device is `"left"` go to the left slot; others go right.
fn order_by_device(a: PendingEntry, b: PendingEntry) -> (PendingEntry, PendingEntry) {
    if a.device_id.as_str() == "left" {
        (a, b)
    } else {
        (b, a)
    }
}

/// Build the set of [`TapCode`] values that have a [`Trigger::DoubleTap`] or
/// [`Trigger::TripleTap`] binding in any layer of `layer_stack`.
///
/// Events for codes **not** in this set have no multi-tap binding reachable
/// via the passthrough walk and can be dispatched immediately.
fn build_needs_wait(layer_stack: &LayerStack) -> std::collections::HashSet<TapCode> {
    use crate::types::Trigger;
    let mut set = std::collections::HashSet::new();
    for profile in layer_stack.walk() {
        for mapping in &profile.mappings {
            if !mapping.enabled {
                continue;
            }
            match &mapping.trigger {
                Trigger::DoubleTap { code } | Trigger::TripleTap { code } => {
                    if let TriggerPattern::Single(tc) = code {
                        set.insert(*tc);
                    }
                }
                _ => {}
            }
        }
    }
    set
}
