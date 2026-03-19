use btleplug::api::BDAddr;

/// Basic information about a discovered Tap device returned by a BLE scan.
#[derive(Debug, Clone)]
pub struct TapDeviceInfo {
    /// User-visible device name (may be `None` if not advertised).
    pub name: Option<String>,
    /// Hardware Bluetooth address.
    pub address: BDAddr,
    /// Signal strength in dBm at time of discovery (`None` if not reported by adapter).
    ///
    /// This is `None` for cached devices that were not actively advertising during the
    /// scan window; those devices should not be shown as connectable.
    pub rssi: Option<i16>,
    /// `true` if the device sent an advertisement packet during the current scan window.
    ///
    /// `false` means the entry came from the OS Bluetooth cache (device may be off or
    /// out of range).  The UI should disable the connect action for cached entries.
    pub seen_in_scan: bool,
    /// `true` if the device currently has an active BLE connection to this host (the OS).
    ///
    /// When `true` the device is not advertising and its BLE connection slot is occupied;
    /// our app cannot connect to it until the OS-level connection is released.  The UI
    /// should show a distinct "Connected to OS" state rather than "Cached" or signal
    /// strength, and disable the connect action.
    pub is_connected_to_os: bool,
}
