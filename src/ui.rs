// UI types: element interaction state enums and toast notifications.
// These types are platform-independent and shared between the core app
// and all platform backends.
// The PaintContext (rendering data bundle) is platform-specific and lives
// in each backend (e.g., platform/windows/paint.rs).
#![allow(dead_code)]

use crate::session::SessionMode;

/// Identifies which element is currently pressed (mouse down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressedElement {
    NewChat,
    NewProject,
    ProjectHeader(usize),
    ProjectMenu(usize),
    RemoveProject(usize),
    SettingsBtn,
    SettingsClose,
    SettingsToggle(usize),
    Reconnect,
    SidebarItem(usize),
    SendButton,
    SessionArchive(usize),
    SessionDelete(usize),
    ModeRadio(SessionMode),
}

/// Identifies which element has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedElement {
    NewChat,
    NewProject,
    ProjectHeader(usize),
    SidebarItem(usize),
    SettingsBtn,
    Input,
    SendButton,
    SettingsClose,
    SettingsToggle(usize),
    Reconnect,
}

/// Identifies which sidebar element is being inline-renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameTarget {
    Session(usize),
    Project(usize),
}

/// Transient toast notification (e.g. agent connection status).
#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub shown_at: std::time::Instant,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}
