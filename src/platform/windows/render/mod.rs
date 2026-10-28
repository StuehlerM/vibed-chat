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

/// Paint the entire window using double buffering.
pub fn paint(hwnd: HWND, ctx: &mut PaintContext) {
    let scale = ctx.app.dpi as f32 / 96.0;
    let width = ctx.app.client_width;
    let height = ctx.app.client_height;

    unsafe {
        let hdc_screen = GetDC(hwnd);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        let old_bitmap = SelectObject(hdc_mem, hbitmap as HGDIOBJ);

        // Fill background
        let bg_rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let bg_brush = CreateSolidBrush(BG_COLOR);
        FillRect(hdc_mem, &bg_rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        SetBkMode(hdc_mem, TRANSPARENT);

        // Draw sidebar
        sidebar::draw_sidebar(hdc_mem, ctx, scale);

        // Draw main chat area (shrinks when settings panel is open - push layout)
        chat_area::draw_chat_area(hdc_mem, ctx, scale);

        // Draw settings panel in the reserved space (push, not overlay)
        if ctx.app.show_settings {
            settings::draw_settings_panel(hdc_mem, ctx, scale);
        }

        // Draw toast notification (top-right corner, above everything)
        toast::draw_toast(hdc_mem, ctx, scale);

        // Blit to screen
        BitBlt(hdc_screen, 0, 0, width, height, hdc_mem, 0, 0, SRCCOPY);

        // Cleanup
        SelectObject(hdc_mem, old_bitmap as HGDIOBJ);
        DeleteObject(hbitmap as HGDIOBJ);
        DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_screen);
    }
}
