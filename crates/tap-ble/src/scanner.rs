use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral as _};
use btleplug::platform::{Adapter, Manager};
use uuid::Uuid;

use crate::{BleError, TapDeviceInfo};

/// Tap proprietary service UUID — used as the BLE scan filter.
///
/// Source: `docs/reference/api-doc.txt`.
///
/// Note: `btleplug 0.12` does not re-export `uuid::Uuid`, so `uuid` is a direct dependency.
pub(crate) const TAP_SERVICE_UUID: Uuid = Uuid::from_u128(0xC3FF0001_1D8B_40FD_A56F_C7BD5D0F3370);

/// Acquire the first available BLE adapter.
///
/// Returns [`BleError::AdapterNotFound`] if the system has no BLE hardware or the
/// BLE subsystem fails to initialise.
pub(crate) async fn get_adapter() -> Result<Adapter, BleError> {
    let manager = Manager::new().await?;
    manager
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or(BleError::AdapterNotFound)
}

/// Scan for nearby Tap devices for `timeout_ms` milliseconds.
///
/// Only devices advertising the Tap proprietary service UUID
/// (`C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370`) are returned.
///
/// Results are sorted by RSSI descending (strongest signal first); devices with no
/// reported RSSI appear last.
///
/// On Linux, the scan explicitly sets `Transport::Le` to avoid coexistence issues on
/// adapters such as the MediaTek MT7921 that malfunction when BR/EDR and BLE are scanned
/// simultaneously. btleplug's own `start_scan` hardcodes `Transport::Auto`, so this
/// function bypasses it on Linux via a direct `bluez-async` session.
///
/// Prefer [`BleManager::scan`] when a `BleManager` is available — it reuses the
/// manager's existing adapter so that discovered peripherals are registered in the
/// same btleplug session used by the subsequent `connect` call.
pub async fn discover_devices(timeout_ms: u64) -> Result<Vec<TapDeviceInfo>, BleError> {
    #[cfg(target_os = "linux")]
    {
        let adapter = get_adapter().await?;
        return discover_devices_le(&adapter, timeout_ms).await;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let adapter = get_adapter().await?;
        scan_with_adapter(&adapter, timeout_ms).await
    }
}

/// Linux-specific scan path that uses `bluez-async` to set `Transport::Le` explicitly.
///
/// btleplug's `start_scan` hardcodes `Transport::Auto` in the `SetDiscoveryFilter` D-Bus
/// call, which causes BR/EDR coexistence interference on some adapters. Opening a separate
/// `bluez-async` session lets us call `SetDiscoveryFilter { Transport: "le" }` directly
/// before `StartDiscovery`. Both sessions talk to the same BlueZ daemon, so peripherals
/// discovered by the LE scan are visible through btleplug's `peripherals()` call on
/// `adapter`.
///
/// `adapter` must be the same btleplug adapter that will be passed to the subsequent
/// `connect` call. This ensures discovered peripherals are registered in the same
/// btleplug session, avoiding stale-session issues after the temporary `bluez-async`
/// session is dropped.
#[cfg(target_os = "linux")]
pub(crate) async fn discover_devices_le(
    adapter: &Adapter,
    timeout_ms: u64,
) -> Result<Vec<TapDeviceInfo>, BleError> {
    use bluez_async::{BluetoothSession, DiscoveryFilter, Transport};

    // BluetoothSession::new() internally spawns the D-Bus resource task via tokio::spawn;
    // the returned future handle does not need to be awaited or kept alive.
    let (_task, session) = BluetoothSession::new()
        .await
        .map_err(|e| BleError::Btleplug(btleplug::Error::RuntimeError(e.to_string())))?;

    let bluez_adapters = session
        .get_adapters()
        .await
        .map_err(|e| BleError::Btleplug(btleplug::Error::RuntimeError(e.to_string())))?;

    let bluez_adapter = bluez_adapters
        .into_iter()
        .next()
        .ok_or(BleError::AdapterNotFound)?;

    let filter = DiscoveryFilter {
        service_uuids: vec![TAP_SERVICE_UUID],
        transport: Some(Transport::Le),
        duplicate_data: Some(true),
        ..Default::default()
    };

    session
        .start_discovery_on_adapter_with_filter(&bluez_adapter.id, &filter)
        .await
        .map_err(|e| BleError::Btleplug(btleplug::Error::RuntimeError(e.to_string())))?;

    tokio::time::sleep(Duration::from_millis(timeout_ms)).await;

    // Ignore "No discovery started" on stop: BlueZ may have already cleaned up the
    // discovery session (e.g. the adapter suppressed scanning while a device was
    // connected). The sleep has elapsed and peripherals are already cached in BlueZ,
    // so a stop failure is harmless — the session drop will clean up automatically.
    if let Err(e) = session.stop_discovery_on_adapter(&bluez_adapter.id).await {
        log::warn!("stop_discovery_on_adapter: {e} (ignored)");
    }

    collect_peripherals(adapter).await
}

/// Inner scan logic, generic over any [`Central`] adapter.
///
/// Separated from `discover_devices` to allow unit testing with a mock adapter.
/// On Linux, `discover_devices` uses [`discover_devices_le`] instead of this function;
/// on other platforms it is called by `discover_devices` directly.
#[cfg(any(test, not(target_os = "linux")))]
pub(crate) async fn scan_with_adapter<C>(
    adapter: &C,
    timeout_ms: u64,
) -> Result<Vec<TapDeviceInfo>, BleError>
where
    C: Central,
{
    use btleplug::api::ScanFilter;

    adapter
        .start_scan(ScanFilter {
            services: vec![TAP_SERVICE_UUID],
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(timeout_ms)).await;

    adapter.stop_scan().await?;

    collect_peripherals(adapter).await
}

/// Query btleplug's peripheral list and return a sorted [`TapDeviceInfo`] vec.
///
/// btleplug maintains one Peripheral entry per hardware address (deduplicated by BlueZ);
/// properties reflect the most recent advertisement seen during the scan.
async fn collect_peripherals<C>(adapter: &C) -> Result<Vec<TapDeviceInfo>, BleError>
where
    C: Central,
{
    let peripherals = adapter.peripherals().await?;
    let mut devices: Vec<TapDeviceInfo> = Vec::with_capacity(peripherals.len());
    for peripheral in peripherals {
        let address = peripheral.address();
        let props = peripheral.properties().await?;
        let (name, rssi) = props.map_or((None, None), |p| (p.local_name, p.rssi));
        devices.push(TapDeviceInfo {
            name,
            address,
            rssi,
        });
    }

    // Sort: higher RSSI value (stronger signal) first; None last.
    devices.sort_by(|a, b| match (a.rssi, b.rssi) {
        (Some(ra), Some(rb)) => rb.cmp(&ra),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeSet;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use btleplug::api::{
        BDAddr, CentralEvent, CentralState, Characteristic, Descriptor, PeripheralProperties,
        ScanFilter, Service, ValueNotification, WriteType,
    };
    use btleplug::platform::PeripheralId;
    use futures::stream::Stream;

    // ── Mock peripheral ──────────────────────────────────────────────────────

    #[derive(Clone, Debug)]
    struct MockPeripheral {
        address: BDAddr,
        name: Option<String>,
        rssi: Option<i16>,
    }

    #[async_trait]
    impl btleplug::api::Peripheral for MockPeripheral {
        fn id(&self) -> PeripheralId {
            unimplemented!("MockPeripheral::id not used in scanner tests")
        }

        fn address(&self) -> BDAddr {
            self.address
        }

        fn mtu(&self) -> u16 {
            btleplug::api::DEFAULT_MTU_SIZE
        }

        async fn properties(&self) -> btleplug::Result<Option<PeripheralProperties>> {
            Ok(Some(PeripheralProperties {
                address: self.address,
                local_name: self.name.clone(),
                rssi: self.rssi,
                ..Default::default()
            }))
        }

        fn services(&self) -> BTreeSet<Service> {
            BTreeSet::new()
        }

        async fn is_connected(&self) -> btleplug::Result<bool> {
            Ok(false)
        }

        async fn connect(&self) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn disconnect(&self) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn discover_services(&self) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn write(
            &self,
            _characteristic: &Characteristic,
            _data: &[u8],
            _write_type: WriteType,
        ) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn read(&self, _characteristic: &Characteristic) -> btleplug::Result<Vec<u8>> {
            unimplemented!()
        }

        async fn subscribe(&self, _characteristic: &Characteristic) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn unsubscribe(&self, _characteristic: &Characteristic) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn notifications(
            &self,
        ) -> btleplug::Result<Pin<Box<dyn Stream<Item = ValueNotification> + Send>>> {
            unimplemented!()
        }

        async fn write_descriptor(
            &self,
            _descriptor: &Descriptor,
            _data: &[u8],
        ) -> btleplug::Result<()> {
            unimplemented!()
        }

        async fn read_descriptor(&self, _descriptor: &Descriptor) -> btleplug::Result<Vec<u8>> {
            unimplemented!()
        }
    }

    // ── Mock adapter ─────────────────────────────────────────────────────────

    #[derive(Clone)]
    struct MockAdapter {
        peripherals: Vec<MockPeripheral>,
        /// Captures the `ScanFilter` passed to `start_scan` for assertion.
        captured_filter: Arc<Mutex<Option<ScanFilter>>>,
    }

    #[async_trait]
    impl btleplug::api::Central for MockAdapter {
        type Peripheral = MockPeripheral;

        async fn events(
            &self,
        ) -> btleplug::Result<Pin<Box<dyn Stream<Item = CentralEvent> + Send>>> {
            unimplemented!()
        }

        async fn start_scan(&self, filter: ScanFilter) -> btleplug::Result<()> {
            *self.captured_filter.lock().expect("lock") = Some(filter);
            Ok(())
        }

        async fn stop_scan(&self) -> btleplug::Result<()> {
            Ok(())
        }

        async fn peripherals(&self) -> btleplug::Result<Vec<MockPeripheral>> {
            Ok(self.peripherals.clone())
        }

        async fn peripheral(&self, _id: &PeripheralId) -> btleplug::Result<MockPeripheral> {
            unimplemented!()
        }

        async fn add_peripheral(
            &self,
            _address: &PeripheralId,
        ) -> btleplug::Result<MockPeripheral> {
            unimplemented!()
        }

        async fn clear_peripherals(&self) -> btleplug::Result<()> {
            Ok(())
        }

        async fn adapter_info(&self) -> btleplug::Result<String> {
            Ok("MockAdapter".to_string())
        }

        async fn adapter_state(&self) -> btleplug::Result<CentralState> {
            Ok(CentralState::PoweredOn)
        }
    }

    // ── Helper ───────────────────────────────────────────────────────────────

    fn mock_peripheral(rssi: Option<i16>) -> MockPeripheral {
        MockPeripheral {
            address: BDAddr::default(),
            name: None,
            rssi,
        }
    }

    fn mock_adapter(peripherals: Vec<MockPeripheral>) -> MockAdapter {
        MockAdapter {
            peripherals,
            captured_filter: Arc::new(Mutex::new(None)),
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn tap_service_uuid_parses_to_correct_string() {
        assert_eq!(
            TAP_SERVICE_UUID.to_string(),
            "c3ff0001-1d8b-40fd-a56f-c7bd5d0f3370"
        );
    }

    #[tokio::test]
    async fn scan_with_adapter_start_scan_passes_tap_service_uuid() {
        let adapter = mock_adapter(vec![]);
        let captured: Arc<Mutex<Option<ScanFilter>>> = Arc::clone(&adapter.captured_filter);

        scan_with_adapter(&adapter, 0).await.expect("scan failed");

        let filter = captured
            .lock()
            .expect("lock")
            .take()
            .expect("start_scan not called");
        assert_eq!(filter.services, vec![TAP_SERVICE_UUID]);
    }

    #[tokio::test]
    async fn scan_with_adapter_multiple_devices_sorts_by_rssi_strongest_first() {
        let adapter = mock_adapter(vec![
            mock_peripheral(Some(-70)),
            mock_peripheral(Some(-50)),
            mock_peripheral(Some(-90)),
        ]);

        let devices = scan_with_adapter(&adapter, 0).await.expect("scan failed");

        assert_eq!(devices[0].rssi, Some(-50));
        assert_eq!(devices[1].rssi, Some(-70));
        assert_eq!(devices[2].rssi, Some(-90));
    }

    #[tokio::test]
    async fn scan_with_adapter_device_with_no_rssi_appears_last() {
        let adapter = mock_adapter(vec![
            mock_peripheral(None),
            mock_peripheral(Some(-60)),
            mock_peripheral(None),
        ]);

        let devices = scan_with_adapter(&adapter, 0).await.expect("scan failed");

        assert_eq!(devices[0].rssi, Some(-60));
        assert!(devices[1].rssi.is_none());
        assert!(devices[2].rssi.is_none());
    }
}
