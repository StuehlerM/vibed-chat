// User interaction: mouse click handling, hover tracking, keyboard focus cycling, layout computation.
#![allow(dead_code)]

use crate::app::AppState;
use crate::chat::Role;
use crate::theme::*;
use crate::ui::{FocusedElement, PressedElement};
use crate::win32::*;

/// A sidebar list entry — either a project header or a session item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarEntry {
    ProjectHeader { project_idx: usize, top: i32, bottom: i32 },
    SessionItem { session_idx: usize, project_idx: usize, top: i32, bottom: i32 },
    NewProjectBtn { top: i32, bottom: i32 },
}

impl AppState {
    pub fn clear_hover(&mut self) {
        self.hover_sidebar_item = None;
        self.hover_new_chat = false;
        self.hover_new_project = false;
        self.hover_project = None;
        self.hover_project_menu = None;
        self.hover_settings_btn = false;
        self.hover_settings_close = false;
        self.hover_thinking_toggle = None;
        self.hover_settings_toggle = None;
        self.hover_scrollbar = false;
        self.hover_sidebar_scrollbar = false;
        self.hover_send_btn = false;
        self.hover_session_header = false;
        self.hover_session_archive = None;
        self.hover_session_delete = None;
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

        let next = match pos {
            Some(p) => {
                if shift {
                    if p == 0 { order.len() - 1 } else { p - 1 }
                } else {
                    (p + 1) % order.len()
                }
            }
            None => {
                if shift { order.len() - 1 } else { 0 }
            }
        };

        self.focused_element = Some(order[next]);

        if let Some(FocusedElement::Input) = self.focused_element {
            if !self.sessions.is_empty() {
                let idx = self.active_session.min(self.sessions.len() - 1);
                self.sessions[idx].chat.input_focused = true;
            }
        }
    }

    // ── Sidebar layout ───────────────────────────────────────

    pub fn sidebar_layout(&self) -> (i32, i32, Vec<SidebarEntry>) {
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

        entries.push(SidebarEntry::NewProjectBtn { top: new_proj_top, bottom: new_proj_bottom });
        total_h += new_proj_h + group_gap;

        for (pidx, project) in self.projects.iter().enumerate() {
            entries.push(SidebarEntry::ProjectHeader {
                project_idx: pidx,
                top: y,
                bottom: y + proj_h,
            });
            y += proj_h;
            total_h += proj_h;

            if project.expanded {
                for &sidx in &project.session_indices {
                    entries.push(SidebarEntry::SessionItem {
                        session_idx: sidx,
                        project_idx: pidx,
                        top: y,
                        bottom: y + item_h,
                    });
                    y += item_h + item_gap;
                    total_h += item_h + item_gap;
                }
            }
            y += group_gap;
            total_h += group_gap;
        }

        if !entries.is_empty() {
            total_h = total_h.saturating_sub(item_gap);
        }

        (list_top, total_h, entries)
    }

    /// Compute the date-grouped sidebar layout matching the rendering in sidebar.rs.
    /// Returns (list_top, total_height, Vec<(session_idx, top, bottom)>).
    /// Archived sessions are included only if show_archived is enabled.
    pub fn date_grouped_sidebar_layout(&self) -> (i32, i32, Vec<(usize, i32, i32)>) {
        let scale = self.dpi as f32 / 96.0;
        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let header_h = (SIDEBAR_HEADER_HEIGHT as f32 * scale) as i32;
        let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
        let item_gap = (SIDEBAR_ITEM_MARGIN as f32 * scale) as i32;
        let group_h = (SIDEBAR_GROUP_HEIGHT as f32 * scale) as i32;
        let group_gap = (SPACE_2 as f32 * scale) as i32;

        let btn_bottom = pad + header_h - pad * 2;
        let list_top = btn_bottom + pad;

        // Build date groups from visible sessions
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for i in self.visible_session_indices() {
            let label = crate::text::date_group_label(&self.sessions[i].created_at);
            if let Some((last_label, indices)) = groups.last_mut() {
                if *last_label == label {
                    indices.push(i);
                    continue;
                }
            }
            groups.push((label, vec![i]));
        }

        let mut y = list_top;
        let mut items = Vec::new();
        let mut total_h = 0i32;

        for (_label, indices) in &groups {
            y += group_h;
            total_h += group_h;
            for &sidx in indices {
                items.push((sidx, y, y + item_h));
                y += item_h + item_gap;
                total_h += item_h + item_gap;
            }
            y += group_gap;
            total_h += group_gap;
        }

        if !items.is_empty() {
            total_h = total_h.saturating_sub(item_gap);
        }

        (list_top, total_h, items)
    }

    // ── Click handling ───────────────────────────────────────

    pub fn handle_click(&mut self, x: i32, y: i32) -> bool {
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
                let reconnect_y = self.reconnect_button_y();
                let btn_h = (36.0 * scale) as i32;
                if y >= reconnect_y && y <= reconnect_y + btn_h {
                    self.connect_acp();
                    return true;
                }

                let toggle = self.settings_toggle_at(x, y);
                if let Some(idx) = toggle {
                    match idx {
                        0 => self.settings.dark_theme = !self.settings.dark_theme,
                        1 => self.settings.auto_scroll = !self.settings.auto_scroll,
                        2 => self.settings.auto_approve = !self.settings.auto_approve,
                        3 => self.settings.show_archived = !self.settings.show_archived,
                        _ => {}
                    }
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

            // Date-grouped session list (matches sidebar rendering)
            let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;
            let (list_top, total_list_height, session_items) = self.date_grouped_sidebar_layout();
            let list_bottom = self.client_height - footer_h - pad;
            let avail_height = list_bottom - list_top;
            let max_scroll = (total_list_height - avail_height).max(0);
            let sidebar_scroll = self.sidebar_scroll_offset.min(max_scroll).max(0);

            let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
            let sidebar_inner_left = pad;
            let sidebar_inner_right = sidebar_w - pad;

            for &(session_idx, item_top, item_bottom) in &session_items {
                let adj_top = item_top - sidebar_scroll;
                let adj_bottom = item_bottom - sidebar_scroll;
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

                if point_in_rect(x, y, archive_rect.0, archive_rect.1, archive_rect.2, archive_rect.3) {
                    self.pressed_element = Some(PressedElement::SessionArchive(session_idx));
                    self.toggle_archive_session(session_idx);
                    return true;
                }
                if point_in_rect(x, y, delete_rect.0, delete_rect.1, delete_rect.2, delete_rect.3) {
                    self.pressed_element = Some(PressedElement::SessionDelete(session_idx));
                    self.delete_session(session_idx);
                    return true;
                }

                // Check if click is on the session item itself
                if y >= adj_top && y <= adj_bottom {
                    self.pressed_element = Some(PressedElement::SidebarItem(session_idx));
                    self.switch_session(session_idx);
                    return true;
                }
            }

            // Settings button
            let settings_btn_top = self.client_height - footer_h + pad / 2;
            let settings_btn_bottom = self.client_height - pad / 2;
            if x >= pad && x <= sidebar_w - pad && y >= settings_btn_top && y <= settings_btn_bottom {
                self.pressed_element = Some(PressedElement::SettingsBtn);
                self.show_settings = !self.show_settings;
                return true;
            }

            return true;
        }

        // Send button click
        if let Some((btn_left, btn_top, btn_right, btn_bottom)) = self.send_btn_rect() {
            if x >= btn_left && x <= btn_right && y >= btn_top && y <= btn_bottom {
                if !self.sessions.is_empty() {
                    let idx = self.active_session.min(self.sessions.len() - 1);
                    let chat = &mut self.sessions[idx].chat;
                    if !chat.input_buffer.trim().is_empty() && !chat.is_waiting_for_agent {
                        let text = chat.take_input();
                        self.send_message(text);
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

        // Click in chat area — focus input
        {
            let input_box_top = self.input_box_top();
            let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
            let chat_left = sidebar_w;
            let chat_right = if self.show_settings {
                self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
            } else {
                self.client_width
            };
            let input_left = chat_left + input_margin;
            let input_right = chat_right - input_margin;
            let idx = self.active_session.min(self.sessions.len().saturating_sub(1));
            let input_h = (self.sessions[idx].chat.input_height as f32 * scale) as i32;
            let input_bottom = input_box_top + input_h;

            if x >= input_left && x <= input_right && y >= input_box_top && y <= input_bottom
                && !self.sessions.is_empty() {
                    let idx = self.active_session.min(self.sessions.len() - 1);
                    self.sessions[idx].chat.input_focused = true;
                    self.focused_element = Some(FocusedElement::Input);
                }
        }
        false
    }

    // ── Hover handling ───────────────────────────────────────

    pub fn update_hover(&mut self, x: i32, y: i32) {
        let scale = self.dpi as f32 / 96.0;
        let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let header_h = (SIDEBAR_HEADER_HEIGHT as f32 * scale) as i32;
        let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;

        self.hover_sidebar_item = None;
        self.hover_new_chat = false;
        self.hover_new_project = false;
        self.hover_project = None;
        self.hover_project_menu = None;
        self.hover_settings_btn = false;
        self.hover_settings_close = false;
        self.hover_thinking_toggle = None;
        self.hover_settings_toggle = None;
        self.hover_scrollbar = false;
        self.hover_sidebar_scrollbar = false;
        self.hover_send_btn = false;
        self.hover_session_header = false;
        self.hover_session_archive = None;
        self.hover_session_delete = None;

        // Settings panel hover
        if self.show_settings {
            let spad = (SPACE_5 as f32 * scale) as i32;
            let close_size = (SPACE_5 as f32 * scale) as i32;
            if x >= self.client_width - spad - close_size
                && x <= self.client_width - spad
                && y >= spad
                && y <= spad + close_size
            {
                self.hover_settings_close = true;
                return;
            }
            if let Some(idx) = self.settings_toggle_at(x, y) {
                self.hover_settings_toggle = Some(idx);
                return;
            }
            let reconnect_y = self.reconnect_button_y();
            let btn_h = (36.0 * scale) as i32;
            let panel_left = self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32;
            if x >= panel_left && x <= self.client_width && y >= reconnect_y && y <= reconnect_y + btn_h {
                return;
            }
        }

        if x < sidebar_w {
            // New Chat button hover
            let btn_top = pad;
            let btn_bottom = btn_top + header_h - pad * 2;
            if x >= pad && x <= sidebar_w - pad && y >= btn_top && y <= btn_bottom {
                self.hover_new_chat = true;
                return;
            }

            // Date-grouped session list hover (matches sidebar rendering)
            let (list_top, total_list_height, session_items) = self.date_grouped_sidebar_layout();
            let list_bottom = self.client_height - footer_h - pad;
            let avail_height = list_bottom - list_top;
            let max_scroll = (total_list_height - avail_height).max(0);
            let sidebar_scroll = self.sidebar_scroll_offset.min(max_scroll).max(0);

            if total_list_height > avail_height {
                let sb_w = (4.0 * scale).max(3.0) as i32;
                if x >= sidebar_w - sb_w - 1 && x <= sidebar_w && y >= list_top && y <= list_bottom {
                    self.hover_sidebar_scrollbar = true;
                    return;
                }
            }

            let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
            let sidebar_inner_left = pad;
            let sidebar_inner_right = sidebar_w - pad;

            for &(session_idx, item_top, item_bottom) in &session_items {
                let adj_top = item_top - sidebar_scroll;
                let adj_bottom = item_bottom - sidebar_scroll;
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

                if point_in_rect(x, y, archive_rect.0, archive_rect.1, archive_rect.2, archive_rect.3) {
                    self.hover_session_archive = Some(session_idx);
                    self.hover_sidebar_item = Some(session_idx);
                    return;
                }
                if point_in_rect(x, y, delete_rect.0, delete_rect.1, delete_rect.2, delete_rect.3) {
                    self.hover_session_delete = Some(session_idx);
                    self.hover_sidebar_item = Some(session_idx);
                    return;
                }

                // Session item hover
                if y >= adj_top && y <= adj_bottom {
                    self.hover_sidebar_item = Some(session_idx);
                    return;
                }
            }

            // Settings button hover
            let settings_btn_top = self.client_height - footer_h + pad / 2;
            let settings_btn_bottom = self.client_height - pad / 2;
            if x >= pad && x <= sidebar_w - pad && y >= settings_btn_top && y <= settings_btn_bottom {
                self.hover_settings_btn = true;
            }
        } else {
            // Send button hover
            if let Some((btn_left, btn_top, btn_right, btn_bottom)) = self.send_btn_rect() {
                if x >= btn_left && x <= btn_right && y >= btn_top && y <= btn_bottom
                    && !self.sessions.is_empty() {
                        let idx = self.active_session.min(self.sessions.len() - 1);
                        let chat = &self.sessions[idx].chat;
                        if !chat.input_buffer.trim().is_empty() && !chat.is_waiting_for_agent {
                            self.hover_send_btn = true;
                            return;
                        }
                    }
            }

            // Session header hover
            let (sh_left, sh_top, sh_right, sh_bottom) = self.session_header_rect();
            if x >= sh_left && x <= sh_right && y >= sh_top && y <= sh_bottom {
                self.hover_session_header = true;
                return;
            }

            // Message scrollbar hover
            if !self.sessions.is_empty() {
                let chat_right = if self.show_settings {
                    self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
                } else {
                    self.client_width
                };
                let sb_w = (4.0 * scale).max(3.0) as i32;
                let idx = self.active_session.min(self.sessions.len() - 1);
                let viewport_top = self.messages_viewport_top();
                let viewport_bottom = self.messages_viewport_bottom();
                let viewport = viewport_bottom - viewport_top;
                let total = self.sessions[idx].chat.total_content_height;
                if total > viewport && x >= chat_right - sb_w && x <= chat_right && y >= viewport_top && y <= viewport_bottom {
                    self.hover_scrollbar = true;
                    return;
                }
            }

            // Thinking toggle hover
            if !self.sessions.is_empty() {
                let idx = self.active_session.min(self.sessions.len() - 1);
                let chat_left = sidebar_w;
                let chat_right = if self.show_settings {
                    self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
                } else {
                    self.client_width
                };
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
                        let toggle_top = adjusted_y + bubble_padding + label_height + (SPACE_1 as f32 * scale) as i32;
                        let toggle_bottom = toggle_top + thinking_header_h;
                        let toggle_left = bubble_left + bubble_padding;
                        let toggle_right = bubble_left + bubble_width - bubble_padding;

                        if x >= toggle_left && x <= toggle_right && y >= toggle_top && y <= toggle_bottom {
                            self.hover_thinking_toggle = Some(msg_idx);
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
        let chat_right = if self.show_settings {
            self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
        } else {
            self.client_width
        };
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
        let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
        let chat_right = if self.show_settings {
            self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
        } else {
            self.client_width
        };
        let header_h = (SESSION_HEADER_HEIGHT as f32 * scale) as i32;
        (sidebar_w, 0, chat_right, header_h)
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

    fn reconnect_button_y(&self) -> i32 {
        let scale = self.dpi as f32 / 96.0;
        let spad = (SPACE_5 as f32 * scale) as i32;
        let row_h = (SPACE_5 as f32 * scale) as i32;
        let section_h = (22.0 * scale) as i32;
        let gap = (SPACE_1 as f32 * scale) as i32;
        let extra_gap = (SPACE_3 as f32 * scale) as i32;
        let info_row_h = (24.0 * scale) as i32;

        let mut ty = spad + (SPACE_6 as f32 * scale) as i32 + (SPACE_1 as f32 * scale) as i32 + (SPACE_4 as f32 * scale) as i32;
        // Appearance: section + 2 toggles
        ty += section_h;
        ty += row_h + gap;
        ty += row_h + gap;
        ty += extra_gap;
        // Agent: section + 1 toggle
        ty += section_h;
        ty += row_h + gap;
        ty += extra_gap;
        // Sessions: section + 1 toggle
        ty += section_h;
        ty += row_h + gap;
        ty += extra_gap;
        // Connection: section + 2 info rows
        ty += section_h;
        ty += info_row_h;
        ty += info_row_h;
        ty += extra_gap;
        // About: section + 2 info rows
        ty += section_h;
        ty += info_row_h;
        ty += info_row_h;
        ty += (SPACE_4 as f32 * scale) as i32;
        ty
    }

    fn scrollbar_at(&self, x: i32, y: i32) -> bool {
        if self.sessions.is_empty() { return false; }
        let scale = self.dpi as f32 / 96.0;
        let chat_right = if self.show_settings {
            self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
        } else {
            self.client_width
        };
        let sb_w = (4.0 * scale).max(3.0) as i32;
        let idx = self.active_session.min(self.sessions.len() - 1);
        let viewport_top = self.messages_viewport_top();
        let viewport_bottom = self.messages_viewport_bottom();
        let viewport = viewport_bottom - viewport_top;
        let total = self.sessions[idx].chat.total_content_height;
        if total <= viewport { return false; }
        x >= chat_right - sb_w && x <= chat_right && y >= viewport_top && y <= viewport_bottom
    }

    fn jump_to_scrollbar_position(&mut self, _x: i32, y: i32) -> bool {
        if self.sessions.is_empty() { return false; }
        let scale = self.dpi as f32 / 96.0;
        let idx = self.active_session.min(self.sessions.len() - 1);
        let viewport_top = self.messages_viewport_top();
        let viewport_bottom = self.messages_viewport_bottom();
        let viewport = viewport_bottom - viewport_top;
        let total = self.sessions[idx].chat.total_content_height;
        if total <= viewport { return false; }

        let max_scroll = (total - viewport).max(0);
        let thumb_ratio = viewport as f32 / total as f32;
        let thumb_h = ((viewport as f32 * thumb_ratio).max(12.0 * scale)) as i32;
        let track_h = viewport - thumb_h;
        if track_h <= 0 { return false; }

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
        let row_h = (SPACE_5 as f32 * scale) as i32;
        let section_h = (22.0 * scale) as i32;
        let gap = (SPACE_1 as f32 * scale) as i32;
        let extra_gap = (SPACE_3 as f32 * scale) as i32;

        let mut ty = spad + (SPACE_6 as f32 * scale) as i32 + (SPACE_1 as f32 * scale) as i32 + (SPACE_4 as f32 * scale) as i32;

        // Appearance section
        ty += section_h;
        let t0_top = ty;
        let t0_bottom = ty + row_h;
        ty += row_h + gap;
        let t1_top = ty;
        let t1_bottom = ty + row_h;
        ty += row_h + gap + extra_gap;

        // Agent section
        ty += section_h;
        let t2_top = ty;
        let t2_bottom = ty + row_h;
        ty += row_h + gap + extra_gap;

        // Sessions section
        ty += section_h;
        let t3_top = ty;
        let t3_bottom = ty + row_h;

        if x >= panel_left + spad && x <= self.client_width - spad {
            if y >= t0_top && y <= t0_bottom { return Some(0); }
            if y >= t1_top && y <= t1_bottom { return Some(1); }
            if y >= t2_top && y <= t2_bottom { return Some(2); }
            if y >= t3_top && y <= t3_bottom { return Some(3); }
        }
        None
    }

    fn thinking_toggle_at(&mut self, x: i32, y: i32) -> bool {
        if self.sessions.is_empty() {
            return false;
        }
        let scale = self.dpi as f32 / 96.0;
        let idx = self.active_session.min(self.sessions.len() - 1);
        let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
        let chat_left = sidebar_w;
        let chat_right = if self.show_settings {
            self.client_width - (SETTINGS_PANEL_WIDTH as f32 * scale) as i32
        } else {
            self.client_width
        };
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
                let toggle_top = adjusted_y + bubble_padding + label_height + (SPACE_1 as f32 * scale) as i32;
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

// ── Folder picker dialog ─────────────────────────────────────

/// Open a folder picker dialog and return the selected path, or None if cancelled.
pub fn pick_folder_dialog() -> Option<String> {
    unsafe {
        let _ = OleInitialize(std::ptr::null_mut());

        let title = to_wide("Select a project folder");
        let mut display_name: Vec<u16> = vec![0; 260];

        let mut bi = BROWSEINFOW {
            hwndOwner: std::ptr::null_mut(),
            pidlRoot: std::ptr::null(),
            pszDisplayName: display_name.as_mut_ptr(),
            lpszTitle: title.as_ptr(),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_NONEWFOLDERBUTTON,
            lpfn: None,
            lParam: 0,
            iImage: 0,
        };

        let pidl = SHBrowseForFolderW(&mut bi);
        if pidl.is_null() {
            return None;
        }

        let mut path_buf: Vec<u16> = vec![0; 260];
        let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr());
        CoTaskMemFree(pidl as *mut std::ffi::c_void);

        if ok == 0 {
            return None;
        }

        Some(from_wide(path_buf.as_ptr()))
    }
}

// ── Session action button geometry ───────────────────────────

/// Compute the (left, top, right, bottom) rects for the archive and delete buttons
/// on a session sidebar item. Returns (archive_rect, delete_rect).
pub fn session_action_rects(
    _item_left: i32,
    item_right: i32,
    item_top: i32,
    item_height: i32,
    scale: f32,
) -> ((i32, i32, i32, i32), (i32, i32, i32, i32)) {
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
pub fn point_in_rect(x: i32, y: i32, left: i32, top: i32, right: i32, bottom: i32) -> bool {
    x >= left && x <= right && y >= top && y <= bottom
}
