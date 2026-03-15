use serde::{Deserialize, Serialize};

/// A keyboard modifier key held during a [`Key`](crate::types::Action::Key) action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    /// Control key.
    Ctrl,
    /// Shift key.
    Shift,
    /// Alt / Option key.
    Alt,
    /// Meta / Super / Windows / Command key.
    Meta,
}
