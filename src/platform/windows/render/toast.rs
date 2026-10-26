// Toast notification: transient popup in the top-right corner (agent connection status).
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;
use crate::ui::{Toast, ToastKind};

/// How long a toast stays visible.
pub const TOAST_DURATION_MS: u64 = 4000;
const TOAST_MAX_WIDTH: i32 = 380;
const TOAST_H_PAD: i32 = SPACE_3;
const TOAST_V_PAD: i32 = SPACE_2;

/// Show a toast, replacing any previous one (only the latest message is shown).
pub fn show_toast(ctx: &mut PaintContext, text: &str, kind: ToastKind) {
    ctx.toast = Some(Toast {
        text: text.to_string(),
        kind,
        shown_at: std::time::Instant::now(),
        duration_ms: TOAST_DURATION_MS,
    });
}

/// Draw the toast popup in the top-right corner, if one is active.
pub fn draw_toast(hdc: HDC, ctx: &PaintContext, scale: f32) {
    let toast = match &ctx.toast {
        Some(t) => t,
        None => return,
    };
    let elapsed_ms = toast.shown_at.elapsed().as_millis() as u64;
    if elapsed_ms >= toast.duration_ms {
        return;
    }

    let (icon, accent) = match toast.kind {
        ToastKind::Info => ('\u{25CF}', ACCENT),
        ToastKind::Success => ('\u{2713}', SUCCESS),
        ToastKind::Error => ('\u{2715}', ERROR_COLOR),
    };

    let margin = (SPACE_4 as f32 * scale) as i32;
    let h_pad = (TOAST_H_PAD as f32 * scale) as i32;
    let v_pad = (TOAST_V_PAD as f32 * scale) as i32;
    let line_h = (SESSION_HEADER_HEIGHT as f32 * 0.0 + 20.0 * scale) as i32;

    unsafe {
        let old_font = SelectObject(hdc, ctx.fonts.hint as HGDIOBJ);

        // Measure text (wrapped) to size the toast
        let max_w = (TOAST_MAX_WIDTH as f32 * scale) as i32;
        let text_w = max_w - 2 * h_pad;
        let wide = to_wide(&toast.text);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: text_w,
            bottom: 0,
        };
        DrawTextW(
            hdc,
            wide.as_ptr(),
            wide.len() as i32 - 1,
            &mut rect as LPRECT,
            DT_CALCRECT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX,
        );

        // Single-line preferred: shrink-to-fit if the text is narrow
        let text_width_used = rect.right.min(text_w).max(40);
        let toast_w = text_width_used + 2 * h_pad;
        let toast_h = rect.bottom.max(line_h) + 2 * v_pad;
        let toast_right = ctx.width - margin;
        let toast_left = toast_right - toast_w;
        let toast_top = margin;
        let toast_bottom = toast_top + toast_h;

        // Background
        let bg = CreateSolidBrush(SURFACE_2);
        let bg_rect = RECT {
            left: toast_left,
            top: toast_top,
            right: toast_right,
            bottom: toast_bottom,
        };
        FillRect(hdc, &bg_rect, bg);
        DeleteObject(bg as HGDIOBJ);

        // Border (accent-colored by kind)
        let pen = CreatePen(PS_SOLID, (1.0 * scale).max(1.0) as i32, accent);
        let old_pen = SelectObject(hdc, pen as HGDIOBJ);
        let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
        let radius = (RADIUS_MD as f32 * scale) as i32;
        RoundRect(
            hdc,
            toast_left,
            toast_top,
            toast_right,
            toast_bottom,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_pen as HGDIOBJ);
        SelectObject(hdc, old_brush as HGDIOBJ);
        DeleteObject(pen as HGDIOBJ);

        // Accent bar on the left edge
        let bar_w = (3.0 * scale).max(2.0) as i32;
        let bar = CreateSolidBrush(accent);
        let bar_rect = RECT {
            left: toast_left,
            top: toast_top + radius,
            right: toast_left + bar_w,
            bottom: toast_bottom - radius,
        };
        FillRect(hdc, &bar_rect, bar);
        DeleteObject(bar as HGDIOBJ);

        // Icon + text (single line, centered vertically)
        SetTextColor(hdc, TEXT_PRIMARY);
        SetBkMode(hdc, TRANSPARENT);

        let label = format!("{}  {}", icon, toast.text);
        let label_wide = to_wide(&label);
        let mut text_rect = RECT {
            left: toast_left + h_pad,
            top: toast_top,
            right: toast_right - h_pad,
            bottom: toast_bottom,
        };
        // If icon + text overflows, fall back to plain text
        let mut sz = POINT { x: 0, y: 0 };
        GetTextExtentPoint32W(
            hdc,
            label_wide.as_ptr(),
            label_wide.len() as i32 - 1,
            &mut sz,
        );
        let text_x;
        if sz.x <= text_rect.right - text_rect.left {
            text_x = text_rect.left + ((text_rect.right - text_rect.left - sz.x) / 2);
        } else {
            text_x = text_rect.left;
        }
        text_rect.left = text_x;
        DrawTextW(
            hdc,
            label_wide.as_ptr(),
            label_wide.len() as i32 - 1,
            &mut text_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        SelectObject(hdc, old_font as HGDIOBJ);
    }
}
