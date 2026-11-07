// Session header: clipped title/mode/status regions using shared mode geometry.
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;

pub fn draw_session_header(
    hdc: HDC,
    ctx: &PaintContext,
    chat_left: i32,
    chat_right: i32,
    scale: f32,
) {
    let header_h = (SESSION_HEADER_HEIGHT as f32 * scale) as i32;
    let pad = (SPACE_4 as f32 * scale) as i32;
    let gap = (SPACE_2 as f32 * scale) as i32;
    let layout = compute_mode_radio_layout(chat_left, chat_right, header_h, scale);
    let option_w = (MODE_RADIO_OPTION_W as f32 * scale) as i32;
    // SAFETY: hdc is the live paint DC; temporary brushes are deleted after use,
    // and every selected font is restored before leaving its drawing scope.
    unsafe {
        let background = CreateSolidBrush(SESSION_HEADER_BG);
        let rect = RECT {
            left: chat_left,
            top: 0,
            right: chat_right,
            bottom: header_h,
        };
        FillRect(hdc, &rect, background);
        DeleteObject(background as HGDIOBJ);
        let border = CreateSolidBrush(SESSION_HEADER_BORDER);
        let line = RECT {
            left: chat_left,
            top: header_h - 1,
            right: chat_right,
            bottom: header_h,
        };
        FillRect(hdc, &line, border);
        DeleteObject(border as HGDIOBJ);
        let stripe_h = (2.0 * scale).max(1.0) as i32;
        for (index, color) in [
            GRADIENT_YELLOW,
            GRADIENT_AMBER,
            GRADIENT_ORANGE,
            GRADIENT_RED_ORANGE,
            GRADIENT_RED,
        ]
        .into_iter()
        .enumerate()
        {
            let stripe = RECT {
                left: chat_left + (chat_right - chat_left) * index as i32 / 5,
                top: 0,
                right: chat_left + (chat_right - chat_left) * (index + 1) as i32 / 5,
                bottom: stripe_h,
            };
            let brush = CreateSolidBrush(color);
            FillRect(hdc, &stripe, brush);
            DeleteObject(brush as HGDIOBJ);
        }
        if !ctx.app.native_editor_active {
            // Keep panel launchers discoverable if native-control creation failed.
            let panels = ctx.app.layout();
            let mut buttons = vec![(panels.settings_button(ctx.app.dpi), "Settings")];
            if panels.sidebar_collapsed {
                buttons.push((panels.history_button(ctx.app.dpi), "History"));
            }
            let font = SelectObject(hdc, ctx.fonts.caption);
            for (bounds, label) in buttons {
                let mut rect = RECT {
                    left: bounds.left,
                    top: bounds.top,
                    right: bounds.right,
                    bottom: bounds.bottom,
                };
                let brush = CreateSolidBrush(SURFACE_2);
                FillRect(hdc, &rect, brush);
                DeleteObject(brush);
                SetTextColor(hdc, TEXT_PRIMARY);
                let label = to_wide(label);
                DrawTextW(
                    hdc,
                    label.as_ptr(),
                    label.len() as i32 - 1,
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
            SelectObject(hdc, font);
        }
        let Some(session) = ctx.app.sessions.get(ctx.app.active_session) else {
            return;
        };
        let title = if session.name.is_empty() {
            "New chat"
        } else {
            &session.name
        };
        let old_font = SelectObject(hdc, ctx.fonts.session_header_name as HGDIOBJ);
        SetTextColor(hdc, SESSION_NAME_TEXT);
        let mut name_rect = RECT {
            left: chat_left
                + pad
                + if ctx.app.layout().sidebar_collapsed {
                    (96.0 * scale) as i32
                } else {
                    0
                },
            top: stripe_h,
            right: chat_right - (100.0 * scale) as i32,
            bottom: (40.0 * scale) as i32,
        };
        if name_rect.right > name_rect.left {
            let title = to_wide(title);
            DrawTextW(
                hdc,
                title.as_ptr(),
                title.len() as i32 - 1,
                &mut name_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        SelectObject(hdc, old_font);
        for (label, mode, left) in [
            (
                "Chat",
                crate::session::SessionMode::Direct,
                layout.direct_left,
            ),
            ("Code", crate::session::SessionMode::Code, layout.code_left),
        ] {
            let active = mode == session.mode;
            let background = CreateSolidBrush(if active { ACCENT } else { SURFACE_2 });
            let mut rect = RECT {
                left,
                top: layout.top,
                right: left + option_w,
                bottom: layout.top + layout.h,
            };
            FillRect(hdc, &rect, background);
            DeleteObject(background as HGDIOBJ);
            let previous = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
            SetTextColor(
                hdc,
                if active {
                    TEXT_ON_ACCENT
                } else {
                    TEXT_SECONDARY
                },
            );
            let text = to_wide(label);
            DrawTextW(
                hdc,
                text.as_ptr(),
                text.len() as i32 - 1,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            SelectObject(hdc, previous);
        }
        let status = ctx.app.active_status();
        let previous = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
        SetTextColor(
            hdc,
            if session.active_turn.is_some() {
                ACCENT
            } else {
                TEXT_SECONDARY
            },
        );
        let mut rect = RECT {
            left: layout.code_left + option_w + gap,
            top: layout.top,
            right: chat_right - pad,
            bottom: layout.top + layout.h,
        };
        if rect.right > rect.left {
            let text = to_wide(status);
            DrawTextW(
                hdc,
                text.as_ptr(),
                text.len() as i32 - 1,
                &mut rect,
                DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        SelectObject(hdc, previous);
    }
}
