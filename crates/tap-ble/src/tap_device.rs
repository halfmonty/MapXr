use std::time::{Duration, Instant};

use btleplug::api::{BDAddr, Central as _, Characteristic, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Peripheral};
use futures::StreamExt as _;
use mapping_core::engine::{DeviceId, RawTapEvent};
use tokio::sync::{broadcast, watch};
use uuid::Uuid;

use mapping_core::types::VibrationPattern;

use crate::{BleError, manager::BleStatusEvent, packet_parser::parse_tap_packet};

// ── Protocol constants ────────────────────────────────────────────────────────

/// NUS RX characteristic — receives controller mode commands.
/// Source: `docs/reference/windows-sdk-guid-reference.txt`
const NUS_RX_UUID: Uuid = Uuid::from_u128(0x6E400002_B5A3_F393_E0A9_E50E24DCCA9E);

/// Tap proprietary device name characteristic (service C3FF0001).
/// Mirrors the GAP name; writing here is expected to update both.
/// Source: `docs/reference/gatt-characteristics.txt`
const CHAR_TAP_DEVICE_NAME: Uuid = Uuid::from_u128(0xC3FF0003_1D8B_40FD_A56F_C7BD5D0F3370);

/// Tap data characteristic — sends tap event notifications.
/// Source: `docs/reference/windows-sdk-guid-reference.txt`
const TAP_DATA_UUID: Uuid = Uuid::from_u128(0xC3FF0005_1D8B_40FD_A56F_C7BD5D0F3370);

/// Haptic characteristic — triggers vibration patterns on the device.
///
/// Properties: `WwR W`. WriteWithoutResponse is preferred for latency.
/// Source: `docs/reference/gatt-characteristics.txt` (`C3FF0009  Haptic  | WwR W`)
const HAPTIC_UUID: Uuid = Uuid::from_u128(0xC3FF0009_1D8B_40FD_A56F_C7BD5D0F3370);

/// Written to NUS RX to enter controller mode.
/// Source: `docs/reference/api-doc.txt`
const ENTER_CONTROLLER_MODE: &[u8] = &[0x03, 0x0C, 0x00, 0x01];

/// Written to NUS RX to exit controller mode.
/// Source: `docs/reference/api-doc.txt`
const EXIT_CONTROLLER_MODE: &[u8] = &[0x03, 0x0C, 0x00, 0x00];

/// Re-send interval for the controller mode keepalive (10 s per spec).
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// Starting delay for the reconnect backoff sequence.
const RECONNECT_BASE_DELAY: Duration = Duration::from_secs(1);

/// Maximum delay between reconnect attempts.
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(10);

// ── TapDevice ─────────────────────────────────────────────────────────────────

/// A connected Tap device.
///
/// Manages controller mode, the 10-second keepalive, tap data streaming, and
/// automatic reconnection on unexpected dropout.
///
/// Prefer calling [`TapDevice::disconnect`] before dropping to ensure the device
/// exits controller mode cleanly. The [`Drop`] impl provides a best-effort
/// fallback via an asynchronous detached task.
pub struct TapDevice {
    peripheral: Peripheral,
    address: BDAddr,
    device_id: DeviceId,
    nus_rx: Characteristic,
    // Retained for `connection_monitor_task` reconnect re-subscription and role reassignment.
    tap_data: Characteristic,
    /// Send `true` to stop all background tasks.
    cancel_tx: watch::Sender<bool>,
}

impl TapDevice {
    /// Connect to the Tap device at `address` on `adapter`, assign it `device_id`,
    /// and forward decoded tap events to `event_tx`.
    ///
    /// On success the device is in controller mode, tap data notifications are
    /// subscribed, and three background tasks are running:
    /// keepalive, notification reader, and connection monitor / reconnect.
    pub async fn connect(
        adapter: &Adapter,
        address: BDAddr,
        device_id: DeviceId,
        event_tx: broadcast::Sender<RawTapEvent>,
        status_tx: broadcast::Sender<BleStatusEvent>,
    ) -> Result<Self, BleError> {
        let peripheral = find_peripheral(adapter, address).await?;

        // connect() triggers OS bonding/pairing on first use.
        if !peripheral.is_connected().await? {
            peripheral
                .connect()
                .await
                .map_err(|e| BleError::ConnectionRefused {
                    address: address.to_string(),
                    reason: e.to_string(),
                })?;
        }

        peripheral.discover_services().await?;

        let (nus_rx, tap_data) = find_characteristics(&peripheral, address)?;

        // Subscribe to tap data notifications before entering controller mode.
        peripheral.subscribe(&tap_data).await?;

        // Enter controller mode.
        peripheral
            .write(&nus_rx, ENTER_CONTROLLER_MODE, WriteType::WithoutResponse)
            .await?;

        let (cancel_tx, cancel_rx) = watch::channel(false);

        tokio::spawn(keepalive_task(
            peripheral.clone(),
            nus_rx.clone(),
            cancel_rx.clone(),
        ));

        tokio::spawn(notification_task(
            peripheral.clone(),
            device_id.clone(),
            event_tx.clone(),
            cancel_rx.clone(),
        ));

        tokio::spawn(connection_monitor_task(
            peripheral.clone(),
            adapter.clone(),
            nus_rx.clone(),
            tap_data.clone(),
            device_id.clone(),
            event_tx,
            status_tx,
            cancel_rx,
        ));

        Ok(TapDevice {
            peripheral,
            address,
            device_id,
            nus_rx,
            tap_data,
            cancel_tx,
        })
    }

    /// Disconnect cleanly: stop all background tasks, exit controller mode, close BLE.
    ///
    /// If the connection has already dropped (e.g. device out of range), the exit
    /// packet write is attempted but errors are swallowed.
    pub async fn disconnect(&self) -> Result<(), BleError> {
        let _ = self.cancel_tx.send(true);

        if self.peripheral.is_connected().await.unwrap_or(false) {
            // Best-effort: if the write fails the device has likely already dropped.
            let _ = self
                .peripheral
                .write(
                    &self.nus_rx,
                    EXIT_CONTROLLER_MODE,
                    WriteType::WithoutResponse,
                )
                .await;

            self.peripheral.disconnect().await?;
        }

        Ok(())
    }

    /// Reassign this device to a new logical role without dropping the BLE connection.
    ///
    /// Cancels the existing background tasks (keepalive, notification, connection monitor)
    /// and immediately respawns them under `new_device_id`. The BLE connection, controller
    /// mode, and notification subscription are all preserved.
    ///
    /// An `ENTER_CONTROLLER_MODE` packet is written before spawning the new tasks to reset
    /// the device's 10-second keepalive timer, preventing a lapse in controller mode.
    pub async fn reassign(
        &mut self,
        new_device_id: DeviceId,
        adapter: &Adapter,
        event_tx: broadcast::Sender<RawTapEvent>,
        status_tx: broadcast::Sender<BleStatusEvent>,
    ) {
        // Stop the existing background tasks.
        let _ = self.cancel_tx.send(true);

        // Reset the keepalive timer on the device immediately.  Best-effort: if the write
        // fails the device is mid-reconnect and the new tasks will re-enter controller mode.
        let _ = self
            .peripheral
            .write(
                &self.nus_rx,
                ENTER_CONTROLLER_MODE,
                WriteType::WithoutResponse,
            )
            .await;

        let (cancel_tx, cancel_rx) = watch::channel(false);

        tokio::spawn(keepalive_task(
            self.peripheral.clone(),
            self.nus_rx.clone(),
            cancel_rx.clone(),
        ));

        tokio::spawn(notification_task(
            self.peripheral.clone(),
            new_device_id.clone(),
            event_tx.clone(),
            cancel_rx.clone(),
        ));

        tokio::spawn(connection_monitor_task(
            self.peripheral.clone(),
            adapter.clone(),
            self.nus_rx.clone(),
            self.tap_data.clone(),
            new_device_id.clone(),
            event_tx,
            status_tx,
            cancel_rx,
        ));

        self.device_id = new_device_id;
        self.cancel_tx = cancel_tx;
    }

    /// The logical role string assigned to this device.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// The BLE hardware address of this device.
    pub fn address(&self) -> BDAddr {
        self.address
    }

    /// The BLE peripheral's display name (from cached peripheral properties).
    ///
    /// Returns `None` if the adapter has no cached properties for this peripheral.
    pub async fn name(&self) -> Option<String> {
        use btleplug::api::Peripheral as _;
        self.peripheral
            .properties()
            .await
            .ok()
            .flatten()
            .and_then(|p| p.local_name)
    }

    /// Write a new friendly name to the device.
    ///
    /// The name is written to the Tap proprietary name characteristic (`C3FF0003`).
    /// The standard GAP Device Name characteristic (`00002A00`) is not written:
    /// despite advertising the WRITE property, the device rejects writes to it
    /// with "Operation Not Authorized". `C3FF0003` alone is sufficient — the
    /// device re-advertises the updated name on the next reconnect.
    ///
    /// The change takes effect after the device reconnects and re-advertises.
    ///
    /// The caller is responsible for validating the name before calling this
    /// method (length, allowed characters).
    pub async fn set_name(&self, name: &str) -> Result<(), BleError> {
        let tap_name_char = self
            .peripheral
            .characteristics()
            .iter()
            .find(|c| c.uuid == CHAR_TAP_DEVICE_NAME)
            .cloned()
            .ok_or_else(|| BleError::MissingCharacteristic {
                uuid: CHAR_TAP_DEVICE_NAME.to_string(),
                address: self.address.to_string(),
            })?;

        self.peripheral
            .write(&tap_name_char, name.as_bytes(), WriteType::WithResponse)
            .await?;

        Ok(())
    }

    /// Send a vibration pattern to the device.
    ///
    /// The pattern is encoded and written to the haptic characteristic (`C3FF0009`)
    /// using WriteWithoutResponse for minimum latency.
    ///
    /// An empty pattern is a no-op — the method returns `Ok(())` without writing.
    /// Sequences longer than 18 elements are truncated to 18 before sending.
    /// Duration values outside [10, 2550] ms are clamped silently.
    ///
    /// See `docs/spec/haptics-spec.md` for full encoding rules and built-in patterns.
    pub async fn vibrate(&self, pattern: &VibrationPattern) -> Result<(), BleError> {
        let payload = pattern.encode();
        if payload.is_empty() {
            return Ok(());
        }

        let haptic_char = self
            .peripheral
            .characteristics()
            .iter()
            .find(|c| c.uuid == HAPTIC_UUID)
            .cloned()
            .ok_or_else(|| BleError::MissingCharacteristic {
                uuid: HAPTIC_UUID.to_string(),
                address: self.address.to_string(),
            })?;

        self.peripheral
            .write(&haptic_char, &payload, WriteType::WithoutResponse)
            .await?;

        Ok(())
    }
}

impl Drop for TapDevice {
    fn drop(&mut self) {
        // Signal background tasks to wind down.
        let _ = self.cancel_tx.send(true);

        // NOTE: best-effort drop; prefer explicit `disconnect()` before dropping.
        // Spawns a detached async task to send the exit packet; only attempted
        // when a tokio runtime is available.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let peripheral = self.peripheral.clone();
            let nus_rx = self.nus_rx.clone();
            handle.spawn(async move {
                let _ = peripheral
                    .write(&nus_rx, EXIT_CONTROLLER_MODE, WriteType::WithoutResponse)
                    .await;
                let _ = peripheral.disconnect().await;
            });
        }
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Find the peripheral with `address` in the adapter's current peripheral list.
///
/// Returns [`BleError::DeviceNotFound`] if the device has not been seen in a recent scan.
async fn find_peripheral(adapter: &Adapter, address: BDAddr) -> Result<Peripheral, BleError> {
    adapter
        .peripherals()
        .await?
        .into_iter()
        .find(|p| p.address() == address)
        .ok_or_else(|| BleError::DeviceNotFound {
            address: address.to_string(),
        })
}

/// Locate the NUS RX and tap data characteristics on a connected peripheral.
///
/// Both characteristics must be present after [`discover_services`](Peripheral::discover_services);
/// returns [`BleError::MissingCharacteristic`] for whichever UUID is absent.
fn find_characteristics(
    peripheral: &Peripheral,
    address: BDAddr,
) -> Result<(Characteristic, Characteristic), BleError> {
    let chars = peripheral.characteristics();

    let nus_rx = chars
        .iter()
        .find(|c| c.uuid == NUS_RX_UUID)
        .cloned()
        .ok_or_else(|| BleError::MissingCharacteristic {
            uuid: NUS_RX_UUID.to_string(),
            address: address.to_string(),
        })?;

    let tap_data = chars
        .iter()
        .find(|c| c.uuid == TAP_DATA_UUID)
        .cloned()
        .ok_or_else(|| BleError::MissingCharacteristic {
            uuid: TAP_DATA_UUID.to_string(),
            address: address.to_string(),
        })?;

    Ok((nus_rx, tap_data))
}

// ── Background tasks ──────────────────────────────────────────────────────────

/// Resends the enter-controller-mode packet every [`KEEPALIVE_INTERVAL`].
///
/// Exits when `cancel` is set to `true`, the sender is dropped, or a write fails
/// (which typically means the connection has dropped).
async fn keepalive_task(
    peripheral: Peripheral,
    nus_rx: Characteristic,
    mut cancel: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            result = cancel.changed() => {
                if result.is_err() || *cancel.borrow() { break; }
            }
            _ = tokio::time::sleep(KEEPALIVE_INTERVAL) => {
                if peripheral
                    .write(&nus_rx, ENTER_CONTROLLER_MODE, WriteType::WithoutResponse)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

/// Reads tap data notifications and forwards them as [`RawTapEvent`]s.
///
/// Exits when `cancel` is set to `true`, the notification stream ends, or the
/// sender is dropped.
async fn notification_task(
    peripheral: Peripheral,
    device_id: DeviceId,
    event_tx: broadcast::Sender<RawTapEvent>,
    mut cancel: watch::Receiver<bool>,
) {
    let stream = match peripheral.notifications().await {
        Ok(s) => s,
        Err(_) => return,
    };
    tokio::pin!(stream);

    loop {
        tokio::select! {
            result = cancel.changed() => {
                if result.is_err() || *cancel.borrow() { break; }
            }
            item = stream.next() => {
                match item {
                    None => break, // stream ended (device disconnected)
                    Some(n) if n.uuid == TAP_DATA_UUID => {
                        if let Some(packet) = parse_tap_packet(&n.value) {
                            let event = RawTapEvent::new_at(
                                device_id.clone(),
                                packet.tap_code,
                                Instant::now(),
                            );
                            // SendError means no active receivers; drop silently.
                            let _ = event_tx.send(event);
                        }
                    }
                    Some(_) => {} // notification for a different characteristic; ignore
                }
            }
        }
    }
}

/// Subscribes to adapter-level events and triggers the reconnect loop on unexpected disconnect.
#[allow(clippy::too_many_arguments)]
async fn connection_monitor_task(
    peripheral: Peripheral,
    adapter: Adapter,
    nus_rx: Characteristic,
    tap_data: Characteristic,
    device_id: DeviceId,
    event_tx: broadcast::Sender<RawTapEvent>,
    status_tx: broadcast::Sender<BleStatusEvent>,
    mut cancel: watch::Receiver<bool>,
) {
    let events = match adapter.events().await {
        Ok(s) => s,
        Err(_) => return,
    };
    tokio::pin!(events);

    let peripheral_id = peripheral.id();
    let address = peripheral.address();

    loop {
        tokio::select! {
            result = cancel.changed() => {
                if result.is_err() || *cancel.borrow() { break; }
            }
            item = events.next() => {
                match item {
                    None => break, // adapter events stream ended
                    Some(btleplug::api::CentralEvent::DeviceDisconnected(id))
                        if id == peripheral_id =>
                    {
                        if *cancel.borrow() { break; }

                        // Notify subscribers of the unexpected disconnect.
                        use btleplug::api::Peripheral as _;
                        let name = peripheral
                            .properties()
                            .await
                            .ok()
                            .flatten()
                            .and_then(|p| p.local_name);
                        let _ = status_tx.send(BleStatusEvent::Disconnected {
                            device_id: device_id.clone(),
                            address,
                            name,
                        });

                        reconnect_loop(
                            &peripheral,
                            &nus_rx,
                            &tap_data,
                            &device_id,
                            &event_tx,
                            &status_tx,
                            &mut cancel,
                        )
                        .await;

                        if *cancel.borrow() { break; }
                    }
                    Some(_) => {}
                }
            }
        }
    }
}

/// Reconnects with exponential backoff: 1 s, 2 s, 4 s … capped at 60 s.
///
/// Retries indefinitely until the connection is re-established or `cancel` fires.
/// On success, restarts the keepalive and notification tasks.
async fn reconnect_loop(
    peripheral: &Peripheral,
    nus_rx: &Characteristic,
    tap_data: &Characteristic,
    device_id: &DeviceId,
    event_tx: &broadcast::Sender<RawTapEvent>,
    status_tx: &broadcast::Sender<BleStatusEvent>,
    cancel: &mut watch::Receiver<bool>,
) {
    let mut delay = RECONNECT_BASE_DELAY;
    let address = peripheral.address();

    loop {
        if *cancel.borrow() {
            return;
        }

        tokio::select! {
            result = cancel.changed() => {
                if result.is_err() || *cancel.borrow() { return; }
            }
            _ = tokio::time::sleep(delay) => {}
        }

        match peripheral.connect().await {
            Ok(()) => {
                // Re-subscribe and re-enter controller mode.
                let _ = peripheral.subscribe(tap_data).await;
                let _ = peripheral
                    .write(nus_rx, ENTER_CONTROLLER_MODE, WriteType::WithoutResponse)
                    .await;

                // Notify subscribers of the successful reconnect.
                use btleplug::api::Peripheral as _;
                let name = peripheral
                    .properties()
                    .await
                    .ok()
                    .flatten()
                    .and_then(|p| p.local_name);
                let _ = status_tx.send(BleStatusEvent::Connected {
                    device_id: device_id.clone(),
                    address,
                    name,
                });

                // Restart keepalive and notification tasks.
                tokio::spawn(keepalive_task(
                    peripheral.clone(),
                    nus_rx.clone(),
                    cancel.clone(),
                ));
                tokio::spawn(notification_task(
                    peripheral.clone(),
                    device_id.clone(),
                    event_tx.clone(),
                    cancel.clone(),
                ));

                return;
            }
            Err(_) => {
                delay = (delay * 2).min(RECONNECT_MAX_DELAY);
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // btleplug does not provide a mockable Peripheral trait, so `set_name` cannot
    // be called without a live BLE device and display server.  These tests cover
    // the pure byte-encoding contract that `set_name` relies on.
    //
    // Manual hardware verification steps (run once against a real device):
    //   1. Connect a TapXR or Tap Strap 2 in the app.
    //   2. Call `rename_device` via the UI with a name such as "MyTap".
    //   3. Confirm no error is shown.
    //   4. Disconnect and reconnect the device.
    //   5. Verify the new name appears in the device list on reconnect.

    #[test]
    fn set_name_ascii_encodes_to_utf8_bytes_without_framing() {
        let name = "MyTap";
        let bytes = name.as_bytes();
        assert_eq!(bytes, b"MyTap");
        assert_eq!(bytes.len(), 5);
        // No null terminator, no length prefix.
        assert!(!bytes.contains(&0x00));
    }

    #[test]
    fn set_name_tap_xr_default_name_round_trips() {
        let name = "TapXR_A036320";
        let bytes = name.as_bytes();
        assert_eq!(
            bytes,
            &[
                0x54, 0x61, 0x70, 0x58, 0x52, 0x5F, 0x41, 0x30, 0x33, 0x36, 0x33, 0x32, 0x30
            ]
        );
    }

    #[test]
    fn set_name_tap_strap2_default_name_round_trips() {
        let name = "Tap_D4252611";
        let bytes = name.as_bytes();
        assert_eq!(
            bytes,
            &[
                0x54, 0x61, 0x70, 0x5F, 0x44, 0x34, 0x32, 0x35, 0x32, 0x36, 0x31, 0x31
            ]
        );
    }

    #[test]
    fn char_tap_device_name_uuid_is_correct() {
        assert_eq!(
            CHAR_TAP_DEVICE_NAME.to_string(),
            "c3ff0003-1d8b-40fd-a56f-c7bd5d0f3370"
        );
    }

    #[test]
    fn haptic_uuid_is_correct() {
        assert_eq!(
            HAPTIC_UUID.to_string(),
            "c3ff0009-1d8b-40fd-a56f-c7bd5d0f3370"
        );
    }

    // Manual hardware verification steps for TapDevice::vibrate (run once against a real device):
    //   1. Connect a TapXR or Tap Strap 2 in the app.
    //   2. Bind a tap combination to a `vibrate` action with pattern [200, 100, 200].
    //   3. Tap the combination — the device should buzz twice with a 100 ms gap.
    //   4. Verify the pattern [80] produces a single short buzz (~80 ms).
    //   5. Verify an empty pattern produces no vibration.
}
