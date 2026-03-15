pub mod commands;
pub mod events;
pub mod platform;
pub mod pump;
pub mod state;

use std::sync::Arc;

use tauri::Manager as _;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialise the `log` facade so tap-ble and other crates can emit log output.
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Build AppState on Tauri's async runtime so that btleplug's internal
            // D-Bus IOResource task (spawned via tokio::spawn inside Manager::new) is
            // owned by the same runtime that drives all subsequent BLE commands.
            //
            // Previously a temporary current_thread runtime was used, which killed all
            // tasks spawned inside it when it was dropped — including btleplug's IOResource.
            // This caused every subsequent D-Bus call through self.adapter to hang forever
            // because no task was left to receive and dispatch the replies.
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let setup_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                tx.send(state::build_app_state(&setup_handle).await)
                    .expect("setup channel send failed");
            });
            let (app_state, event_rx, status_rx) = rx
                .recv()
                .expect("setup channel recv failed")
                .expect("failed to initialise app state");

            let state_arc = Arc::new(app_state);

            // Register state so commands can receive it via `State<'_, Arc<AppState>>`.
            app.manage(Arc::clone(&state_arc));

            // Spawn the event pump and BLE status listener.
            let pump_state = Arc::clone(&state_arc);
            let pump_app = app_handle.clone();
            tauri::async_runtime::spawn(pump::run_event_pump(pump_app, pump_state, event_rx));

            let status_app = app_handle.clone();
            tauri::async_runtime::spawn(pump::run_ble_status_listener(status_app, status_rx));

            // Reconnect previously paired devices in the background.
            // Runs after the pump tasks so status events are routed to the UI.
            let reconnect_state = Arc::clone(&state_arc);
            tauri::async_runtime::spawn(state::auto_reconnect(app_handle.clone(), reconnect_state));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_devices,
            commands::connect_device,
            commands::disconnect_device,
            commands::list_profiles,
            commands::load_profile,
            commands::save_profile,
            commands::delete_profile,
            commands::activate_profile,
            commands::deactivate_profile,
            commands::push_layer,
            commands::pop_layer,
            commands::set_debug_mode,
            commands::get_engine_state,
        ])
        // Task 3.16 / 4.21: graceful shutdown — exit controller mode before the
        // OS window closes so the Tap device returns to text mode.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app = window.app_handle().clone();
                if let Some(state) = app.try_state::<Arc<state::AppState>>() {
                    let state = Arc::clone(&state);
                    // block_on is acceptable here: CloseRequested fires on the main
                    // thread after the event loop has stopped processing new events.
                    tauri::async_runtime::block_on(async move {
                        if let Some(ble) = &state.ble_manager {
                            let mut manager = ble.lock().await;
                            let roles: Vec<_> = manager.connected_ids().cloned().collect();
                            for role in roles {
                                if let Err(e) = manager.disconnect(&role).await {
                                    log::warn!("shutdown disconnect failed for '{role}': {e}");
                                }
                            }
                        }
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
