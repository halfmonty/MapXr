//! macOS focused-window monitor — stub only.
//!
//! macOS support is planned but not implemented in this build. The stub
//! logs a one-time info message and returns a receiver that never changes.

use tokio::sync::watch;

use super::FocusedWindow;

/// Return a watch receiver that never produces a value.
///
/// Logs once to inform developers that context switching is disabled on macOS.
pub fn start() -> watch::Receiver<Option<FocusedWindow>> {
    log::info!(
        "context switching: macOS support is not yet implemented — \
         automatic profile switching is disabled"
    );
    let (_tx, rx) = watch::channel(None);
    rx
}
