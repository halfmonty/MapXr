//! Platform-specific start-at-login helpers.
//!
//! Each function registers or deregisters the current executable so that the
//! OS launches it automatically on user login.  The executable path is obtained
//! from [`std::env::current_exe`] at call time so it always reflects the actual
//! installed binary path.

/// Enable or disable start-at-login for the current executable.
///
/// Returns `Ok(())` on success.  Returns an error string on failure or on
/// platforms that are not supported.
pub fn set_start_at_login(enabled: bool) -> Result<(), String> {
    set_start_at_login_impl(enabled)
}

// ── Linux ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn set_start_at_login_impl(enabled: bool) -> Result<(), String> {
    let autostart_dir = dirs_path()?;
    let desktop_path = autostart_dir.join("tap-mapper.desktop");

    if enabled {
        std::fs::create_dir_all(&autostart_dir)
            .map_err(|e| format!("failed to create autostart dir: {e}"))?;

        let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;

        let contents = format!(
            "[Desktop Entry]\nType=Application\nName=tap-mapper\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );

        std::fs::write(&desktop_path, contents)
            .map_err(|e| format!("failed to write autostart file: {e}"))
    } else {
        match std::fs::remove_file(&desktop_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("failed to remove autostart file: {e}")),
        }
    }
}

#[cfg(target_os = "linux")]
fn dirs_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME env var not set".to_string())?;
    Ok(std::path::PathBuf::from(home)
        .join(".config")
        .join("autostart"))
}

// ── macOS ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn set_start_at_login_impl(enabled: bool) -> Result<(), String> {
    let launch_agents_dir = {
        let home = std::env::var("HOME").map_err(|_| "HOME env var not set".to_string())?;
        std::path::PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
    };

    let plist_path = launch_agents_dir.join("com.mapxr.tap-mapper.plist");

    if enabled {
        std::fs::create_dir_all(&launch_agents_dir)
            .map_err(|e| format!("failed to create LaunchAgents dir: {e}"))?;

        let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;

        let contents = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \t<key>Label</key><string>com.mapxr.tap-mapper</string>\n\
             \t<key>ProgramArguments</key>\n\
             \t<array><string>{}</string></array>\n\
             \t<key>RunAtLoad</key><true/>\n\
             </dict>\n\
             </plist>\n",
            exe.display()
        );

        std::fs::write(&plist_path, contents)
            .map_err(|e| format!("failed to write LaunchAgent plist: {e}"))
    } else {
        match std::fs::remove_file(&plist_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("failed to remove LaunchAgent plist: {e}")),
        }
    }
}

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn set_start_at_login_impl(enabled: bool) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| format!("failed to open Run registry key: {e}"))?;

    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;
        run_key
            .set_value("tap-mapper", &exe.to_string_lossy().as_ref())
            .map_err(|e| format!("failed to set registry value: {e}"))
    } else {
        match run_key.delete_value("tap-mapper") {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("failed to delete registry value: {e}")),
        }
    }
}

// ── Unsupported fallback ──────────────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn set_start_at_login_impl(_enabled: bool) -> Result<(), String> {
    Err("start-at-login is not supported on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_start_at_login_disable_is_idempotent() {
        // Disabling when not registered should succeed (file-not-found is ignored).
        // This test runs on all platforms.
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        assert!(set_start_at_login(false).is_ok());
    }
}
