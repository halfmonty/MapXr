/// Returns the directory where mapxr reads and writes profile `.json` files.
///
/// **Local override (desktop only):** if a `profiles/` directory exists next to
/// the running executable (e.g. the workspace root when running `cargo tauri dev`),
/// that directory is returned as-is and the OS config path is ignored. This allows
/// a developer to use the repo's own `profiles/` directory without touching
/// `~/.config/mapxr/`. On Android the check always fails harmlessly and the
/// app-data path is used.
///
/// If no local `profiles/` is found, falls back to the platform config dir via
/// Tauri's path API and creates it on demand:
///
/// | OS      | Path                                             |
/// |---------|--------------------------------------------------|
/// | Linux   | `~/.config/mapxr/profiles/`                     |
/// | macOS   | `~/Library/Application Support/mapxr/profiles/` |
/// | Windows | `%APPDATA%\mapxr\profiles\`                     |
/// | Android | `/data/data/com.mapxr.app/files/mapxr/profiles/` |
pub fn profile_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // Check for a local `profiles/` next to the executable first.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let local = exe_dir.join("profiles");
            if local.is_dir() {
                return Ok(local);
            }
        }
    }

    // Fall back to the OS config directory, creating it on demand.
    use tauri::Manager;
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("could not resolve app config dir: {e}"))?;
    let profiles = base.join("profiles");
    std::fs::create_dir_all(&profiles)
        .map_err(|e| format!("could not create profiles dir: {e}"))?;
    Ok(profiles)
}
