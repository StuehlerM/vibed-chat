// User interaction: mouse click handling, hover tracking, keyboard focus cycling, layout computation.
// This module is platform-independent - it operates on abstract coordinates and
// app state. Platform-specific dialogs (folder picker, etc.) are delegated to
// the platform backend trait.
#![allow(dead_code)]

use crate::app::{AppState, HoverState};
use crate::chat::Role;
use crate::theme::{
    compute_settings_layout, HINT_ROW_HEIGHT, INPUT_MARGIN, MESSAGE_INNER_PADDING,
    MESSAGE_MAX_WIDTH_RATIO, MESSAGE_PADDING, PROJECT_HEADER_HEIGHT, PROJECT_MENU_BTN_SIZE,
    SEND_BTN_MARGIN, SEND_BTN_SIZE, SESSION_ACTION_BTN_GAP, SESSION_ACTION_BTN_SIZE,
    SESSION_HEADER_HEIGHT, SETTINGS_PANEL_WIDTH, SIDEBAR_FOOTER_HEIGHT, SIDEBAR_HEADER_HEIGHT,
    SIDEBAR_ITEM_HEIGHT, SIDEBAR_ITEM_MARGIN, SIDEBAR_PADDING, SIDEBAR_WIDTH, SPACE_1, SPACE_4,
    SPACE_5, THINKING_HEADER_HEIGHT,
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

/// What a double-click on the sidebar list hit, computed inside
/// `with_sidebar_layout` and applied afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoubleClickHit {
    ProjectHeader(usize),
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

impl AppState {
    pub fn clear_hover(&mut self) {
        self.hover = HoverState::default();
    }

    pub fn cycle_focus(&mut self, shift: bool) {
        let mut order: Vec<FocusedElement> = Vec::new();
        order.push(FocusedElement::NewChat);
        order.push(FocusedElement::NewProject);
        for i in 0..self.projects.len() {
            order.push(FocusedElement::ProjectHeader(i));
        }
        for i in 0..self.sessions.len() {
            order.push(FocusedElement::SidebarItem(i));
        }
        order.push(FocusedElement::SettingsBtn);
        order.push(FocusedElement::Input);
        order.push(FocusedElement::SendButton);
        if self.show_settings {
            order.push(FocusedElement::SettingsClose);
            order.push(FocusedElement::SettingsToggle(0));
            order.push(FocusedElement::SettingsToggle(1));
            order.push(FocusedElement::SettingsToggle(2));
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

        if self.focused_element == Some(FocusedElement::Input) && !self.sessions.is_empty() {
            let idx = self.active_session.min(self.sessions.len() - 1);
            self.sessions[idx].chat.input_focused = true;
        }
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
                self.connect_acp();
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

        if !entries.is_empty() {
            total_h = total_h.saturating_sub(item_gap).max(0);
        }

        (list_top, total_h, entries)
    }

    // ── Click handling ───────────────────────────────────────

    // Central mouse dispatch: one match arm per UI region/element.
    #[allow(clippy::too_many_lines)]
    pub fn handle_click(&mut self, x: i32, y: i32) -> bool {
        if self.rename_target.is_some() {
            let scale = self.dpi as f32 / 96.0;
            let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
            let is_in_sidebar = x < sidebar_w;
            if is_in_sidebar {
                let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
                let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;
                let rename_target = self.rename_target;
                let sidebar_scroll_offset = self.sidebar_scroll_offset;
                let client_height = self.client_height;
                let click_on_renaming =
                    self.with_sidebar_layout(|list_top, total_list_height, entries| {
                        let list_bottom = client_height - footer_h - pad;
                        let avail_height = list_bottom - list_top;
                        let max_scroll = (total_list_height - avail_height).max(0);
                        let sidebar_scroll = sidebar_scroll_offset.min(max_scroll).max(0);
                        entries.iter().any(|entry| match (rename_target, entry) {
                            (
                                Some(RenameTarget::Session(idx)),
                                SidebarEntry::SessionItem {
                                    session_idx,
                                    top,
                                    bottom,
                                    ..
                                },
                            ) => {
                                *session_idx == idx
                                    && y >= top - sidebar_scroll
                                    && y <= bottom - sidebar_scroll
                            }
                            (
                                Some(RenameTarget::Project(pidx)),
                                SidebarEntry::ProjectHeader {
                                    project_idx,
                                    top,
                                    bottom,
                                },
                            ) => {
                                *project_idx == pidx
                                    && y >= top - sidebar_scroll
                                    && y <= bottom - sidebar_scroll
                            }
                            _ => false,
                        })
                    });
                if !click_on_renaming {
                    self.cancel_rename();
                }
            } else {
                self.cancel_rename();
            }
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
                self.show_settings = false;
                return true;
            }

            if x >= panel_left {
                let layout = compute_settings_layout(scale);
                if y >= layout.reconnect_y && y <= layout.reconnect_y + layout.reconnect_btn_h {
                    self.connect_acp();
                    return true;
                }

                if let Some(idx) = self.settings_toggle_at(x, y) {
                    self.toggle_setting(idx);
                    return true;
                }
                return true;
            }
        }

        // Sidebar clicks
        if x < sidebar_w {
            // New Chat button
            let btn_top = pad;
            let btn_bottom = btn_top + header_h - pad * 2;
            if x >= pad && x <= sidebar_w - pad && y >= btn_top && y <= btn_bottom {
                self.pressed_element = Some(PressedElement::NewChat);
                self.new_chat();
                return true;
            }

            // Project-aware sidebar list (matches sidebar rendering)
            let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;
            let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
            let sidebar_inner_left = pad;
            let sidebar_inner_right = sidebar_w - pad;
            let client_height = self.client_height;
            let sidebar_scroll_offset = self.sidebar_scroll_offset;
            let projects_len = self.projects.len();
            let hit = self.with_sidebar_layout(|list_top, total_list_height, entries| {
                let list_bottom = client_height - footer_h - pad;
                let avail_height = list_bottom - list_top;
                let max_scroll = (total_list_height - avail_height).max(0);
                let sidebar_scroll = sidebar_scroll_offset.min(max_scroll).max(0);

                // "+ New Project" button sits above list_top
                if let Some(SidebarEntry::NewProjectBtn { top, bottom }) = entries.first() {
                    let adj_top = top - sidebar_scroll;
                    let adj_bottom = bottom - sidebar_scroll;
                    if y >= adj_top && y <= adj_bottom && x >= pad && x <= sidebar_w - pad {
                        return Some(SidebarHit::NewProject);
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

                            // Check archive/delete buttons first (they're on top of the item)
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
            match hit {
                Some(SidebarHit::NewProject) => {
                    self.pressed_element = Some(PressedElement::NewProject);
                    self.new_project_requested = true;
                    return true;
                }
                Some(SidebarHit::ProjectMenu(pidx)) => {
                    self.pressed_element = Some(PressedElement::ProjectMenu(pidx));
                    self.project_menu_requested = Some(pidx);
                    return true;
                }
                Some(SidebarHit::ProjectHeader(pidx)) => {
                    self.pressed_element = Some(PressedElement::ProjectHeader(pidx));
                    self.toggle_project(pidx);
                    return true;
                }
                Some(SidebarHit::SessionArchive(sidx)) => {
                    self.pressed_element = Some(PressedElement::SessionArchive(sidx));
                    self.toggle_archive_session(sidx);
                    return true;
                }
                Some(SidebarHit::SessionDelete(sidx)) => {
                    self.pressed_element = Some(PressedElement::SessionDelete(sidx));
                    self.delete_session(sidx);
                    return true;
                }
                Some(SidebarHit::SessionItem(sidx)) => {
                    self.pressed_element = Some(PressedElement::SidebarItem(sidx));
                    self.switch_session(sidx);
                    return true;
                }
                None => {}
            }

            // Settings button
            let settings_btn_top = self.client_height - footer_h + pad / 2;
            let settings_btn_bottom = self.client_height - pad / 2;
            if x >= pad && x <= sidebar_w - pad && y >= settings_btn_top && y <= settings_btn_bottom
            {
                self.pressed_element = Some(PressedElement::SettingsBtn);
                self.show_settings = !self.show_settings;
                return true;
            }

            return true;
        }

        // Send/Stop button click
        if let Some((btn_left, btn_top, btn_right, btn_bottom)) = self.send_btn_rect() {
            if x >= btn_left && x <= btn_right && y >= btn_top && y <= btn_bottom {
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
                return true;
            }
        }

        // Message scrollbar click/drag
        if self.scrollbar_at(x, y) {
            self.jump_to_scrollbar_position(x, y);
            return true;
        }

        // Thinking toggle clicks
        if self.thinking_toggle_at(x, y) {
            return true;
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
            let input_h = (self.sessions[idx].chat.input_height as f32 * scale) as i32;
            let input_bottom = input_box_top + input_h;

            if x >= input_left
                && x <= input_right
                && y >= input_box_top
                && y <= input_bottom
                && !self.sessions.is_empty()
            {
                let idx = self.active_session.min(self.sessions.len() - 1);
                self.sessions[idx].chat.input_focused = true;
                self.focused_element = Some(FocusedElement::Input);
            }
        }
        false
    }

    /// Check if a click landed on a link rect. Returns the URL if so.
    pub fn click_link(&self, x: i32, y: i32) -> Option<String> {
        self.link_rects
            .iter()
            .find(|(left, top, right, bottom, _)| {
                x >= *left && x <= *right && y >= *top && y <= *bottom
            })
            .map(|(_, _, _, _, url)| url.clone())
    }

    /// Check if a click landed on a code block copy button. Returns the code text if so.
    pub fn click_code_copy(&self, x: i32, y: i32) -> Option<String> {
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

    /// Handle a double-click. Returns true if handled.
    /// Double-clicking a session starts inline rename; double-clicking a
    /// project header toggles expand/collapse.
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> bool {
        let scale = self.dpi as f32 / 96.0;
        let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
        if x >= sidebar_w {
            return false;
        }

        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;
        let client_height = self.client_height;
        let sidebar_scroll_offset = self.sidebar_scroll_offset;
        let projects_len = self.projects.len();
        let hit = self.with_sidebar_layout(|list_top, total_list_height, entries| {
            let list_bottom = client_height - footer_h - pad;
            let avail_height = list_bottom - list_top;
            let max_scroll = (total_list_height - avail_height).max(0);
            let sidebar_scroll = sidebar_scroll_offset.min(max_scroll).max(0);

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
                        if y >= adj_top && y <= adj_bottom {
                            return Some(DoubleClickHit::ProjectHeader(project_idx));
                        }
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
                        if y >= adj_top && y <= adj_bottom {
                            return Some(DoubleClickHit::SessionItem(session_idx));
                        }
                    }
                    SidebarEntry::NewProjectBtn { .. } => {}
                }
            }
            None
        });
        match hit {
            Some(DoubleClickHit::ProjectHeader(pidx)) => {
                if pidx < projects_len {
                    self.toggle_project(pidx);
                }
                true
            }
            Some(DoubleClickHit::SessionItem(sidx)) => {
                self.start_rename(sidx);
                true
            }
            None => false,
        }
    }

    // ── Hover handling ───────────────────────────────────────

    // Mirrors handle_click's region layout for hover highlight state.
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
            let hover_hit = self.with_sidebar_layout(|list_top, total_list_height, entries| {
                let list_bottom = client_height - footer_h - pad;
                let avail_height = list_bottom - list_top;
                let max_scroll = (total_list_height - avail_height).max(0);
                let sidebar_scroll = sidebar_scroll_offset.min(max_scroll).max(0);

                // "+ New Project" button sits above list_top
                if let Some(SidebarEntry::NewProjectBtn { top, bottom }) = entries.first() {
                    let adj_top = top - sidebar_scroll;
                    let adj_bottom = bottom - sidebar_scroll;
                    if y >= adj_top && y <= adj_bottom && x >= pad && x <= sidebar_w - pad {
                        return Some(HoverHit::NewProject);
                    }
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

                            // Check archive/delete button hover first
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
                if y >= top && y <= top + layout.toggle_row_h {
                    return Some(idx);
                }
            }
        }
        None
    }

    fn thinking_toggle_at(&mut self, x: i32, y: i32) -> bool {
        if self.sessions.is_empty() {
            return false;
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
                    self.sessions[idx].chat.messages[msg_idx].thinking_collapsed =
                        !self.sessions[idx].chat.messages[msg_idx].thinking_collapsed;
                    return true;
                }
            }
        }
        false
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
    x >= left && x <= right && y >= top && y <= bottom
}
