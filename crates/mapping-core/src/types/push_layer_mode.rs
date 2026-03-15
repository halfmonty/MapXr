use serde::{Deserialize, Serialize};

/// Controls how long a pushed layer stays on the stack.
///
/// Used in [`Action::PushLayer`](crate::types::Action::PushLayer). Serialised
/// as an internally-tagged object with a `"mode"` field, flattened into the
/// parent `push_layer` action JSON object:
///
/// ```json
/// { "type": "push_layer", "layer": "nav", "mode": "permanent" }
/// { "type": "push_layer", "layer": "nav", "mode": "count",   "count": 3        }
/// { "type": "push_layer", "layer": "nav", "mode": "timeout", "timeout_ms": 2000 }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PushLayerMode {
    /// Layer stays until explicitly popped with `pop_layer`.
    Permanent,
    /// Layer pops automatically after `count` resolved trigger firings.
    Count {
        /// Number of trigger firings before the layer auto-pops.
        count: u32,
    },
    /// Layer pops automatically after `timeout_ms` milliseconds.
    Timeout {
        /// Milliseconds before the layer auto-pops.
        timeout_ms: u64,
    },
}
