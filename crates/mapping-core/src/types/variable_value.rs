use serde::{Deserialize, Serialize};

/// The value of a profile variable.
///
/// Serialises and deserialises as a plain JSON boolean or integer (untagged),
/// matching the profile variable declaration format:
///
/// ```json
/// "variables": { "muted": false, "tap_count": 0 }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VariableValue {
    /// A boolean variable.
    Bool(bool),
    /// An integer variable.
    Int(i64),
}
