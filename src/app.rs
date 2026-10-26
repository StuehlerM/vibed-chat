// Application state: projects, sessions, ACP coordination, settings, fonts, DPI.
#![allow(dead_code)]

use crate::acp::{AcpClient, AcpEvent};
use crate::chat::{ChatState, Role};
use crate::fonts::Fonts;
use crate::session::{Project, Session};
use crate::theme::Settings;
use crate::ui::{FocusedElement, Toast, ToastKind};

/// Shared application state stored in the window's user data.
pub struct AppState {
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub active_session: usize,
    pub show_settings: bool,
    pub settings: Settings,
    pub fonts: Option<Fonts>,
    pub dpi: u32,
    pub acp: Option<AcpClient>,
    pub acp_connected: bool,
    pub client_width: i32,
    pub client_height: i32,
    pub should_quit: bool,
    pub last_cursor_blink: std::time::Instant,
    pub cursor_visible: bool,
    pub caret_blink_ms: u32,
    pub hover_sidebar_item: Option<usize>,
    pub hover_new_chat: bool,
    pub hover_new_project: bool,
    pub hover_project: Option<usize>,
    pub hover_project_menu: Option<usize>,
    pub hover_settings_btn: bool,
    pub hover_settings_close: bool,
    pub hover_reconnect: bool,
    pub hover_thinking_toggle: Option<usize>,
    pub hover_settings_toggle: Option<usize>,
    pub hover_send_btn: bool,
    pub hover_session_header: bool,
    pub hover_session_archive: Option<usize>,
    pub hover_session_delete: Option<usize>,
    pub pressed_element: Option<crate::ui::PressedElement>,
    pub focused_element: Option<FocusedElement>,
    pub sidebar_scroll_offset: i32,
    pub hover_sidebar_scrollbar: bool,
    pub hover_scrollbar: bool,
    pub streaming_dots: u8,
    pub pending_new_session: bool,
    pub pending_working_dir: String,
    pub session_counter: usize,
    pub default_working_dir: String,
    pub toast: Option<Toast>,
}

impl AppState {
    pub fn new() -> Self {
        let caret_blink = unsafe { crate::win32::GetCaretBlinkTime() };
        let default_wd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
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
            client_width: 1100,
            client_height: 700,
            should_quit: false,
            last_cursor_blink: std::time::Instant::now(),
            cursor_visible: true,
            caret_blink_ms: if caret_blink == 0 { 500 } else { caret_blink },
            hover_sidebar_item: None,
            hover_new_chat: false,
            hover_new_project: false,
            hover_project: None,
            hover_project_menu: None,
            hover_settings_btn: false,
            hover_settings_close: false,
            hover_reconnect: false,
            hover_thinking_toggle: None,
            hover_settings_toggle: None,
            hover_send_btn: false,
            hover_session_header: false,
            hover_session_archive: None,
            hover_session_delete: None,
            pressed_element: None,
            focused_element: Some(FocusedElement::Input),
            sidebar_scroll_offset: 0,
            hover_sidebar_scrollbar: false,
            hover_scrollbar: false,
            streaming_dots: 0,
            pending_new_session: false,
            pending_working_dir: String::new(),
            session_counter: 0,
            default_working_dir: default_wd,
            toast: None,
        }
    }

    pub fn init_fonts(&mut self) {
        self.fonts = Some(Fonts::new(self.dpi));
    }

    pub fn scale_fonts(&mut self) {
        self.init_fonts();
    }

    pub fn active_chat(&mut self) -> Option<&mut ChatState> {
        if self.sessions.is_empty() {
            return None;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        Some(&mut self.sessions[idx].chat)
    }

    // ── Project management ───────────────────────────────────

    pub fn ensure_project(&mut self, path: &str) -> usize {
        if let Some(idx) = self.projects.iter().position(|p| p.path == path) {
            return idx;
        }
        let project = Project::new(path);
        self.projects.push(project);
        self.projects.len() - 1
    }

    pub fn rebuild_project_indices(&mut self) {
        for project in &mut self.projects {
            project.session_indices.clear();
        }
        for (i, session) in self.sessions.iter().enumerate() {
            if let Some(pidx) = self
                .projects
                .iter()
                .position(|p| p.path == session.working_dir)
            {
                self.projects[pidx].session_indices.push(i);
            }
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
    }

    // ── Session management ───────────────────────────────────

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
    }

    pub fn create_local_session_default(&mut self) {
        let wd = self.default_working_dir.clone();
        self.create_local_session(&wd);
    }

    pub fn switch_session(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.active_session = idx;
        }
    }

    /// Toggle the archived state of a session.
    pub fn toggle_archive_session(&mut self, idx: usize) {
        if let Some(session) = self.sessions.get_mut(idx) {
            session.archived = !session.archived;
            // If we just archived the active session, switch to the nearest non-archived
            if session.archived && self.active_session == idx {
                self.active_session = self.nearest_visible_session(idx);
            }
        }
    }

    /// Permanently delete a session. Returns true if a session was removed.
    pub fn delete_session(&mut self, idx: usize) -> bool {
        if idx >= self.sessions.len() {
            return false;
        }
        // Don't delete the session that's currently streaming
        if self.sessions[idx].chat.is_waiting_for_agent {
            return false;
        }
        self.sessions.remove(idx);
        self.rebuild_project_indices();
        // Adjust active_session index
        if self.sessions.is_empty() {
            self.active_session = 0;
            self.create_local_session_default();
        } else if self.active_session >= self.sessions.len() {
            self.active_session = self.sessions.len() - 1;
        } else if self.active_session > idx {
            self.active_session -= 1;
        }
        true
    }

    /// Find the nearest non-archived session to the given index.
    fn nearest_visible_session(&self, from_idx: usize) -> usize {
        let show_archived = self.settings.show_archived;
        // Search outward from from_idx
        for dist in 0..self.sessions.len() {
            // Try below
            if from_idx + dist < self.sessions.len() {
                let s = &self.sessions[from_idx + dist];
                if !s.archived || show_archived {
                    return from_idx + dist;
                }
            }
            // Try above
            if dist > 0 && from_idx >= dist {
                let s = &self.sessions[from_idx - dist];
                if !s.archived || show_archived {
                    return from_idx - dist;
                }
            }
        }
        from_idx.min(self.sessions.len().saturating_sub(1))
    }

    /// Get indices of sessions visible in the sidebar (respects show_archived setting).
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
                        self.sessions[idx].acp_session_id = session_id.clone();
                        self.pending_new_session = false;
                        self.pending_working_dir.clear();
                        self.add_system_message_to_active(
                            "Session ready. Type a message to begin.",
                        );
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
                    } else if let Some(chat) = self.active_chat() {
                        chat.finish_streaming();
                    }
                }
                AcpEvent::ToolCall {
                    session_id,
                    name,
                    input,
                } => {
                    let msg = format!("[Tool: {}] {}", name, truncate_str(&input, 200));
                    self.add_system_message_to_session(&session_id, &msg);
                }
                AcpEvent::ToolResult {
                    session_id,
                    call_id: _,
                    output,
                } => {
                    let msg = format!("[Result] {}", truncate_str(&output, 200));
                    self.add_system_message_to_session(&session_id, &msg);
                }
                AcpEvent::PermissionRequest {
                    id,
                    session_id,
                    method,
                    params: _,
                } => {
                    if self.settings.auto_approve {
                        let msg = format!("[Permission: {} (id={}) - auto-approved]", method, id);
                        self.add_system_message_to_session(&session_id, &msg);
                        if let Some(ref acp) = self.acp {
                            acp.respond_permission(id, true);
                        }
                    } else {
                        let msg = format!(
                            "[Permission requested: {} (id={}) - auto-approve is OFF]",
                            method, id
                        );
                        self.add_system_message_to_session(&session_id, &msg);
                    }
                }
                AcpEvent::ProcessDied { code } => {
                    self.acp_connected = false;
                    let msg = match code {
                        Some(c) => {
                            format!("Agent process exited (code {}). Press F5 to reconnect", c)
                        }
                        None => "Agent process died. Press F5 to reconnect".to_string(),
                    };
                    self.show_toast(&msg, ToastKind::Error);
                }
                AcpEvent::Error(msg) => {
                    self.show_toast(&format!("Error: {}", msg), ToastKind::Error);
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
            duration_ms: crate::render::TOAST_DURATION_MS,
        });
    }

    fn add_system_message_to_session(&mut self, acp_session_id: &str, text: &str) {
        if let Some(idx) = self.find_session_by_acp_id(acp_session_id) {
            self.sessions[idx].chat.add_system_message(text);
        } else {
            self.add_system_message_to_active(text);
        }
    }

    pub fn send_message(&mut self, text: String) {
        if self.sessions.is_empty() {
            return;
        }

        let idx = self.active_session.min(self.sessions.len() - 1);
        let session = &mut self.sessions[idx];

        session.chat.add_message(Role::User, &text);
        session.auto_name();

        if let Some(ref acp) = self.acp {
            if self.acp_connected && !session.acp_session_id.is_empty() {
                session.chat.start_streaming();
                acp.send_prompt(&session.acp_session_id, &text);
            } else {
                self.show_toast(
                    "Not connected to agent. Press F5 to reconnect",
                    ToastKind::Error,
                );
            }
        } else {
            self.show_toast("Agent not started. Press F5 to connect", ToastKind::Error);
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
                self.show_toast(&format!("Failed to start agent: {}", e), ToastKind::Error);
            }
        }
    }

    pub fn cancel_turn(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        let session_id = self.sessions[idx].acp_session_id.clone();
        if let Some(ref acp) = self.acp {
            if !session_id.is_empty() {
                acp.cancel(&session_id);
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
}

pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
