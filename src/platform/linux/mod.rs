// Linux platform backend (skeleton).
// Not yet implemented. The core application logic is platform-independent
// and ready to be wired to a Linux backend (e.g., raw X11/Wayland + Cairo,
// or a minimal framebuffer approach matching the zero-dependency philosophy).

use crate::platform::backend::{Clipboard, Dialogs, Platform, PlatformInfo};

pub struct LinuxPlatform {
    clipboard: LinuxClipboard,
    dialogs: LinuxDialogs,
}

impl LinuxPlatform {
    pub const fn new() -> Self {
        Self {
            clipboard: LinuxClipboard,
            dialogs: LinuxDialogs,
        }
    }
}

impl Platform for LinuxPlatform {
    fn info(&self) -> PlatformInfo {
        PlatformInfo {
            caret_blink_ms: 500,
            system_font: "Sans",
            mono_font: "Monospace",
        }
    }

    fn clipboard(&self) -> &dyn Clipboard {
        &self.clipboard
    }

    fn dialogs(&self) -> &dyn Dialogs {
        &self.dialogs
    }
}

pub struct LinuxClipboard;

impl Clipboard for LinuxClipboard {
    fn set_text(&self, _text: &str) {
        // TODO: implement via X11/Wayland clipboard
    }

    fn get_text(&self) -> Option<String> {
        // TODO: implement via X11/Wayland clipboard
        None
    }
}

pub struct LinuxDialogs;

impl Dialogs for LinuxDialogs {
    fn pick_folder(&self, _title: &str) -> Option<String> {
        // TODO: implement via XDG portal or zenity/kdialog
        None
    }

    fn confirm(&self, _title: &str, _message: &str) -> bool {
        // TODO: implement via zenity/kdialog or native dialog
        false
    }

    fn show_error(&self, _title: &str, message: &str) {
        // TODO: implement via zenity/kdialog or native dialog
        eprintln!("Error: {message}");
    }
}

/// Create the platform instance for use by the core app.
#[allow(dead_code)]
pub const fn create_platform() -> LinuxPlatform {
    LinuxPlatform::new()
}

/// Entry point: run the Linux application.
/// Currently a stub - the Linux backend is not yet implemented.
pub fn run() {
    eprintln!(
        "vibe-chat: Linux backend is not yet implemented.\n\
         The Windows version is fully functional. The Linux skeleton is in place\n\
         at src/platform/linux/ - ready for a raw X11/Wayland + Cairo implementation."
    );
    std::process::exit(1);
}
