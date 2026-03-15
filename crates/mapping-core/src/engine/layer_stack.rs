use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::types::{Action, Profile, PushLayerMode, VariableValue};

// ── Layer pop mode ─────────────────────────────────────────────────────────

/// The runtime auto-pop mode for a pushed layer.
#[derive(Debug, Clone)]
enum ActiveMode {
    /// Layer stays until explicitly popped.
    Permanent,
    /// Layer pops automatically after `remaining` trigger firings reach zero.
    Count { remaining: u32 },
    /// Layer pops automatically once `deadline` is reached.
    Timeout { deadline: Instant },
}

// ── Layer entry ────────────────────────────────────────────────────────────

/// A single entry on the [`LayerStack`], pairing a profile with its runtime
/// pop-mode and per-instance variable state.
#[derive(Debug)]
struct LayerEntry {
    /// The profile driving this layer's bindings.
    profile: Profile,
    /// How (if at all) this layer auto-pops.
    mode: ActiveMode,
    /// Variable state for this layer instance.
    ///
    /// Initialised from `profile.variables` when the layer is pushed; persists
    /// independently of the profile so that two pushes of the same profile have
    /// separate state.
    variables: HashMap<String, VariableValue>,
}

impl LayerEntry {
    fn new(profile: Profile, mode: ActiveMode) -> Self {
        let variables = profile.variables.clone();
        Self {
            profile,
            mode,
            variables,
        }
    }
}

// ── LayerStack ─────────────────────────────────────────────────────────────

/// An ordered stack of active [`Profile`] layers.
///
/// The last element is the top (active) layer; the first element is the base
/// layer. The stack always holds at least one layer — [`pop`] is a no-op when
/// only the base remains.
///
/// # Layer lifecycle
///
/// 1. [`new`] creates the stack with a base layer.
/// 2. [`push`] adds a layer on top; the pushed profile's `on_enter` action is
///    returned to the caller for dispatch.
/// 3. [`pop`] removes the top layer (unless it is the base) and returns the
///    profile's `on_exit` action.
/// 4. [`switch_to`] replaces the entire stack with a single new layer.
///
/// # Auto-pop modes (tasks 2.26–2.28)
///
/// - `Permanent` — no auto-pop; must be popped explicitly.
/// - `Count { count }` — pops after `count` trigger firings via
///   [`on_trigger_fired`].
/// - `Timeout { timeout_ms }` — pops when checked via [`check_timeout`] after
///   `timeout_ms` milliseconds. The caller is responsible for calling
///   `check_timeout` periodically (e.g. via a tokio interval in the Tauri
///   layer).
///
/// [`new`]: LayerStack::new
/// [`push`]: LayerStack::push
/// [`pop`]: LayerStack::pop
/// [`switch_to`]: LayerStack::switch_to
/// [`on_trigger_fired`]: LayerStack::on_trigger_fired
/// [`check_timeout`]: LayerStack::check_timeout
pub struct LayerStack {
    stack: Vec<LayerEntry>,
}

impl LayerStack {
    /// Create a new stack with `base` as the only (bottom) layer.
    pub fn new(base: Profile) -> Self {
        Self {
            stack: vec![LayerEntry::new(base, ActiveMode::Permanent)],
        }
    }

    /// Push a new layer, returning its `on_enter` action (if any).
    pub fn push(&mut self, profile: Profile, mode: PushLayerMode, now: Instant) -> Option<Action> {
        let on_enter = profile.on_enter.clone();
        let active_mode = match mode {
            PushLayerMode::Permanent => ActiveMode::Permanent,
            PushLayerMode::Count { count } => ActiveMode::Count { remaining: count },
            PushLayerMode::Timeout { timeout_ms } => ActiveMode::Timeout {
                deadline: now + Duration::from_millis(timeout_ms),
            },
        };
        self.stack.push(LayerEntry::new(profile, active_mode));
        on_enter
    }

    /// Pop the top layer, returning its `on_exit` action (if any).
    ///
    /// If only the base layer remains, this is a no-op and returns `None`
    /// (stack underflow guard, per spec rule 5).
    pub fn pop(&mut self) -> Option<Action> {
        if self.stack.len() <= 1 {
            return None;
        }
        let entry = self.stack.pop().expect("stack.len() > 1");
        entry.profile.on_exit
    }

    /// Replace the entire stack with a single new layer.
    ///
    /// All existing layers (including the base) are discarded without firing
    /// their `on_exit` actions. The new layer becomes the permanent base.
    pub fn switch_to(&mut self, profile: Profile) {
        self.stack.clear();
        self.stack
            .push(LayerEntry::new(profile, ActiveMode::Permanent));
    }

    /// The currently active (top) layer profile.
    pub fn top(&self) -> &Profile {
        &self.stack.last().expect("stack is never empty").profile
    }

    /// Number of layers currently on the stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// `true` if the stack contains no layers (never true in normal use).
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// `true` if only the base layer is present.
    pub fn is_at_base(&self) -> bool {
        self.stack.len() == 1
    }

    /// Signal that one trigger firing has occurred on the top layer.
    ///
    /// If the top layer is `Count`-mode and the remaining count reaches zero,
    /// it is popped automatically. Returns the `on_exit` action if popped.
    pub fn on_trigger_fired(&mut self) -> Option<Action> {
        let should_pop = if let Some(entry) = self.stack.last_mut() {
            if let ActiveMode::Count { remaining } = &mut entry.mode {
                *remaining = remaining.saturating_sub(1);
                *remaining == 0
            } else {
                false
            }
        } else {
            false
        };
        if should_pop { self.pop() } else { None }
    }

    /// Return the deadline of the top layer if it uses [`ActiveMode::Timeout`],
    /// or `None` if the layer has no auto-pop deadline.
    ///
    /// Used by [`ComboEngine::next_deadline`] so the event pump can sleep
    /// until the earliest pending expiry rather than polling on a fixed tick.
    pub fn next_timeout(&self) -> Option<Instant> {
        self.stack.last().and_then(|e| match e.mode {
            ActiveMode::Timeout { deadline } => Some(deadline),
            _ => None,
        })
    }

    /// Check whether the top layer's timeout has elapsed, popping it if so.
    ///
    /// Returns `on_exit` if the layer was popped, `None` otherwise.
    ///
    /// The caller is responsible for calling this periodically (e.g. via a
    /// tokio interval in the Tauri layer — see task 2.28).
    pub fn check_timeout(&mut self, now: Instant) -> Option<Action> {
        let timed_out = self
            .stack
            .last()
            .is_some_and(|e| matches!(e.mode, ActiveMode::Timeout { deadline } if now >= deadline));
        if timed_out { self.pop() } else { None }
    }

    /// Read a variable value from the top layer's variable state.
    pub fn get_variable(&self, name: &str) -> Option<&VariableValue> {
        self.stack.last()?.variables.get(name)
    }

    /// Set a variable on the top layer.
    pub fn set_variable(&mut self, name: &str, value: VariableValue) {
        if let Some(entry) = self.stack.last_mut() {
            entry.variables.insert(name.to_owned(), value);
        }
    }

    /// Toggle a boolean variable on the top layer.
    ///
    /// If the variable exists and is a `Bool`, flips it and returns the new
    /// value. Returns `None` if the variable is absent or is not a `Bool`.
    pub fn toggle_variable(&mut self, name: &str) -> Option<VariableValue> {
        let entry = self.stack.last_mut()?;
        if let Some(VariableValue::Bool(b)) = entry.variables.get_mut(name) {
            *b = !*b;
            Some(VariableValue::Bool(*b))
        } else {
            None
        }
    }

    /// Iterate layers from top (active) to bottom (base).
    pub fn walk(&self) -> impl Iterator<Item = &Profile> {
        self.stack.iter().rev().map(|e| &e.profile)
    }

    /// Layer ids from top to bottom (for debug events).
    pub fn layer_ids(&self) -> Vec<String> {
        self.stack
            .iter()
            .rev()
            .map(|e| e.profile.layer_id.clone())
            .collect()
    }

    /// All variable values on the top layer.
    ///
    /// Returns an empty map if the stack is somehow empty (should never occur
    /// in normal operation).
    pub fn top_variables(&self) -> &HashMap<String, VariableValue> {
        // SAFETY: stack always has at least one entry (invariant).
        static EMPTY: std::sync::OnceLock<HashMap<String, VariableValue>> =
            std::sync::OnceLock::new();
        self.stack
            .last()
            .map(|e| &e.variables)
            .unwrap_or_else(|| EMPTY.get_or_init(HashMap::new))
    }
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::types::{Hand, ProfileKind, ProfileSettings};

    fn make_profile(layer_id: &str) -> Profile {
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
            mappings: vec![],
        }
    }

    fn make_profile_with_enter_exit(
        layer_id: &str,
        on_enter: Option<Action>,
        on_exit: Option<Action>,
    ) -> Profile {
        Profile {
            on_enter,
            on_exit,
            ..make_profile(layer_id)
        }
    }

    fn key_action(key: &str) -> Action {
        use crate::types::KeyDef;
        Action::Key {
            key: KeyDef::new_unchecked(key),
            modifiers: vec![],
        }
    }

    #[test]
    fn layer_stack_new_has_one_layer() {
        let stack = LayerStack::new(make_profile("base"));
        assert_eq!(stack.len(), 1);
        assert!(stack.is_at_base());
    }

    #[test]
    fn layer_stack_top_returns_base_profile() {
        let stack = LayerStack::new(make_profile("base"));
        assert_eq!(stack.top().layer_id, "base");
    }

    #[test]
    fn layer_stack_push_increases_depth_and_changes_top() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(make_profile("nav"), PushLayerMode::Permanent, now);
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.top().layer_id, "nav");
        assert!(!stack.is_at_base());
    }

    #[test]
    fn layer_stack_push_returns_on_enter_action() {
        let mut stack = LayerStack::new(make_profile("base"));
        let profile = make_profile_with_enter_exit("nav", Some(key_action("f13")), None);
        let now = Instant::now();
        let on_enter = stack.push(profile, PushLayerMode::Permanent, now);
        assert_eq!(on_enter, Some(key_action("f13")));
    }

    #[test]
    fn layer_stack_pop_returns_on_exit_and_restores_previous() {
        let mut stack = LayerStack::new(make_profile("base"));
        let profile = make_profile_with_enter_exit("nav", None, Some(key_action("f14")));
        let now = Instant::now();
        stack.push(profile, PushLayerMode::Permanent, now);

        let on_exit = stack.pop();
        assert_eq!(on_exit, Some(key_action("f14")));
        assert_eq!(stack.top().layer_id, "base");
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn layer_stack_pop_at_base_is_noop() {
        let mut stack = LayerStack::new(make_profile("base"));
        let result = stack.pop();
        assert!(result.is_none(), "pop at base should return None");
        assert_eq!(stack.len(), 1, "stack size unchanged");
    }

    #[test]
    fn layer_stack_switch_to_replaces_all_layers() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(make_profile("nav"), PushLayerMode::Permanent, now);
        stack.push(make_profile("symbols"), PushLayerMode::Permanent, now);
        assert_eq!(stack.len(), 3);

        stack.switch_to(make_profile("gaming"));
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.top().layer_id, "gaming");
    }

    #[test]
    fn layer_stack_count_mode_pops_after_n_firings() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(
            make_profile_with_enter_exit("nav", None, Some(key_action("f14"))),
            PushLayerMode::Count { count: 3 },
            now,
        );
        assert_eq!(stack.len(), 2);

        // First two firings: no pop.
        assert!(stack.on_trigger_fired().is_none());
        assert!(stack.on_trigger_fired().is_none());
        assert_eq!(stack.len(), 2);

        // Third firing: layer pops.
        let on_exit = stack.on_trigger_fired();
        assert_eq!(on_exit, Some(key_action("f14")));
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn layer_stack_timeout_mode_pops_after_deadline() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(
            make_profile_with_enter_exit("nav", None, Some(key_action("f14"))),
            PushLayerMode::Timeout { timeout_ms: 1000 },
            now,
        );

        // Before deadline: no pop.
        let before = now + Duration::from_millis(500);
        assert!(stack.check_timeout(before).is_none());
        assert_eq!(stack.len(), 2);

        // After deadline: pop fires.
        let after = now + Duration::from_millis(1001);
        let on_exit = stack.check_timeout(after);
        assert_eq!(on_exit, Some(key_action("f14")));
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn layer_stack_permanent_mode_never_auto_pops() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(make_profile("nav"), PushLayerMode::Permanent, now);

        for _ in 0..100 {
            assert!(stack.on_trigger_fired().is_none());
        }
        let far_future = now + Duration::from_secs(10_000);
        assert!(stack.check_timeout(far_future).is_none());
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn layer_stack_variable_set_and_get() {
        let mut profile = make_profile("base");
        profile
            .variables
            .insert("muted".into(), VariableValue::Bool(false));
        let mut stack = LayerStack::new(profile);

        assert_eq!(
            stack.get_variable("muted"),
            Some(&VariableValue::Bool(false))
        );
        stack.set_variable("muted", VariableValue::Bool(true));
        assert_eq!(
            stack.get_variable("muted"),
            Some(&VariableValue::Bool(true))
        );
    }

    #[test]
    fn layer_stack_toggle_variable_flips_bool() {
        let mut profile = make_profile("base");
        profile
            .variables
            .insert("muted".into(), VariableValue::Bool(false));
        let mut stack = LayerStack::new(profile);

        let new_val = stack.toggle_variable("muted");
        assert_eq!(new_val, Some(VariableValue::Bool(true)));
        assert_eq!(
            stack.get_variable("muted"),
            Some(&VariableValue::Bool(true))
        );
    }

    #[test]
    fn layer_stack_variables_are_per_instance_not_shared() {
        let mut profile = make_profile("base");
        profile
            .variables
            .insert("muted".into(), VariableValue::Bool(false));
        let mut stack = LayerStack::new(profile.clone());
        let now = Instant::now();
        stack.push(profile, PushLayerMode::Permanent, now);

        // Top layer variable — set it.
        stack.set_variable("muted", VariableValue::Bool(true));
        // Pop the top layer.
        stack.pop();
        // Base layer should still have the original value.
        assert_eq!(
            stack.get_variable("muted"),
            Some(&VariableValue::Bool(false))
        );
    }

    #[test]
    fn layer_stack_walk_yields_top_to_bottom() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(make_profile("mid"), PushLayerMode::Permanent, now);
        stack.push(make_profile("top"), PushLayerMode::Permanent, now);

        let ids: Vec<&str> = stack.walk().map(|p| p.layer_id.as_str()).collect();
        assert_eq!(ids, vec!["top", "mid", "base"]);
    }

    #[test]
    fn layer_stack_layer_ids_top_to_bottom() {
        let mut stack = LayerStack::new(make_profile("base"));
        let now = Instant::now();
        stack.push(make_profile("nav"), PushLayerMode::Permanent, now);

        assert_eq!(stack.layer_ids(), vec!["nav", "base"]);
    }
}
