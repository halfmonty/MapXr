//! X11 focused-window monitor via `_NET_ACTIVE_WINDOW`.
//!
//! Works on native X11 sessions and on Wayland sessions with XWayland
//! (e.g. GNOME Wayland, KDE Wayland) for apps that run under XWayland.

use std::time::Duration;

use tokio::sync::watch;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

use super::FocusedWindow;

// ── One-shot query ────────────────────────────────────────────────────────────

/// Query X11 for the currently focused window.
///
/// Returns `None` if X11 is unavailable, no window is focused, or any
/// property read fails.
pub fn query() -> Option<FocusedWindow> {
    let (conn, screen_num) = RustConnection::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;

    // Intern the atoms we need.
    let active_window_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let net_wm_name_atom = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let utf8_string_atom = conn
        .intern_atom(false, b"UTF8_STRING")
        .ok()?
        .reply()
        .ok()?
        .atom;

    // Read the active window ID from the root window.
    let reply = conn
        .get_property(false, root, active_window_atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let window_id: u32 = reply.value32()?.next()?;
    if window_id == 0 {
        return None;
    }

    // Read the window title (_NET_WM_NAME, UTF-8).
    // Fall back to WM_NAME (Latin-1) if _NET_WM_NAME is absent.
    let title = {
        let r = conn
            .get_property(
                false,
                window_id,
                net_wm_name_atom,
                utf8_string_atom,
                0,
                2048,
            )
            .ok()?
            .reply()
            .ok()?;
        if r.value.is_empty() {
            // Try legacy WM_NAME
            let r2 = conn
                .get_property(
                    false,
                    window_id,
                    AtomEnum::WM_NAME,
                    AtomEnum::STRING,
                    0,
                    2048,
                )
                .ok()?
                .reply()
                .ok()?;
            String::from_utf8_lossy(&r2.value).into_owned()
        } else {
            String::from_utf8_lossy(&r.value).into_owned()
        }
    };

    // Read the application name from WM_CLASS (first null-terminated string = instance name).
    let class_reply = conn
        .get_property(
            false,
            window_id,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            0,
            2048,
        )
        .ok()?
        .reply()
        .ok()?;
    let null_pos = class_reply
        .value
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(class_reply.value.len());
    let app = String::from_utf8_lossy(&class_reply.value[..null_pos]).into_owned();

    if app.is_empty() && title.is_empty() {
        return None;
    }

    Some(FocusedWindow { app, title })
}

// ── Polling thread ────────────────────────────────────────────────────────────

/// Spawn a background thread that polls X11 every 500 ms and sends focus
/// changes via `tx`.
///
/// The thread exits when `cancel` is set to `true` or the watch sender
/// is closed (all receivers dropped).
pub fn start(tx: watch::Sender<Option<FocusedWindow>>, cancel: watch::Receiver<bool>) {
    std::thread::Builder::new()
        .name("x11-focus-monitor".into())
        .spawn(move || {
            let mut prev: Option<FocusedWindow> = None;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                if *cancel.borrow() {
                    break;
                }
                let current = query();
                if current != prev {
                    prev = current.clone();
                    if tx.send(current).is_err() {
                        // All receivers dropped — exit.
                        break;
                    }
                }
            }
        })
        .ok();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Full X11 queries require a live display server and cannot run in CI.
    // Manual verification steps:
    //   1. Run the app on an X11 or XWayland session.
    //   2. Focus a terminal window — verify the context monitor log shows the
    //      correct app (e.g. "alacritty") and title.
    //   3. Switch to a browser — verify the log updates.

    #[test]
    fn query_returns_none_without_display() {
        // Save and clear DISPLAY so the X11 connection attempt fails.
        // This tests that query() returns None gracefully rather than panicking.
        let saved = std::env::var_os("DISPLAY");
        // SAFETY: test-only env manipulation; run with RUST_TEST_THREADS=1 if needed.
        unsafe { std::env::remove_var("DISPLAY") };
        let result = query();
        // Restore.
        if let Some(v) = saved {
            unsafe { std::env::set_var("DISPLAY", v) };
        }
        // query() must not panic; result is None without a display.
        assert!(result.is_none());
    }
}
