use crate::engine::DebugEvent;
use crate::types::Action;

/// The result returned by [`ComboEngine::push_event`] for each resolved
/// trigger.
///
/// A single call to `push_event` may produce zero, one, or more `EngineOutput`
/// values — zero when the event is buffered pending further input, one for a
/// single resolved tap, and multiple when a pending buffer is flushed as
/// individual taps.
///
/// [`ComboEngine::push_event`]: crate::engine::ComboEngine::push_event
#[derive(Debug, Clone)]
pub struct EngineOutput {
    /// The actions the platform layer should execute, in order.
    pub actions: Vec<Action>,
    /// Structured timing metadata emitted when debug mode is enabled.
    /// `None` when debug mode is off.
    pub debug: Option<DebugEvent>,
    /// `true` when this output represents (or accompanies) a layer-stack
    /// change that the platform layer must announce to the frontend.
    ///
    /// Set by the engine when `Action::PopLayer` fires from a mapping.
    /// `PushLayer` and `SwitchLayer` are handled entirely by the pump, which
    /// always calls `emit_layer_changed` directly — they do not use this flag.
    pub layer_changed: bool,
}

impl EngineOutput {
    /// Create an `EngineOutput` with actions and no debug event.
    pub fn actions(actions: Vec<Action>) -> Self {
        Self {
            actions,
            debug: None,
            layer_changed: false,
        }
    }

    /// Create an `EngineOutput` with actions and an attached debug event.
    pub fn with_debug(actions: Vec<Action>, debug: DebugEvent) -> Self {
        Self {
            actions,
            debug: Some(debug),
            layer_changed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::KeyDef;

    fn key_action() -> Action {
        Action::Key {
            key: KeyDef::new_unchecked("space"),
            modifiers: vec![],
        }
    }

    #[test]
    fn engine_output_actions_has_no_debug() {
        let out = EngineOutput::actions(vec![key_action()]);
        assert_eq!(out.actions.len(), 1);
        assert!(out.debug.is_none());
    }

    #[test]
    fn engine_output_empty_actions_is_valid() {
        let out = EngineOutput::actions(vec![]);
        assert!(out.actions.is_empty());
        assert!(out.debug.is_none());
    }

    #[test]
    fn engine_output_with_debug_attaches_event() {
        let debug = DebugEvent::Unmatched {
            pattern: "xoooo".into(),
            device: "solo".into(),
            passthrough_layers_checked: vec![],
        };
        let out = EngineOutput::with_debug(vec![key_action()], debug);
        assert!(out.debug.is_some());
    }
}
