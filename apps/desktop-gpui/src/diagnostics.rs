//! The rolling file log's location.
//!
//! The Feedback page's Diagnostic Report and its A/V sync self-test went away
//! with the page; the log file itself stays, because `main.rs` installs the
//! rolling appender from here.

use std::path::PathBuf;

/// The rolling log file's prefix. Deliberately *not* the Tauri app's
/// `cap-desktop.log`: both apps write into the same directory on macOS, and a
/// shared prefix would interleave two processes' lines into one file and make
/// either app's "newest log" lookup pick up the other's.
pub const LOG_FILE_PREFIX: &str = "cap-gpui.log";

/// `MAX_SIZE` in `src-tauri/src/logging.rs`.

// ---------------------------------------------------------------------------
// Log file
// ---------------------------------------------------------------------------

/// Where the rolling file log lives.
///
/// macOS uses `~/Library/Logs/so.cap.desktop` -- the same directory the Tauri
/// app writes into, so a user handing over "the Cap logs folder" hands over
/// both apps' logs -- and every other platform uses
/// `<local data>/so.cap.desktop/logs`, matching `main.rs` over there.
pub fn logs_dir() -> PathBuf {
    if let Ok(directory) = std::env::var("CAP_GPUI_LOG_DIR")
        && !directory.trim().is_empty()
    {
        return PathBuf::from(directory);
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Logs")
            .join("so.cap.desktop")
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("so.cap.desktop")
            .join("logs")
    }
}
