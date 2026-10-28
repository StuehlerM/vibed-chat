// Windows paint context: bundles all data needed for a GDI paint pass.
// This is the Windows-specific rendering context that wraps the platform-agnostic
// UI state types from crate::ui.
#![allow(dead_code)]

use crate::chat::ChatState;
use crate::platform::windows::fonts::Fonts;
use crate::theme::*;

/// Bundles all data needed for a GDI paint pass.
///
/// Carries a borrowed AppState (all app fields are reachable through it),
/// the platform-agnostic handle for the GDI fonts, and the paint-local
/// scratch state (clickable rect registries).
pub struct PaintContext<'a> {
    pub app: &'a mut crate::app::AppState,
    pub fonts: &'a Fonts,
}

impl<'a> PaintContext<'a> {
    pub fn active_chat(&mut self) -> Option<&mut ChatState> {
        self.app.active_chat()
    }

    pub fn sidebar_w(&self) -> i32 {
        (SIDEBAR_WIDTH as f32 * self.app.dpi as f32 / 96.0) as i32
    }

    pub fn settings_w(&self) -> i32 {
        (SETTINGS_PANEL_WIDTH as f32 * self.app.dpi as f32 / 96.0) as i32
    }

    pub fn chat_left(&self) -> i32 {
        self.app.chat_left()
    }

    pub fn chat_right(&self) -> i32 {
        self.app.chat_right()
    }

    pub fn chat_width(&self) -> i32 {
        self.chat_right() - self.chat_left()
    }

    pub fn session_header_h(&self) -> i32 {
        (SESSION_HEADER_HEIGHT as f32 * self.app.dpi as f32 / 96.0) as i32
    }

    pub fn hint_row_h(&self) -> i32 {
        (HINT_ROW_HEIGHT as f32 * self.app.dpi as f32 / 96.0) as i32
    }

    pub fn send_btn_size(&self) -> i32 {
        (SEND_BTN_SIZE as f32 * self.app.dpi as f32 / 96.0) as i32
    }

    pub fn input_margin(&self) -> i32 {
        (INPUT_MARGIN as f32 * self.app.dpi as f32 / 96.0) as i32
    }
}
