pub mod commands;
pub mod events;
pub mod platform;
pub mod state;

#[cfg(not(mobile))]
pub mod context_rules;
#[cfg(not(mobile))]
pub mod focus_monitor;
#[cfg(not(mobile))]
pub mod login_item;
#[cfg(not(mobile))]
pub mod pump;

use std::sync::Arc;

use tauri::{Manager as _};

#[cfg(not(mobile))]
use tauri::image::Image;
#[cfg(not(mobile))]
use tauri::menu::{MenuBuilder, MenuItemBuilder};
#[cfg(not(mobile))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(not(mobile))]
use tauri::WebviewWindow;

#[cfg(not(mobile))]
const TRAY_ID: &str = "main-tray";
#[cfg(not(mobile))]
const TRAY_ITEM_SHOW: &str = "show";
#[cfg(not(mobile))]
const TRAY_ITEM_HIDE: &str = "hide";
#[cfg(not(mobile))]
const TRAY_ITEM_PROFILE: &str = "profile-label";
#[cfg(not(mobile))]
const TRAY_ITEM_CHECK_UPDATES: &str = "check-updates";
#[cfg(not(mobile))]
const TRAY_ITEM_QUIT: &str = "quit";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    #[cfg(mobile)]
    run_mobile();

    #[cfg(not(mobile))]
    run_desktop();
}

// ── Mobile entry point ────────────────────────────────────────────────────────

#[cfg(mobile)]
fn run_mobile() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let setup_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                tx.send(state::build_app_state(&setup_handle).await)
                    .expect("setup channel send failed");
            });
            let app_state = rx
                .recv()
                .expect("setup channel recv failed")
                .expect("failed to initialise app state");

            app.manage(Arc::new(app_state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::read_file_text,
            commands::get_platform,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Desktop entry point ───────────────────────────────────────────────────────

#[cfg(not(mobile))]
fn run_desktop() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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

            // Snapshot start_minimised before we move app_state into managed state.
            let start_minimised = tauri::async_runtime::block_on(async {
                app_state.preferences.lock().await.start_minimised
            });

            let state_arc = Arc::new(app_state);

            // Register state so commands can receive it via `State<'_, Arc<AppState>>`.
            app.manage(Arc::clone(&state_arc));

            // ── System tray ───────────────────────────────────────────────────

            // Embed icon bytes at compile time so they're always available
            // regardless of install path (RPM, DEB, AppImage, dev).
            let icon = Image::from_bytes(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/icons/32x32.png"
            )))
            .expect("embedded tray icon is valid PNG");

            let menu = build_tray_menu(&app_handle, "No profile active")?;

            TrayIconBuilder::with_id(TRAY_ID)
                .icon(icon)
                .menu(&menu)
                .tooltip("tap-mapper\nNo profile active · 0 devices connected")
                .on_tray_icon_event(|tray, event| {
                    // Left-click: toggle window visibility.
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            toggle_window_visibility(&win);
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    TRAY_ITEM_SHOW => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    TRAY_ITEM_HIDE => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.hide();
                        }
                    }
                    TRAY_ITEM_CHECK_UPDATES => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            trigger_update_check(&handle).await;
                            // Show the window so the user sees the banner (or lack thereof).
                            if let Some(win) = handle.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        });
                    }
                    TRAY_ITEM_QUIT => {
                        // Emit a synthetic quit event so the close handler can do
                        // BLE cleanup before we exit.  We exit directly here; the
                        // close handler's prevent_close guard is bypassed because we
                        // are calling exit(), not closing the window.
                        if let Some(state) = app.try_state::<Arc<state::AppState>>() {
                            let state = Arc::clone(&state);
                            tauri::async_runtime::block_on(async move {
                                ble_disconnect_all(&state).await;
                            });
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // ── Spawn background tasks ────────────────────────────────────────

            let pump_state = Arc::clone(&state_arc);
            let pump_app = app_handle.clone();
            tauri::async_runtime::spawn(pump::run_event_pump(pump_app, pump_state, event_rx));

            let status_app = app_handle.clone();
            tauri::async_runtime::spawn(pump::run_ble_status_listener(status_app, status_rx));

            // Reconnect previously paired devices in the background.
            let reconnect_state = Arc::clone(&state_arc);
            tauri::async_runtime::spawn(state::auto_reconnect(app_handle.clone(), reconnect_state));

            // Spawn the context-switching monitor.
            let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            let ctx_state = Arc::clone(&state_arc);
            tauri::async_runtime::spawn(pump::run_context_monitor(
                app_handle.clone(),
                ctx_state,
                cancel_rx,
                cancel_tx,
            ));

            // Spawn the background update checker (waits 5 s, then checks every 24 h).
            tauri::async_runtime::spawn(run_update_checker(app_handle.clone()));

            // ── start_minimised ───────────────────────────────────────────────

            if start_minimised {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_devices,
            commands::connect_device,
            commands::disconnect_device,
            commands::reassign_device_role,
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
            commands::read_file_text,
            commands::rename_device,
            commands::list_context_rules,
            commands::save_context_rules,
            commands::get_preferences,
            commands::save_preferences,
            commands::check_for_update,
            commands::download_and_install_update,
            commands::get_platform,
        ])
        // Task 3.16 / 4.21 + Epic 12: handle window close.
        // If close_to_tray is enabled (default), hide instead of quitting.
        // Otherwise perform BLE cleanup and allow the close to proceed.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle().clone();

                // Read close_to_tray via the atomic mirror — no block_on needed.
                let close_to_tray = app
                    .try_state::<Arc<state::AppState>>()
                    .map(|s| {
                        s.close_to_tray
                            .load(std::sync::atomic::Ordering::Relaxed)
                    })
                    .unwrap_or(false);

                if close_to_tray {
                    // Prevent the OS close and hide to the system tray.
                    // With decorations disabled (frameless window) the
                    // compositor has no server-side frame to lose track of,
                    // so hide/show works correctly on KDE/Wayland.
                    api.prevent_close();
                    let _ = window.hide();

                    // Show a one-time notification so the user knows the app is
                    // still running in the tray.
                    maybe_show_tray_hint(&app);
                } else {
                    // Normal exit path — disconnect BLE cleanly first.
                    if let Some(state) = app.try_state::<Arc<state::AppState>>() {
                        let state = Arc::clone(&state);
                        tauri::async_runtime::block_on(async move {
                            ble_disconnect_all(&state).await;
                        });
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build (or rebuild) the tray context menu with the given active profile name.
fn build_tray_menu(
    app: &tauri::AppHandle,
    profile_name: &str,
) -> Result<tauri::menu::Menu<tauri::Wry>, tauri::Error> {
    let show = MenuItemBuilder::with_id(TRAY_ITEM_SHOW, "Show").build(app)?;
    let hide = MenuItemBuilder::with_id(TRAY_ITEM_HIDE, "Hide").build(app)?;
    let profile_label =
        MenuItemBuilder::with_id(TRAY_ITEM_PROFILE, format!("Active profile: {profile_name}"))
            .enabled(false)
            .build(app)?;
    let quit = MenuItemBuilder::with_id(TRAY_ITEM_QUIT, "Quit").build(app)?;

    let check_updates =
        MenuItemBuilder::with_id(TRAY_ITEM_CHECK_UPDATES, "Check for updates").build(app)?;

    MenuBuilder::new(app)
        .item(&show)
        .item(&hide)
        .separator()
        .item(&profile_label)
        .separator()
        .item(&check_updates)
        .item(&quit)
        .build()
}

/// Toggle window between visible-and-focused and hidden.
fn toggle_window_visibility(win: &WebviewWindow) {
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Disconnect all connected BLE devices.
async fn ble_disconnect_all(state: &state::AppState) {
    if let Some(ble) = &state.ble_manager {
        let mut manager = ble.lock().await;
        let roles: Vec<_> = manager.connected_ids().cloned().collect();
        for role in roles {
            if let Err(e) = manager.disconnect(&role).await {
                log::warn!("shutdown disconnect failed for '{role}': {e}");
            }
        }
    }
}

/// Show the first-hide tray notification (once only).
fn maybe_show_tray_hint(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = match app.try_state::<Arc<state::AppState>>() {
            Some(s) => s,
            None => return,
        };
        let already_shown = {
            let prefs = state.preferences.lock().await;
            prefs.shown_tray_hint
        };
        if already_shown {
            return;
        }
        {
            let mut prefs = state.preferences.lock().await;
            prefs.shown_tray_hint = true;
            if let Err(e) = prefs.save(&state.preferences_path) {
                log::warn!("failed to save shown_tray_hint: {e}");
            }
        }
        use tauri_plugin_notification::NotificationExt as _;
        let _ = app
            .notification()
            .builder()
            .title("tap-mapper")
            .body("tap-mapper is still running in the background. Click the tray icon to bring it back.")
            .show();
    });
}

/// Fire a best-effort OS desktop notification.
///
/// Errors from the notification subsystem are logged at `warn` level and never
/// propagated — notifications are advisory only.
pub(crate) fn send_notification(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt as _;
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification failed: {e}");
    }
}

/// Check for an available update and emit [`events::UPDATE_AVAILABLE`] if one is found.
///
/// Best-effort — errors are logged at `warn` level and never propagated.
pub(crate) async fn trigger_update_check(app: &tauri::AppHandle) {
    use tauri::Emitter as _;
    use tauri_plugin_updater::UpdaterExt as _;
    let Ok(updater) = app.updater() else { return };
    let Ok(Some(update)) = updater.check().await else { return };
    let _ = app.emit(
        events::UPDATE_AVAILABLE,
        events::UpdateAvailablePayload {
            version: update.version.clone(),
            release_notes: update.body.clone(),
        },
    );
}

/// Background task: wait 5 s on startup, then check for updates every 24 hours.
async fn run_update_checker(app: tauri::AppHandle) {
    use tokio::time::{sleep, Duration};
    sleep(Duration::from_secs(5)).await;
    loop {
        trigger_update_check(&app).await;
        sleep(Duration::from_secs(24 * 60 * 60)).await;
    }
}

/// Update the tray tooltip and menu profile label with the current engine state.
///
/// Called from the event pump after layer changes and device connect/disconnect.
pub(crate) fn update_tray(app: &tauri::AppHandle, profile_name: &str, device_count: usize) {
    let tooltip = format!(
        "tap-mapper\n{profile_name} · {} device{} connected",
        device_count,
        if device_count == 1 { "" } else { "s" },
    );

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(&tooltip));
        if let Ok(menu) = build_tray_menu(app, profile_name) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}
