// Application state: projects, sessions, ACP coordination, settings, fonts, DPI.
// This module is platform-independent. Platform-specific resources (fonts)
// are stored as opaque handles managed by the platform backend.
#![allow(dead_code)]

use crate::acp::{AcpClient, AcpEvent};
use crate::chat::{ChatState, Role};
use crate::responses::{ResponsesClient, ResponsesEvent};
use crate::session::{ActiveTurn, Project, Session, SessionMode, TurnKey};
use crate::theme::Settings;
use crate::ui::{FocusedElement, RenameTarget, Toast, ToastKind};

/// Opaque handle to platform-specific font resources.
/// On Windows this wraps GDI HFONT handles; on other platforms it will
/// wrap the native font handles. The core app never inspects the contents.
#[derive(Clone)]
pub struct FontsHandle {
    inner: std::rc::Rc<dyn std::any::Any>,
}

impl FontsHandle {
    pub fn new<T: 'static>(fonts: T) -> Self {
        Self {
            inner: std::rc::Rc::new(fonts),
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

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub id: crate::json::Json,
    pub key: TurnKey,
    pub title: String,
    pub details: String,
    pub options: Vec<(String, String)>,
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
    pub acp_connecting: bool,
    pub chat_key_configured: bool,
    pub configure_agent_process: fn(&mut std::process::Command),
    pub pending_permissions: std::collections::VecDeque<PendingPermission>,
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
    pub pointer_press: Option<crate::interaction::PointerPress>,
    pub last_pointer_session: Option<String>,
    pub focused_element: Option<FocusedElement>,
    pub sidebar_scroll_offset: i32,
    pub streaming_dots: u8,
    pub session_counter: usize,
    next_turn_id: u64,
    pub default_working_dir: String,
    pub toast: Option<Toast>,
    pub config_dir: String,
    /// Durable warning, visible until a successful save (or manual recovery).
    pub persistence_error: Option<String>,
    /// Never overwrite damaged/future-version storage with a new empty state.
    pub persistence_blocked: bool,
    pub persistence_pending: bool,
    persistence_worker: Option<crate::persistence::PersistenceWorker>,
    save_revision: u64,
    pending_deletions: Vec<String>,
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
    /// Modal platform effects block further interactive commands, not timer/paint.
    pub modal_active: bool,
    pub window_focused: bool,
    pub pending_high_surrogate: Option<u16>,
    pub consumed_character: Option<u32>,
    pub ime_composing: bool,
    pub wheel_remainder: i32,
    pub repaint_needed: bool,
    pub window_minimized: bool,
    pub draft_save_due: Option<std::time::Instant>,
    pub last_checkpoint: std::time::Instant,
    /// Set by interaction when a project's menu button is clicked;
    /// the platform backend shows the project context menu.
    pub project_menu_requested: Option<usize>,
    pub mode_change_requested: Option<(String, SessionMode)>,
    pub setup_help_requested: Option<SessionMode>,
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
            acp_connecting: false,
            chat_key_configured: crate::responses::credentials_configured(),
            configure_agent_process: |_| {},
            pending_permissions: std::collections::VecDeque::new(),
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
            pointer_press: None,
            last_pointer_session: None,
            focused_element: Some(FocusedElement::Input),
            sidebar_scroll_offset: 0,
            streaming_dots: 0,
            session_counter: 0,
            next_turn_id: 1,
            default_working_dir: default_wd,
            toast: None,
            config_dir: crate::persistence::default_config_dir(),
            persistence_error: None,
            persistence_blocked: false,
            persistence_pending: false,
            persistence_worker: None,
            save_revision: 0,
            pending_deletions: Vec::new(),
            rename_target: None,
            rename_buffer: String::new(),
            rename_cursor: 0,
            link_rects: Vec::new(),
            code_block_rects: Vec::new(),
            saved_window: None,
            reconnect_attempts: 0,
            last_reconnect_attempt: None,
            new_project_requested: false,
            modal_active: false,
            window_focused: true,
            pending_high_surrogate: None,
            consumed_character: None,
            ime_composing: false,
            wheel_remainder: 0,
            repaint_needed: true,
            window_minimized: false,
            draft_save_due: None,
            last_checkpoint: std::time::Instant::now(),
            project_menu_requested: None,
            mode_change_requested: None,
            setup_help_requested: None,
            sidebar_cache: SidebarLayoutCache::default(),
            sidebar_version: 0,
        }
    }

    /// Invalidate the cached sidebar layout. Call after any change that can
    /// affect the sidebar's structure or geometry.
    pub fn invalidate_sidebar_cache(&mut self) {
        self.cancel_pointer_press();
        self.last_pointer_session = None;
        self.link_rects.clear();
        self.code_block_rects.clear();
        self.sidebar_version = self.sidebar_version.wrapping_add(1);
    }

    /// Toggle a settings switch by index (0=dark, 1=auto-scroll, 2=auto-approve,
    /// 3=show-archived). Shared by mouse and keyboard activation paths.
    pub fn toggle_setting(&mut self, idx: usize) {
        match idx {
            0 => self.settings.dark_theme = !self.settings.dark_theme,
            1 => self.settings.auto_scroll = !self.settings.auto_scroll,
            2 => {
                self.settings.auto_approve = false;
                self.show_toast(
                    "Tool permissions always require an explicit choice",
                    ToastKind::Info,
                );
                return;
            }
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
        let sessions = &self.sessions;
        for project in &mut self.projects {
            project
                .session_indices
                .sort_by_key(|&idx| std::cmp::Reverse(sessions[idx].last_activity));
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
        let ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| s.working_dir == path)
            .map(|s| s.local_id.clone())
            .collect();
        self.remove_sessions(&ids, Some(&path));
    }

    // ── Session management ──────────────────────────────────────────────────

    fn allocate_session(&self, name: &str, working_dir: &str) -> Result<Session, String> {
        for _ in 0..64 {
            let session = Session::new("", name, working_dir);
            if !self
                .sessions
                .iter()
                .any(|existing| existing.local_id == session.local_id)
                && !crate::persistence::session_id_in_storage(&self.config_dir, &session.local_id)?
            {
                return Ok(session);
            }
        }
        Err("Could not allocate a unique session identity".to_string())
    }

    pub fn create_local_session(&mut self, working_dir: &str) {
        self.session_counter += 1;
        let name = format!("Chat {}", self.session_counter);
        let session = match self.allocate_session(&name, working_dir) {
            Ok(session) => session,
            Err(error) => {
                self.show_toast(&error, ToastKind::Error);
                return;
            }
        };
        self.sessions.push(session);
        self.active_session = self.sessions.len() - 1;
        self.ensure_project(working_dir);
        self.rebuild_project_indices();
        self.save_index();
    }

    pub fn create_local_session_default(&mut self) {
        let wd = self.default_working_dir.clone();
        self.create_local_session(&wd);
    }

    pub fn switch_session(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.cancel_pointer_press();
            self.last_pointer_session = None;
            self.active_session = idx;
            self.cancel_rename();
            self.link_rects.clear();
            self.code_block_rects.clear();
            self.hover = HoverState::default();
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
                            self.sessions[idx].title_is_automatic = false;
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
        if !self.settings.show_archived && self.sessions.iter().all(|s| s.archived) {
            self.settings.show_archived = true;
            self.show_toast(
                "All chats are archived; showing archived chats",
                ToastKind::Info,
            );
        }
        self.invalidate_sidebar_cache();
        self.save_index();
    }

    /// Permanently delete a session. Returns true if a session was removed.
    pub fn delete_session(&mut self, idx: usize) -> bool {
        let Some(session) = self.sessions.get(idx) else {
            return false;
        };
        let id = session.local_id.clone();
        self.remove_sessions(std::slice::from_ref(&id), None)
    }

    /// One removal path for individual chats and project groups. Refuse active
    /// work explicitly; stable IDs preserve the selected surviving chat.
    fn remove_sessions(&mut self, ids: &[String], project: Option<&str>) -> bool {
        if self.persistence_blocked {
            self.show_toast(
                "Cannot delete while storage is in read-only recovery mode",
                ToastKind::Error,
            );
            return false;
        }
        if self
            .sessions
            .iter()
            .any(|s| ids.contains(&s.local_id) && s.chat.is_waiting_for_agent)
        {
            self.show_toast(
                "Stop the active response before deleting its chat or project",
                ToastKind::Error,
            );
            return false;
        }
        let replacement = if self.sessions.iter().all(|s| ids.contains(&s.local_id)) {
            match self.allocate_session("New Chat", &self.default_working_dir) {
                Ok(session) => Some(session),
                Err(error) => {
                    self.show_toast(&error, ToastKind::Error);
                    return false;
                }
            }
        } else {
            None
        };
        let selected = self
            .sessions
            .get(self.active_session)
            .map(|s| s.local_id.clone());
        self.sessions.retain(|s| !ids.contains(&s.local_id));
        if let Some(path) = project {
            self.projects.retain(|p| p.path != path);
        }
        if let Some(session) = replacement {
            let wd = session.working_dir.clone();
            self.sessions.push(session);
            self.ensure_project(&wd);
        }
        self.active_session = selected
            .and_then(|id| self.sessions.iter().position(|s| s.local_id == id))
            .unwrap_or(0);
        self.cancel_rename();
        self.hover = HoverState::default();
        self.pressed_element = None;
        self.focused_element = Some(FocusedElement::Input);
        self.project_menu_requested = None;
        self.link_rects.clear();
        self.code_block_rects.clear();
        self.rebuild_project_indices();
        // Publish the removal before erasing any history. A save failure keeps
        // all recovery material, even though the in-memory removal is unsaved.
        for id in ids {
            if !self.pending_deletions.contains(id) {
                self.pending_deletions.push(id.clone());
            }
        }
        self.save_all();
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
        // Bounded channel and per-frame budget: bursts cannot monopolize the UI.
        let started = std::time::Instant::now();
        for _ in 0..256 {
            if started.elapsed().as_millis() >= 4 {
                break;
            }
            let event = self.acp.as_ref().and_then(AcpClient::try_recv);
            if event.is_some() {
                self.repaint_needed = true;
            }
            let Some(event) = event else {
                break;
            };
            match event {
                AcpEvent::Connected => {
                    self.acp_connected = true;
                    self.acp_connecting = false;
                    self.reset_reconnect();
                    self.advance_code_turns();
                    self.show_toast("Connected to agent", ToastKind::Success);
                }
                AcpEvent::SessionReady { key, session_id } => {
                    if let Some(idx) = self.turn_index(&key) {
                        self.sessions[idx].acp_session_id = session_id;
                        self.advance_code_turns();
                    }
                }
                AcpEvent::StreamChunk { key, text } => {
                    if let Some(idx) = self.turn_index(&key) {
                        self.sessions[idx].chat.append_stream_chunk(&text);
                    }
                }
                AcpEvent::ThoughtChunk { key, text } => {
                    if let Some(idx) = self.turn_index(&key) {
                        self.sessions[idx].chat.append_thought_chunk(&text);
                    }
                }
                AcpEvent::TurnComplete { key, stop_reason } => self.settle_turn(&key, &stop_reason),
                AcpEvent::RequestFailed { key, message } => {
                    self.settle_turn(&key, &format!("Failed: {message}"))
                }
                AcpEvent::ToolCall {
                    key,
                    name,
                    input,
                    payload,
                } => {
                    self.record_tool_activity(
                        &key,
                        format!("[Tool: {name}] {input}"),
                        payload,
                        false,
                    );
                }
                AcpEvent::ToolResult {
                    key,
                    call_id,
                    output,
                    payload,
                } => {
                    self.record_tool_activity(
                        &key,
                        format!("[Result {call_id}] {output}"),
                        payload,
                        true,
                    );
                }
                AcpEvent::PermissionRequest {
                    id,
                    key,
                    method,
                    params,
                } => {
                    let options = params
                        .get("options")
                        .and_then(crate::json::Json::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    let id = item.get("optionId")?.as_str()?;
                                    let kind = item.get("kind")?.as_str()?;
                                    if !matches!(
                                        kind,
                                        "allow_once"
                                            | "allow_always"
                                            | "reject_once"
                                            | "reject_always"
                                    ) || id.is_empty()
                                    {
                                        return None;
                                    }
                                    Some((
                                        id.to_string(),
                                        format!("{} ({kind})", item.get("name")?.as_str()?),
                                    ))
                                })
                                .take(32)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if self.turn_index(&key).is_some()
                        && !options.is_empty()
                        && self.pending_permissions.len() < 32
                    {
                        self.pending_permissions.push_back(PendingPermission {
                            id,
                            key: key.clone(),
                            title: format!("{} — {method}", key.local_id),
                            details: params
                                .get("toolCall")
                                .map(ToString::to_string)
                                .unwrap_or_default(),
                            options,
                        });
                    } else if let Some(acp) = &self.acp {
                        if let Err(e) = acp.respond_permission(id, None) {
                            self.connection_failed(&e);
                        }
                    }
                }
                AcpEvent::ProcessDied { code } => {
                    self.connection_failed(&format!("Agent disconnected ({code:?})"));
                    break;
                }
                AcpEvent::Error(message) => {
                    self.connection_failed(&message);
                    break;
                }
            }
        }
    }

    pub fn turn_index(&self, key: &TurnKey) -> Option<usize> {
        self.sessions.iter().position(|s| {
            s.local_id == key.local_id
                && s.active_turn.as_ref().is_some_and(|turn| turn.key == *key)
        })
    }

    fn delta_turn_index(&self, key: &TurnKey) -> Option<usize> {
        self.turn_index(key).filter(|&idx| {
            self.sessions[idx]
                .active_turn
                .as_ref()
                .is_some_and(|t| !t.cancelling)
        })
    }

    fn settle_turn(&mut self, key: &TurnKey, outcome: &str) {
        if let Some(idx) = self.turn_index(key) {
            self.cancel_permissions(key);
            if let Some(acp) = &self.acp {
                acp.retire_turn(key);
            }
            use crate::chat::TurnStatus;
            let status = if outcome.starts_with("Failed") {
                TurnStatus::Failed
            } else if outcome.eq_ignore_ascii_case("cancelled") {
                TurnStatus::Cancelled
            } else if outcome.starts_with("Incomplete") || outcome == "max_tokens" {
                TurnStatus::Incomplete
            } else {
                TurnStatus::Completed
            };
            let session = &mut self.sessions[idx];
            if status == TurnStatus::Failed {
                session.failed_turn = session.active_turn.clone();
                if session.chat.input_buffer.is_empty() {
                    if let Some(turn) = &session.active_turn {
                        session.chat.input_buffer = turn.text.clone();
                        session.chat.cursor_pos = session.chat.input_buffer.len();
                    }
                }
            }
            for message in &mut session.chat.messages {
                if message.turn_id == Some(key.turn_id) {
                    message.outcome = Some(status);
                }
            }
            self.sessions[idx].chat.finish_streaming();
            self.sessions[idx].active_turn = None;
            self.sessions[idx].last_activity = std::time::SystemTime::now();
            self.rebuild_project_indices();
            self.sessions[idx]
                .chat
                .add_system_message(&format!("[Turn {}: {outcome}]", key.turn_id));
            self.save_session(idx);
        }
    }

    fn cancel_permissions(&mut self, key: &TurnKey) {
        let mut remaining = std::collections::VecDeque::new();
        while let Some(permission) = self.pending_permissions.pop_front() {
            if permission.key == *key {
                if let Some(acp) = &self.acp {
                    let _ = acp.respond_permission(permission.id, None);
                }
            } else {
                remaining.push_back(permission);
            }
        }
        self.pending_permissions = remaining;
    }

    pub fn resolve_permission(&mut self, permission: PendingPermission, option: Option<String>) {
        // Revalidate after the modal UI: the original turn may have died meanwhile.
        let selected = option.filter(|id| {
            self.turn_index(&permission.key).is_some()
                && permission.options.iter().any(|(offered, _)| offered == id)
        });
        if let Some(acp) = &self.acp {
            if let Err(e) = acp.respond_permission(permission.id, selected) {
                self.connection_failed(&e);
            }
        }
    }

    fn record_tool_activity(
        &mut self,
        key: &TurnKey,
        display: String,
        payload: crate::json::Json,
        is_update: bool,
    ) {
        if let Some(idx) = self.delta_turn_index(key) {
            let activity = crate::chat::ToolActivity {
                call_id: payload
                    .get("toolCallId")
                    .and_then(crate::json::Json::as_str)
                    .unwrap_or("")
                    .to_string(),
                status: payload
                    .get("status")
                    .and_then(crate::json::Json::as_str)
                    .map(str::to_string),
                is_update,
                payload: std::sync::Arc::new(payload),
            };
            let session = &mut self.sessions[idx];
            session.chat.add_system_message(&display);
            if let Some(message) = session.chat.messages.back_mut() {
                message.turn_id = Some(key.turn_id);
                message.backend = Some(SessionMode::Code);
                message.outcome = Some(crate::chat::TurnStatus::Streaming);
                message.tool = Some(activity);
            }
        }
    }

    fn connection_failed(&mut self, message: &str) {
        let diagnostics = self
            .acp
            .as_ref()
            .map(AcpClient::diagnostic_tail)
            .unwrap_or_default();
        let message = if diagnostics.is_empty() {
            message.to_string()
        } else {
            format!("{message}\nAgent diagnostics:\n{diagnostics}")
        };
        self.acp_connected = false;
        self.acp_connecting = false;
        self.reconnect_attempts = self.reconnect_attempts.max(1);
        self.pending_permissions.clear();
        if let Some(acp) = self.acp.take() {
            acp.shutdown();
        }
        let keys: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|s| s.active_turn.as_ref())
            .filter(|t| t.backend == SessionMode::Code)
            .map(|t| t.key.clone())
            .collect();
        for session in &mut self.sessions {
            session.acp_session_id.clear();
        }
        for key in keys {
            self.settle_turn(&key, &format!("Failed: {message}"));
        }
        self.last_reconnect_attempt = Some(std::time::Instant::now());
        self.show_toast(&message, ToastKind::Error);
    }

    fn advance_code_turns(&mut self) {
        if !self.acp_connected {
            return;
        }
        let mut failures = Vec::new();
        if let Some(acp) = &self.acp {
            for session in &mut self.sessions {
                let Some(turn) = &mut session.active_turn else {
                    continue;
                };
                if turn.backend != SessionMode::Code || turn.submitted {
                    continue;
                }
                let result = if session.acp_session_id.is_empty() {
                    if turn.session_requested {
                        continue;
                    }
                    turn.session_requested = true;
                    acp.request_session(turn.key.clone(), &session.working_dir)
                } else {
                    let result =
                        acp.send_prompt(turn.key.clone(), &session.acp_session_id, &turn.text);
                    if result.is_ok() {
                        turn.submitted = true;
                    }
                    result
                };
                if let Err(error) = result {
                    failures.push((turn.key.clone(), error));
                } else if turn.submitted {
                    session.start_turn_streaming();
                }
            }
        }
        for (key, error) in failures {
            self.settle_turn(&key, &format!("Failed: {error}"));
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
    pub(crate) fn show_toast(&mut self, text: &str, kind: ToastKind) {
        self.repaint_needed = true;
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
        let Some(session) = self.sessions.get_mut(self.active_session) else {
            return;
        };
        if text.trim().is_empty() || session.active_turn.is_some() {
            return;
        }
        let turn_id = self.next_turn_id;
        let Some(next) = turn_id.checked_add(1) else {
            session.chat.input_buffer = text.to_string();
            session.chat.cursor_pos = text.len();
            self.show_toast("Turn identity space exhausted", ToastKind::Error);
            return;
        };
        self.next_turn_id = next;
        let key = TurnKey {
            local_id: session.local_id.clone(),
            turn_id,
        };
        let mode = session.mode;
        let retry = session
            .failed_turn
            .take()
            .is_some_and(|turn| turn.text == text && turn.backend == mode);
        if !retry {
            session.chat.add_message(Role::User, text);
        }
        if let Some(message) = session
            .chat
            .messages
            .iter_mut()
            .rev()
            .find(|message| message.role == Role::User)
        {
            message.turn_id = Some(turn_id);
            message.backend = Some(mode);
            message.model =
                (mode == SessionMode::Direct).then(|| crate::responses::DEFAULT_MODEL.to_string());
            message.outcome = Some(crate::chat::TurnStatus::Queued);
        }
        session.last_activity = std::time::SystemTime::now();
        session.auto_name();
        session.active_turn = Some(ActiveTurn {
            key,
            backend: mode,
            text: text.to_string(),
            submitted: false,
            session_requested: false,
            cancelling: false,
            started: std::time::Instant::now(),
        });
        session.chat.is_waiting_for_agent = true;
        self.rebuild_project_indices();
        let idx = self.active_session;
        self.save_session(idx);
        match mode {
            SessionMode::Direct => self.send_direct(idx),
            SessionMode::Code => {
                if self.acp.is_none() {
                    self.connect_acp();
                }
                self.advance_code_turns();
            }
        }
    }

    /// Send the next Direct-mode turn for session `idx` via the Responses API.
    fn send_direct(&mut self, idx: usize) {
        let Some(turn) = self.sessions[idx].active_turn.clone() else {
            return;
        };
        let history = self.sessions[idx].responses_history();
        let result = self
            .responses
            .as_ref()
            .ok_or_else(|| "Direct transport unavailable".to_string())
            .and_then(|client| {
                client.send(
                    turn.key.clone(),
                    history,
                    crate::session::ReasoningEffort::None,
                )
            });
        match result {
            Ok(()) => {
                if let Some(turn) = &mut self.sessions[idx].active_turn {
                    turn.submitted = true;
                }
                self.sessions[idx].start_turn_streaming();
            }
            Err(error) => {
                self.settle_turn(&turn.key, &format!("Failed: {error}"));
                self.show_toast(&error, ToastKind::Error);
            }
        }
    }

    /// Install the Direct-mode transport (called by the platform backend on
    /// startup). Idempotent.
    pub fn install_responses_transport(
        &mut self,
        transport: Box<dyn crate::responses::HttpTransport + Send>,
    ) {
        if self.responses.is_none() {
            match ResponsesClient::new(transport) {
                Ok(client) => self.responses = Some(client),
                Err(error) => self.show_toast(&error, ToastKind::Error),
            }
        }
        self.responses_wanted = false;
    }

    /// Drain pending Responses API events into the UI state. Called from the
    /// platform timer tick alongside poll_acp.
    pub fn poll_responses(&mut self) {
        let started = std::time::Instant::now();
        for _ in 0..256 {
            if started.elapsed().as_millis() >= 4 {
                break;
            }
            let event = self.responses.as_ref().and_then(ResponsesClient::try_recv);
            if event.is_some() {
                self.repaint_needed = true;
            }
            let Some(event) = event else {
                break;
            };
            match event {
                ResponsesEvent::TextDelta { key, text } => {
                    if let Some(idx) = self.delta_turn_index(&key) {
                        self.sessions[idx].chat.append_stream_chunk(&text);
                    }
                }
                ResponsesEvent::ReasoningDelta { key, text } => {
                    if let Some(idx) = self.delta_turn_index(&key) {
                        self.sessions[idx].chat.append_thought_chunk(&text);
                    }
                }
                ResponsesEvent::Terminal { key, outcome } => {
                    use crate::responses::StreamOutcome;
                    let text = match outcome {
                        StreamOutcome::Completed => "Completed".to_string(),
                        StreamOutcome::Incomplete(reason) => format!("Incomplete: {reason}"),
                        StreamOutcome::Cancelled => "Cancelled".to_string(),
                        StreamOutcome::Failed(error) => format!("Failed: {error}"),
                    };
                    if self.turn_index(&key).is_some() {
                        self.settle_turn(&key, &text);
                        self.show_toast(&text, ToastKind::Info);
                    }
                }
            }
        }
    }

    pub fn activate_backend_settings(&mut self) {
        match self
            .sessions
            .get(self.active_session)
            .map(|session| session.mode)
        {
            Some(SessionMode::Code) => self.connect_acp(),
            _ => self.setup_help_requested = Some(SessionMode::Direct),
        }
    }

    /// Status belongs to the current turn's backend, not the next-turn selector.
    pub fn active_status(&self) -> &'static str {
        let Some(session) = self.sessions.get(self.active_session) else {
            return "No chat selected";
        };
        if let Some(turn) = &session.active_turn {
            if turn.cancelling {
                return "Stopping...";
            }
            if self
                .pending_permissions
                .iter()
                .any(|permission| permission.key == turn.key)
            {
                return "Waiting for approval";
            }
            match turn.backend {
                SessionMode::Code if !self.acp_connected => return "Starting agent...",
                SessionMode::Code if !turn.submitted => return "Opening project session...",
                SessionMode::Code => return "Agent working...",
                SessionMode::Direct => {
                    let received = session
                        .chat
                        .streaming_message_idx
                        .and_then(|idx| session.chat.messages.get(idx))
                        .is_some_and(|message| {
                            !message.text.is_empty() || !message.thinking.is_empty()
                        });
                    return if received {
                        "Responding..."
                    } else {
                        "Sending..."
                    };
                }
            }
        }
        match session.mode {
            SessionMode::Direct if self.chat_key_configured => "Ready to send",
            SessionMode::Direct => "API key missing",
            SessionMode::Code if self.acp_connecting => "Starting agent...",
            SessionMode::Code if self.acp_connected => "Agent ready",
            SessionMode::Code => "Agent starts on Send",
        }
    }

    /// Switch the active session's mode (radio control in the header).
    pub fn set_session_mode(&mut self, mode: SessionMode) {
        if let Some(session) = self.sessions.get(self.active_session) {
            if session.mode != mode {
                self.mode_change_requested = Some((session.local_id.clone(), mode));
            }
        }
    }

    pub fn apply_session_mode(&mut self, local_id: &str, mode: SessionMode) {
        let Some(idx) = self.sessions.iter().position(|s| s.local_id == local_id) else {
            return;
        };
        if self.sessions[idx].mode != mode {
            self.sessions[idx].mode = mode;
            let notice = match mode {
                SessionMode::Code => "Code mode uses agent context, not the Direct transcript. A fresh agent session does not know earlier Direct messages.",
                SessionMode::Direct => "Direct mode uploads user/assistant text from this chat to the API. Tool activity and displayed thinking are not uploaded.",
            };
            self.sessions[idx].chat.add_system_message(notice);
            self.show_toast(notice, ToastKind::Info);
            self.save_index();
        }
    }

    pub fn connect_acp(&mut self) {
        if self.acp_connecting {
            return;
        }
        if self.acp.is_some() {
            self.connection_failed("Agent connection restarted");
        }
        self.acp_connected = false;
        self.acp_connecting = true;
        self.reconnect_attempts = self.reconnect_attempts.saturating_add(1);
        self.last_reconnect_attempt = Some(std::time::Instant::now());
        for session in &mut self.sessions {
            session.acp_session_id.clear();
        }
        match AcpClient::spawn(self.configure_agent_process) {
            Ok(client) => {
                self.acp = Some(client);
                self.show_toast("Connecting to agent...", ToastKind::Info);
            }
            Err(e) => {
                self.connection_failed(&format!("Failed to start agent: {e}"));
            }
        }
    }

    pub fn cancel_turn(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        let acp_session_id = self.sessions[idx].acp_session_id.clone();
        let Some(turn) = self.sessions[idx].active_turn.clone() else {
            return;
        };
        if let Some(ref acp) = self.acp {
            if turn.backend == SessionMode::Code && !acp_session_id.is_empty() {
                let _ = acp.cancel(&acp_session_id);
            }
        }
        if turn.backend == SessionMode::Direct {
            if let Some(ref client) = self.responses {
                if client.abort(&turn.key) {
                    self.settle_turn(&turn.key, "Cancelled");
                    return;
                }
                if let Some(active) = &mut self.sessions[idx].active_turn {
                    active.cancelling = true;
                }
                self.show_toast("Cancelling request...", ToastKind::Info);
                return; // native cleanup must report its terminal first
            }
        }
        // ACP updates lack turn IDs: never reuse a cancelled remote session.
        if turn.backend == SessionMode::Code {
            self.sessions[idx].acp_session_id.clear();
        }
        self.settle_turn(&turn.key, "Cancelled");
    }

    /// Create a new chat in the default working directory.
    pub fn new_chat(&mut self) {
        let wd = self.default_working_dir.clone();
        self.create_local_session(&wd);
    }

    /// Create a new chat in a specific working directory (from folder picker).
    pub fn new_chat_in_dir(&mut self, working_dir: &str) {
        self.create_local_session(working_dir);
    }

    // ── Persistence ────────────────────────────────────────────────────────

    /// Save the state index (settings, projects, session metadata) to disk.
    /// Failures surface as a toast — a silent save loss must never look like success.
    pub fn save_index(&mut self) {
        self.save_all();
    }

    fn record_save_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.persistence_error = None,
            Err(e) => {
                self.persistence_error = Some(format!("Storage error: {e}"));
                self.show_toast(&format!("Save failed: {e}"), ToastKind::Error);
            }
        }
    }

    fn save_complete_snapshot(&mut self) {
        if self.persistence_worker.is_none() {
            match crate::persistence::PersistenceWorker::new() {
                Ok(worker) => self.persistence_worker = Some(worker),
                Err(error) => {
                    self.record_save_result(Err(error));
                    return;
                }
            }
        }
        let Some(revision) = self.save_revision.checked_add(1) else {
            self.record_save_result(Err("Snapshot revision space exhausted".to_string()));
            return;
        };
        self.save_revision = revision;
        let snapshot = crate::persistence::SaveSnapshot::capture(
            revision,
            &self.config_dir,
            &self.settings,
            &self.projects,
            &self.sessions,
            self.saved_window.as_ref(),
        )
        .with_removals(self.pending_deletions.clone());
        let result = match &self.persistence_worker {
            Some(worker) => worker.submit(snapshot),
            None => Err("Snapshot worker unavailable".to_string()),
        };
        self.persistence_pending = result.is_ok();
        self.repaint_needed = true;
        if let Err(error) = result {
            self.record_save_result(Err(error));
        }
    }

    pub fn poll_persistence(&mut self) {
        let result = self
            .persistence_worker
            .as_ref()
            .and_then(crate::persistence::PersistenceWorker::try_result);
        if let Some(result) = result {
            if result.result.is_ok() {
                self.pending_deletions
                    .retain(|id| !result.removed.contains(id));
            }
            self.repaint_needed = true;
            if result.revision == self.save_revision {
                self.persistence_pending = false;
                self.record_save_result(result.result);
            } else if let Err(error) = result.result {
                self.record_save_result(Err(error));
            }
        }
    }

    pub fn finish_saves(&mut self) {
        self.save_all();
        let result = match self.persistence_worker.as_mut() {
            Some(worker) => worker.finish(std::time::Duration::from_secs(5)),
            None => Ok(()),
        };
        self.poll_persistence();
        if let Err(error) = result {
            self.record_save_result(Err(error));
        }
    }

    /// A session save publishes one coherent generation, including its index.
    pub fn save_session(&mut self, idx: usize) {
        if self.sessions.get(idx).is_some() {
            self.save_all();
        }
    }

    /// Save the state index and all session files to disk.
    pub fn save_all(&mut self) {
        if !self.persistence_blocked {
            self.save_complete_snapshot();
        }
    }

    /// Load persisted state from disk. Returns true if state was loaded.
    pub fn load_state(&mut self) -> bool {
        self.invalidate_sidebar_cache();
        let result = crate::persistence::load_all(&self.config_dir);
        self.install_loaded_state(result)
    }

    pub fn load_backup(&mut self) -> bool {
        let result = crate::persistence::load_backup(&self.config_dir);
        // Recovery is deliberately read-only; do not replace the damaged index.
        self.persistence_blocked = true;
        self.install_loaded_state(result)
    }

    fn install_loaded_state(
        &mut self,
        result: Result<Option<crate::persistence::LoadedState>, crate::persistence::LoadError>,
    ) -> bool {
        let loaded = match result {
            Ok(Some(loaded)) => loaded,
            Ok(None) => return false,
            Err(e) => {
                self.persistence_blocked = true;
                self.persistence_error = Some(format!("Storage read-only: {e}"));
                return false;
            }
        };
        {
            if !loaded.warnings.is_empty() {
                self.persistence_blocked = true;
                self.persistence_error = Some(format!(
                    "Recovery required (read-only): {}",
                    loaded.warnings.join("; ")
                ));
            }
            self.settings = loaded.settings;
            self.projects = loaded.projects;
            self.sessions = loaded.sessions;
            if !self.sessions.is_empty() && self.sessions.iter().all(|s| s.archived) {
                self.settings.show_archived = true;
            }
            self.next_turn_id = self
                .sessions
                .iter()
                .flat_map(|s| s.chat.messages.iter())
                .filter_map(|message| message.turn_id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
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
        }
    }

    fn saved_window_state(&self) -> Option<crate::persistence::WindowState> {
        self.saved_window.clone()
    }

    /// Delete a session's file from disk.
    pub fn delete_session_file(&mut self, local_id: &str) {
        if self.persistence_blocked || self.sessions.iter().any(|s| s.local_id == local_id) {
            return;
        }
        if let Err(error) = crate::persistence::LocalSessionId::parse(local_id) {
            self.record_save_result(Err(error));
            return;
        }
        if !self.pending_deletions.iter().any(|id| id == local_id) {
            self.pending_deletions.push(local_id.to_string());
        }
        self.save_all();
    }

    /// Check if an auto-reconnect should be attempted. Called from the timer tick.
    /// Returns true if a reconnect was initiated.
    pub fn try_auto_reconnect(&mut self) -> bool {
        self.poll_persistence();
        let draft_due = self
            .draft_save_due
            .is_some_and(|edit| edit.elapsed().as_millis() >= 750);
        let checkpoint_due = self.last_checkpoint.elapsed().as_secs() >= 15
            && self.sessions.iter().any(|s| s.active_turn.is_some());
        if draft_due || checkpoint_due {
            self.draft_save_due = None;
            self.last_checkpoint = std::time::Instant::now();
            self.save_all();
        }
        let expired: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|session| {
                let turn = session.active_turn.as_ref()?;
                let limit = if turn.submitted { 600 } else { 30 };
                (turn.backend == SessionMode::Code && turn.started.elapsed().as_secs() > limit)
                    .then(|| (turn.key.clone(), session.acp_session_id.clone()))
            })
            .collect();
        for (key, remote) in expired {
            if let Some(acp) = &self.acp {
                if !remote.is_empty() {
                    let _ = acp.cancel(&remote);
                }
            }
            if let Some(idx) = self.turn_index(&key) {
                self.sessions[idx].acp_session_id.clear();
            }
            self.settle_turn(&key, "Failed: agent request timed out");
        }
        const MAX_AUTO_RECONNECTS: u32 = 5;
        const RECONNECT_DELAY_SECS: u64 = 3;

        if self.acp_connecting {
            if self
                .last_reconnect_attempt
                .is_some_and(|t| t.elapsed().as_secs() > 30)
            {
                self.connection_failed("Agent initialization timed out");
            }
            return false;
        }
        if self.reconnect_attempts == 0 || self.reconnect_attempts >= MAX_AUTO_RECONNECTS {
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
