// Session header bar: session name on the left, connection status on the right.
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::platform::windows::text::truncate_to_width;
use crate::theme::*;

pub fn draw_session_header(
    hdc: HDC,
    ctx: &PaintContext,
    chat_left: i32,
    chat_right: i32,
    scale: f32,
) {
    let header_h = (SESSION_HEADER_HEIGHT as f32 * scale) as i32;
    let header_rect = RECT {
        left: chat_left,
        top: 0,
        right: chat_right,
        bottom: header_h,
    };

    unsafe {
        // Background
        let bg_brush = CreateSolidBrush(SESSION_HEADER_BG);
        FillRect(hdc, &header_rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        // Bottom border
        let border_rect = RECT {
            left: chat_left,
            top: header_h - 1,
            right: chat_right,
            bottom: header_h,
        };
        let border_brush = CreateSolidBrush(SESSION_HEADER_BORDER);
        FillRect(hdc, &border_rect, border_brush);
        DeleteObject(border_brush as HGDIOBJ);

        // Thin gradient accent stripe at top of header
        let stripe_h = (2.0 * scale).max(1.0) as i32;
        let stripe_colors = [
            GRADIENT_YELLOW,
            GRADIENT_AMBER,
            GRADIENT_ORANGE,
            GRADIENT_RED_ORANGE,
            GRADIENT_RED,
        ];
        let seg_w = (chat_right - chat_left) / 5;
        for (i, &color) in stripe_colors.iter().enumerate() {
            let seg_rect = RECT {
                left: chat_left + i as i32 * seg_w,
                top: 0,
                right: chat_left + (i as i32 + 1) * seg_w,
                bottom: stripe_h,
            };
            let brush = CreateSolidBrush(color);
            FillRect(hdc, &seg_rect, brush);
            DeleteObject(brush as HGDIOBJ);
        }

        let pad = (SPACE_4 as f32 * scale) as i32;
        let idx = ctx
            .app
            .active_session
            .min(ctx.app.sessions.len().saturating_sub(1));
        let session = &ctx.app.sessions[idx];

        // Session name (truncated)
        if !session.chat.messages.is_empty() {
            let name = if session.name.is_empty() {
                format!("Chat {}", idx + 1)
            } else {
                session.name.clone()
            };
            let name_font = ctx.fonts.session_header_name;
            let max_name_w = (chat_right - chat_left) - 2 * pad - (120.0 * scale) as i32;
            let display_name = truncate_to_width(hdc, &name, max_name_w, name_font);
            let old_font = SelectObject(hdc, name_font as HGDIOBJ);
            SetTextColor(hdc, SESSION_NAME_TEXT);
            let name_wide = to_wide(&display_name);
            let mut name_rect = RECT {
                left: chat_left + pad,
                top: 0,
                right: chat_left + pad + max_name_w,
                bottom: header_h,
            };
            DrawTextW(
                hdc,
                name_wide.as_ptr(),
                name_wide.len() as i32 - 1,
                &mut name_rect as LPRECT,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP | DT_END_ELLIPSIS,
            );
            SelectObject(hdc, old_font as HGDIOBJ);
        }

        // Mode radio: Default (Direct/Responses API) | Code (vibe-acp)
        {
            let layout = compute_mode_radio_layout(chat_left, chat_right, header_h, scale);
            let session_mode = session.mode;
            let option_w = (MODE_RADIO_OPTION_W as f32 * scale) as i32;
            for (i, (label, m)) in [
                ("Default", crate::session::SessionMode::Direct),
                ("Code", crate::session::SessionMode::Code),
            ]
            .iter()
            .enumerate()
            {
                let opt_left = if i == 0 {
                    layout.direct_left
                } else {
                    layout.code_left
                };
                let active = *m == session_mode;
                let (bg, fg) = if active {
                    (ACCENT, TEXT_ON_ACCENT)
                } else {
                    (SURFACE_2, TEXT_SECONDARY)
                };
                let bg_brush = CreateSolidBrush(bg);
                let opt_rect = RECT {
                    left: opt_left,
                    top: layout.top,
                    right: opt_left + option_w,
                    bottom: layout.top + layout.h,
                };
                FillRect(hdc, &opt_rect, bg_brush);
                DeleteObject(bg_brush as HGDIOBJ);
                let old_f = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
                SetTextColor(hdc, fg);
                let label_wide = to_wide(label);
                let mut tr = opt_rect;
                DrawTextW(
                    hdc,
                    label_wide.as_ptr(),
                    label_wide.len() as i32 - 1,
                    &mut tr as LPRECT,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                );
                SelectObject(hdc, old_f as HGDIOBJ);
            }
        }

        // Connection status indicator (right-aligned)
        let (status_text, status_color, pulse_dot) = if session.chat.is_waiting_for_agent {
            ("Agent responding...".to_string(), ACCENT, true)
        } else if session.chat.streaming_message_idx.is_some() {
            ("Streaming".to_string(), ACCENT, false)
        } else if !ctx.app.acp_connected {
            ("Disconnected".to_string(), ERROR_COLOR, false)
        } else {
            ("Connected".to_string(), SUCCESS, false)
        };

        let old_font = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
        SetTextColor(hdc, status_color);
        let status_wide = to_wide(&status_text);
        let mut status_rect = RECT {
            left: chat_left,
            top: 0,
            right: chat_right - pad,
            bottom: header_h,
        };
        DrawTextW(
            hdc,
            status_wide.as_ptr(),
            status_wide.len() as i32 - 1,
            &mut status_rect as LPRECT,
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // Status dot
        let dot_r = (3.0 * scale) as i32;
        let dot_cx = chat_right - pad;
        let dot_cy = header_h / 2;
        let old_font2 = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
        let mut sz = POINT { x: 0, y: 0 };
        GetTextExtentPoint32W(
            hdc,
            status_wide.as_ptr(),
            status_wide.len() as i32 - 1,
            &mut sz,
        );
        SelectObject(hdc, old_font2 as HGDIOBJ);
        let dot_x = dot_cx - sz.x - dot_r - (6.0 * scale) as i32;
        let dot_color = if pulse_dot {
            let pulse = (ctx.app.streaming_dots % 30) < 15;
            if pulse {
                status_color
            } else {
                (status_color & 0x00FFFFFF) | 0x00404040
            }
        } else {
            status_color
        };
        let dot_brush = CreateSolidBrush(dot_color);
        let dot_pen = CreatePen(PS_SOLID, 1, dot_color);
        let old_pen = SelectObject(hdc, dot_pen as HGDIOBJ);
        let old_brush = SelectObject(hdc, dot_brush as HGDIOBJ);
        Ellipse(
            hdc,
            dot_x - dot_r,
            dot_cy - dot_r,
            dot_x + dot_r,
            dot_cy + dot_r,
        );
        SelectObject(hdc, old_pen as HGDIOBJ);
        SelectObject(hdc, old_brush as HGDIOBJ);
        DeleteObject(dot_pen as HGDIOBJ);
        DeleteObject(dot_brush as HGDIOBJ);
    }
}
