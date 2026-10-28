// Chat area orchestrator: coordinates session header, messages, scrollbar, input box, hint row.
#![allow(dead_code)]

use super::empty_state::draw_empty_state;
use super::header::draw_session_header;
use super::input_box::{compute_input_height, draw_hint_row, draw_input_box};
use super::messages::{compute_message_heights, draw_message_scrollbar, draw_messages};
use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;

pub fn draw_chat_area(hdc: HDC, ctx: &mut PaintContext, scale: f32) {
    if ctx.app.sessions.is_empty() {
        return;
    }
    let chat_left = ctx.chat_left();
    let chat_right = ctx.chat_right();
    let chat_w = ctx.chat_width();
    let height = ctx.app.client_height;

    unsafe {
        let old_font = SelectObject(hdc, ctx.fonts.normal as HGDIOBJ);
        let mut tm = TEXTMETRICW {
            tmHeight: 0,
            tmAscent: 0,
            tmDescent: 0,
            tmInternalLeading: 0,
            tmExternalLeading: 0,
            tmAveCharWidth: 0,
            tmMaxCharWidth: 0,
            tmWeight: 0,
            tmOverhang: 0,
            tmDigitizedAspectX: 0,
            tmDigitizedAspectY: 0,
            tmFirstChar: 0,
            tmLastChar: 0,
            tmDefaultChar: 0,
            tmBreakChar: 0,
            tmItalic: 0,
            tmUnderlined: 0,
            tmStruckOut: 0,
            tmPitchAndFamily: 0,
            tmCharSet: 0,
        };
        GetTextMetricsW(hdc, &mut tm);
        let line_height = (tm.tmHeight as f32 * LINE_HEIGHT_RATIO) as i32;
        SelectObject(hdc, old_font as HGDIOBJ);

        let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
        let hint_row_h = (HINT_ROW_HEIGHT as f32 * scale) as i32;
        let session_header_h = (SESSION_HEADER_HEIGHT as f32 * scale) as i32;
        let msg_padding = (MESSAGE_PADDING as f32 * scale) as i32;
        let content_width = chat_w - 2 * msg_padding;

        // Compute dynamic input height based on content
        let idx = ctx
            .app
            .active_session
            .min(ctx.app.sessions.len().saturating_sub(1));
        let logical_input_h = compute_input_height(
            hdc,
            ctx.fonts,
            &ctx.app.sessions[idx].chat,
            chat_w,
            line_height,
            scale,
        );
        ctx.app.sessions[idx].chat.input_height = logical_input_h;
        let input_h = (logical_input_h as f32 * scale) as i32;

        let input_area_total_h = input_h + hint_row_h + input_margin;
        let input_area_top = height - input_area_total_h;
        let input_box_top = input_area_top;
        let hint_row_top = input_box_top + input_h;
        let messages_area_bottom = input_area_top - input_margin;

        let messages_area_top = session_header_h;

        // Draw session header bar
        draw_session_header(hdc, ctx, chat_left, chat_right, scale);

        // Recompute message layout
        {
            let idx = ctx
                .app
                .active_session
                .min(ctx.app.sessions.len().saturating_sub(1));
            let chat = &mut ctx.app.sessions[idx].chat;
            let viewport_h = messages_area_bottom - messages_area_top;
            compute_message_heights(hdc, ctx.fonts, chat, content_width, line_height, scale);
            if chat.is_at_bottom {
                chat.scroll_to_bottom(viewport_h);
            }
        }

        // Clip messages to area between session header and input
        {
            let clip_rgn = CreateRectRgn(
                chat_left,
                messages_area_top,
                chat_right,
                messages_area_bottom,
            );
            SelectClipRgn(hdc, clip_rgn);

            if ctx.app.sessions[idx].chat.messages.is_empty() {
                draw_empty_state(
                    hdc,
                    ctx,
                    chat_left,
                    chat_right,
                    messages_area_bottom,
                    messages_area_top,
                    scale,
                );
            } else {
                draw_messages(
                    hdc,
                    ctx,
                    chat_left,
                    chat_right,
                    messages_area_bottom,
                    messages_area_top,
                    line_height,
                    scale,
                );
            }

            SelectClipRgn(hdc, std::ptr::null_mut());
            DeleteObject(clip_rgn as HGDIOBJ);
        }

        // Draw message scrollbar
        draw_message_scrollbar(
            hdc,
            ctx,
            chat_left,
            chat_right,
            messages_area_bottom,
            messages_area_top,
            scale,
        );

        // Draw input box with send button
        draw_input_box(
            hdc,
            &mut ctx.app.sessions,
            ctx.app.active_session,
            ctx.fonts,
            chat_left,
            chat_right,
            input_box_top,
            input_h,
            line_height,
            scale,
            ctx.app.cursor_visible,
            ctx.app.focused_element,
            ctx.app.hover.send_btn,
            ctx.app.pressed_element,
        );

        // Draw hint row below input
        draw_hint_row(hdc, ctx, chat_left, chat_right, hint_row_top, scale);
    }
}
