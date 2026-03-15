/// All errors produced by the `tap-ble` crate.
#[derive(Debug, thiserror::Error)]
pub enum BleError {
    #[error("no BLE adapter found on this system")]
    AdapterNotFound,

    #[error("characteristic {uuid} not found on device {address}")]
    MissingCharacteristic { uuid: String, address: String },

    #[error("connection refused to {address}: {reason}")]
    ConnectionRefused { address: String, reason: String },

    #[error("device {address} not found in scan results — run a scan first")]
    DeviceNotFound { address: String },

    #[error("device {address} disconnected unexpectedly")]
    UnexpectedDisconnect { address: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("BLE driver error: {0}")]
    Btleplug(#[from] btleplug::Error),
}
