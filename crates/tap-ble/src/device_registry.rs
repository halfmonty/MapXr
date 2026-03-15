use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr as _;

use btleplug::api::BDAddr;
use mapping_core::engine::DeviceId;
use serde::{Deserialize, Serialize};

use crate::BleError;

// ── On-disk format ────────────────────────────────────────────────────────────

/// JSON representation of the registry.
///
/// `BDAddr` is stored as an uppercase colon-delimited hex string
/// (e.g. `"AA:BB:CC:DD:EE:FF"`) for human-readability and interoperability.
#[derive(Serialize, Deserialize)]
struct StoredRegistry {
    version: u32,
    devices: HashMap<String, String>,
}

// ── DeviceRegistry ────────────────────────────────────────────────────────────

/// Persists the mapping from logical device roles to BLE hardware addresses across sessions.
///
/// Stored as `devices.json` in the app config directory.  The path is passed in
/// from `src-tauri` and is not hard-coded here.
///
/// ```json
/// {
///   "version": 1,
///   "devices": {
///     "solo":  "AA:BB:CC:DD:EE:FF",
///     "left":  "11:22:33:44:55:66",
///     "right": "77:88:99:AA:BB:CC"
///   }
/// }
/// ```
#[derive(Debug, Default)]
pub struct DeviceRegistry {
    entries: HashMap<DeviceId, BDAddr>,
}

impl DeviceRegistry {
    /// Load the registry from `path`.
    ///
    /// Returns an empty registry if the file does not exist.  Any other I/O
    /// error or a JSON parse failure is propagated as a [`BleError`].
    pub fn load(path: &Path) -> Result<Self, BleError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(BleError::Io(e)),
        };

        let stored: StoredRegistry = serde_json::from_str(&text)?;

        let entries = stored
            .devices
            .into_iter()
            .map(|(role, addr_str)| {
                let addr = BDAddr::from_str(&addr_str).map_err(|_| {
                    // Wrap the parse failure as an IO error
                    BleError::Io(std::io::Error::other(format!(
                        "invalid BDAddr string: {addr_str}"
                    )))
                })?;
                Ok((DeviceId::new(role), addr))
            })
            .collect::<Result<HashMap<_, _>, BleError>>()?;

        Ok(Self { entries })
    }

    /// Persist the registry to `path`.
    ///
    /// Uses a write-then-rename strategy for atomicity: the data is first
    /// written to a sibling `.json.tmp` file, then renamed over the target.
    /// If the target's parent directory does not exist the operation fails with
    /// [`BleError::Io`].
    pub fn save(&self, path: &Path) -> Result<(), BleError> {
        let stored = StoredRegistry {
            version: 1,
            devices: self
                .entries
                .iter()
                .map(|(id, addr)| (id.to_string(), addr.to_string()))
                .collect(),
        };

        let json = serde_json::to_string_pretty(&stored)?;

        // Write to a sibling temp file, then rename atomically so a crash
        // mid-write never leaves a partial `devices.json`.
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, path)?;

        Ok(())
    }

    /// Assign `address` to `device_id`.
    ///
    /// Any existing entry — for `device_id` **or** for any other role that
    /// was previously mapped to the same physical address — is removed first.
    /// This prevents one physical device from appearing under multiple roles
    /// (e.g. both "solo" and "right") after the user changes its assignment.
    pub fn assign(&mut self, device_id: DeviceId, address: BDAddr) {
        self.entries.retain(|_, a| *a != address);
        self.entries.insert(device_id, address);
    }

    /// Return the address registered for `device_id`, if any.
    pub fn address_for(&self, device_id: &DeviceId) -> Option<BDAddr> {
        self.entries.get(device_id).copied()
    }

    /// Remove the entry for `device_id`.
    pub fn remove(&mut self, device_id: &DeviceId) {
        self.entries.remove(device_id);
    }

    /// Iterate over all registered (device_id, address) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&DeviceId, &BDAddr)> {
        self.entries.iter()
    }

    /// Number of registered devices.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if no devices are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;

    fn addr(s: &str) -> BDAddr {
        BDAddr::from_str(s).expect("valid BDAddr in test")
    }

    // ── assign / address_for / remove ─────────────────────────────────────────

    #[test]
    fn device_registry_assign_stores_address() {
        let mut reg = DeviceRegistry::default();
        reg.assign(DeviceId::new("solo"), addr("AA:BB:CC:DD:EE:FF"));
        assert_eq!(
            reg.address_for(&DeviceId::new("solo")),
            Some(addr("AA:BB:CC:DD:EE:FF"))
        );
    }

    #[test]
    fn device_registry_assign_replaces_existing_entry() {
        let mut reg = DeviceRegistry::default();
        reg.assign(DeviceId::new("left"), addr("11:22:33:44:55:66"));
        reg.assign(DeviceId::new("left"), addr("AA:BB:CC:DD:EE:FF"));
        assert_eq!(
            reg.address_for(&DeviceId::new("left")),
            Some(addr("AA:BB:CC:DD:EE:FF"))
        );
    }

    #[test]
    fn device_registry_assign_removes_stale_role_for_same_address() {
        // Simulates: device was "right", user reassigns it as "solo".
        // The old "right" entry must be gone so auto-reconnect doesn't
        // connect the same physical device under both roles.
        let mut reg = DeviceRegistry::default();
        reg.assign(DeviceId::new("right"), addr("AA:BB:CC:DD:EE:FF"));
        reg.assign(DeviceId::new("solo"), addr("AA:BB:CC:DD:EE:FF"));
        assert_eq!(reg.len(), 1);
        assert_eq!(
            reg.address_for(&DeviceId::new("solo")),
            Some(addr("AA:BB:CC:DD:EE:FF"))
        );
        assert_eq!(reg.address_for(&DeviceId::new("right")), None);
    }

    #[test]
    fn device_registry_address_for_missing_returns_none() {
        let reg = DeviceRegistry::default();
        assert_eq!(reg.address_for(&DeviceId::new("right")), None);
    }

    #[test]
    fn device_registry_remove_deletes_entry() {
        let mut reg = DeviceRegistry::default();
        reg.assign(DeviceId::new("solo"), addr("AA:BB:CC:DD:EE:FF"));
        reg.remove(&DeviceId::new("solo"));
        assert_eq!(reg.address_for(&DeviceId::new("solo")), None);
    }

    #[test]
    fn device_registry_remove_nonexistent_does_not_panic() {
        let mut reg = DeviceRegistry::default();
        reg.remove(&DeviceId::new("solo")); // should not panic
    }

    #[test]
    fn device_registry_len_reflects_entry_count() {
        let mut reg = DeviceRegistry::default();
        assert_eq!(reg.len(), 0);
        reg.assign(DeviceId::new("left"), addr("11:22:33:44:55:66"));
        assert_eq!(reg.len(), 1);
        reg.assign(DeviceId::new("right"), addr("77:88:99:AA:BB:CC"));
        assert_eq!(reg.len(), 2);
    }

    // ── load / save round-trip ────────────────────────────────────────────────

    #[test]
    fn device_registry_save_and_load_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("devices.json");

        let mut reg = DeviceRegistry::default();
        reg.assign(DeviceId::new("solo"), addr("AA:BB:CC:DD:EE:FF"));
        reg.assign(DeviceId::new("left"), addr("11:22:33:44:55:66"));
        reg.save(&path).expect("save");

        let loaded = DeviceRegistry::load(&path).expect("load");
        assert_eq!(
            loaded.address_for(&DeviceId::new("solo")),
            Some(addr("AA:BB:CC:DD:EE:FF"))
        );
        assert_eq!(
            loaded.address_for(&DeviceId::new("left")),
            Some(addr("11:22:33:44:55:66"))
        );
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn device_registry_load_missing_file_returns_empty_registry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nonexistent.json");

        let reg = DeviceRegistry::load(&path).expect("load of missing file should succeed");
        assert!(reg.is_empty());
    }

    #[test]
    fn device_registry_load_malformed_json_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("devices.json");
        std::fs::write(&path, b"not valid json").expect("write fixture");

        assert!(DeviceRegistry::load(&path).is_err());
    }

    #[test]
    fn device_registry_save_produces_valid_json_with_version_1() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("devices.json");

        let mut reg = DeviceRegistry::default();
        reg.assign(DeviceId::new("solo"), addr("AA:BB:CC:DD:EE:FF"));
        reg.save(&path).expect("save");

        let text = std::fs::read_to_string(&path).expect("read saved file");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["version"], 1);
        assert_eq!(value["devices"]["solo"], "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn device_registry_load_empty_devices_object_returns_empty_registry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("devices.json");
        std::fs::write(&path, r#"{"version":1,"devices":{}}"#).expect("write fixture");

        let reg = DeviceRegistry::load(&path).expect("load");
        assert!(reg.is_empty());
    }
}
