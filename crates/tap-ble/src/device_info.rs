use btleplug::api::BDAddr;

/// Basic information about a discovered Tap device returned by a BLE scan.
#[derive(Debug, Clone)]
pub struct TapDeviceInfo {
    /// User-visible device name (may be `None` if not advertised).
    pub name: Option<String>,
    /// Hardware Bluetooth address.
    pub address: BDAddr,
    /// Signal strength in dBm at time of discovery (`None` if not reported by adapter).
    pub rssi: Option<i16>,
}
