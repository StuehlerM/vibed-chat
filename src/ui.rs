// UI types: element interaction state enums and toast notifications.
// These types are platform-independent and shared between the core app
// and all platform backends.
// The PaintContext (rendering data bundle) is platform-specific and lives
// in each backend (e.g., platform/windows/paint.rs).
#![allow(dead_code)]

/// Identifies which element is currently pressed (mouse down).
#[derive(Debug, Clone, Copy, PartialEq)]
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
}

/// Identifies which element has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Transient toast notification (e.g. agent connection status).
#[derive(Clone)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub shown_at: std::time::Instant,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}
