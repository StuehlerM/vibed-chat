// Platform backend trait definitions.
// The core application talks to the OS through these traits.
// Each platform module (windows/, linux/, macos/) provides its own implementation.
#![allow(dead_code)]

/// Clipboard operations available on all platforms.
pub trait Clipboard {
    fn set_text(&self, text: &str);
    fn get_text(&self) -> Option<String>;
}

/// Platform-specific dialogs.
pub trait Dialogs {
    /// Open a folder picker dialog and return the selected path, or None if cancelled.
    fn pick_folder(&self, title: &str) -> Option<String>;

    /// Show a confirmation dialog. Returns true if the user clicked "Yes".
    fn confirm(&self, title: &str, message: &str) -> bool;

    /// Show an error message box.
    fn show_error(&self, title: &str, message: &str);
}

/// Platform capabilities reported to the core app.
pub struct PlatformInfo {
    /// Default caret blink interval in milliseconds.
    pub caret_blink_ms: u32,
    /// Default system font family name.
    pub system_font: &'static str,
    /// Monospace font family name.
    pub mono_font: &'static str,
}

/// Trait representing the OS platform backend.
/// Implemented by each platform module.
pub trait Platform {
    fn info(&self) -> PlatformInfo;
    fn clipboard(&self) -> &dyn Clipboard;
    fn dialogs(&self) -> &dyn Dialogs;
}

/// Run the application on the current platform.
/// This is the platform-specific entry point called from `platform::run()`.
pub fn run_platform() {
    #[cfg(windows)]
    {
        crate::platform::windows::run();
    }

    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::run();
    }

    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::run();
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        eprintln!("Unsupported platform");
        std::process::exit(1);
    }
}
