/// A stable user-assigned role string that identifies a Tap device in the
/// engine. Conventional values are `"solo"` (single-device setup), `"left"`,
/// and `"right"` (dual-device setup). The engine does not enforce the naming
/// convention — any non-empty string is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    /// Create a `DeviceId` from any string value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Return the role string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for DeviceId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for DeviceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_new_stores_string() {
        let id = DeviceId::new("left");
        assert_eq!(id.as_str(), "left");
    }

    #[test]
    fn device_id_display_returns_inner_string() {
        let id = DeviceId::new("right");
        assert_eq!(id.to_string(), "right");
    }

    #[test]
    fn device_id_from_str_creates_id() {
        let id: DeviceId = "solo".into();
        assert_eq!(id.as_str(), "solo");
    }

    #[test]
    fn device_id_equality_matches_same_string() {
        assert_eq!(DeviceId::new("left"), DeviceId::new("left"));
    }

    #[test]
    fn device_id_equality_differs_on_different_strings() {
        assert_ne!(DeviceId::new("left"), DeviceId::new("right"));
    }
}
