use tauri_build::{Attributes, DefaultPermissionRule, InlinedPlugin};

// Commands exposed by BlePlugin.kt via @Command annotations.
// registerListener / removeListener are base Plugin class commands required by
// addPluginListener() in the JS layer — they register Channels for trigger() events.
const BLE_COMMANDS: &[&str] = &[
    "checkBlePermissions",
    "requestBlePermissions",
    "listBondedDevices",
    "startForegroundService",
    "stopForegroundService",
    "updateServiceNotification",
    "startScan",
    "stopScan",
    "connect",
    "disconnect",
    "registerListener",
    "removeListener",
];

// Commands exposed by ShizukuPlugin.kt via @Command annotations.
const SHIZUKU_COMMANDS: &[&str] = &[
    "getShizukuState",
    "requestShizukuPermission",
    "openShizukuApp",
];

// Commands exposed by BatteryPlugin.kt via @Command annotations.
const BATTERY_COMMANDS: &[&str] = &[
    "getOemInfo",
    "checkBatteryExemptionGranted",
    "requestBatteryExemption",
    "openOemBatterySettings",
];

fn main() {
    tauri_build::try_build(
        Attributes::new()
            .plugin(
                "ble",
                InlinedPlugin::new()
                    .commands(BLE_COMMANDS)
                    .default_permission(DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "shizuku",
                InlinedPlugin::new()
                    .commands(SHIZUKU_COMMANDS)
                    .default_permission(DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "battery",
                InlinedPlugin::new()
                    .commands(BATTERY_COMMANDS)
                    .default_permission(DefaultPermissionRule::AllowAllCommands),
            ),
    )
    .expect("failed to run build script");
}
