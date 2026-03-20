//! Windows focused-window monitor via `GetForegroundWindow`.
//!
//! Polls `GetForegroundWindow()` every 500 ms. App name is derived from
//! the process executable stem (e.g. `"firefox"`, `"Code"`).

use std::path::Path;
use std::time::Duration;

use tokio::sync::watch;
use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

use super::FocusedWindow;

// ── One-shot query ────────────────────────────────────────────────────────────

/// Query Win32 for the currently focused window.
///
/// Returns `None` if no window has focus (e.g. the desktop is focused) or any
/// Win32 call fails.
pub fn query() -> Option<FocusedWindow> {
    // SAFETY: all Win32 calls here follow documented usage patterns.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        // Window title.
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        // Process ID.
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        // Process executable path → file stem = app name.
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path_buf = vec![0u16; MAX_PATH as usize];
        let mut size = path_buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(path_buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;

        let path_str = String::from_utf16_lossy(&path_buf[..size as usize]);
        let app = Path::new(&path_str)
            .file_stem()?
            .to_string_lossy()
            .into_owned();

        Some(FocusedWindow { app, title })
    }
}

// ── Polling thread ────────────────────────────────────────────────────────────

/// Start the Win32 focus monitor.
///
/// Spawns a background thread that polls every 500 ms and returns a watch
/// receiver that is updated when the focused window changes.
pub fn start(mut cancel: watch::Receiver<bool>) -> watch::Receiver<Option<FocusedWindow>> {
    let (tx, rx) = watch::channel(None);

    std::thread::Builder::new()
        .name("win32-focus-monitor".into())
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
                        break;
                    }
                }
            }
        })
        .ok();

    rx
}
