// User interaction: mouse click handling, hover tracking, keyboard focus cycling, layout computation.
// This module is platform-independent - it operates on abstract coordinates and
// app state. Platform-specific dialogs (folder picker, etc.) are delegated to
// the platform backend trait.
#![allow(dead_code)]

use crate::app::{AppState, HoverState};
use crate::chat::Role;
use crate::session::{SessionMode, TurnKey};
use crate::theme::{
    compute_mode_radio_layout, compute_settings_layout, HINT_ROW_HEIGHT, INPUT_MARGIN,
    MESSAGE_INNER_PADDING, MESSAGE_MAX_WIDTH_RATIO, MESSAGE_PADDING, MODE_RADIO_OPTION_W,
    PROJECT_HEADER_HEIGHT, PROJECT_MENU_BTN_SIZE, SEND_BTN_MARGIN, SEND_BTN_SIZE,
    SESSION_ACTION_BTN_GAP, SESSION_ACTION_BTN_SIZE, SESSION_HEADER_HEIGHT, SETTINGS_PANEL_WIDTH,
    SIDEBAR_FOOTER_HEIGHT, SIDEBAR_HEADER_HEIGHT, SIDEBAR_ITEM_HEIGHT, SIDEBAR_ITEM_MARGIN,
    SIDEBAR_PADDING, SIDEBAR_WIDTH, SPACE_1, SPACE_4, SPACE_5, THINKING_HEADER_HEIGHT,
};
use crate::ui::{FocusedElement, PressedElement, RenameTarget};

/// A sidebar list entry - either a project header or a session item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarEntry {
    ProjectHeader {
        project_idx: usize,
        top: i32,
        bottom: i32,
    },
    SessionItem {
        session_idx: usize,
        project_idx: usize,
        top: i32,
        bottom: i32,
    },
    NewProjectBtn {
        top: i32,
        bottom: i32,
    },
}

/// What a click on the sidebar list hit, computed inside
/// `with_sidebar_layout` (which borrows the layout entries) and applied
/// afterwards so `&mut self` mutations don't conflict with the borrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarHit {
    NewProject,
    ProjectMenu(usize),
    ProjectHeader(usize),
    SessionArchive(usize),
    SessionDelete(usize),
    SessionItem(usize),
}

/// What a mouse-move hover hit on the sidebar list, computed inside
/// `with_sidebar_layout` and applied to `HoverState` afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoverHit {
    NewProject,
    SidebarScrollbar,
    ProjectMenu(usize),
    Project(usize),
    SessionArchive(usize),
    SessionDelete(usize),
    SessionItem(usize),
}

/// Stable command identity captured on pointer down, never a mutable row index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerAction {
    NewChat,
    NewProject,
    ProjectHeader(String),
    ProjectMenu(String),
    SessionItem(String),
    RenameSession(String),
    SessionArchive(String),
    SessionDelete(String),
    SettingsBtn,
    SettingsClose,
    SettingsToggle(usize),
    BackendSettings {
        local_id: String,
        mode: SessionMode,
    },
    Mode {
        local_id: String,
        mode: SessionMode,
    },
    Send {
        local_id: String,
        mode: SessionMode,
        draft: String,
        last_message: Option<u64>,
    },
    Stop(TurnKey),
    Scrollbar(String),
    Reasoning {
        local_id: String,
        message_id: u64,
    },
    OpenLink {
        local_id: String,
        url: String,
    },
    CopyCode {
        local_id: String,
        code: String,
    },
    Input,
}

#[derive(Debug)]
pub struct PointerPress {
    action: PointerAction,
    origin: (i32, i32),
}

/// Owned platform work emitted only after a valid release.
pub enum PointerEffect {
    DeleteSession(String),
    OpenLink(String),
    CopyCode(String),
}

impl AppState {
    pub fn clear_hover(&mut self) {
        self.hover = HoverState::default();
    }

    pub fn cycle_focus(&mut self, shift: bool) {
        let mut order: Vec<FocusedElement> = Vec::new();
        order.push(FocusedElement::NewChat);
        order.push(FocusedElement::NewProject);
        self.with_sidebar_layout(|_, _, entries| {
            for entry in entries {
                match *entry {
                    SidebarEntry::ProjectHeader { project_idx, .. } => {
                        order.push(FocusedElement::ProjectHeader(project_idx))
                    }
                    SidebarEntry::SessionItem { session_idx, .. } => {
                        order.push(FocusedElement::SidebarItem(session_idx))
                    }
                    SidebarEntry::NewProjectBtn { .. } => {}
                }
            }
        });
        order.push(FocusedElement::SettingsBtn);
        order.push(FocusedElement::Input);
        order.push(FocusedElement::SendButton);
        if self.show_settings {
            order.push(FocusedElement::SettingsClose);
            order.push(FocusedElement::SettingsToggle(1));
            order.push(FocusedElement::SettingsToggle(3));
            order.push(FocusedElement::Reconnect);
        }

        if order.is_empty() {
            self.focused_element = Some(FocusedElement::Input);
            return;
        }

        let current = self.focused_element;
        let pos = current.and_then(|c| order.iter().position(|f| *f == c));

        let next = pos.map_or_else(
            || {
                if shift {
                    order.len() - 1
                } else {
                    0
                }
            },
            |p| {
                if shift {
                    if p == 0 {
                        order.len() - 1
                    } else {
                        p - 1
                    }
                } else {
                    (p + 1) % order.len()
                }
            },
        );

        self.focused_element = Some(order[next]);
    }

    /// Activate the currently focused element (Enter/Space).
    /// Returns true if the keypress was consumed.
    /// Dialog-triggering actions (`NewProject` folder picker, project menu) are
    /// deferred via request flags that the platform backend handles.
    pub fn activate_focused_element(&mut self) -> bool {
        match self.focused_element {
            Some(FocusedElement::NewChat) => {
                self.pressed_element = Some(PressedElement::NewChat);
                self.new_chat();
                true
            }
            Some(FocusedElement::NewProject) => {
                self.pressed_element = Some(PressedElement::NewProject);
                self.new_project_requested = true;
                true
            }
            Some(FocusedElement::ProjectHeader(i)) => {
                if i < self.projects.len() {
                    self.pressed_element = Some(PressedElement::ProjectHeader(i));
                    self.toggle_project(i);
                }
                true
            }
            Some(FocusedElement::SidebarItem(i)) => {
                if i < self.sessions.len() {
                    self.pressed_element = Some(PressedElement::SidebarItem(i));
                    self.switch_session(i);
                }
                true
            }
            Some(FocusedElement::SettingsBtn) => {
                self.pressed_element = Some(PressedElement::SettingsBtn);
                self.show_settings = !self.show_settings;
                true
            }
            Some(FocusedElement::SettingsClose) => {
                self.show_settings = false;
                true
            }
            Some(FocusedElement::SettingsToggle(idx)) => {
                self.toggle_setting(idx);
                true
            }
            Some(FocusedElement::Reconnect) => {
                self.pressed_element = Some(PressedElement::Reconnect);
                self.activate_backend_settings();
                true
            }
            Some(FocusedElement::SendButton) => {
                if !self.sessions.is_empty() {
                    let idx = self.active_session.min(self.sessions.len() - 1);
                    if self.sessions[idx].chat.is_waiting_for_agent {
                        self.pressed_element = Some(PressedElement::SendButton);
                        self.cancel_turn();
                    } else {
                        let chat = &mut self.sessions[idx].chat;
                        if !chat.input_buffer.trim().is_empty() {
                            let text = chat.take_input();
                            self.send_message(&text);
                        }
                    }
                }
                true
            }
            Some(FocusedElement::Input) | None => false,
        }
    }

    // ── Sidebar layout ───────────────────────────────────────

    /// Compute the project-aware sidebar layout matching the rendering in `sidebar.rs`.
    /// Returns `(list_top, total_height, entries)`. Archived sessions are included
    /// only if `show_archived` is enabled.
    ///
    /// Results are cached in `self.sidebar_cache` keyed by `sidebar_version`:
    /// the layout is pure in (projects, sessions, settings, dpi, `client_height`),
    /// all of which bump the version when they change, so repeat calls within
    /// one event are free. The cached entries are borrowed, not cloned —
    /// callers must not hold the slice across a mutation of `self`.
    pub fn with_sidebar_layout<R>(&mut self, f: impl FnOnce(i32, i32, &[SidebarEntry]) -> R) -> R {
        if self.sidebar_cache.version != self.sidebar_version {
            self.refresh_sidebar_layout();
        }
        let cache = &self.sidebar_cache;
        f(cache.list_top, cache.total_height, &cache.entries)
    }

    /// Recompute the sidebar layout and refresh the cache.
    pub fn refresh_sidebar_layout(&mut self) {
        let (list_top, total_height, entries) = self.compute_sidebar_layout();
        self.sidebar_cache = crate::app::SidebarLayoutCache {
            version: self.sidebar_version,
            list_top,
            total_height,
            entries,
        };
    }

    fn compute_sidebar_layout(&self) -> (i32, i32, Vec<SidebarEntry>) {
        let scale = self.dpi as f32 / 96.0;
        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let header_h = (SIDEBAR_HEADER_HEIGHT as f32 * scale) as i32;
        let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
        let item_gap = (SIDEBAR_ITEM_MARGIN as f32 * scale) as i32;
        let proj_h = (PROJECT_HEADER_HEIGHT as f32 * scale) as i32;
        let new_proj_h = (28.0 * scale) as i32;
        let group_gap = (SPACE_1 as f32 * scale) as i32;

        let btn_bottom = pad + header_h - pad * 2;
        let new_proj_top = btn_bottom + (SPACE_1 as f32 * scale) as i32;
        let new_proj_bottom = new_proj_top + new_proj_h;
        let list_top = new_proj_bottom + group_gap;

        let mut y = list_top;
        let mut entries = Vec::new();
        let mut total_h = 0i32;

        entries.push(SidebarEntry::NewProjectBtn {
            top: new_proj_top,
            bottom: new_proj_bottom,
        });

        // A synthetic "Other" group index for sessions without a project
        let other_idx = self.projects.len();

        let mut grouped: Vec<(usize, Vec<usize>)> = Vec::new();
        for (pidx, project) in self.projects.iter().enumerate() {
            let visible: Vec<usize> = project
                .session_indices
                .iter()
                .copied()
                .filter(|&i| {
                    self.sessions
                        .get(i)
                        .is_some_and(|s| self.settings.show_archived || !s.archived)
                })
                .collect();
            grouped.push((pidx, visible));
        }
        let other_sessions: Vec<usize> = self
            .visible_session_indices()
            .into_iter()
            .filter(|&i| self.project_for_session(i).is_none())
            .collect();
        if !other_sessions.is_empty() {
            grouped.push((other_idx, other_sessions));
        }

        for (group_idx, (pidx, session_indices)) in grouped.iter().enumerate() {
            entries.push(SidebarEntry::ProjectHeader {
                project_idx: *pidx,
                top: y,
                bottom: y + proj_h,
            });
            y += proj_h;
            total_h += proj_h;

            let expanded = if *pidx == other_idx {
                true
            } else {
                self.projects.get(*pidx).is_none_or(|p| p.expanded)
            };
            if expanded {
                for &sidx in session_indices {
                    entries.push(SidebarEntry::SessionItem {
                        session_idx: sidx,
                        project_idx: *pidx,
                        top: y,
                        bottom: y + item_h,
                    });
                    y += item_h + item_gap;
                    total_h += item_h + item_gap;
                }
            }
            if group_idx + 1 < grouped.len() {
                y += group_gap;
                total_h += group_gap;
            }
        }

        if matches!(entries.last(), Some(SidebarEntry::SessionItem { .. })) {
            total_h = total_h.saturating_sub(item_gap).max(0);
        }

        (list_top, total_h, entries)
    }

    // ── Click handling ───────────────────────────────────────

    // One hit-test path for both pointer down and release. No command is run here.
    #[allow(clippy::too_many_lines)]
    pub fn pointer_action_at(&mut self, x: i32, y: i32) -> Option<PointerAction> {
        if x < 0 || y < 0 || x >= self.client_width || y >= self.client_height {
            return None;
        }
        let scale = self.dpi as f32 / 96.0;
        let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let header_h = (SIDEBAR_HEADER_HEIGHT as f32 * scale) as i32;

        // Settings panel interactions
        if self.show_settings {
            let panel_left = self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32;
            let spad = (SPACE_5 as f32 * scale) as i32;
            let close_size = (SPACE_5 as f32 * scale) as i32;
            if x >= self.client_width - spad - close_size
                && x <= self.client_width - spad
                && y >= spad
                && y <= spad + close_size
            {
                return Some(PointerAction::SettingsClose);
            }

            if x >= panel_left {
                let layout = compute_settings_layout(scale);
                if x >= panel_left + spad
                    && x < self.client_width - spad
                    && y >= layout.reconnect_y
                    && y < layout.reconnect_y + layout.reconnect_btn_h
                {
                    return self.sessions.get(self.active_session).map(|session| {
                        PointerAction::BackendSettings {
                            local_id: session.local_id.clone(),
                            mode: session.mode,
                        }
                    });
                }

                return self
                    .settings_toggle_at(x, y)
                    .map(PointerAction::SettingsToggle);
            }
        }

        // Sidebar clicks
        if x < sidebar_w {
            // New Chat button
            let btn_top = pad;
            let btn_bottom = btn_top + header_h - pad * 2;
            if x >= pad && x <= sidebar_w - pad && y >= btn_top && y <= btn_bottom {
                return Some(PointerAction::NewChat);
            }

            // Project-aware sidebar list (matches sidebar rendering)
            let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;
            let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
            let sidebar_inner_left = pad;
            let sidebar_inner_right = sidebar_w - pad;
            let client_height = self.client_height;
            let sidebar_scroll_offset = self.sidebar_scroll_offset;
            let projects_len = self.projects.len();
            let active_session = self.active_session;
            let hit = self.with_sidebar_layout(|list_top, total_list_height, entries| {
                let list_bottom = client_height - footer_h - pad;
                let avail_height = list_bottom - list_top;
                let max_scroll = (total_list_height - avail_height).max(0);
                let sidebar_scroll = sidebar_scroll_offset.min(max_scroll).max(0);

                // "+ New Project" button sits above list_top
                if let Some(SidebarEntry::NewProjectBtn { top, bottom }) = entries.first() {
                    if y >= *top && y < *bottom && x >= pad && x < sidebar_w - pad {
                        return Some(SidebarHit::NewProject);
                    }
                }
                if y < list_top || y >= list_bottom || x < pad || x >= sidebar_w - pad {
                    return None;
                }

                for entry in entries {
                    match *entry {
                        SidebarEntry::ProjectHeader {
                            project_idx,
                            top,
                            bottom,
                        } => {
                            let adj_top = top - sidebar_scroll;
                            let adj_bottom = bottom - sidebar_scroll;
                            if adj_bottom <= list_top || adj_top >= list_bottom {
                                continue;
                            }
                            if y < adj_top || y >= adj_bottom {
                                continue;
                            }
                            // Menu button on the right (real projects only)
                            if project_idx < projects_len {
                                let menu_btn_size = (PROJECT_MENU_BTN_SIZE as f32 * scale) as i32;
                                let menu_right = sidebar_w - pad - (SPACE_1 as f32 * scale) as i32;
                                let menu_left = menu_right - menu_btn_size;
                                if x >= menu_left && x <= menu_right {
                                    return Some(SidebarHit::ProjectMenu(project_idx));
                                }
                            }
                            return Some(SidebarHit::ProjectHeader(project_idx));
                        }
                        SidebarEntry::SessionItem {
                            session_idx,
                            top,
                            bottom,
                            ..
                        } => {
                            let adj_top = top - sidebar_scroll;
                            let adj_bottom = bottom - sidebar_scroll;
                            if adj_bottom <= list_top || adj_top >= list_bottom {
                                continue;
                            }

                            if y < adj_top || y >= adj_bottom {
                                continue;
                            }
                            if session_idx == active_session {
                                return Some(SidebarHit::SessionItem(session_idx));
                            }
                            // Only inactive rows paint action buttons.
                            let (archive_rect, delete_rect) = session_action_rects(
                                sidebar_inner_left,
                                sidebar_inner_right,
                                adj_top,
                                item_h,
                                scale,
                            );

                            if point_in_rect(
                                x,
                                y,
                                archive_rect.0,
                                archive_rect.1,
                                archive_rect.2,
                                archive_rect.3,
                            ) {
                                return Some(SidebarHit::SessionArchive(session_idx));
                            }
                            if point_in_rect(
                                x,
                                y,
                                delete_rect.0,
                                delete_rect.1,
                                delete_rect.2,
                                delete_rect.3,
                            ) {
                                return Some(SidebarHit::SessionDelete(session_idx));
                            }

                            // Check if click is on the session item itself
                            if y >= adj_top && y <= adj_bottom {
                                return Some(SidebarHit::SessionItem(session_idx));
                            }
                        }
                        SidebarEntry::NewProjectBtn { .. } => {}
                    }
                }
                None
            });
            if let Some(hit) = hit {
                return match hit {
                    SidebarHit::NewProject => Some(PointerAction::NewProject),
                    SidebarHit::ProjectMenu(idx) => self
                        .projects
                        .get(idx)
                        .map(|p| PointerAction::ProjectMenu(p.path.clone())),
                    SidebarHit::ProjectHeader(idx) => self
                        .projects
                        .get(idx)
                        .map(|p| PointerAction::ProjectHeader(p.path.clone())),
                    SidebarHit::SessionArchive(idx) => self
                        .sessions
                        .get(idx)
                        .map(|s| PointerAction::SessionArchive(s.local_id.clone())),
                    SidebarHit::SessionDelete(idx) => self
                        .sessions
                        .get(idx)
                        .map(|s| PointerAction::SessionDelete(s.local_id.clone())),
                    SidebarHit::SessionItem(idx) => self
                        .sessions
                        .get(idx)
                        .map(|s| PointerAction::SessionItem(s.local_id.clone())),
                };
            }

            // Settings button
            let settings_btn_top = self.client_height - footer_h + pad / 2;
            let settings_btn_bottom = self.client_height - pad / 2;
            if x >= pad && x <= sidebar_w - pad && y >= settings_btn_top && y <= settings_btn_bottom
            {
                return Some(PointerAction::SettingsBtn);
            }

            return None;
        }

        // Mode radio (header): Chat | Code
        {
            let header_h = (SESSION_HEADER_HEIGHT as f32 * scale) as i32;
            let sidebar_w_scaled = sidebar_w;
            let layout =
                compute_mode_radio_layout(sidebar_w_scaled, self.chat_right(), header_h, scale);
            if y >= layout.top && y <= layout.top + layout.h {
                let option_w = (MODE_RADIO_OPTION_W as f32 * scale) as i32;
                if x >= layout.direct_left && x < layout.direct_left + option_w {
                    return self
                        .sessions
                        .get(self.active_session)
                        .map(|s| PointerAction::Mode {
                            local_id: s.local_id.clone(),
                            mode: SessionMode::Direct,
                        });
                }
                if x >= layout.code_left && x < layout.code_left + option_w {
                    return self
                        .sessions
                        .get(self.active_session)
                        .map(|s| PointerAction::Mode {
                            local_id: s.local_id.clone(),
                            mode: SessionMode::Code,
                        });
                }
            }
        }

        // Send/Stop button click
        if let Some((btn_left, btn_top, btn_right, btn_bottom)) = self.send_btn_rect() {
            if x >= btn_left && x <= btn_right && y >= btn_top && y <= btn_bottom {
                let session = self.sessions.get(self.active_session)?;
                if let Some(turn) = &session.active_turn {
                    return (!turn.cancelling).then(|| PointerAction::Stop(turn.key.clone()));
                }
                return (!session.chat.is_waiting_for_agent
                    && !session.chat.input_buffer.trim().is_empty())
                .then(|| PointerAction::Send {
                    local_id: session.local_id.clone(),
                    mode: session.mode,
                    draft: session.chat.input_buffer.clone(),
                    last_message: session.chat.messages.back().map(|m| m.id),
                });
            }
        }

        let local_id = self.sessions.get(self.active_session)?.local_id.clone();
        if let Some(url) = self.click_link(x, y) {
            return Some(PointerAction::OpenLink { local_id, url });
        }
        if let Some(code) = self.click_code_copy(x, y) {
            return Some(PointerAction::CopyCode { local_id, code });
        }
        if self.scrollbar_at(x, y) {
            return Some(PointerAction::Scrollbar(local_id));
        }
        if let Some(message_id) = self.thinking_message_at(x, y) {
            return Some(PointerAction::Reasoning {
                local_id,
                message_id,
            });
        }

        // Click in chat area - focus input
        {
            let input_box_top = self.input_box_top();
            let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
            let chat_left = self.chat_left();
            let chat_right = self.chat_right();
            let input_left = chat_left + input_margin;
            let input_right = chat_right - input_margin;
            let idx = self
                .active_session
                .min(self.sessions.len().saturating_sub(1));
            let session = self.sessions.get(idx)?;
            let input_h = (session.chat.input_height as f32 * scale) as i32;
            let input_bottom = input_box_top + input_h;

            if x >= input_left
                && x <= input_right
                && y >= input_box_top
                && y <= input_bottom
                && !self.sessions.is_empty()
            {
                return Some(PointerAction::Input);
            }
        }
        None
    }

    /// Capture intent without executing it. Text focus still follows pointer down.
    pub fn begin_pointer_press(&mut self, x: i32, y: i32, double_click: bool) -> bool {
        self.cancel_pointer_press();
        let mut action = self.pointer_action_at(x, y);
        if double_click {
            action = match action {
                Some(PointerAction::SessionItem(id))
                    if self.last_pointer_session.as_ref() == Some(&id) =>
                {
                    Some(PointerAction::RenameSession(id))
                }
                _ => None, // Never turn a double Send into an immediate Stop.
            };
        }
        self.last_pointer_session = None;
        let on_rename = match (self.rename_target, &action) {
            (Some(RenameTarget::Session(idx)), Some(PointerAction::SessionItem(id))) => {
                self.sessions.get(idx).is_some_and(|s| &s.local_id == id)
            }
            (Some(RenameTarget::Project(idx)), Some(PointerAction::ProjectHeader(path))) => {
                self.projects.get(idx).is_some_and(|p| &p.path == path)
            }
            _ => false,
        };
        if on_rename {
            return false;
        }
        self.cancel_rename(); // Existing click-away policy: cancel, never silently commit.
        let Some(action) = action else {
            return false;
        };
        if action == PointerAction::Input {
            self.focused_element = Some(FocusedElement::Input);
            return false;
        }
        self.pressed_element = self.pointer_visual(&action);
        self.pointer_press = Some(PointerPress {
            action,
            origin: (x, y),
        });
        true
    }

    pub fn cancel_pointer_press(&mut self) {
        self.pointer_press = None;
        self.pressed_element = None;
    }

    fn pointer_press_matches(&mut self, press: &PointerPress, x: i32, y: i32) -> bool {
        let expected = match &press.action {
            PointerAction::RenameSession(id) => PointerAction::SessionItem(id.clone()),
            action => action.clone(),
        };
        self.pointer_action_at(press.origin.0, press.origin.1)
            .as_ref()
            == Some(&expected)
            && self.pointer_action_at(x, y).as_ref() == Some(&expected)
    }

    /// Drag-out is a permanent cancellation, even if the pointer later returns.
    pub fn update_pointer_press(&mut self, x: i32, y: i32) {
        if let Some(press) = self.pointer_press.take() {
            if self.pointer_press_matches(&press, x, y) {
                self.pointer_press = Some(press);
            } else {
                self.pressed_element = None;
            }
        }
    }

    pub fn finish_pointer_press(&mut self, x: i32, y: i32) -> Option<PointerEffect> {
        let press = self.pointer_press.take()?;
        self.pressed_element = None;
        if !self.pointer_press_matches(&press, x, y) {
            return None;
        }
        self.activate_pointer_action(press.action, x, y)
    }

    fn pointer_visual(&self, action: &PointerAction) -> Option<PressedElement> {
        Some(match action {
            PointerAction::NewChat => PressedElement::NewChat,
            PointerAction::NewProject => PressedElement::NewProject,
            PointerAction::ProjectHeader(path) => {
                PressedElement::ProjectHeader(self.projects.iter().position(|p| &p.path == path)?)
            }
            PointerAction::ProjectMenu(path) => {
                PressedElement::ProjectMenu(self.projects.iter().position(|p| &p.path == path)?)
            }
            PointerAction::SessionItem(id) | PointerAction::RenameSession(id) => {
                PressedElement::SidebarItem(self.sessions.iter().position(|s| &s.local_id == id)?)
            }
            PointerAction::SessionArchive(id) => PressedElement::SessionArchive(
                self.sessions.iter().position(|s| &s.local_id == id)?,
            ),
            PointerAction::SessionDelete(id) => {
                PressedElement::SessionDelete(self.sessions.iter().position(|s| &s.local_id == id)?)
            }
            PointerAction::SettingsBtn => PressedElement::SettingsBtn,
            PointerAction::SettingsClose => PressedElement::SettingsClose,
            PointerAction::SettingsToggle(idx) => PressedElement::SettingsToggle(*idx),
            PointerAction::BackendSettings { .. } => PressedElement::Reconnect,
            PointerAction::Mode { mode, .. } => PressedElement::ModeRadio(*mode),
            PointerAction::Send { .. } | PointerAction::Stop(_) => PressedElement::SendButton,
            _ => return None,
        })
    }

    fn activate_pointer_action(
        &mut self,
        action: PointerAction,
        x: i32,
        y: i32,
    ) -> Option<PointerEffect> {
        match action {
            PointerAction::NewChat => {
                self.new_chat();
            }
            PointerAction::NewProject => {
                self.focused_element = Some(FocusedElement::NewProject);
                self.new_project_requested = true;
            }
            PointerAction::ProjectHeader(path) => {
                if let Some(idx) = self.projects.iter().position(|p| p.path == path) {
                    self.toggle_project(idx);
                }
            }
            PointerAction::ProjectMenu(path) => {
                self.project_menu_requested = self.projects.iter().position(|p| p.path == path);
            }
            PointerAction::SessionItem(id) => {
                if let Some(idx) = self.sessions.iter().position(|s| s.local_id == id) {
                    self.switch_session(idx);
                    self.last_pointer_session = Some(id);
                }
            }
            PointerAction::RenameSession(id) => {
                if let Some(idx) = self.sessions.iter().position(|s| s.local_id == id) {
                    self.start_rename(idx);
                }
            }
            PointerAction::SessionArchive(id) => {
                if let Some(idx) = self.sessions.iter().position(|s| s.local_id == id) {
                    self.toggle_archive_session(idx);
                }
            }
            PointerAction::SessionDelete(id) => return Some(PointerEffect::DeleteSession(id)),
            PointerAction::SettingsBtn => {
                self.focused_element = Some(FocusedElement::SettingsBtn);
                self.show_settings = !self.show_settings;
                self.invalidate_sidebar_cache();
            }
            PointerAction::SettingsClose => {
                self.show_settings = false;
            }
            PointerAction::SettingsToggle(idx) => {
                self.focused_element = Some(FocusedElement::SettingsToggle(idx));
                self.toggle_setting(idx);
            }
            PointerAction::BackendSettings { .. } => {
                self.activate_backend_settings();
            }
            PointerAction::Mode { mode, .. } => {
                self.set_session_mode(mode);
            }
            PointerAction::Send { draft, .. } => {
                if let Some(chat) = self.active_chat() {
                    chat.take_input();
                }
                self.send_message(&draft);
            }
            PointerAction::Stop(_) => {
                self.cancel_turn();
            }
            PointerAction::Scrollbar(_) => {
                self.jump_to_scrollbar_position(x, y);
            }
            PointerAction::Reasoning { message_id, .. } => {
                if let Some(chat) = self.active_chat() {
                    if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                        message.thinking_collapsed = !message.thinking_collapsed;
                    }
                }
                self.link_rects.clear();
                self.code_block_rects.clear();
            }
            PointerAction::OpenLink { url, .. } => return Some(PointerEffect::OpenLink(url)),
            PointerAction::CopyCode { code, .. } => return Some(PointerEffect::CopyCode(code)),
            PointerAction::Input => {}
        }
        None
    }

    /// Check if a click landed on a link rect. Returns the URL if so.
    pub fn click_link(&self, x: i32, y: i32) -> Option<String> {
        if !self.in_transcript(x, y) {
            return None;
        }
        self.link_rects
            .iter()
            .find(|(left, top, right, bottom, _)| {
                x >= *left && x <= *right && y >= *top && y <= *bottom
            })
            .map(|(_, _, _, _, url)| url.clone())
    }

    /// Check if a click landed on a code block copy button. Returns the code text if so.
    fn in_transcript(&self, x: i32, y: i32) -> bool {
        x >= self.chat_left()
            && x < self.chat_right()
            && y >= self.messages_viewport_top()
            && y < self.messages_viewport_bottom()
    }

    pub fn click_code_copy(&self, x: i32, y: i32) -> Option<String> {
        if !self.in_transcript(x, y) {
            return None;
        }
        self.code_block_rects
            .iter()
            .find(|(left, top, right, bottom, _)| {
                x >= *left && x <= *right && y >= *top && y <= *bottom
            })
            .map(|(_, _, _, _, code)| code.clone())
    }

    /// Update link and code-copy hover states.
    pub fn update_link_hover(&mut self, x: i32, y: i32) {
        self.hover.link = self
            .link_rects
            .iter()
            .position(|(left, top, right, bottom, _)| {
                x >= *left && x <= *right && y >= *top && y <= *bottom
            });
        self.hover.code_copy =
            self.code_block_rects
                .iter()
                .position(|(left, top, right, bottom, _)| {
                    x >= *left && x <= *right && y >= *top && y <= *bottom
                });
    }

    // ── Hover handling ───────────────────────────────────────

    // Mirrors pointer_action_at's region layout for hover highlight state.
    #[allow(clippy::too_many_lines)]
    pub fn update_hover(&mut self, x: i32, y: i32) {
        let scale = self.dpi as f32 / 96.0;
        let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let header_h = (SIDEBAR_HEADER_HEIGHT as f32 * scale) as i32;
        let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;

        self.hover = HoverState::default();

        // Settings panel hover
        if self.show_settings {
            let spad = (SPACE_5 as f32 * scale) as i32;
            let close_size = (SPACE_5 as f32 * scale) as i32;
            if x >= self.client_width - spad - close_size
                && x <= self.client_width - spad
                && y >= spad
                && y <= spad + close_size
            {
                self.hover.settings_close = true;
                return;
            }
            if let Some(idx) = self.settings_toggle_at(x, y) {
                self.hover.settings_toggle = Some(idx);
                return;
            }
            let layout = compute_settings_layout(scale);
            let panel_left = self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32;
            if x >= panel_left
                && x <= self.client_width
                && y >= layout.reconnect_y
                && y <= layout.reconnect_y + layout.reconnect_btn_h
            {
                self.hover.reconnect = true;
                return;
            }
        }

        if x < sidebar_w {
            // New Chat button hover
            let btn_top = pad;
            let btn_bottom = btn_top + header_h - pad * 2;
            if x >= pad && x <= sidebar_w - pad && y >= btn_top && y <= btn_bottom {
                self.hover.new_chat = true;
                return;
            }

            // Project-aware sidebar list hover (matches sidebar rendering).
            // The hit test runs inside `with_sidebar_layout` (which borrows the
            // cached entries) and returns a small enum; the hover state is
            // applied afterwards so the borrow is released before mutation.
            let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
            let sidebar_inner_left = pad;
            let sidebar_inner_right = sidebar_w - pad;
            let client_height = self.client_height;
            let sidebar_scroll_offset = self.sidebar_scroll_offset;
            let projects_len = self.projects.len();
            let active_session = self.active_session;
            let hover_hit = self.with_sidebar_layout(|list_top, total_list_height, entries| {
                let list_bottom = client_height - footer_h - pad;
                let avail_height = list_bottom - list_top;
                let max_scroll = (total_list_height - avail_height).max(0);
                let sidebar_scroll = sidebar_scroll_offset.min(max_scroll).max(0);

                // "+ New Project" button sits above list_top
                if let Some(SidebarEntry::NewProjectBtn { top, bottom }) = entries.first() {
                    if y >= *top && y < *bottom && x >= pad && x < sidebar_w - pad {
                        return Some(HoverHit::NewProject);
                    }
                }
                if y < list_top || y >= list_bottom {
                    return None;
                }

                if total_list_height > avail_height {
                    let sb_w = (4.0 * scale).max(3.0) as i32;
                    if x >= sidebar_w - sb_w - 1
                        && x <= sidebar_w
                        && y >= list_top
                        && y <= list_bottom
                    {
                        return Some(HoverHit::SidebarScrollbar);
                    }
                }

                for entry in entries {
                    match *entry {
                        SidebarEntry::ProjectHeader {
                            project_idx,
                            top,
                            bottom,
                        } => {
                            let adj_top = top - sidebar_scroll;
                            let adj_bottom = bottom - sidebar_scroll;
                            if adj_bottom <= list_top || adj_top >= list_bottom {
                                continue;
                            }
                            if y < adj_top || y > adj_bottom {
                                continue;
                            }
                            // Menu button hover (real projects only)
                            if project_idx < projects_len {
                                let menu_btn_size = (PROJECT_MENU_BTN_SIZE as f32 * scale) as i32;
                                let menu_right = sidebar_w - pad - (SPACE_1 as f32 * scale) as i32;
                                let menu_left = menu_right - menu_btn_size;
                                if x >= menu_left && x <= menu_right {
                                    return Some(HoverHit::ProjectMenu(project_idx));
                                }
                            }
                            return Some(HoverHit::Project(project_idx));
                        }
                        SidebarEntry::SessionItem {
                            session_idx,
                            top,
                            bottom,
                            ..
                        } => {
                            let adj_top = top - sidebar_scroll;
                            let adj_bottom = bottom - sidebar_scroll;
                            if adj_bottom <= list_top || adj_top >= list_bottom {
                                continue;
                            }

                            if y < adj_top || y >= adj_bottom {
                                continue;
                            }
                            if session_idx == active_session {
                                return Some(HoverHit::SessionItem(session_idx));
                            }
                            // Only inactive rows paint action buttons.
                            let (archive_rect, delete_rect) = session_action_rects(
                                sidebar_inner_left,
                                sidebar_inner_right,
                                adj_top,
                                item_h,
                                scale,
                            );

                            if point_in_rect(
                                x,
                                y,
                                archive_rect.0,
                                archive_rect.1,
                                archive_rect.2,
                                archive_rect.3,
                            ) {
                                return Some(HoverHit::SessionArchive(session_idx));
                            }
                            if point_in_rect(
                                x,
                                y,
                                delete_rect.0,
                                delete_rect.1,
                                delete_rect.2,
                                delete_rect.3,
                            ) {
                                return Some(HoverHit::SessionDelete(session_idx));
                            }

                            // Session item hover
                            if y >= adj_top && y <= adj_bottom {
                                return Some(HoverHit::SessionItem(session_idx));
                            }
                        }
                        SidebarEntry::NewProjectBtn { .. } => {}
                    }
                }
                None
            });
            match hover_hit {
                Some(HoverHit::NewProject) => {
                    self.hover.new_project = true;
                    return;
                }
                Some(HoverHit::SidebarScrollbar) => {
                    self.hover.sidebar_scrollbar = true;
                    return;
                }
                Some(HoverHit::ProjectMenu(pidx)) => {
                    self.hover.project_menu = Some(pidx);
                    self.hover.project = Some(pidx);
                    return;
                }
                Some(HoverHit::Project(pidx)) => {
                    self.hover.project = Some(pidx);
                    return;
                }
                Some(HoverHit::SessionArchive(sidx)) => {
                    self.hover.session_archive = Some(sidx);
                    self.hover.sidebar_item = Some(sidx);
                    return;
                }
                Some(HoverHit::SessionDelete(sidx)) => {
                    self.hover.session_delete = Some(sidx);
                    self.hover.sidebar_item = Some(sidx);
                    return;
                }
                Some(HoverHit::SessionItem(sidx)) => {
                    self.hover.sidebar_item = Some(sidx);
                    return;
                }
                None => {}
            }

            // Settings button hover
            let settings_btn_top = self.client_height - footer_h + pad / 2;
            let settings_btn_bottom = self.client_height - pad / 2;
            if x >= pad && x <= sidebar_w - pad && y >= settings_btn_top && y <= settings_btn_bottom
            {
                self.hover.settings_btn = true;
            }
        } else {
            // Send/Stop button hover
            if let Some((btn_left, btn_top, btn_right, btn_bottom)) = self.send_btn_rect() {
                if x >= btn_left
                    && x <= btn_right
                    && y >= btn_top
                    && y <= btn_bottom
                    && !self.sessions.is_empty()
                {
                    let idx = self.active_session.min(self.sessions.len() - 1);
                    let chat = &self.sessions[idx].chat;
                    if chat.is_waiting_for_agent || !chat.input_buffer.trim().is_empty() {
                        self.hover.send_btn = true;
                        return;
                    }
                }
            }

            // Session header hover
            let (sh_left, sh_top, sh_right, sh_bottom) = self.session_header_rect();
            if x >= sh_left && x <= sh_right && y >= sh_top && y <= sh_bottom {
                self.hover.session_header = true;
                return;
            }

            // Message scrollbar hover
            if !self.sessions.is_empty() {
                let chat_right = self.chat_right();
                let sb_w = (4.0 * scale).max(3.0) as i32;
                let idx = self.active_session.min(self.sessions.len() - 1);
                let viewport_top = self.messages_viewport_top();
                let viewport_bottom = self.messages_viewport_bottom();
                let viewport = viewport_bottom - viewport_top;
                let total = self.sessions[idx].chat.total_content_height;
                if total > viewport
                    && x >= chat_right - sb_w
                    && x <= chat_right
                    && y >= viewport_top
                    && y <= viewport_bottom
                {
                    self.hover.scrollbar = true;
                    return;
                }
            }

            // Thinking toggle hover
            if !self.sessions.is_empty() {
                let idx = self.active_session.min(self.sessions.len() - 1);
                let chat_left = self.chat_left();
                let chat_right = self.chat_right();
                let chat_w = chat_right - chat_left;
                let viewport_top = self.messages_viewport_top();
                let viewport_bottom = self.messages_viewport_bottom();
                let viewport = viewport_bottom - viewport_top;

                let bubble_padding = (MESSAGE_INNER_PADDING as f32 * scale) as i32;
                let label_height = (SPACE_4 as f32 * scale) as i32;
                let thinking_header_h = (THINKING_HEADER_HEIGHT as f32 * scale) as i32;
                let msg_padding = (MESSAGE_PADDING as f32 * scale) as i32;
                let content_width = chat_w - 2 * msg_padding;
                let max_bubble_width = (content_width as f32 * MESSAGE_MAX_WIDTH_RATIO) as i32;

                let scroll_offset = self.sessions[idx].chat.scroll_offset;
                let visible = {
                    let chat = &self.sessions[idx].chat;
                    chat.visible_messages(viewport, scroll_offset)
                };

                for (msg_idx, y_top) in visible {
                    let msg = &self.sessions[idx].chat.messages[msg_idx];
                    if msg.role == Role::Agent && !msg.thinking.is_empty() {
                        let text_width = max_bubble_width - 2 * bubble_padding;

                        let bubble_width = (text_width + 2 * bubble_padding).min(max_bubble_width);
                        let bubble_left = chat_left + msg_padding;
                        let adjusted_y = y_top + viewport_top;
                        let toggle_top = adjusted_y
                            + bubble_padding
                            + label_height
                            + (SPACE_1 as f32 * scale) as i32;
                        let toggle_bottom = toggle_top + thinking_header_h;
                        let toggle_left = bubble_left + bubble_padding;
                        let toggle_right = bubble_left + bubble_width - bubble_padding;

                        if x >= toggle_left
                            && x <= toggle_right
                            && y >= toggle_top
                            && y <= toggle_bottom
                        {
                            self.hover.thinking_toggle = Some(msg_idx);
                            return;
                        }
                    }
                }
            }
        }
    }

    // ── Layout helpers ───────────────────────────────────────

    pub fn send_btn_rect(&self) -> Option<(i32, i32, i32, i32)> {
        if self.sessions.is_empty() {
            return None;
        }
        let scale = self.dpi as f32 / 96.0;
        let chat_right = self.chat_right();
        let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
        let hint_row_h = (HINT_ROW_HEIGHT as f32 * scale) as i32;
        let send_btn_size = (SEND_BTN_SIZE as f32 * scale) as i32;
        let send_btn_margin = (SEND_BTN_MARGIN as f32 * scale) as i32;

        let idx = self.active_session.min(self.sessions.len() - 1);
        let input_h = (self.sessions[idx].chat.input_height as f32 * scale) as i32;
        let input_area_total_h = input_h + hint_row_h + input_margin;
        let input_box_top = self.client_height - input_area_total_h;

        let input_right = chat_right - input_margin;
        let btn_left = input_right - send_btn_margin - send_btn_size;
        let btn_top = input_box_top + (input_h - send_btn_size) / 2;
        let btn_right = input_right - send_btn_margin;
        let btn_bottom = btn_top + send_btn_size;

        Some((btn_left, btn_top, btn_right, btn_bottom))
    }

    pub fn session_header_rect(&self) -> (i32, i32, i32, i32) {
        let scale = self.dpi as f32 / 96.0;
        let chat_right = self.chat_right();
        let header_h = (SESSION_HEADER_HEIGHT as f32 * scale) as i32;
        (self.chat_left(), 0, chat_right, header_h)
    }

    pub fn messages_viewport_top(&self) -> i32 {
        let scale = self.dpi as f32 / 96.0;
        (SESSION_HEADER_HEIGHT as f32 * scale) as i32
    }

    pub fn messages_viewport_bottom(&self) -> i32 {
        let scale = self.dpi as f32 / 96.0;
        let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
        let hint_row_h = (HINT_ROW_HEIGHT as f32 * scale) as i32;
        if self.sessions.is_empty() {
            return self.client_height - input_margin - hint_row_h - input_margin;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        let input_h = (self.sessions[idx].chat.input_height as f32 * scale) as i32;
        self.client_height - input_h - hint_row_h - input_margin - input_margin
    }

    pub fn input_box_top(&self) -> i32 {
        let scale = self.dpi as f32 / 96.0;
        let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
        let hint_row_h = (HINT_ROW_HEIGHT as f32 * scale) as i32;
        if self.sessions.is_empty() {
            return self.client_height - (44.0 * scale) as i32 - hint_row_h - input_margin;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        let input_h = (self.sessions[idx].chat.input_height as f32 * scale) as i32;
        self.client_height - input_h - hint_row_h - input_margin
    }

    fn scrollbar_at(&self, x: i32, y: i32) -> bool {
        if self.sessions.is_empty() {
            return false;
        }
        let scale = self.dpi as f32 / 96.0;
        let chat_right = self.chat_right();
        let sb_w = (4.0 * scale).max(3.0) as i32;
        let idx = self.active_session.min(self.sessions.len() - 1);
        let viewport_top = self.messages_viewport_top();
        let viewport_bottom = self.messages_viewport_bottom();
        let viewport = viewport_bottom - viewport_top;
        let total = self.sessions[idx].chat.total_content_height;
        if total <= viewport {
            return false;
        }
        x >= chat_right - sb_w && x <= chat_right && y >= viewport_top && y <= viewport_bottom
    }

    fn jump_to_scrollbar_position(&mut self, _x: i32, y: i32) -> bool {
        if self.sessions.is_empty() {
            return false;
        }
        let scale = self.dpi as f32 / 96.0;
        let idx = self.active_session.min(self.sessions.len() - 1);
        let viewport_top = self.messages_viewport_top();
        let viewport_bottom = self.messages_viewport_bottom();
        let viewport = viewport_bottom - viewport_top;
        let total = self.sessions[idx].chat.total_content_height;
        if total <= viewport {
            return false;
        }

        let max_scroll = (total - viewport).max(0);
        let thumb_ratio = viewport as f32 / total as f32;
        let thumb_h = ((viewport as f32 * thumb_ratio).max(12.0 * scale)) as i32;
        let track_h = viewport - thumb_h;
        if track_h <= 0 {
            return false;
        }

        let click_offset = (y - viewport_top - thumb_h / 2).max(0).min(track_h);
        let click_ratio = click_offset as f32 / track_h as f32;
        let new_scroll = (click_ratio * max_scroll as f32) as i32;

        let chat = &mut self.sessions[idx].chat;
        chat.scroll_offset = new_scroll.min(max_scroll).max(0);
        chat.is_at_bottom = chat.scroll_offset >= max_scroll;
        true
    }

    fn settings_toggle_at(&self, x: i32, y: i32) -> Option<usize> {
        let scale = self.dpi as f32 / 96.0;
        let panel_left = self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32;
        let spad = (SPACE_5 as f32 * scale) as i32;
        let layout = compute_settings_layout(scale);

        if x >= panel_left + spad && x <= self.client_width - spad {
            for (idx, &top) in layout.toggle_tops.iter().enumerate() {
                if idx == 0 || idx == 2 {
                    continue;
                }
                if y >= top && y <= top + layout.toggle_row_h {
                    return Some(idx);
                }
            }
        }
        None
    }

    fn thinking_message_at(&self, x: i32, y: i32) -> Option<u64> {
        if !self.in_transcript(x, y) || self.sessions.is_empty() {
            return None;
        }
        let scale = self.dpi as f32 / 96.0;
        let idx = self.active_session.min(self.sessions.len() - 1);
        let chat_left = self.chat_left();
        let chat_right = self.chat_right();
        let chat_w = chat_right - chat_left;
        let viewport_top = self.messages_viewport_top();
        let viewport_bottom = self.messages_viewport_bottom();
        let viewport = viewport_bottom - viewport_top;

        let bubble_padding = (MESSAGE_INNER_PADDING as f32 * scale) as i32;
        let label_height = (SPACE_4 as f32 * scale) as i32;
        let thinking_header_h = (THINKING_HEADER_HEIGHT as f32 * scale) as i32;
        let msg_padding = (MESSAGE_PADDING as f32 * scale) as i32;
        let content_width = chat_w - 2 * msg_padding;
        let max_bubble_width = (content_width as f32 * MESSAGE_MAX_WIDTH_RATIO) as i32;

        let scroll_offset = self.sessions[idx].chat.scroll_offset;
        let visible = {
            let chat = &self.sessions[idx].chat;
            chat.visible_messages(viewport, scroll_offset)
        };

        for (msg_idx, y_top) in visible {
            let msg = &self.sessions[idx].chat.messages[msg_idx];
            if msg.role == Role::Agent && !msg.thinking.is_empty() {
                let text_width = max_bubble_width - 2 * bubble_padding;
                let bubble_width = (text_width + 2 * bubble_padding).min(max_bubble_width);
                let bubble_left = chat_left + msg_padding;
                let adjusted_y = y_top + viewport_top;
                let toggle_top =
                    adjusted_y + bubble_padding + label_height + (SPACE_1 as f32 * scale) as i32;
                let toggle_bottom = toggle_top + thinking_header_h;
                let toggle_left = bubble_left + bubble_padding;
                let toggle_right = bubble_left + bubble_width - bubble_padding;

                if x >= toggle_left && x <= toggle_right && y >= toggle_top && y <= toggle_bottom {
                    return Some(msg.id);
                }
            }
        }
        None
    }
}

// ── Session action button geometry ───────────────────────────

/// A rectangle in `(left, top, right, bottom)` order.
pub type Rect = (i32, i32, i32, i32);

/// Compute the rects for the archive and delete buttons on a session sidebar
/// item. Returns `(archive_rect, delete_rect)`.
pub fn session_action_rects(
    _item_left: i32,
    item_right: i32,
    item_top: i32,
    item_height: i32,
    scale: f32,
) -> (Rect, Rect) {
    let btn_size = (SESSION_ACTION_BTN_SIZE as f32 * scale) as i32;
    let btn_gap = (SESSION_ACTION_BTN_GAP as f32 * scale) as i32;
    let pad = (SIDEBAR_PADDING as f32 * scale) as i32;

    let delete_right = item_right - pad;
    let delete_left = delete_right - btn_size;
    let archive_right = delete_left - btn_gap;
    let archive_left = archive_right - btn_size;
    let btn_top = item_top + (item_height - btn_size) / 2;
    let btn_bottom = btn_top + btn_size;

    (
        (archive_left, btn_top, archive_right, btn_bottom),
        (delete_left, btn_top, delete_right, btn_bottom),
    )
}

/// Check if point (x, y) is inside a rect (left, top, right, bottom).
pub const fn point_in_rect(x: i32, y: i32, left: i32, top: i32, right: i32, bottom: i32) -> bool {
    x >= left && x < right && y >= top && y < bottom
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{ActiveTurn, Session};

    // In-memory only: never load/save real data or start provider/agent workers.
    fn app() -> AppState {
        let mut state = AppState::new(500);
        state.sessions.push(Session::new("", "First", "."));
        state.sessions.push(Session::new("", "Second", "."));
        state.invalidate_sidebar_cache();
        state
    }

    fn button_center(state: &AppState) -> (i32, i32) {
        let (left, top, right, bottom) = state.send_btn_rect().unwrap();
        ((left + right) / 2, (top + bottom) / 2)
    }

    fn new_project_center(state: &mut AppState) -> (i32, i32) {
        state.with_sidebar_layout(|_, _, entries| {
            entries
                .iter()
                .find_map(|entry| match entry {
                    SidebarEntry::NewProjectBtn { top, bottom } => Some((50, (top + bottom) / 2)),
                    _ => None,
                })
                .unwrap()
        })
    }

    fn row_center(state: &mut AppState, idx: usize) -> (i32, i32) {
        state.with_sidebar_layout(|_, _, entries| {
            entries
                .iter()
                .find_map(|entry| match entry {
                    SidebarEntry::SessionItem {
                        session_idx,
                        top,
                        bottom,
                        ..
                    } if *session_idx == idx => Some((50, (top + bottom) / 2)),
                    _ => None,
                })
                .unwrap()
        })
    }

    fn set_turn(state: &mut AppState, turn_id: u64) {
        let session = &mut state.sessions[state.active_session];
        session.active_turn = Some(ActiveTurn {
            key: TurnKey {
                local_id: session.local_id.clone(),
                turn_id,
            },
            backend: SessionMode::Direct,
            text: "Question".into(),
            submitted: true,
            session_requested: false,
            cancelling: false,
            started: std::time::Instant::now(),
        });
        session.chat.is_waiting_for_agent = true;
    }

    #[test]
    fn button_activates_once_on_release_not_down() {
        let mut state = app();
        let (x, y) = new_project_center(&mut state);
        assert!(state.begin_pointer_press(x, y, false));
        assert!(!state.new_project_requested);
        state.finish_pointer_press(x, y);
        assert!(state.new_project_requested);
        state.new_project_requested = false;
        state.finish_pointer_press(x, y);
        assert!(!state.new_project_requested);
    }

    #[test]
    fn drag_out_and_capture_cancellation_never_activate() {
        let mut state = app();
        let (x, y) = new_project_center(&mut state);
        state.begin_pointer_press(x, y, false);
        state.update_pointer_press(-1, y);
        state.update_pointer_press(x, y);
        state.finish_pointer_press(x, y);
        assert!(!state.new_project_requested);
        state.begin_pointer_press(x, y, false);
        state.cancel_pointer_press();
        state.finish_pointer_press(x, y);
        assert!(!state.new_project_requested);
        state.begin_pointer_press(x, y, false);
        state.finish_pointer_press(x, -1);
        assert!(!state.new_project_requested);
    }

    #[test]
    fn delete_emits_confirmation_target_without_removing_chat() {
        let mut state = app();
        let (_, y) = row_center(&mut state, 1);
        let (x, _, _, _) = session_action_rects(
            SIDEBAR_PADDING,
            SIDEBAR_WIDTH - SIDEBAR_PADDING,
            y - SIDEBAR_ITEM_HEIGHT / 2,
            SIDEBAR_ITEM_HEIGHT,
            1.0,
        )
        .1;
        let id = state.sessions[1].local_id.clone();
        assert!(state.begin_pointer_press(x + 1, y, false));
        assert_eq!(state.sessions.len(), 2);
        assert!(
            matches!(state.finish_pointer_press(x + 1, y), Some(PointerEffect::DeleteSession(target)) if target == id)
        );
        assert_eq!(state.sessions.len(), 2);
    }

    #[test]
    fn row_index_reuse_does_not_retarget_press() {
        let mut state = app();
        let (x, y) = row_center(&mut state, 1);
        state.begin_pointer_press(x, y, false);
        // Even without layout invalidation, reordering cannot retarget a press.
        state.sessions.swap(0, 1);
        state.finish_pointer_press(x, y);
        assert_eq!(state.active_session, 0);
    }

    #[test]
    fn stop_becoming_send_does_not_submit_next_draft() {
        let mut state = app();
        state.sessions[0].chat.insert_str("Keep next draft");
        set_turn(&mut state, 1);
        let (x, y) = button_center(&state);
        state.begin_pointer_press(x, y, false);
        state.sessions[0].active_turn = None;
        state.sessions[0].chat.is_waiting_for_agent = false;
        state.finish_pointer_press(x, y);
        assert_eq!(state.sessions[0].chat.input_buffer, "Keep next draft");
        assert!(state.sessions[0].active_turn.is_none());
    }

    #[test]
    fn send_becoming_stop_does_not_cancel_new_turn() {
        let mut state = app();
        state.sessions[0].chat.insert_str("Keep next draft");
        let (x, y) = button_center(&state);
        state.begin_pointer_press(x, y, false);
        set_turn(&mut state, 1);
        state.finish_pointer_press(x, y);
        assert!(!state.sessions[0].active_turn.as_ref().unwrap().cancelling);
        assert_eq!(state.sessions[0].chat.input_buffer, "Keep next draft");
    }

    #[test]
    fn stop_press_is_bound_to_original_turn_not_just_session() {
        let mut state = app();
        set_turn(&mut state, 1);
        let (x, y) = button_center(&state);
        state.begin_pointer_press(x, y, false);
        set_turn(&mut state, 2);
        state.finish_pointer_press(x, y);
        assert!(!state.sessions[0].active_turn.as_ref().unwrap().cancelling);
    }

    #[test]
    fn double_click_cannot_turn_send_into_stop() {
        let mut state = app();
        set_turn(&mut state, 1);
        let (x, y) = button_center(&state);
        assert!(!state.begin_pointer_press(x, y, true));
        state.finish_pointer_press(x, y);
        assert!(!state.sessions[0].active_turn.as_ref().unwrap().cancelling);
    }

    #[test]
    fn session_double_click_renames_only_after_release() {
        let mut state = app();
        let (x, y) = row_center(&mut state, 1);
        state.begin_pointer_press(x, y, false);
        state.finish_pointer_press(x, y);
        assert_eq!(state.active_session, 1);
        assert!(state.begin_pointer_press(x, y, true));
        assert!(state.rename_target.is_none());
        state.finish_pointer_press(x, y);
        assert_eq!(state.rename_target, Some(RenameTarget::Session(1)));
    }

    #[test]
    fn changed_draft_or_session_cancels_send_press() {
        let mut state = app();
        state.sessions[0].chat.insert_str("First draft");
        state.sessions[1].chat.insert_str("Other draft");
        let (x, y) = button_center(&state);
        state.begin_pointer_press(x, y, false);
        state.sessions[0].chat.insert_char('!');
        state.finish_pointer_press(x, y);
        assert_eq!(state.sessions[0].chat.input_buffer, "First draft!");
        state.begin_pointer_press(x, y, false);
        state.active_session = 1;
        state.finish_pointer_press(x, y);
        assert_eq!(state.sessions[1].chat.input_buffer, "Other draft");
        assert!(state
            .sessions
            .iter()
            .all(|session| session.active_turn.is_none()));
    }

    #[test]
    fn rectangles_are_half_open() {
        assert!(point_in_rect(0, 0, 0, 0, 10, 10));
        assert!(!point_in_rect(10, 0, 0, 0, 10, 10));
        assert!(!point_in_rect(0, 10, 0, 0, 10, 10));
    }
}
