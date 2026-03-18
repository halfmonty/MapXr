// Physical device integration tests.
//
// These tests require a real Tap Strap 2 or TapXR device to be in range and powered on.
// Run with:
//
//   cargo test -p tap-ble -- --ignored --test-threads=1
//
// IMPORTANT: use --test-threads=1. Several tests connect to the same physical device;
// running them in parallel causes BlueZ to return "In Progress" errors.
//
// They are marked `#[ignore]` so they are skipped in normal CI runs.
// Each test should document which device (TapXR / Tap Strap 2) it was validated against.

/// Connect to the first discovered device, enter controller mode, then disconnect.
///
/// Validates tasks 3.7–3.15 end-to-end against real hardware. Checks that the
/// device is connectable and that characteristic discovery finds both the NUS RX
/// and tap data UUIDs.
#[tokio::test]
#[ignore = "requires physical Tap device"]
async fn connect_and_disconnect_cleanly() {
    let mut manager = tap_ble::BleManager::new().await.expect("no BLE adapter");
    let devices = manager.scan(5000).await.expect("scan failed");
    assert!(!devices.is_empty(), "no Tap devices found");

    let first = &devices[0];
    manager
        .connect(mapping_core::engine::DeviceId::new("solo"), first.address)
        .await
        .expect("connect failed");

    // Give controller mode a moment to settle, then disconnect cleanly.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    manager
        .disconnect(&mapping_core::engine::DeviceId::new("solo"))
        .await
        .expect("disconnect failed");
}

/// Smoke test: asserts that at least one Tap device is discoverable within 5 seconds.
///
/// On Linux this exercises the LE-only scan path (Transport::Le via bluez-async) which
/// fixes MT7921 coexistence interference. On other platforms it uses btleplug's default
/// scan.
///
/// Requires a powered-on Tap Strap 2 or TapXR within BLE range.
#[tokio::test]
#[ignore = "requires physical Tap device"]
async fn discover_devices_finds_at_least_one_tap_device() {
    let devices = tap_ble::discover_devices(5000).await.expect("scan failed");
    assert!(!devices.is_empty(), "no Tap devices found within 5 s");
}

/// Connects to the first discovered Tap device, discovers all GATT services and
/// characteristics, reads every readable characteristic, and prints the results.
///
/// Run with:
///   cargo test -p tap-ble -- --ignored probe_gatt --nocapture --test-threads=1
///
/// Purpose: enumerate undocumented characteristics and confirm the device name
/// characteristic (C3FF0003) is readable/writable.
#[tokio::test]
#[ignore = "requires physical Tap device; diagnostic output only"]
async fn probe_gatt_characteristics() {
    use btleplug::api::{Central as _, Peripheral as _};

    let adapter = tap_ble::scanner::get_adapter().await.expect("no BLE adapter");

    // Scan using existing discover_devices to get an address.
    let devices = tap_ble::discover_devices(5000).await.expect("scan failed");
    assert!(!devices.is_empty(), "no Tap devices found within 5 s");

    let target_address = devices[0].address;
    println!(
        "\nProbing device: {} (name: {:?}, rssi: {:?})",
        target_address, devices[0].name, devices[0].rssi
    );

    // Find the btleplug peripheral by address in the adapter's peripheral list.
    let peripheral = adapter
        .peripherals()
        .await
        .expect("peripherals() failed")
        .into_iter()
        .find(|p| p.address() == target_address)
        .expect("peripheral not found in adapter cache after scan");

    peripheral.connect().await.expect("connect failed");
    peripheral.discover_services().await.expect("discover_services failed");

    println!("\n=== GATT Services & Characteristics ===\n");
    for service in peripheral.services() {
        println!("Service: {}", service.uuid);
        for characteristic in &service.characteristics {
            println!(
                "  Characteristic: {}  props: {:?}",
                characteristic.uuid, characteristic.properties
            );

            // Attempt to read if the Read property is present.
            if characteristic.properties.contains(btleplug::api::CharPropFlags::READ) {
                match peripheral.read(characteristic).await {
                    Ok(bytes) => {
                        let hex: String = bytes
                            .iter()
                            .map(|b| format!("{b:02X}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let text = String::from_utf8_lossy(&bytes);
                        println!("    value (hex): {hex}");
                        println!("    value (utf8): {text}");
                    }
                    Err(e) => println!("    read error: {e}"),
                }
            }
        }
    }

    peripheral.disconnect().await.expect("disconnect failed");
}

/// Asserts that a device discovered via the LE scan is connectable.
///
/// This guards against a regression where switching to LE-only scanning prevents
/// subsequent connection (e.g. because the device address type changes).
///
/// Requires a powered-on Tap Strap 2 or TapXR within BLE range.
#[tokio::test]
#[ignore = "requires physical Tap device"]
async fn le_scan_device_is_connectable() {
    let mut manager = tap_ble::BleManager::new().await.expect("no BLE adapter");

    // discover_devices uses LE-only transport on Linux.
    let devices = manager.scan(5000).await.expect("scan failed");
    assert!(!devices.is_empty(), "no Tap devices found within 5 s");

    let first = &devices[0];
    manager
        .connect(mapping_core::engine::DeviceId::new("solo"), first.address)
        .await
        .expect("connect failed after LE-only scan");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    manager
        .disconnect(&mapping_core::engine::DeviceId::new("solo"))
        .await
        .expect("disconnect failed");
}
