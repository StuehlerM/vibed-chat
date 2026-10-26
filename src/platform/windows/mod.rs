// Windows platform backend.
// Implements the Platform trait using Win32/GDI.
// All Win32-specific code lives in this module and its submodules.

pub mod clipboard;
pub mod dialogs;
pub mod ffi;
pub mod fonts;
pub mod gfx;
pub mod paint;
pub mod render;
pub mod text;
pub mod window;

use super::backend::{Clipboard, Dialogs, Platform, PlatformInfo};

/// Windows platform implementation.
pub struct WindowsPlatform {
    clipboard: clipboard::WinClipboard,
    dialogs: dialogs::WinDialogs,
}

#[allow(dead_code)]
impl WindowsPlatform {
    pub fn new() -> Self {
        Self {
            clipboard: clipboard::WinClipboard,
            dialogs: dialogs::WinDialogs,
        }
    }
}

impl Platform for WindowsPlatform {
    fn info(&self) -> PlatformInfo {
        let blink = unsafe { ffi::GetCaretBlinkTime() };
        PlatformInfo {
            caret_blink_ms: if blink == 0 { 500 } else { blink },
            system_font: "Segoe UI",
            mono_font: "Consolas",
        }
    }

    fn clipboard(&self) -> &dyn Clipboard {
        &self.clipboard
    }

    fn dialogs(&self) -> &dyn Dialogs {
        &self.dialogs
    }
}

/// Create the platform instance for use by the core app.
#[allow(dead_code)]
pub fn create_platform() -> WindowsPlatform {
    WindowsPlatform::new()
}

/// Entry point: run the Windows application.
pub fn run() {
    crate::app_entry();
}
