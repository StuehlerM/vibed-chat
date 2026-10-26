// Platform abstraction layer.
// Each OS gets its own backend module under platform/<os>/.
// The core application logic (app, chat, session, acp, markdown, json)
// is platform-independent and talks to the backend through traits
// defined in this module.

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

pub mod backend;

#[allow(unused_imports)]
pub use backend::Platform;

/// Run the application. Implemented by each platform backend.
/// This is the single entry point called from main.rs.
pub fn run() {
    backend::run_platform();
}
