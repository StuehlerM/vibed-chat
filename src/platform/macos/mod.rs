// macOS platform backend (skeleton).
// Not yet implemented. The core application logic is platform-independent
// and ready to be wired to a macOS backend (e.g., raw AppKit/Cocoa via FFI,
// or a minimal Metal/CoreGraphics approach matching the zero-dependency philosophy).

use crate::platform::backend::{Clipboard, Dialogs, Platform, PlatformInfo};

pub struct MacosPlatform {
    clipboard: MacosClipboard,
    dialogs: MacosDialogs,
}

impl MacosPlatform {
    pub fn new() -> Self {
        Self {
            clipboard: MacosClipboard,
            dialogs: MacosDialogs,
        }
    }
}

impl Platform for MacosPlatform {
    fn info(&self) -> PlatformInfo {
        PlatformInfo {
            caret_blink_ms: 500,
            system_font: "San Francisco",
            mono_font: "Menlo",
        }
    }

    fn clipboard(&self) -> &dyn Clipboard {
        &self.clipboard
    }

    fn dialogs(&self) -> &dyn Dialogs {
        &self.dialogs
    }
}

pub struct MacosClipboard;

impl Clipboard for MacosClipboard {
    fn set_text(&self, _text: &str) {
        // TODO: implement via NSPasteboard
    }

    fn get_text(&self) -> Option<String> {
        // TODO: implement via NSPasteboard
        None
    }
}

pub struct MacosDialogs;

impl Dialogs for MacosDialogs {
    fn pick_folder(&self, _title: &str) -> Option<String> {
        // TODO: implement via NSOpenPanel
        None
    }

    fn confirm(&self, _title: &str, _message: &str) -> bool {
        // TODO: implement via NSAlert
        false
    }

    fn show_error(&self, _title: &str, message: &str) {
        // TODO: implement via NSAlert
        eprintln!("Error: {}", message);
    }
}

/// Create the platform instance for use by the core app.
#[allow(dead_code)]
pub fn create_platform() -> MacosPlatform {
    MacosPlatform::new()
}

/// Entry point: run the macOS application.
/// Currently a stub - the macOS backend is not yet implemented.
pub fn run() {
    eprintln!(
        "vibe-chat: macOS backend is not yet implemented.\n\
         The Windows version is fully functional. The macOS skeleton is in place\n\
         at src/platform/macos/ - ready for a raw AppKit/CoreGraphics implementation."
    );
    std::process::exit(1);
}
