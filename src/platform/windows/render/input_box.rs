// Input box rendering: text area with cursor, selection, placeholder, send button, hint row.
#![allow(dead_code)]

use crate::chat::ChatState;
use crate::platform::windows::ffi::*;
use crate::platform::windows::fonts::Fonts;
use crate::platform::windows::paint::PaintContext;
use crate::platform::windows::text::measure_text_width;
use crate::session::Session;
use crate::theme::*;
use crate::ui::{FocusedElement, PressedElement};

/// Compute the required input box height in logical pixels based on content.
pub fn compute_input_height(
    hdc: HDC,
    fonts: &Fonts,
    chat: &ChatState,
    chat_w: i32,
    line_height: i32,
    scale: f32,
) -> i32 {
    if chat.input_buffer.is_empty() {
        return INPUT_MIN_HEIGHT;
    }

    let input_margin_dev = (INPUT_MARGIN as f32 * scale) as i32;
    let h_pad_dev = (SPACE_4 as f32 * scale) as i32;
    let send_btn_dev = (SEND_BTN_SIZE as f32 * scale) as i32;
    let send_btn_margin_dev = (SEND_BTN_MARGIN as f32 * scale) as i32;
    let text_width = (chat_w
        - 2 * input_margin_dev
        - 2 * h_pad_dev
        - send_btn_dev
        - send_btn_margin_dev
        - h_pad_dev / 2)
        .max(100);

    unsafe {
        let old_font = SelectObject(hdc, fonts.normal as HGDIOBJ);

        // Count visual lines exactly the way draw_input_box renders them:
        // each logical line wrapped at the same text width, one line_height per
        // visual line (DrawTextW's DT_CALCRECT uses the raw font height, which
        // underestimates the rendered height and made the box too short for
        // multi-line input, pushing the chat up without showing the new line).
        let mut visual_lines: i32 = 0;
        for raw_line in chat.input_buffer.split('\n') {
            if raw_line.is_empty() {
                visual_lines += 1;
                continue;
            }
            let wide = to_wide(raw_line);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: text_width,
                bottom: 0,
            };
            DrawTextW(
                hdc,
                wide.as_ptr(),
                wide.len() as i32 - 1,
                &mut rect as LPRECT,
                DT_CALCRECT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX,
            );
            let lines_in_raw = if rect.bottom > 0 {
                let raw_line_h = rect.bottom;
                let raw_line_h_logical = (raw_line_h as f32 / scale).ceil() as i32;
                let line_h_logical = (line_height as f32 / scale).ceil() as i32;
                ((raw_line_h_logical + line_h_logical - 1) / line_h_logical).max(1)
            } else {
                1
            };
            visual_lines += lines_in_raw;
        }
        SelectObject(hdc, old_font as HGDIOBJ);

        let v_pad_dev = (INPUT_LINE_PADDING as f32 * scale) as i32;
        let text_height_dev = visual_lines.max(1) * line_height;
        let total_dev = text_height_dev + 2 * v_pad_dev;
        let total_logical = (total_dev as f32 / scale).round() as i32;

        let line_height_logical = (line_height as f32 / scale).round() as i32;
        let max_logical = INPUT_MAX_LINES * line_height_logical + 2 * INPUT_LINE_PADDING;

        total_logical
            .max(INPUT_MIN_HEIGHT)
            .min(max_logical.max(INPUT_MIN_HEIGHT))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_input_box(
    hdc: HDC,
    sessions: &mut [Session],
    active_session: usize,
    fonts: &Fonts,
    chat_left: i32,
    chat_right: i32,
    input_top: i32,
    input_h: i32,
    line_height: i32,
    scale: f32,
    cursor_visible: bool,
    focused_element: Option<FocusedElement>,
    hover_send_btn: bool,
    pressed_element: Option<PressedElement>,
) {
    let input_margin = (INPUT_MARGIN as f32 * scale) as i32;
    let height = input_top + input_h + input_margin;

    let idx = active_session.min(sessions.len().saturating_sub(1));
    let chat = &mut sessions[idx].chat;
    let is_disabled = chat.is_waiting_for_agent;
    let is_focused = focused_element == Some(FocusedElement::Input) || chat.input_focused;

    let send_btn_size = (SEND_BTN_SIZE as f32 * scale) as i32;
    let send_btn_margin = (SEND_BTN_MARGIN as f32 * scale) as i32;
    let has_text = !chat.input_buffer.trim().is_empty();
    let send_enabled = has_text && !is_disabled;

    unsafe {
        let input_left = chat_left + input_margin;
        let input_right = chat_right - input_margin;
        let input_bottom = height - input_margin;
        let input_rect = RECT {
            left: input_left,
            top: input_top,
            right: input_right,
            bottom: input_bottom,
        };

        // Input background
        let bg_color = if is_disabled {
            DISABLED_BG
        } else {
            INPUT_BG_COLOR
        };
        let bg_brush = CreateSolidBrush(bg_color);
        FillRect(hdc, &input_rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        // Border
        let border_color = if is_focused {
            INPUT_FOCUS_BORDER
        } else if is_disabled {
            BORDER_DISABLED
        } else {
            INPUT_BORDER_COLOR
        };
        let border_width = if is_focused {
            (2.0 * scale).max(2.0) as i32
        } else {
            (1.0 * scale).max(1.0) as i32
        };
        let border_pen = CreatePen(PS_SOLID, border_width, border_color);
        let old_pen = SelectObject(hdc, border_pen as HGDIOBJ);
        let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
        let radius = (RADIUS_MD as f32 * scale) as i32;
        RoundRect(
            hdc,
            input_rect.left,
            input_rect.top,
            input_rect.right,
            input_rect.bottom,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_pen as HGDIOBJ);
        SelectObject(hdc, old_brush as HGDIOBJ);
        DeleteObject(border_pen as HGDIOBJ);

        // Send button
        let send_btn_rect = RECT {
            left: input_right - send_btn_margin - send_btn_size,
            top: input_top + (input_h - send_btn_size) / 2,
            right: input_right - send_btn_margin,
            bottom: input_top + (input_h - send_btn_size) / 2 + send_btn_size,
        };
        draw_send_button(
            hdc,
            fonts,
            &send_btn_rect,
            send_enabled,
            hover_send_btn,
            pressed_element,
            focused_element == Some(FocusedElement::SendButton),
            is_disabled,
            scale,
        );

        // Text area
        let h_pad = (SPACE_4 as f32 * scale) as i32;
        let v_pad = (INPUT_LINE_PADDING as f32 * scale) as i32;
        let text_left = input_left + h_pad;
        let text_right = send_btn_rect.left - send_btn_margin - h_pad / 2;
        let text_top = input_top + v_pad;
        let text_width = text_right - text_left;

        let old_font = SelectObject(hdc, fonts.normal as HGDIOBJ);
        let text_color = if is_disabled {
            DISABLED_TEXT
        } else {
            INPUT_TEXT_COLOR
        };
        SetTextColor(hdc, text_color);

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
        let cursor_h = tm.tmHeight.max((10.0 * scale) as i32);

        // Wrap input buffer into visual lines
        let buffer = &chat.input_buffer;
        let raw_lines: Vec<&str> = if buffer.is_empty() {
            vec![""]
        } else {
            buffer.split('\n').collect()
        };

        struct WrappedLine {
            text: String,
            raw_line_idx: usize,
            byte_offset: usize,
        }
        let mut wrapped_lines: Vec<WrappedLine> = Vec::new();
        let mut byte_pos = 0usize;
        for (raw_idx, raw_line) in raw_lines.iter().enumerate() {
            let line_start_byte = byte_pos;
            if raw_line.is_empty() {
                wrapped_lines.push(WrappedLine {
                    text: String::new(),
                    raw_line_idx: raw_idx,
                    byte_offset: line_start_byte,
                });
            } else {
                // Tokenize into runs of spaces and non-spaces, preserving original text.
                let bytes = raw_line.as_bytes();
                let mut tokens: Vec<(usize, &str)> = Vec::new();
                let mut tok_start = 0usize;
                let mut in_space = bytes[0] == b' ' || bytes[0] == b'\t';
                for (i, &b) in bytes.iter().enumerate().skip(1) {
                    let cur_space = b == b' ' || b == b'\t';
                    if cur_space != in_space {
                        tokens.push((tok_start, &raw_line[tok_start..i]));
                        tok_start = i;
                        in_space = cur_space;
                    }
                }
                tokens.push((tok_start, &raw_line[tok_start..]));

                let mut current = String::new();
                let mut seg_start = line_start_byte;
                for &(tok_off, tok) in &tokens {
                    let test = format!("{}{}", current, tok);
                    let wide = to_wide(&test);
                    let mut sz = POINT { x: 0, y: 0 };
                    GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut sz);
                    if sz.x <= text_width || current.is_empty() {
                        current = test;
                    } else {
                        wrapped_lines.push(WrappedLine {
                            text: current.clone(),
                            raw_line_idx: raw_idx,
                            byte_offset: seg_start,
                        });
                        seg_start = line_start_byte + tok_off;
                        current = tok.to_string();
                    }
                }
                if !current.is_empty() {
                    wrapped_lines.push(WrappedLine {
                        text: current,
                        raw_line_idx: raw_idx,
                        byte_offset: seg_start,
                    });
                }
            }
            byte_pos += raw_line.len();
            if raw_idx < raw_lines.len() - 1 {
                byte_pos += 1;
            }
        }

        let total_wrapped = wrapped_lines.len() as i32;
        let visible_lines = ((input_h - 2 * v_pad) / line_height).max(1);
        let max_scroll = (total_wrapped - visible_lines).max(0);

        // Adjust scroll offset to keep cursor visible
        let (cursor_raw_line, cursor_col_bytes) = chat.cursor_line_col();
        let mut cursor_wrapped_idx = 0usize;
        for (i, wl) in wrapped_lines.iter().enumerate() {
            if wl.raw_line_idx == cursor_raw_line {
                let seg_end = wl.byte_offset + wl.text.len();
                let cursor_byte = chat.line_start_pos(chat.cursor_pos) + cursor_col_bytes;
                if cursor_byte >= wl.byte_offset && cursor_byte <= seg_end {
                    cursor_wrapped_idx = i;
                    break;
                }
                if i + 1 >= wrapped_lines.len()
                    || wrapped_lines[i + 1].raw_line_idx != cursor_raw_line
                {
                    cursor_wrapped_idx = i;
                    break;
                }
            }
        }

        let scroll = if cursor_wrapped_idx < chat.input_scroll_offset as usize {
            cursor_wrapped_idx as i32
        } else if cursor_wrapped_idx >= (chat.input_scroll_offset + visible_lines) as usize {
            (cursor_wrapped_idx - visible_lines as usize + 1) as i32
        } else {
            chat.input_scroll_offset
        };
        let scroll = scroll.max(0).min(max_scroll);
        chat.input_scroll_offset = scroll;

        // Draw visible wrapped lines
        SetBkMode(hdc, OPAQUE);
        SetBkColor(hdc, bg_color);

        let mut y = text_top;
        for i in scroll..(scroll + visible_lines).min(total_wrapped) {
            let wl = &wrapped_lines[i as usize];
            if wl.text.is_empty() {
                y += line_height;
                continue;
            }
            let wide = to_wide(&wl.text);
            let mut text_rect = RECT {
                left: text_left,
                top: y,
                right: text_right,
                bottom: y + line_height,
            };
            DrawTextW(
                hdc,
                wide.as_ptr(),
                wide.len() as i32 - 1,
                &mut text_rect as LPRECT,
                DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP | DT_NOPREFIX,
            );
            y += line_height;
        }

        // Selection highlight
        if let Some((sel_start, sel_end)) = chat.selection_range() {
            let sel_brush = CreateSolidBrush(SELECTION_COLOR);

            for i in scroll..(scroll + visible_lines).min(total_wrapped) {
                let wl = &wrapped_lines[i as usize];
                let seg_start = wl.byte_offset;
                let seg_end = seg_start + wl.text.len();

                let overlap_start = sel_start.max(seg_start);
                let overlap_end = sel_end.min(seg_end);

                if overlap_start < overlap_end
                    || (sel_start == sel_end && overlap_start == seg_end && seg_start <= sel_start)
                {
                    let line_y = text_top + (i - scroll) * line_height;
                    let line_bottom = line_y + cursor_h;

                    let extends_past = sel_end > seg_end && seg_end < buffer.len();

                    let hl_left = if overlap_start <= seg_start {
                        text_left
                    } else {
                        text_left
                            + measure_text_width(hdc, &buffer[seg_start..overlap_start], fonts)
                    };
                    let hl_right = if extends_past || overlap_end >= seg_end {
                        text_right
                    } else {
                        text_left + measure_text_width(hdc, &buffer[seg_start..overlap_end], fonts)
                    };

                    if hl_right > hl_left {
                        let sel_rect = RECT {
                            left: hl_left,
                            top: line_y,
                            right: hl_right,
                            bottom: line_bottom,
                        };
                        FillRect(hdc, &sel_rect, sel_brush);
                    } else if extends_past {
                        let sel_rect = RECT {
                            left: hl_left,
                            top: line_y,
                            right: hl_left + (4.0 * scale) as i32,
                            bottom: line_bottom,
                        };
                        FillRect(hdc, &sel_rect, sel_brush);
                    }
                }
            }
            DeleteObject(sel_brush as HGDIOBJ);
        }

        // Cursor
        if cursor_visible && chat.input_focused {
            let cursor_byte = chat.cursor_pos;
            let cursor_wrapped = cursor_wrapped_idx as i32;
            if cursor_wrapped >= scroll && cursor_wrapped < scroll + visible_lines {
                let wl = &wrapped_lines[cursor_wrapped_idx];
                let col_in_segment = cursor_byte
                    .saturating_sub(wl.byte_offset)
                    .min(wl.text.len());
                let cursor_text = &wl.text[..col_in_segment];
                let cursor_x = text_left + measure_text_width(hdc, cursor_text, fonts);
                let cursor_y = text_top + (cursor_wrapped - scroll) * line_height;
                let cursor_rect = RECT {
                    left: cursor_x,
                    top: cursor_y,
                    right: cursor_x + (1.0 * scale).max(1.0) as i32,
                    bottom: cursor_y + cursor_h,
                };
                let cursor_brush = CreateSolidBrush(CURSOR_COLOR);
                FillRect(hdc, &cursor_rect, cursor_brush);
                DeleteObject(cursor_brush as HGDIOBJ);
            }
        }

        // Placeholder
        if chat.input_buffer.is_empty() && !chat.input_focused {
            SetTextColor(hdc, INPUT_PLACEHOLDER);
            let placeholder = if chat.is_waiting_for_agent {
                to_wide("Waiting for response... (Esc to cancel)")
            } else {
                to_wide("Type a message... (Shift+Enter for new line)")
            };
            let mut ph_rect = RECT {
                left: text_left,
                top: text_top,
                right: text_right,
                bottom: text_top + line_height,
            };
            DrawTextW(
                hdc,
                placeholder.as_ptr(),
                placeholder.len() as i32 - 1,
                &mut ph_rect as LPRECT,
                DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP,
            );
        }

        SelectObject(hdc, old_font as HGDIOBJ);
        SetBkMode(hdc, TRANSPARENT);
    }
}

/// Draw the send/stop button (circular, with arrow or stop-square icon).
#[allow(clippy::too_many_arguments)]
fn draw_send_button(
    hdc: HDC,
    fonts: &Fonts,
    btn_rect: &RECT,
    enabled: bool,
    hover: bool,
    pressed_element: Option<PressedElement>,
    focused: bool,
    is_waiting: bool,
    scale: f32,
) {
    unsafe {
        let is_pressed = pressed_element == Some(PressedElement::SendButton);

        let btn_color = if is_waiting {
            if is_pressed {
                STOP_BTN_PRESSED
            } else if hover {
                STOP_BTN_HOVER
            } else {
                STOP_BTN_BG
            }
        } else if !enabled {
            SEND_BTN_DISABLED
        } else if is_pressed {
            SEND_BTN_PRESSED
        } else if hover {
            SEND_BTN_HOVER
        } else {
            SEND_BTN_BG
        };

        let radius = (btn_rect.height() / 2).max((RADIUS_MD as f32 * scale) as i32);
        let gfx = crate::platform::windows::gfx::Gfx::new(hdc);
        match gfx {
            Some(ref g) => {
                if hover && enabled && !is_pressed {
                    g.button_with_ring(*btn_rect, radius, btn_color, btn_color);
                } else {
                    g.fill_round_rect(*btn_rect, radius, btn_color);
                }
            }
            None => {
                let btn_brush = CreateSolidBrush(btn_color);
                let btn_pen = CreatePen(PS_SOLID, 1, btn_color);
                let old_pen = SelectObject(hdc, btn_pen as HGDIOBJ);
                let old_brush = SelectObject(hdc, btn_brush as HGDIOBJ);
                RoundRect(
                    hdc,
                    btn_rect.left,
                    btn_rect.top,
                    btn_rect.right,
                    btn_rect.bottom,
                    radius * 2,
                    radius * 2,
                );
                SelectObject(hdc, old_pen as HGDIOBJ);
                SelectObject(hdc, old_brush as HGDIOBJ);
                DeleteObject(btn_pen as HGDIOBJ);
                DeleteObject(btn_brush as HGDIOBJ);
            }
        }

        // Focus ring
        if focused {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            RoundRect(
                hdc,
                btn_rect.left - 2,
                btn_rect.top - 2,
                btn_rect.right + 2,
                btn_rect.bottom + 2,
                (radius + 2) * 2,
                (radius + 2) * 2,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        // Arrow or stop icon
        let (icon, icon_color) = if is_waiting {
            ("\u{25A0}", STOP_BTN_ICON)
        } else if enabled {
            ("\u{27A4}", SEND_BTN_ICON)
        } else {
            ("\u{27A4}", DISABLED_TEXT)
        };
        let old_font = SelectObject(hdc, fonts.bold as HGDIOBJ);
        SetTextColor(hdc, icon_color);
        let icon_wide = to_wide(icon);
        let mut icon_rect = *btn_rect;
        if !is_waiting {
            icon_rect.left += (2.0 * scale) as i32;
        }
        DrawTextW(
            hdc,
            icon_wide.as_ptr(),
            icon_wide.len() as i32 - 1,
            &mut icon_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);
    }
}

/// Draw the hint row below the input box.
pub fn draw_hint_row(
    hdc: HDC,
    ctx: &PaintContext,
    chat_left: i32,
    chat_right: i32,
    hint_top: i32,
    scale: f32,
) {
    let hint_h = (HINT_ROW_HEIGHT as f32 * scale) as i32;
    let input_margin = (INPUT_MARGIN as f32 * scale) as i32;

    unsafe {
        let old_font = SelectObject(hdc, ctx.fonts.hint as HGDIOBJ);
        SetTextColor(hdc, INPUT_HINT_TEXT);

        let idx = ctx.active_session.min(ctx.sessions.len().saturating_sub(1));
        let is_waiting = ctx.sessions[idx].chat.is_waiting_for_agent;

        let hint_text = if is_waiting {
            "Esc to cancel"
        } else {
            "Enter to send \u{00B7} Shift+Enter for newline"
        };

        let hint_wide = to_wide(hint_text);
        let mut hint_rect = RECT {
            left: chat_left + input_margin,
            top: hint_top,
            right: chat_right - input_margin,
            bottom: hint_top + hint_h,
        };
        DrawTextW(
            hdc,
            hint_wide.as_ptr(),
            hint_wide.len() as i32 - 1,
            &mut hint_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);
    }
}
