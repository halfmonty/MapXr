//! Per-platform focused-window monitor.
//!
//! [`start_monitor`] spawns a background OS thread (or threads) appropriate for the
//! current platform and returns a [`tokio::sync::watch`] receiver that is updated
//! whenever the focused window changes.
//!
//! Platform support:
//! - **Linux**: tries the Wayland `wlr-foreign-toplevel-management` protocol first;
//!   falls back to X11 `_NET_ACTIVE_WINDOW` if the compositor does not support it.
//! - **Windows**: polls `GetForegroundWindow` every 500 ms.
//! - **macOS**: stub — returns a receiver that never changes and logs a warning.

use tokio::sync::watch;

#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(target_os = "macos")]
mod macos;

// ── Public types ─────────────────────────────────────────────────────────────

/// The currently focused window on the desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedWindow {
    /// Application name.
    ///
    /// - Linux X11: `WM_CLASS` instance name (e.g. `"firefox"`, `"code"`)
    /// - Linux Wayland: `app_id` from `wlr-foreign-toplevel-management`
    /// - Windows: process executable stem (e.g. `"firefox"`, `"Code"`)
    pub app: String,
    /// Window title (e.g. `"index.rs — mapxr"`, `"Mozilla Firefox"`).
    pub title: String,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Start the focused-window monitor.
///
/// Spawns a platform-specific background thread and returns a watch receiver
/// that is updated with the currently focused window. The receiver holds `None`
/// until the first window focus event is detected.
///
/// Pass the app's cancel receiver so the background thread can exit cleanly
/// when the app shuts down.
pub fn start_monitor(cancel: watch::Receiver<bool>) -> watch::Receiver<Option<FocusedWindow>> {
    #[cfg(target_os = "linux")]
    return linux_start(cancel);

    #[cfg(target_os = "windows")]
    return windows_impl::start(cancel);

    #[cfg(target_os = "macos")]
    return macos::start();

    // Fallback for unsupported platforms — return a receiver that never changes.
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let (_tx, rx) = watch::channel(None);
        rx
    }
}

// ── Linux dispatch ────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn linux_start(cancel: watch::Receiver<bool>) -> watch::Receiver<Option<FocusedWindow>> {
    let (tx, rx) = watch::channel(None);

    // Try Wayland first if WAYLAND_DISPLAY is set.
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        match wayland::start(tx.clone()) {
            Ok(()) => {
                log::info!("context switching: Wayland backend started");
                return rx;
            }
            Err(e) => {
                log::info!(
                    "context switching: Wayland backend unavailable ({e}), falling back to X11"
                );
            }
        }
    }

    // Fall back to X11 if DISPLAY is set.
    if std::env::var_os("DISPLAY").is_some() {
        x11::start(tx, cancel);
        log::info!("context switching: X11 backend started");
    } else {
        log::info!("context switching: no display available — disabled");
    }

    rx
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_window_equality_matches_on_both_fields() {
        let a = FocusedWindow {
            app: "firefox".into(),
            title: "Mozilla Firefox".into(),
        };
        let b = FocusedWindow {
            app: "firefox".into(),
            title: "Mozilla Firefox".into(),
        };
        let c = FocusedWindow {
            app: "code".into(),
            title: "editor".into(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn focused_window_different_title_not_equal() {
        let a = FocusedWindow {
            app: "firefox".into(),
            title: "Tab 1".into(),
        };
        let b = FocusedWindow {
            app: "firefox".into(),
            title: "Tab 2".into(),
        };
        assert_ne!(a, b);
    }
}
