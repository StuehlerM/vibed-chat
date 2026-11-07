// Shared adaptive panel geometry in device pixels; no native API dependencies.
#![allow(dead_code)]

use crate::theme::{SESSION_HEADER_HEIGHT, SETTINGS_PANEL_WIDTH, SIDEBAR_WIDTH};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Rect {
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }
    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AppLayout {
    pub sidebar: Rect,
    pub chat: Rect,
    pub settings: Rect,
    pub sidebar_collapsed: bool,
    pub settings_sheet: bool,
    pub content_visible: bool,
    pub header_height: i32,
}
impl AppLayout {
    pub fn history_button(self, dpi: u32) -> Rect {
        let s = dpi as f32 / 96.0;
        Rect {
            left: self.chat.left + (8.0 * s) as i32,
            top: (8.0 * s) as i32,
            right: self.chat.left + (96.0 * s) as i32,
            bottom: (36.0 * s) as i32,
        }
    }
    pub fn settings_button(self, dpi: u32) -> Rect {
        let s = dpi as f32 / 96.0;
        Rect {
            left: self.chat.right - (96.0 * s) as i32,
            top: (8.0 * s) as i32,
            right: self.chat.right - (8.0 * s) as i32,
            bottom: (36.0 * s) as i32,
        }
    }
    pub fn new(width: i32, height: i32, dpi: u32, settings: bool, drawer: bool) -> Self {
        let scale = dpi.max(1) as f32 / 96.0;
        let px = |v: i32| (v as f32 * scale) as i32;
        let width = width.max(0);
        let height = height.max(0);
        // Collapse before the minimum window would leave a cramped reading
        // column; retain a 400px hard floor when docking settings on wide windows.
        let sidebar_collapsed = width < px(SIDEBAR_WIDTH + 520);
        let sidebar_w = if !sidebar_collapsed || drawer {
            px(SIDEBAR_WIDTH).min(width)
        } else {
            0
        };
        let left = if sidebar_collapsed { 0 } else { sidebar_w };
        let settings_sheet =
            settings && (sidebar_collapsed || width - left < px(400 + SETTINGS_PANEL_WIDTH));
        let settings_left = if settings_sheet {
            0
        } else {
            width - px(SETTINGS_PANEL_WIDTH)
        };
        Self {
            sidebar: Rect {
                left: 0,
                top: 0,
                right: sidebar_w,
                bottom: height,
            },
            chat: Rect {
                left,
                top: 0,
                right: if settings && !settings_sheet {
                    settings_left
                } else {
                    width
                },
                bottom: height,
            },
            settings: Rect {
                left: settings_left.max(0),
                top: 0,
                right: width,
                bottom: height,
            },
            sidebar_collapsed,
            settings_sheet,
            content_visible: !(settings_sheet || sidebar_collapsed && drawer),
            header_height: px(SESSION_HEADER_HEIGHT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panels_never_consume_chat_minimum() {
        for dpi in [96, 120, 144, 192] {
            for width in [360, 640, 680, 720, 900, 976, 1100, 1600] {
                for settings in [false, true] {
                    let l = AppLayout::new(
                        width * dpi / 96,
                        480 * dpi / 96,
                        dpi as u32,
                        settings,
                        false,
                    );
                    assert!(l.chat.width() >= width.min(400) * dpi / 96);
                    assert!(l.settings.width() >= 0);
                    if l.settings_sheet {
                        assert!(!l.content_visible);
                    }
                }
            }
        }
    }
    #[test]
    fn drawer_does_not_reflow_chat() {
        let closed = AppLayout::new(640, 480, 96, false, false);
        let open = AppLayout::new(640, 480, 96, false, true);
        assert_eq!(closed.chat, open.chat);
        assert_eq!(open.sidebar.width(), 280);
        assert!(!open.content_visible);
    }
}
