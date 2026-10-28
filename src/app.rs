// Application state: projects, sessions, ACP coordination, settings, fonts, DPI.
// This module is platform-independent. Platform-specific resources (fonts)
// are stored as opaque handles managed by the platform backend.
#![allow(dead_code)]

use crate::acp::{AcpClient, AcpEvent};
use crate::chat::{ChatState, Role};
use crate::responses::{ResponsesClient, ResponsesEvent};
use crate::session::{Project, Session, SessionMode};
use crate::theme::Settings;
use crate::ui::{FocusedElement, RenameTarget, Toast, ToastKind};

/// Opaque handle to platform-specific font resources.
/// On Windows this wraps GDI HFONT handles; on other platforms it will
/// wrap the native font handles. The core app never inspects the contents.
pub struct FontsHandle {
    inner: Box<dyn std::any::Any + Send>,
}

impl FontsHandle {
    pub fn new<T: Send + 'static>(fonts: T) -> Self {
        Self {
            inner: Box::new(fonts),
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref()
    }
}

/// Duration (ms) for which a toast notification stays visible.
pub const TOAST_DURATION_MS: u64 = 4000;

/// A clickable rectangle with associated text (URL for links, code for copy buttons).
pub type ClickableRect = (i32, i32, i32, i32, String);

/// Cached result of `with_sidebar_layout()`. The layout only changes when the
/// session/project structure, DPI, window height, or archived-visibility
/// setting changes — none of which happen on mouse-move — so we cache it and
/// hand out borrows instead of rebuilding (and re-allocating) the entry
/// vector on every mouse event.
#[derive(Debug, Clone, Default)]
pub struct SidebarLayoutCache {
    /// Structure version this layout was computed at.
    pub version: u64,
    pub list_top: i32,
    pub total_height: i32,
    pub entries: Vec<crate::interaction::SidebarEntry>,
}

/// All transient hover state (which element the mouse is over).
// Many independent bools are deliberate here: each tracks a distinct
// hoverable widget, and an enum-per-widget state machine would obscure
// the "which element is hovered" lookup more than it would help.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HoverState {
    pub sidebar_item: Option<usize>,
    pub new_chat: bool,
    pub new_project: bool,
    pub project: Option<usize>,
    pub project_menu: Option<usize>,
    pub settings_btn: bool,
    pub settings_close: bool,
    pub reconnect: bool,
    pub thinking_toggle: Option<usize>,
    pub settings_toggle: Option<usize>,
    pub send_btn: bool,
    pub session_header: bool,
    pub session_archive: Option<usize>,
    pub session_delete: Option<usize>,
    pub scrollbar: bool,
    pub sidebar_scrollbar: bool,
    pub link: Option<usize>,
    pub code_copy: Option<usize>,
}

/// Shared application state stored in the window's user data.
// The handful of bools (show_settings, acp_connected, should_quit, ...) are
// independent flags on a UI-state bag, not a state machine; grouping them
// into enums would hurt readability.
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub active_session: usize,
    pub show_settings: bool,
    pub settings: Settings,
    pub fonts: Option<FontsHandle>,
    pub dpi: u32,
    pub acp: Option<AcpClient>,
    pub acp_connected: bool,
    /// Direct-mode (Responses API) client. Created once on first use.
    pub responses: Option<ResponsesClient>,
    /// Set when the UI requests a transport factory for Direct mode;
    /// the platform backend installs it on first Direct turn.
    pub responses_wanted: bool,
    /// Token count of the active session's Responses history, if known.
    pub input_tokens: Option<u64>,
    pub client_width: i32,
    pub client_height: i32,
    pub should_quit: bool,
    pub last_cursor_blink: std::time::Instant,
    pub cursor_visible: bool,
    pub caret_blink_ms: u32,
    pub hover: HoverState,
    pub pressed_element: Option<crate::ui::PressedElement>,
    pub focused_element: Option<FocusedElement>,
    pub sidebar_scroll_offset: i32,
    pub streaming_dots: u8,
    pub pending_new_session: bool,
    pub pending_working_dir: String,
    pub session_counter: usize,
    pub default_working_dir: String,
    pub toast: Option<Toast>,
    pub config_dir: String,
    pub rename_target: Option<RenameTarget>,
    pub rename_buffer: String,
    pub rename_cursor: usize,
    /// Clickable link rects collected during the last paint pass.
    /// Plain `Vec` (not `RefCell`): paint and hit-testing both run on the UI
    /// thread, so interior mutability is unnecessary and only adds a
    /// re-entrancy panic risk.
    pub link_rects: Vec<ClickableRect>,
    /// Clickable code-block copy-button rects collected during the last paint.
    pub code_block_rects: Vec<ClickableRect>,
    pub saved_window: Option<crate::persistence::WindowState>,
    pub reconnect_attempts: u32,
    pub last_reconnect_attempt: Option<std::time::Instant>,
    /// Set by interaction when the "+ New Project" button is clicked;
    /// the platform backend picks a folder and calls `create_project`.
    pub new_project_requested: bool,
    /// Set by interaction when a project's menu button is clicked;
    /// the platform backend shows the project context menu.
    pub project_menu_requested: Option<usize>,
    /// Cache of the last computed sidebar layout plus the structure version it
    /// was computed at. Invalidated (version mismatch) on any structural change.
    pub sidebar_cache: SidebarLayoutCache,
    /// Monotonic counter bumped whenever the sidebar structure may have changed
    /// (session/project add/remove, archive toggle, expand/collapse, DPI or
    /// window resize, show-archived setting change).
    pub sidebar_version: u64,
}

impl AppState {
    pub fn new(caret_blink_ms: u32) -> Self {
        let default_wd = std::env::current_dir()
            .map_or_else(|_| ".".to_string(), |p| p.to_string_lossy().to_string());
        Self {
            projects: Vec::new(),
            sessions: Vec::new(),
            active_session: 0,
            show_settings: false,
            settings: Settings::default(),
            fonts: None,
            dpi: 96,
            acp: None,
            acp_connected: false,
            responses: None,
            responses_wanted: false,
            input_tokens: None,
            client_width: 1100,
            client_height: 700,
            should_quit: false,
            last_cursor_blink: std::time::Instant::now(),
            cursor_visible: true,
            caret_blink_ms: if caret_blink_ms == 0 {
                500
            } else {
                caret_blink_ms
            },
            hover: HoverState::default(),
            pressed_element: None,
            focused_element: Some(FocusedElement::Input),
            sidebar_scroll_offset: 0,
            streaming_dots: 0,
            pending_new_session: false,
            pending_working_dir: String::new(),
            session_counter: 0,
            default_working_dir: default_wd,
            toast: None,
            config_dir: crate::persistence::default_config_dir(),
            rename_target: None,
            rename_buffer: String::new(),
            rename_cursor: 0,
            link_rects: Vec::new(),
            code_block_rects: Vec::new(),
            saved_window: None,
            reconnect_attempts: 0,
            last_reconnect_attempt: None,
            new_project_requested: false,
            project_menu_requested: None,
            sidebar_cache: SidebarLayoutCache::default(),
            sidebar_version: 0,
        }
    }

    /// Invalidate the cached sidebar layout. Call after any change that can
    /// affect the sidebar's structure or geometry.
    pub const fn invalidate_sidebar_cache(&mut self) {
        self.sidebar_version = self.sidebar_version.wrapping_add(1);
    }

    /// Toggle a settings switch by index (0=dark, 1=auto-scroll, 2=auto-approve,
    /// 3=show-archived). Shared by mouse and keyboard activation paths.
    pub fn toggle_setting(&mut self, idx: usize) {
        match idx {
            0 => self.settings.dark_theme = !self.settings.dark_theme,
            1 => self.settings.auto_scroll = !self.settings.auto_scroll,
            2 => self.settings.auto_approve = !self.settings.auto_approve,
            3 => {
                self.settings.show_archived = !self.settings.show_archived;
                self.invalidate_sidebar_cache();
            }
            _ => return,
        }
        self.save_index();
    }

    pub fn active_chat(&mut self) -> Option<&mut ChatState> {
        if self.sessions.is_empty() {
            return None;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        Some(&mut self.sessions[idx].chat)
    }

    /// Left edge of the chat area (right edge of the sidebar), DPI-scaled.
    pub fn chat_left(&self) -> i32 {
        (crate::theme::SIDEBAR_WIDTH as f32 * self.dpi as f32 / 96.0) as i32
    }

    /// Right edge of the chat area - shrinks when the settings panel is open.
    pub fn chat_right(&self) -> i32 {
        if self.show_settings {
            let scale = self.dpi as f32 / 96.0;
            self.client_width - (crate::theme::SETTINGS_PANEL_WIDTH as f32 * scale) as i32
        } else {
            self.client_width
        }
    }

    // ── Project management ──────────────────────────────────────────────────

    pub fn ensure_project(&mut self, path: &str) -> usize {
        if let Some(idx) = self.projects.iter().position(|p| p.path == path) {
            return idx;
        }
        let project = Project::new(path);
        self.projects.push(project);
        self.projects.len() - 1
    }

    pub fn rebuild_project_indices(&mut self) {
        self.invalidate_sidebar_cache();
        for project in &mut self.projects {
            project.session_indices.clear();
        }
        // Build a path -> project-index map once so the per-session lookup is
        // O(1) instead of a linear scan (was O(projects * sessions)).
        let index_by_path: std::collections::HashMap<&str, usize> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| (p.path.as_str(), i))
            .collect();
        let mut pushes: Vec<(usize, usize)> = Vec::new();
        for (i, session) in self.sessions.iter().enumerate() {
            if let Some(&pidx) = index_by_path.get(session.working_dir.as_str()) {
                pushes.push((pidx, i));
            }
        }
        for (pidx, sidx) in pushes {
            self.projects[pidx].session_indices.push(sidx);
        }
    }

    pub fn project_for_session(&self, session_idx: usize) -> Option<usize> {
        if session_idx >= self.sessions.len() {
            return None;
        }
        let wd = &self.sessions[session_idx].working_dir;
        self.projects.iter().position(|p| p.path == *wd)
    }

    pub fn toggle_project(&mut self, idx: usize) {
        if let Some(project) = self.projects.get_mut(idx) {
            project.expanded = !project.expanded;
        }
        self.invalidate_sidebar_cache();
        self.save_index();
    }

    pub fn remove_project(&mut self, project_idx: usize) {
        if project_idx >= self.projects.len() {
            return;
        }
        let path = self.projects[project_idx].path.clone();
        let sessions_to_remove: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.working_dir == path)
            .map(|(i, _)| i)
            .collect();
        for &idx in sessions_to_remove.iter().rev() {
            self.sessions.remove(idx);
        }
        self.projects.remove(project_idx);
        if self.active_session >= self.sessions.len() && !self.sessions.is_empty() {
            self.active_session = self.sessions.len() - 1;
        } else if self.sessions.is_empty() {
            self.active_session = 0;
        }
        self.rebuild_project_indices();
        self.save_index();
    }

    // ── Session management ──────────────────────────────────────────────────

    pub fn create_local_session(&mut self, working_dir: &str) {
        self.session_counter += 1;
        let name = format!("Chat {}", self.session_counter);
        let session = Session::new("", &name, working_dir);
        self.sessions.push(session);
        self.active_session = self.sessions.len() - 1;
        self.pending_new_session = true;
        self.pending_working_dir = working_dir.to_string();
        self.ensure_project(working_dir);
        self.rebuild_project_indices();
        self.save_index();
    }

    pub fn create_local_session_default(&mut self) {
        let wd = self.default_working_dir.clone();
        self.create_local_session(&wd);
    }

    pub const fn switch_session(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.active_session = idx;
        }
    }

    /// Start inline rename for a session.
    pub fn start_rename(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.rename_target = Some(RenameTarget::Session(idx));
            self.rename_buffer = self.sessions[idx].name.clone();
            self.rename_cursor = self.rename_buffer.len();
        }
    }

    /// Start inline rename for a project.
    pub fn start_rename_project(&mut self, idx: usize) {
        if idx < self.projects.len() {
            self.rename_target = Some(RenameTarget::Project(idx));
            self.rename_buffer = self.projects[idx].name.clone();
            self.rename_cursor = self.rename_buffer.len();
        }
    }

    /// Commit the rename buffer to the target's name.
    pub fn commit_rename(&mut self) {
        if let Some(target) = self.rename_target.take() {
            let name = self.rename_buffer.trim().to_string();
            if !name.is_empty() {
                match target {
                    RenameTarget::Session(idx) => {
                        if idx < self.sessions.len() {
                            self.sessions[idx].name = name;
                        }
                    }
                    RenameTarget::Project(idx) => {
                        if idx < self.projects.len() {
                            self.projects[idx].name = name;
                        }
                    }
                }
                self.save_index();
            }
            self.rename_buffer.clear();
            self.rename_cursor = 0;
        }
    }

    /// Cancel the inline rename.
    pub fn cancel_rename(&mut self) {
        self.rename_target = None;
        self.rename_buffer.clear();
        self.rename_cursor = 0;
    }

    /// Insert a character into the rename buffer.
    pub fn rename_insert_char(&mut self, c: char) {
        if self.rename_target.is_some() && !c.is_control() {
            self.rename_buffer.insert(self.rename_cursor, c);
            self.rename_cursor += c.len_utf8();
        }
    }

    /// Backspace in the rename buffer.
    pub fn rename_backspace(&mut self) {
        if self.rename_target.is_some() && self.rename_cursor > 0 {
            let pos = crate::chat::prev_char_boundary(&self.rename_buffer, self.rename_cursor);
            self.rename_buffer.remove(pos);
            self.rename_cursor = pos;
        }
    }

    /// Move rename cursor left.
    pub fn rename_cursor_left(&mut self) {
        if self.rename_target.is_some() && self.rename_cursor > 0 {
            self.rename_cursor =
                crate::chat::prev_char_boundary(&self.rename_buffer, self.rename_cursor);
        }
    }

    /// Move rename cursor right.
    pub fn rename_cursor_right(&mut self) {
        if self.rename_target.is_some() && self.rename_cursor < self.rename_buffer.len() {
            self.rename_cursor =
                crate::chat::next_char_boundary(&self.rename_buffer, self.rename_cursor);
        }
    }

    /// Toggle the archived state of a session.
    pub fn toggle_archive_session(&mut self, idx: usize) {
        if let Some(session) = self.sessions.get_mut(idx) {
            session.archived = !session.archived;
            if session.archived && self.active_session == idx {
                self.active_session = self.nearest_visible_session(idx);
            }
        }
        self.invalidate_sidebar_cache();
        self.save_index();
    }

    /// Permanently delete a session. Returns true if a session was removed.
    pub fn delete_session(&mut self, idx: usize) -> bool {
        if idx >= self.sessions.len() {
            return false;
        }
        if self.sessions[idx].chat.is_waiting_for_agent {
            return false;
        }
        let local_id = self.sessions[idx].local_id.clone();
        self.sessions.remove(idx);
        self.delete_session_file(&local_id);
        self.rebuild_project_indices();
        if self.sessions.is_empty() {
            self.active_session = 0;
            self.create_local_session_default();
        } else if self.active_session >= self.sessions.len() {
            self.active_session = self.sessions.len() - 1;
        } else if self.active_session > idx {
            self.active_session -= 1;
        }
        self.save_index();
        true
    }

    /// Find the nearest non-archived session to the given index.
    fn nearest_visible_session(&self, from_idx: usize) -> usize {
        let show_archived = self.settings.show_archived;
        for dist in 0..self.sessions.len() {
            if from_idx + dist < self.sessions.len() {
                let s = &self.sessions[from_idx + dist];
                if !s.archived || show_archived {
                    return from_idx + dist;
                }
            }
            if dist > 0 && from_idx >= dist {
                let s = &self.sessions[from_idx - dist];
                if !s.archived || show_archived {
                    return from_idx - dist;
                }
            }
        }
        from_idx.min(self.sessions.len().saturating_sub(1))
    }

    /// Get indices of sessions visible in the sidebar (respects `show_archived` setting).
    pub fn visible_session_indices(&self) -> Vec<usize> {
        if self.settings.show_archived {
            (0..self.sessions.len()).collect()
        } else {
            self.sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.archived)
                .map(|(i, _)| i)
                .collect()
        }
    }

    /// Count archived sessions.
    pub fn archived_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.archived).count()
    }

    /// Poll ACP events and update chat state.
    // One match arm per AcpEvent variant keeps the dispatch table readable.
    #[allow(clippy::too_many_lines)]
    pub fn poll_acp(&mut self) {
        let events: Vec<AcpEvent> = {
            let mut collected = Vec::new();
            if let Some(ref acp) = self.acp {
                while let Some(event) = acp.try_recv() {
                    collected.push(event);
                }
            }
            collected
        };

        for event in events {
            match event {
                AcpEvent::Connected => {
                    self.acp_connected = true;
                    self.reset_reconnect();
                    if self.sessions.is_empty() {
                        self.create_local_session_default();
                    }
                    let wd = if self.pending_new_session && !self.pending_working_dir.is_empty() {
                        self.pending_working_dir.clone()
                    } else {
                        self.default_working_dir.clone()
                    };
                    if let Some(ref acp) = self.acp {
                        acp.request_session(&wd);
                    }
                    self.show_toast("Connected to agent", ToastKind::Success);
                }
                AcpEvent::SessionReady { session_id } => {
                    if self.pending_new_session && !self.sessions.is_empty() {
                        let idx = self.sessions.len() - 1;
                        self.sessions[idx].acp_session_id.clone_from(&session_id);
                        self.pending_new_session = false;
                        self.pending_working_dir.clear();
                        self.add_system_message_to_active(
                            "Session ready. Type a message to begin.",
                        );
                        self.save_index();
                        self.save_session(idx);
                    } else {
                        self.session_counter += 1;
                        let name = format!("Chat {}", self.session_counter);
                        let wd = self.default_working_dir.clone();
                        let mut session = Session::new(&session_id, &name, &wd);
                        session
                            .chat
                            .add_system_message("Session ready. Type a message to begin.");
                        self.sessions.push(session);
                        self.active_session = self.sessions.len() - 1;
                        self.ensure_project(&wd);
                        self.rebuild_project_indices();
                        self.save_index();
                        self.save_session(self.active_session);
                    }
                }
                AcpEvent::StreamChunk { session_id, text } => {
                    if let Some(idx) = self.find_session_by_acp_id(&session_id) {
                        if self.sessions[idx].chat.streaming_message_idx.is_none() {
                            self.sessions[idx].chat.start_streaming();
                        }
                        self.sessions[idx].chat.append_stream_chunk(&text);
                    }
                }
                AcpEvent::ThoughtChunk { session_id, text } => {
                    if let Some(idx) = self.find_session_by_acp_id(&session_id) {
                        if self.sessions[idx].chat.streaming_message_idx.is_none() {
                            self.sessions[idx].chat.start_streaming();
                        }
                        self.sessions[idx].chat.append_thought_chunk(&text);
                    }
                }
                AcpEvent::TurnComplete {
                    session_id,
                    stop_reason: _,
                } => {
                    if let Some(idx) = self.find_session_by_acp_id(&session_id) {
                        self.sessions[idx].chat.finish_streaming();
                        self.save_session(idx);
                    } else if let Some(chat) = self.active_chat() {
                        chat.finish_streaming();
                        let idx = self
                            .active_session
                            .min(self.sessions.len().saturating_sub(1));
                        self.save_session(idx);
                    }
                }
                AcpEvent::ToolCall {
                    session_id,
                    name,
                    input,
                } => {
                    let msg = format!("[Tool: {name}] {}", truncate_cow(&input, 200));
                    self.add_system_message_to_session(&session_id, &msg);
                    if let Some(idx) = self.find_session_by_acp_id(&session_id) {
                        self.save_session(idx);
                    }
                }
                AcpEvent::ToolResult {
                    session_id,
                    call_id: _,
                    output,
                } => {
                    let msg = format!("[Result] {}", truncate_cow(&output, 200));
                    self.add_system_message_to_session(&session_id, &msg);
                    if let Some(idx) = self.find_session_by_acp_id(&session_id) {
                        self.save_session(idx);
                    }
                }
                AcpEvent::PermissionRequest {
                    id,
                    session_id,
                    method,
                    params: _,
                } => {
                    if self.settings.auto_approve {
                        let msg = format!("[Permission: {method} (id={id}) - auto-approved]");
                        self.add_system_message_to_session(&session_id, &msg);
                        if let Some(ref acp) = self.acp {
                            acp.respond_permission(id, true);
                        }
                    } else {
                        let msg = format!(
                            "[Permission requested: {method} (id={id}) - auto-approve is OFF]"
                        );
                        self.add_system_message_to_session(&session_id, &msg);
                    }
                    if let Some(idx) = self.find_session_by_acp_id(&session_id) {
                        self.save_session(idx);
                    }
                }
                AcpEvent::ProcessDied { code } => {
                    self.acp_connected = false;
                    let msg = code.map_or_else(
                        || "Agent process died".to_string(),
                        |c| format!("Agent process exited (code {c})"),
                    );
                    self.show_toast(&msg, ToastKind::Error);
                    // Auto-reconnect: schedule a retry
                    self.reconnect_attempts += 1;
                    self.last_reconnect_attempt = Some(std::time::Instant::now());
                }
                AcpEvent::Error(msg) => {
                    self.show_toast(&format!("Error: {msg}"), ToastKind::Error);
                }
            }
        }
    }

    fn find_session_by_acp_id(&self, acp_id: &str) -> Option<usize> {
        self.sessions
            .iter()
            .position(|s| s.acp_session_id == acp_id)
    }

    fn add_system_message_to_active(&mut self, text: &str) {
        if !self.sessions.is_empty() {
            let idx = self.active_session.min(self.sessions.len() - 1);
            self.sessions[idx].chat.add_system_message(text);
        }
    }

    /// Show a transient toast (replaces any previous one) in the top-right corner.
    fn show_toast(&mut self, text: &str, kind: ToastKind) {
        self.toast = Some(Toast {
            text: text.to_string(),
            kind,
            shown_at: std::time::Instant::now(),
            duration_ms: TOAST_DURATION_MS,
        });
    }

    fn add_system_message_to_session(&mut self, acp_session_id: &str, text: &str) {
        if let Some(idx) = self.find_session_by_acp_id(acp_session_id) {
            self.sessions[idx].chat.add_system_message(text);
        } else {
            self.add_system_message_to_active(text);
        }
    }

    pub fn send_message(&mut self, text: &str) {
        if self.sessions.is_empty() {
            return;
        }

        let idx = self.active_session.min(self.sessions.len() - 1);

        let mode;
        let acp_session_id;
        {
            let session = &mut self.sessions[idx];
            session.chat.add_message(Role::User, text);
            session.auto_name();
            mode = session.mode;
            acp_session_id = session.acp_session_id.clone();
        }
        self.save_session(idx);
        self.save_index();

        match mode {
            SessionMode::Direct => self.send_direct(idx),
            SessionMode::Code => {
                if let Some(ref acp) = self.acp {
                    if self.acp_connected && !acp_session_id.is_empty() {
                        self.sessions[idx].chat.start_streaming();
                        acp.send_prompt(&acp_session_id, text);
                    } else if self.acp_connected && acp_session_id.is_empty() {
                        // Lazy agent spawn: connect now, queue the session.
                        let wd = self.sessions[idx].working_dir.clone();
                        self.pending_new_session = true;
                        self.pending_working_dir = wd;
                        self.sessions[idx].chat.start_streaming();
                        // The queued prompt is lost this turn; tell the user.
                        self.show_toast(
                            "Connecting to agent - send again when ready",
                            ToastKind::Info,
                        );
                    } else {
                        self.show_toast(
                            "Not connected to agent. Press F5 to reconnect",
                            ToastKind::Error,
                        );
                    }
                } else {
                    self.connect_acp();
                    self.show_toast("Starting agent - send again when ready", ToastKind::Info);
                }
            }
        }
    }

    /// Send the next Direct-mode turn for session `idx` via the Responses API.
    fn send_direct(&mut self, idx: usize) {
        if self.responses.is_none() {
            self.responses_wanted = true;
            self.show_toast(
                "Direct mode needs an HTTP transport - not available on this platform",
                ToastKind::Error,
            );
            return;
        }
        let history = self.sessions[idx].responses_history();
        let session_id = self.sessions[idx].local_id.clone();
        self.sessions[idx].chat.start_streaming();
        if let Some(ref client) = self.responses {
            client.send(&session_id, history, crate::session::ReasoningEffort::High);
        }
    }

    /// Install the Direct-mode transport (called by the platform backend on
    /// startup). Idempotent.
    pub fn install_responses_transport(
        &mut self,
        transport: Box<dyn crate::responses::HttpTransport + Send>,
    ) {
        if self.responses.is_none() {
            self.responses = Some(ResponsesClient::new(transport));
        }
        self.responses_wanted = false;
    }

    /// Drain pending Responses API events into the UI state. Called from the
    /// platform timer tick alongside poll_acp.
    pub fn poll_responses(&mut self) {
        let events: Vec<ResponsesEvent> = {
            match self.responses {
                Some(ref client) => client.try_recv().into_iter().collect(),
                None => return,
            }
        };
        for event in events {
            let local_id = match &event {
                ResponsesEvent::TextDelta { session_id, .. }
                | ResponsesEvent::ReasoningDelta { session_id, .. }
                | ResponsesEvent::Completed { session_id }
                | ResponsesEvent::Failed { session_id, .. }
                | ResponsesEvent::Incomplete { session_id }
                | ResponsesEvent::InputTokens { session_id, .. } => session_id.clone(),
            };
            let idx = self.sessions.iter().position(|s| s.local_id == local_id);
            let idx = match idx {
                Some(i) => i,
                None => continue,
            };
            match event {
                ResponsesEvent::TextDelta { text, .. } => {
                    self.sessions[idx].chat.append_stream_chunk(&text);
                }
                ResponsesEvent::ReasoningDelta { text, .. } => {
                    self.sessions[idx].chat.append_thought_chunk(&text);
                }
                ResponsesEvent::Completed { .. } => {
                    self.sessions[idx].chat.finish_streaming();
                    let history = self.sessions[idx].responses_history();
                    if let Some(ref client) = self.responses {
                        client.count_input_tokens(&local_id, history);
                    }
                }
                ResponsesEvent::Failed { message, .. } => {
                    self.sessions[idx].chat.finish_streaming();
                    // Persist the failure in the conversation: a 4-second
                    // toast is too easy to miss when debugging.
                    self.sessions[idx]
                        .chat
                        .add_system_message(&format!("Direct mode error: {}", message));
                    self.show_toast(&format!("Direct mode error: {}", message), ToastKind::Error);
                    self.save_session(idx);
                }
                ResponsesEvent::Incomplete { .. } => {
                    self.sessions[idx].chat.finish_streaming();
                    self.show_toast("Response stopped early (token limit)", ToastKind::Info);
                }
                ResponsesEvent::InputTokens { tokens, .. } => {
                    self.input_tokens = Some(tokens);
                }
            }
        }
    }

    /// Switch the active session's mode (radio control in the header).
    pub fn set_session_mode(&mut self, mode: SessionMode) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        if self.sessions[idx].mode != mode {
            self.sessions[idx].mode = mode;
            self.save_index();
            if mode == SessionMode::Code && self.acp.is_none() {
                // Lazy spawn: connect the agent on first Code-mode use.
                self.connect_acp();
            }
        }
    }

    pub fn connect_acp(&mut self) {
        if let Some(acp) = self.acp.take() {
            acp.shutdown();
        }

        match AcpClient::spawn() {
            Ok(client) => {
                self.acp = Some(client);
                self.show_toast("Connecting to agent...", ToastKind::Info);
            }
            Err(e) => {
                self.show_toast(&format!("Failed to start agent: {e}"), ToastKind::Error);
            }
        }
    }

    pub fn cancel_turn(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        let acp_session_id = self.sessions[idx].acp_session_id.clone();
        let local_id = self.sessions[idx].local_id.clone();
        let mode = self.sessions[idx].mode;
        if let Some(ref acp) = self.acp {
            if mode == SessionMode::Code && !acp_session_id.is_empty() {
                acp.cancel(&acp_session_id);
            }
        }
        if mode == SessionMode::Direct {
            if let Some(ref client) = self.responses {
                client.abort(&local_id);
            }
        }
        self.sessions[idx].chat.finish_streaming();
    }

    /// Create a new chat in the default working directory.
    pub fn new_chat(&mut self) {
        let wd = self.default_working_dir.clone();
        self.create_local_session(&wd);
        if self.acp_connected {
            if let Some(ref acp) = self.acp {
                acp.request_session(&wd);
            }
        }
    }

    /// Create a new chat in a specific working directory (from folder picker).
    pub fn new_chat_in_dir(&mut self, working_dir: &str) {
        self.create_local_session(working_dir);
        if self.acp_connected {
            if let Some(ref acp) = self.acp {
                acp.request_session(working_dir);
            }
        }
    }

    // ── Persistence ────────────────────────────────────────────────────────

    /// Save the state index (settings, projects, session metadata) to disk.
    /// Failures surface as a toast — a silent save loss must never look like success.
    pub fn save_index(&mut self) {
        let window = self.saved_window_state();
        if let Err(e) = crate::persistence::save_index(
            &self.config_dir,
            &self.settings,
            &self.projects,
            &self.sessions,
            window.as_ref(),
        ) {
            self.show_toast(&format!("Save failed: {e}"), ToastKind::Error);
        }
    }

    /// Save a single session's message history to disk.
    pub fn save_session(&mut self, idx: usize) {
        let result = self
            .sessions
            .get(idx)
            .map(|session| crate::persistence::save_session(&self.config_dir, session));
        if let Some(Err(e)) = result {
            self.show_toast(&format!("Save failed: {e}"), ToastKind::Error);
        }
    }

    /// Save the state index and all session files to disk.
    pub fn save_all(&mut self) {
        let window = self.saved_window_state();
        if let Err(e) = crate::persistence::save_all(
            &self.config_dir,
            &self.settings,
            &self.projects,
            &self.sessions,
            window.as_ref(),
        ) {
            self.show_toast(&format!("Save failed: {e}"), ToastKind::Error);
        }
    }

    /// Load persisted state from disk. Returns true if state was loaded.
    pub fn load_state(&mut self) -> bool {
        self.invalidate_sidebar_cache();
        if let Some(loaded) = crate::persistence::load_all(&self.config_dir) {
            self.settings = loaded.settings;
            self.projects = loaded.projects;
            self.sessions = loaded.sessions;
            self.active_session = 0;
            self.session_counter = self
                .sessions
                .iter()
                .map(|s| {
                    s.name
                        .strip_prefix("Chat ")
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            self.rebuild_project_indices();
            if let Some(w) = loaded.window {
                self.saved_window = Some(w);
            }
            true
        } else {
            false
        }
    }

    fn saved_window_state(&self) -> Option<crate::persistence::WindowState> {
        self.saved_window.clone()
    }

    /// Delete a session's file from disk.
    pub fn delete_session_file(&self, local_id: &str) {
        crate::persistence::delete_session_file(&self.config_dir, local_id);
    }

    /// Check if an auto-reconnect should be attempted. Called from the timer tick.
    /// Returns true if a reconnect was initiated.
    pub fn try_auto_reconnect(&mut self) -> bool {
        const MAX_AUTO_RECONNECTS: u32 = 5;
        const RECONNECT_DELAY_SECS: u64 = 3;

        if self.reconnect_attempts == 0 || self.reconnect_attempts > MAX_AUTO_RECONNECTS {
            return false;
        }
        if self.acp_connected {
            self.reconnect_attempts = 0;
            return false;
        }
        let should_retry = self
            .last_reconnect_attempt
            .is_some_and(|t| t.elapsed().as_secs() >= RECONNECT_DELAY_SECS);
        if should_retry {
            self.show_toast(
                &format!("Reconnecting... (attempt {})", self.reconnect_attempts),
                ToastKind::Info,
            );
            self.connect_acp();
            return true;
        }
        false
    }

    /// Reset reconnect counter (call on successful connection).
    pub const fn reset_reconnect(&mut self) {
        self.reconnect_attempts = 0;
        self.last_reconnect_attempt = None;
    }
}

pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = crate::chat::prev_char_boundary(s, max_len + 1);
        format!("{}...", &s[..end])
    }
}

/// Borrowing variant of `truncate_str`: returns the input slice unchanged
/// when it is short enough, avoiding a full copy for the common case.
pub fn truncate_cow<'a>(s: &'a str, max_len: usize) -> std::borrow::Cow<'a, str> {
    if s.len() <= max_len {
        return s.into();
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end]).into()
}
