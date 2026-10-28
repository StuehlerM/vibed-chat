// Windows paint context: bundles all data needed for a GDI paint pass.
// This is the Windows-specific rendering context that wraps the platform-agnostic
// UI state types from crate::ui.
#![allow(dead_code)]

use crate::chat::ChatState;
use crate::platform::windows::fonts::Fonts;
use crate::session::{Project, Session};
use crate::theme::*;
use crate::ui::{FocusedElement, PressedElement, RenameTarget, Toast};

/// Bundles all data needed for a GDI paint pass.
pub struct PaintContext<'a> {
    pub toast: Option<Toast>,
    pub projects: &'a [Project],
    pub sessions: &'a mut Vec<Session>,
    pub active_session: usize,
    pub show_settings: bool,
    pub settings: &'a Settings,
    pub fonts: &'a Fonts,
    pub dpi: u32,
    pub width: i32,
    pub height: i32,
    pub cursor_visible: bool,
    pub acp_connected: bool,
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
    pub pressed_element: Option<PressedElement>,
    pub focused_element: Option<FocusedElement>,
    pub sidebar_scroll_offset: i32,
    pub hover_scrollbar: bool,
    pub hover_sidebar_scrollbar: bool,
    pub streaming_dots: u8,
    pub hover_send_btn: bool,
    pub hover_session_header: bool,
    pub hover_session_archive: Option<usize>,
    pub hover_session_delete: Option<usize>,
    pub rename_target: Option<RenameTarget>,
    pub rename_buffer: &'a str,
    pub rename_cursor: usize,
    pub hover_link: Option<usize>,
    pub link_rects: &'a std::cell::RefCell<Vec<crate::app::ClickableRect>>,
    pub code_block_rects: &'a std::cell::RefCell<Vec<crate::app::ClickableRect>>,
    pub hover_code_copy: Option<usize>,
}

impl<'a> PaintContext<'a> {
    pub fn active_chat(&mut self) -> Option<&mut ChatState> {
        if self.sessions.is_empty() {
            return None;
        }
        let idx = self.active_session.min(self.sessions.len() - 1);
        Some(&mut self.sessions[idx].chat)
    }

    pub fn sidebar_w(&self) -> i32 {
        (SIDEBAR_WIDTH as f32 * self.dpi as f32 / 96.0) as i32
    }

    pub fn settings_w(&self) -> i32 {
        (SETTINGS_PANEL_WIDTH as f32 * self.dpi as f32 / 96.0) as i32
    }

    pub fn chat_left(&self) -> i32 {
        self.sidebar_w()
    }

    pub fn chat_right(&self) -> i32 {
        if self.show_settings {
            self.width - self.settings_w()
        } else {
            self.width
        }
    }

    pub fn chat_width(&self) -> i32 {
        self.chat_right() - self.chat_left()
    }

    pub fn session_header_h(&self) -> i32 {
        (SESSION_HEADER_HEIGHT as f32 * self.dpi as f32 / 96.0) as i32
    }

    pub fn hint_row_h(&self) -> i32 {
        (HINT_ROW_HEIGHT as f32 * self.dpi as f32 / 96.0) as i32
    }

    pub fn send_btn_size(&self) -> i32 {
        (SEND_BTN_SIZE as f32 * self.dpi as f32 / 96.0) as i32
    }

    pub fn input_margin(&self) -> i32 {
        (INPUT_MARGIN as f32 * self.dpi as f32 / 96.0) as i32
    }
}
