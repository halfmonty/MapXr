use std::collections::HashMap;

use btleplug::api::BDAddr;
use mapping_core::engine::{DeviceId, RawTapEvent};
use mapping_core::types::{Profile, ProfileKind};
use tokio::sync::broadcast;

use crate::{
    BleError, TapDeviceInfo,
    device_registry::DeviceRegistry,
    scanner::get_adapter,
    tap_device::TapDevice,
};

#[cfg(target_os = "linux")]
use crate::scanner::discover_devices_le;

#[cfg(not(target_os = "linux"))]
use crate::scanner::scan_with_adapter;

/// Capacity of the device status broadcast channel.
const STATUS_CHANNEL_CAPACITY: usize = 16;

/// Notification of a BLE connection state change.
///
/// Emitted by [`BleManager`] when a device connects or disconnects (including
/// reconnects triggered by the background reconnect loop in [`TapDevice`]).
#[derive(Debug, Clone)]
pub enum BleStatusEvent {
    /// A device successfully connected (or reconnected after dropout).
    Connected {
        device_id: DeviceId,
        address: BDAddr,
    },
    /// A device disconnected (either explicitly or unexpectedly).
    Disconnected {
        device_id: DeviceId,
        address: BDAddr,
    },
}

/// Capacity of the [`RawTapEvent`] broadcast channel.
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Top-level BLE coordinator.
///
/// Owns the BLE adapter, manages the scan-to-connect lifecycle, and distributes
/// [`RawTapEvent`]s to subscribers via a broadcast channel.
///
/// In the Tauri layer (Epic 4) this will be stored behind a `Mutex` inside
/// `tauri::State`. Methods use `&mut self` so the lock is the caller's concern.
pub struct BleManager {
    adapter: btleplug::platform::Adapter,
    event_tx: broadcast::Sender<RawTapEvent>,
    /// Connection state change notifications (connect / disconnect / reconnect).
    status_tx: broadcast::Sender<BleStatusEvent>,
    connected: HashMap<DeviceId, TapDevice>,
}

impl BleManager {
    /// Initialise the manager, acquiring the first available BLE adapter.
    ///
    /// Returns [`BleError::AdapterNotFound`] if no BLE hardware is present.
    pub async fn new() -> Result<Self, BleError> {
        let adapter = get_adapter().await?;
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (status_tx, _) = broadcast::channel(STATUS_CHANNEL_CAPACITY);
        Ok(Self {
            adapter,
            event_tx,
            status_tx,
            connected: HashMap::new(),
        })
    }

    /// Subscribe to the tap event stream.
    ///
    /// Returns a [`broadcast::Receiver`] that yields every [`RawTapEvent`] from
    /// all currently connected devices.
    pub fn subscribe(&self) -> broadcast::Receiver<RawTapEvent> {
        self.event_tx.subscribe()
    }

    /// Subscribe to BLE connection state change notifications.
    ///
    /// Returns a [`broadcast::Receiver`] that yields [`BleStatusEvent`]s when
    /// devices connect, disconnect, or reconnect.
    pub fn subscribe_status(&self) -> broadcast::Receiver<BleStatusEvent> {
        self.status_tx.subscribe()
    }

    /// Scan for nearby Tap devices for `timeout_ms` milliseconds.
    ///
    /// Results are sorted by RSSI descending. The adapter's peripheral cache is
    /// populated during the scan, making discovered addresses available for
    /// [`connect`](Self::connect).
    ///
    /// On Linux, uses `Transport::Le` via a separate `bluez-async` session to avoid
    /// MT7921 coexistence interference, while reusing `self.adapter` so that
    /// discovered peripherals are registered in the same btleplug session used by
    /// the subsequent `connect` call.
    pub async fn scan(&mut self, timeout_ms: u64) -> Result<Vec<TapDeviceInfo>, BleError> {
        #[cfg(target_os = "linux")]
        return discover_devices_le(timeout_ms).await;

        #[cfg(not(target_os = "linux"))]
        return scan_with_adapter(&self.adapter, timeout_ms).await;
    }

    /// Connect to the Tap device at `address` and assign it `device_id`.
    ///
    /// The device must have been seen in a recent [`scan`](Self::scan); returns
    /// [`BleError::DeviceNotFound`] otherwise.
    ///
    /// If `device_id` is already connected, the existing device is disconnected
    /// first.
    pub async fn connect(&mut self, device_id: DeviceId, address: BDAddr) -> Result<(), BleError> {
        // Disconnect any existing device with the same role.
        if let Some(existing) = self.connected.remove(&device_id) {
            let _ = existing.disconnect().await;
        }

        let device = TapDevice::connect(
            &self.adapter,
            address,
            device_id.clone(),
            self.event_tx.clone(),
            self.status_tx.clone(),
        )
        .await?;

        // Notify subscribers — ignore SendError (no active receivers).
        let _ = self.status_tx.send(BleStatusEvent::Connected {
            device_id: device_id.clone(),
            address,
        });

        self.connected.insert(device_id, device);
        Ok(())
    }

    /// Disconnect the device assigned to `device_id`.
    ///
    /// Returns `Ok(())` if no device was connected under that role.
    pub async fn disconnect(&mut self, device_id: &DeviceId) -> Result<(), BleError> {
        if let Some(device) = self.connected.remove(device_id) {
            let address = device.address();
            device.disconnect().await?;
            let _ = self.status_tx.send(BleStatusEvent::Disconnected {
                device_id: device_id.clone(),
                address,
            });
        }
        Ok(())
    }

    /// Returns the set of currently connected device roles.
    pub fn connected_ids(&self) -> impl Iterator<Item = &DeviceId> {
        self.connected.keys()
    }

    /// Returns `(device_id, address)` pairs for all currently connected devices.
    pub fn connected_devices(&self) -> impl Iterator<Item = (&DeviceId, btleplug::api::BDAddr)> {
        self.connected.iter().map(|(id, dev)| (id, dev.address()))
    }

    /// Reassign the device currently connected under `old_id` to `new_id`.
    ///
    /// The BLE connection is preserved; only the logical role changes. Background tasks
    /// (keepalive, notification, connection monitor) are restarted under `new_id` so that
    /// subsequent tap events and status notifications carry the correct role.
    ///
    /// Emits [`BleStatusEvent::Disconnected`] for `old_id` then [`BleStatusEvent::Connected`]
    /// for `new_id` so the Tauri event pump updates the frontend automatically.
    ///
    /// Returns [`BleError::DeviceNotFound`] if no device is connected under `old_id`.
    pub async fn reassign_role(
        &mut self,
        old_id: &DeviceId,
        new_id: DeviceId,
    ) -> Result<(), BleError> {
        if old_id == &new_id {
            return Ok(());
        }

        let mut device =
            self.connected
                .remove(old_id)
                .ok_or_else(|| BleError::DeviceNotFound {
                    address: old_id.to_string(),
                })?;

        let address = device.address();

        device
            .reassign(
                new_id.clone(),
                &self.adapter,
                self.event_tx.clone(),
                self.status_tx.clone(),
            )
            .await;

        self.connected.insert(new_id.clone(), device);

        let _ = self.status_tx.send(BleStatusEvent::Disconnected {
            device_id: old_id.clone(),
            address,
        });
        let _ = self.status_tx.send(BleStatusEvent::Connected {
            device_id: new_id,
            address,
        });

        Ok(())
    }

    /// Warn if `profile` requires two devices but `registry` has fewer than two entries.
    ///
    /// This is a diagnostic hint for the caller — it does not return an error.
    /// Log at `warn` level so the message surfaces in the app log without
    /// blocking startup.
    pub fn check_roles(profile: &Profile, registry: &DeviceRegistry) {
        if profile.kind == ProfileKind::Dual {
            let count = registry.len();
            if count < 2 {
                log::warn!(
                    "Profile '{}' is a Dual profile but only {} device(s) are registered in the \
                     device registry — connect and assign both a 'left' and 'right' device.",
                    profile.name,
                    count,
                );
            }
        }
    }
}
