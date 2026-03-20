//! Wayland focused-window monitor using `wlr-foreign-toplevel-management-unstable-v1`.
//!
//! Supported compositors: Sway, Hyprland, River, Niri, KDE Plasma ≥ 5.27, COSMIC.
//! GNOME (Mutter) does not implement this protocol; callers fall back to X11.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::watch;
use wayland_client::{
    backend::ObjectData, protocol::wl_registry, Connection, Dispatch, EventQueue, Proxy,
    QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

use super::FocusedWindow;

// ── Toplevel tracking ─────────────────────────────────────────────────────────

/// Per-toplevel state accumulated between `title`/`app_id`/`state` events and
/// committed when the `done` event fires.
#[derive(Default)]
struct ToplevelInfo {
    title: String,
    app_id: String,
    activated: bool,
    // Pending fields, set by individual property events before `done` commits them.
    pending_title: Option<String>,
    pending_app_id: Option<String>,
    pending_activated: Option<bool>,
}

// ── Dispatch state ────────────────────────────────────────────────────────────

struct WaylandState {
    /// All known toplevels keyed by their proxy handle.
    toplevels: HashMap<ZwlrForeignToplevelHandleV1, ToplevelInfo>,
    /// Set to true once the manager global is bound.
    manager_found: bool,
    /// Sends the currently focused window to the tokio watch receiver.
    tx: watch::Sender<Option<FocusedWindow>>,
}

impl WaylandState {
    /// Re-derive the focused window from all tracked toplevels and publish it.
    fn publish_focused(&self) {
        let focused = self
            .toplevels
            .values()
            .find(|t| t.activated)
            .map(|t| FocusedWindow {
                app: t.app_id.clone(),
                title: t.title.clone(),
            });
        // SendError means the receiver was dropped — ignore.
        let _ = self.tx.send(focused);
    }
}

// ── Dispatch implementations ──────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == ZwlrForeignToplevelManagerV1::interface().name {
                // Bind at most version 3 (highest we handle).
                registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(name, version.min(3), qh, ());
                state.manager_found = true;
            }
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                state.toplevels.insert(toplevel, ToplevelInfo::default());
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                // Manager destroyed — the compositor stopped supporting the protocol.
                log::warn!("context switching: wlr-foreign-toplevel-management manager destroyed");
            }
            _ => {}
        }
    }

    fn event_created_child(_opcode: u16, qh: &QueueHandle<Self>) -> Arc<dyn ObjectData> {
        // The only child object created by this interface is ZwlrForeignToplevelHandleV1
        // (via the `toplevel` event). Tell wayland-client to dispatch its events through
        // our Dispatch<ZwlrForeignToplevelHandleV1, ()> impl.
        qh.make_data::<ZwlrForeignToplevelHandleV1, ()>(())
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                state
                    .toplevels
                    .entry(handle.clone())
                    .or_default()
                    .pending_title = Some(title);
            }
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                state
                    .toplevels
                    .entry(handle.clone())
                    .or_default()
                    .pending_app_id = Some(app_id);
            }
            zwlr_foreign_toplevel_handle_v1::Event::State { state: raw } => {
                // The state is encoded as a Wayland array of u32 values (native-endian).
                // Activated = 2 per wlr-foreign-toplevel-management-unstable-v1 protocol.
                let activated = raw
                    .chunks_exact(4)
                    .any(|b| u32::from_ne_bytes(b.try_into().unwrap()) == 2);
                state
                    .toplevels
                    .entry(handle.clone())
                    .or_default()
                    .pending_activated = Some(activated);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                // Commit pending updates for this toplevel.
                if let Some(info) = state.toplevels.get_mut(handle) {
                    if let Some(t) = info.pending_title.take() {
                        info.title = t;
                    }
                    if let Some(a) = info.pending_app_id.take() {
                        info.app_id = a;
                    }
                    if let Some(act) = info.pending_activated.take() {
                        info.activated = act;
                    }
                }
                state.publish_focused();
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                state.toplevels.remove(handle);
                state.publish_focused();
            }
            _ => {}
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Start the Wayland focus monitor.
///
/// Connects to the Wayland compositor, binds the `wlr-foreign-toplevel-management`
/// protocol global, and spawns a background thread to process events. Returns
/// `Ok(())` on success or `Err(message)` if the protocol is unsupported.
///
/// On success the background thread writes focus changes to `tx` until the
/// receiver end is dropped or the Wayland connection is broken.
pub fn start(tx: watch::Sender<Option<FocusedWindow>>) -> Result<(), String> {
    let conn =
        Connection::connect_to_env().map_err(|e| format!("Wayland connection failed: {e}"))?;

    let mut event_queue: EventQueue<WaylandState> = conn.new_event_queue();
    let qh = event_queue.handle();

    // Request the registry so we receive Global events.
    conn.display().get_registry(&qh, ());

    let mut state = WaylandState {
        toplevels: HashMap::new(),
        manager_found: false,
        tx,
    };

    // First roundtrip: receive and bind all advertised globals.
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| format!("Wayland roundtrip failed: {e}"))?;

    if !state.manager_found {
        return Err("compositor does not support wlr-foreign-toplevel-management".into());
    }

    // Second roundtrip: receive initial toplevel state from the compositor.
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| format!("Wayland roundtrip (initial state) failed: {e}"))?;

    // Spawn background thread to process ongoing events.
    // The thread runs until the Wayland connection breaks or the watch sender is closed.
    std::thread::Builder::new()
        .name("wayland-focus-monitor".into())
        .spawn(move || {
            loop {
                // blocking_dispatch waits until at least one event arrives.
                if event_queue.blocking_dispatch(&mut state).is_err() {
                    log::debug!("context switching: Wayland event loop ended");
                    break;
                }
                if state.tx.is_closed() {
                    break;
                }
            }
        })
        .map_err(|e| format!("failed to spawn Wayland thread: {e}"))?;

    Ok(())
}
