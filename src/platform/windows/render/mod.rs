// Render module: GDI double-buffered painting.
// Submodules: sidebar, messages, input_box, header, settings, empty_state, chat_area.
#![allow(dead_code)]

mod chat_area;
mod empty_state;
mod header;
mod input_box;
mod messages;
mod settings;
mod sidebar;
mod toast;

#[allow(unused_imports)]
pub use chat_area::draw_chat_area;
#[allow(unused_imports)]
pub use empty_state::draw_empty_state;
#[allow(unused_imports)]
pub use header::draw_session_header;
#[allow(unused_imports)]
pub use input_box::{compute_input_height, draw_hint_row, draw_input_box};
#[allow(unused_imports)]
pub use messages::{compute_message_heights, draw_message_scrollbar, draw_messages};
#[allow(unused_imports)]
pub use settings::draw_settings_panel;
#[allow(unused_imports)]
pub use sidebar::draw_sidebar;
#[allow(unused_imports)]
pub use toast::draw_toast;

use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;

struct BackBuffer {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    width: i32,
    height: i32,
}
impl BackBuffer {
    fn new(screen: HDC, width: i32, height: i32) -> Option<Self> {
        // SAFETY: screen is the live BeginPaint DC; each failed acquisition is
        // unwound before another handle can be used. Only the UI thread owns it.
        unsafe {
            let dc = CreateCompatibleDC(screen);
            if dc.is_null() {
                return None;
            }
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            if bitmap.is_null() {
                DeleteDC(dc);
                return None;
            }
            let previous = SelectObject(dc, bitmap as HGDIOBJ);
            if previous.is_null() || previous as isize == -1 {
                DeleteObject(bitmap as HGDIOBJ);
                DeleteDC(dc);
                return None;
            }
            Some(Self {
                dc,
                bitmap,
                previous,
                width,
                height,
            })
        }
    }
}
impl Drop for BackBuffer {
    fn drop(&mut self) {
        // SAFETY: restore the borrowed previous bitmap before deleting ours.
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
        }
    }
}
thread_local! { static BACKBUFFER: std::cell::RefCell<Option<BackBuffer>> = const { std::cell::RefCell::new(None) }; }

/// The caller owns BeginPaint/EndPaint. Reuse the backbuffer until resize;
/// allocation failure falls back to the valid paint DC rather than a null one.
pub fn paint(screen: HDC, ctx: &mut PaintContext) {
    let width = ctx.app.client_width;
    let height = ctx.app.client_height;
    if screen.is_null() || width <= 0 || height <= 0 {
        return;
    }
    BACKBUFFER.with(|storage| {
        let Ok(mut buffer) = storage.try_borrow_mut() else {
            return;
        };
        if buffer
            .as_ref()
            .is_none_or(|b| b.width != width || b.height != height)
        {
            *buffer = BackBuffer::new(screen, width, height);
        }
        let dc = buffer.as_ref().map_or(screen, |b| b.dc);
        let scale = ctx.app.dpi as f32 / 96.0;
        // SAFETY: dc is either the live paint DC or our selected backbuffer.
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            let background = CreateSolidBrush(BG_COLOR);
            if !background.is_null() {
                FillRect(dc, &rect, background);
                DeleteObject(background as HGDIOBJ);
            }
            SetBkMode(dc, TRANSPARENT);
        }
        if ctx.app.layout().content_visible {
            chat_area::draw_chat_area(dc, ctx, scale);
        }
        if ctx.app.layout().sidebar.width() > 0 && !ctx.app.layout().settings_sheet {
            sidebar::draw_sidebar(dc, ctx, scale);
        }
        if ctx.app.show_settings {
            settings::draw_settings_panel(dc, ctx, scale);
        }
        toast::draw_toast(dc, ctx, scale);
        if dc != screen {
            // SAFETY: both DCs are live and the source bitmap matches the window.
            unsafe {
                BitBlt(screen, 0, 0, width, height, dc, 0, 0, SRCCOPY);
            }
        }
    });
}

pub fn release_backbuffer() {
    BACKBUFFER.with(|buffer| {
        buffer.borrow_mut().take();
    });
}
