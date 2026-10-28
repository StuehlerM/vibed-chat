// Message rendering: bubbles, thinking sections, text layout, scrollbar.
#![allow(dead_code)]

use crate::chat::{ChatState, Message, Role};
use crate::markdown::Style;
use crate::platform::windows::ffi::*;
use crate::platform::windows::fonts::{select_font_for_style, Fonts};
use crate::platform::windows::paint::PaintContext;
use crate::platform::windows::text::wrap_text;
use crate::theme::*;

#[allow(clippy::too_many_arguments)]
pub fn draw_messages(
    hdc: HDC,
    ctx: &mut PaintContext,
    chat_left: i32,
    chat_right: i32,
    area_bottom: i32,
    area_top: i32,
    line_height: i32,
    scale: f32,
) {
    let chat_w = chat_right - chat_left;
    let content_width = chat_w - 2 * (MESSAGE_PADDING as f32 * scale) as i32;
    let max_bubble_width = (content_width as f32 * MESSAGE_MAX_WIDTH_RATIO) as i32;

    let scroll_offset = ctx.sessions[ctx.active_session].chat.scroll_offset;
    let streaming_idx = ctx.sessions[ctx.active_session].chat.streaming_message_idx;
    let messages_len = ctx.sessions[ctx.active_session].chat.messages.len();
    let hover_thinking = ctx.hover_thinking_toggle;
    let viewport = area_bottom - area_top;

    let visible = {
        let chat = &ctx.sessions[ctx.active_session].chat;
        chat.visible_messages(viewport, scroll_offset)
    };

    ctx.link_rects.borrow_mut().clear();
    ctx.code_block_rects.borrow_mut().clear();

    for (msg_idx, y_top) in visible {
        if msg_idx >= messages_len {
            break;
        }
        let msg = &ctx.sessions[ctx.active_session].chat.messages[msg_idx];
        let is_streaming = streaming_idx == Some(msg_idx);
        let is_thinking_hover = hover_thinking == Some(msg_idx);

        let adjusted_y = y_top + area_top;

        draw_message(
            hdc,
            msg,
            adjusted_y,
            chat_left,
            chat_w,
            max_bubble_width,
            line_height,
            scale,
            ctx.fonts,
            is_streaming,
            is_thinking_hover,
            ctx.hover_link,
            ctx.hover_code_copy,
            ctx.link_rects,
            ctx.code_block_rects,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_message(
    hdc: HDC,
    msg: &Message,
    y_top: i32,
    chat_left: i32,
    chat_w: i32,
    max_bubble_width: i32,
    line_height: i32,
    scale: f32,
    fonts: &Fonts,
    is_streaming: bool,
    hover_thinking: bool,
    hover_link: Option<usize>,
    hover_code_copy: Option<usize>,
    link_rects: &std::cell::RefCell<Vec<crate::app::ClickableRect>>,
    code_block_rects: &std::cell::RefCell<Vec<crate::app::ClickableRect>>,
) {
    unsafe {
        let label_height = (SPACE_4 as f32 * scale) as i32;
        let bubble_padding = (MESSAGE_INNER_PADDING as f32 * scale) as i32;
        let thinking_header_h = (THINKING_HEADER_HEIGHT as f32 * scale) as i32;

        let old_font = SelectObject(hdc, fonts.normal as HGDIOBJ);
        let text_width = max_bubble_width - 2 * bubble_padding;

        // Calculate thinking section height
        let has_thinking = !msg.thinking.is_empty();
        let thinking_text_height = if has_thinking && !msg.thinking_collapsed {
            let mut th = 0i32;
            for run in &msg.thinking_runs {
                let style_font = if run.style.code {
                    fonts.code
                } else if run.style.bold {
                    fonts.bold
                } else {
                    fonts.normal
                };
                SelectObject(hdc, style_font as HGDIOBJ);
                for line_text in run.text.split('\n') {
                    if line_text.is_empty() {
                        th += line_height;
                        continue;
                    }
                    let wrapped = wrap_text(hdc, line_text, text_width, &run.style, fonts);
                    th += wrapped.len() as i32 * line_height;
                }
            }
            if th == 0 {
                th = line_height;
            }
            th
        } else {
            0
        };

        // Calculate main text height
        let mut total_text_height = 0i32;
        let mut lines: Vec<(String, Style, i32)> = Vec::new();

        for run in &msg.runs {
            let style_font = if run.style.code {
                fonts.code
            } else if run.style.bold {
                fonts.bold
            } else {
                fonts.normal
            };
            SelectObject(hdc, style_font as HGDIOBJ);
            for line_text in run.text.split('\n') {
                if line_text.is_empty() {
                    total_text_height += line_height;
                    lines.push((String::new(), run.style.clone(), 0));
                    continue;
                }
                let wrapped = wrap_text(hdc, line_text, text_width, &run.style, fonts);
                for w in wrapped {
                    total_text_height += line_height;
                    lines.push(w);
                }
            }
        }
        if total_text_height == 0 {
            total_text_height = line_height;
        }

        // Calculate bubble dimensions
        let thinking_section_h = if has_thinking {
            thinking_header_h
                + if msg.thinking_collapsed {
                    0
                } else {
                    thinking_text_height + bubble_padding
                }
        } else {
            0
        };

        let bubble_height = if msg.role == Role::System {
            total_text_height + 2 * bubble_padding
        } else {
            total_text_height
                + 2 * bubble_padding
                + label_height
                + (SPACE_1 as f32 * scale) as i32
                + thinking_section_h
        };
        let msg_padding = (MESSAGE_PADDING as f32 * scale) as i32;
        let bubble_width = if msg.role == Role::System {
            chat_w - 2 * msg_padding
        } else {
            (text_width + 2 * bubble_padding).min(max_bubble_width)
        };

        let (bubble_left, label_color) = match msg.role {
            Role::User => {
                let left = chat_left + chat_w - msg_padding - bubble_width;
                (left, LABEL_USER)
            }
            Role::Agent => (chat_left + msg_padding, LABEL_AGENT),
            Role::System => (chat_left + msg_padding, SYSTEM_TEXT_COLOR),
        };

        let bubble_top = y_top;

        // Draw bubble background
        if msg.role != Role::System {
            let bubble_rect = RECT {
                left: bubble_left,
                top: bubble_top,
                right: bubble_left + bubble_width,
                bottom: bubble_top + bubble_height,
            };
            let bubble_color = match msg.role {
                Role::User => USER_BUBBLE_BG,
                Role::Agent => AGENT_BUBBLE_BG,
                Role::System => BG_COLOR,
            };
            let radius = (RADIUS_MD as f32 * scale) as i32;
            let brush = CreateSolidBrush(bubble_color);
            let pen = CreatePen(PS_SOLID, 1, bubble_color);
            let old_pen = SelectObject(hdc, pen as HGDIOBJ);
            let old_brush = SelectObject(hdc, brush as HGDIOBJ);
            RoundRect(
                hdc,
                bubble_rect.left,
                bubble_rect.top,
                bubble_rect.right,
                bubble_rect.bottom,
                radius * 2,
                radius * 2,
            );
            SelectObject(hdc, old_pen as HGDIOBJ);
            SelectObject(hdc, old_brush as HGDIOBJ);
            DeleteObject(pen as HGDIOBJ);
            DeleteObject(brush as HGDIOBJ);
        }

        // Draw role label
        let mut content_y = bubble_top + bubble_padding;
        if msg.role != Role::System {
            let label_font = SelectObject(hdc, fonts.label as HGDIOBJ);
            SetTextColor(hdc, label_color);
            let label_text = if is_streaming {
                format!("{} \u{25CF}", msg.role.label())
            } else {
                msg.role.label().to_string()
            };
            let label_wide = to_wide(&label_text);
            let mut label_rect = RECT {
                left: bubble_left + bubble_padding,
                top: content_y,
                right: bubble_left + bubble_width - bubble_padding,
                bottom: content_y + label_height,
            };
            DrawTextW(
                hdc,
                label_wide.as_ptr(),
                label_wide.len() as i32 - 1,
                &mut label_rect as LPRECT,
                DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP,
            );
            SelectObject(hdc, label_font as HGDIOBJ);
            content_y += label_height + (SPACE_1 as f32 * scale) as i32;
        }

        // Draw thinking section (agent messages only)
        if has_thinking && msg.role == Role::Agent {
            let arrow = if msg.thinking_collapsed {
                "\u{25B6}"
            } else {
                "\u{25BC}"
            };
            let header_text = if msg.thinking_collapsed {
                format!("{} Thinking...", arrow)
            } else {
                format!("{} Thinking", arrow)
            };
            let header_color = if hover_thinking {
                ACCENT
            } else {
                THINKING_TEXT_COLOR
            };

            let header_font = SelectObject(hdc, fonts.label as HGDIOBJ);
            SetTextColor(hdc, header_color);
            let header_wide = to_wide(&header_text);
            let mut header_rect = RECT {
                left: bubble_left + bubble_padding,
                top: content_y,
                right: bubble_left + bubble_width - bubble_padding,
                bottom: content_y + thinking_header_h,
            };
            DrawTextW(
                hdc,
                header_wide.as_ptr(),
                header_wide.len() as i32 - 1,
                &mut header_rect as LPRECT,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
            );
            SelectObject(hdc, header_font as HGDIOBJ);
            content_y += thinking_header_h;

            // Thinking content (if expanded)
            if !msg.thinking_collapsed {
                let think_bg_rect = RECT {
                    left: bubble_left + bubble_padding,
                    top: content_y,
                    right: bubble_left + bubble_width - bubble_padding,
                    bottom: content_y + thinking_text_height + bubble_padding,
                };
                let think_brush = CreateSolidBrush(THINKING_BG_COLOR);
                FillRect(hdc, &think_bg_rect, think_brush);
                DeleteObject(think_brush as HGDIOBJ);

                let mut think_y = content_y + bubble_padding / 2;
                for run in &msg.thinking_runs {
                    let style_font = if run.style.code {
                        fonts.code
                    } else if run.style.bold {
                        fonts.bold
                    } else {
                        fonts.italic
                    };
                    SelectObject(hdc, style_font as HGDIOBJ);
                    SetTextColor(hdc, THINKING_TEXT_COLOR);
                    for line_text in run.text.split('\n') {
                        if line_text.is_empty() {
                            think_y += line_height;
                            continue;
                        }
                        let wrapped = wrap_text(hdc, line_text, text_width, &run.style, fonts);
                        for (w_text, w_style, _) in &wrapped {
                            if !w_text.is_empty() {
                                let w_font = if w_style.code {
                                    fonts.code
                                } else if w_style.italic {
                                    fonts.italic
                                } else {
                                    fonts.normal
                                };
                                SelectObject(hdc, w_font as HGDIOBJ);
                                let wide = to_wide(w_text);
                                let mut tr = RECT {
                                    left: bubble_left + bubble_padding + bubble_padding / 2,
                                    top: think_y,
                                    right: bubble_left + bubble_width - bubble_padding,
                                    bottom: think_y + line_height,
                                };
                                DrawTextW(
                                    hdc,
                                    wide.as_ptr(),
                                    wide.len() as i32 - 1,
                                    &mut tr as LPRECT,
                                    DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP,
                                );
                            }
                            think_y += line_height;
                        }
                    }
                }
                SelectObject(hdc, fonts.italic as HGDIOBJ);
                content_y += thinking_text_height + bubble_padding;
            }
        }

        // Draw main text lines, register link rects, detect code blocks
        let mut text_y = content_y;
        let mut local_link_idx = link_rects.borrow().len();
        let mut in_code_block = false;
        let mut code_block_start_y = 0i32;
        let mut code_block_lines: Vec<String> = Vec::new();
        let mut code_block_lang: Option<String> = None;
        let code_block_left = bubble_left + bubble_padding;
        let code_block_right = bubble_left + bubble_width - bubble_padding;
        let mut code_block_global_idx = code_block_rects.borrow().len();

        for (line_text, style, _width) in &lines {
            if line_text.is_empty() {
                if in_code_block {
                    code_block_lines.push(String::new());
                }
                text_y += line_height;
                continue;
            }
            let style_font = select_font_for_style(hdc, style, fonts);

            // Code block tracking
            if style.is_code_block {
                if !in_code_block {
                    in_code_block = true;
                    code_block_start_y = text_y;
                    code_block_lines.clear();
                    code_block_lang = style.code_lang.clone();
                }
                code_block_lines.push(line_text.clone());
            } else if in_code_block {
                in_code_block = false;
                finish_code_block(
                    hdc,
                    fonts,
                    code_block_start_y,
                    text_y,
                    code_block_left,
                    code_block_right,
                    &code_block_lang,
                    &code_block_lines,
                    hover_code_copy == Some(code_block_global_idx),
                    code_block_rects,
                    line_height,
                    scale,
                );
                code_block_global_idx = code_block_rects.borrow().len();
            }

            let is_link_hover = style.is_link && hover_link == Some(local_link_idx);
            let text_color = if style.is_link {
                if is_link_hover {
                    MD_LINK_HOVER
                } else {
                    MD_LINK
                }
            } else {
                match msg.role {
                    Role::User => USER_BUBBLE_TEXT,
                    Role::System => SYSTEM_TEXT_COLOR,
                    Role::Agent => style.color,
                }
            };
            SetTextColor(hdc, text_color);
            let wide = to_wide(line_text);
            let draw_format = if msg.role == Role::System {
                DT_CENTER | DT_TOP | DT_SINGLELINE | DT_NOCLIP
            } else {
                DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP
            };
            let mut text_rect = RECT {
                left: bubble_left + bubble_padding,
                top: text_y,
                right: bubble_left + bubble_width - bubble_padding,
                bottom: text_y + line_height,
            };
            DrawTextW(
                hdc,
                wide.as_ptr(),
                wide.len() as i32 - 1,
                &mut text_rect as LPRECT,
                draw_format,
            );

            // Measure text width
            let mut sz = POINT { x: 0, y: 0 };
            GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut sz);

            // Underline + register clickable rect for links
            if style.is_link {
                let underline_y = text_y + line_height - 1;
                let underline_rect = RECT {
                    left: text_rect.left,
                    top: underline_y,
                    right: text_rect.left + sz.x,
                    bottom: underline_y + 1,
                };
                let underline_brush = CreateSolidBrush(text_color);
                FillRect(hdc, &underline_rect, underline_brush);
                DeleteObject(underline_brush as HGDIOBJ);

                link_rects.borrow_mut().push((
                    text_rect.left,
                    text_y,
                    text_rect.left + sz.x,
                    text_y + line_height,
                    line_text.clone(),
                ));
                local_link_idx += 1;
            }

            SelectObject(hdc, style_font as HGDIOBJ);
            text_y += line_height;
        }

        // Handle code block that extends to end of message
        if in_code_block {
            finish_code_block(
                hdc,
                fonts,
                code_block_start_y,
                text_y,
                code_block_left,
                code_block_right,
                &code_block_lang,
                &code_block_lines,
                hover_code_copy == Some(code_block_global_idx),
                code_block_rects,
                line_height,
                scale,
            );
        }

        SelectObject(hdc, old_font as HGDIOBJ);
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_code_block(
    hdc: HDC,
    fonts: &Fonts,
    start_y: i32,
    end_y: i32,
    left: i32,
    right: i32,
    lang: &Option<String>,
    code_lines: &[String],
    is_hover: bool,
    code_block_rects: &std::cell::RefCell<Vec<crate::app::ClickableRect>>,
    line_height: i32,
    scale: f32,
) {
    unsafe {
        // Draw language label at top-right
        if let Some(ref l) = *lang {
            let old_font = SelectObject(hdc, fonts.caption as HGDIOBJ);
            SetTextColor(hdc, TEXT_TERTIARY);
            let label_wide = to_wide(&l.to_uppercase());
            let mut label_rect = RECT {
                left,
                top: start_y,
                right: right - (SPACE_2 as f32 * scale) as i32,
                bottom: start_y + line_height,
            };
            DrawTextW(
                hdc,
                label_wide.as_ptr(),
                label_wide.len() as i32 - 1,
                &mut label_rect as LPRECT,
                DT_RIGHT | DT_TOP | DT_SINGLELINE | DT_NOCLIP,
            );
            SelectObject(hdc, old_font as HGDIOBJ);
        }

        // Draw copy button at top-right (below language label if present)
        let btn_size = (14.0 * scale).max(12.0) as i32;
        let btn_right = right - (SPACE_1 as f32 * scale) as i32;
        let btn_top = start_y + (SPACE_1 as f32 * scale) as i32;
        let copy_color = if is_hover {
            TEXT_PRIMARY
        } else {
            TEXT_TERTIARY
        };
        let old_font = SelectObject(hdc, fonts.caption as HGDIOBJ);
        SetTextColor(hdc, copy_color);
        let copy_icon = to_wide("\u{2398}");
        let mut btn_rect = RECT {
            left: btn_right - btn_size,
            top: btn_top,
            right: btn_right,
            bottom: btn_top + btn_size,
        };
        DrawTextW(
            hdc,
            copy_icon.as_ptr(),
            copy_icon.len() as i32 - 1,
            &mut btn_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // Register clickable rect with the full code text
        let code_text = code_lines.join("\n");
        code_block_rects.borrow_mut().push((
            btn_rect.left,
            btn_rect.top,
            btn_rect.right,
            btn_rect.bottom,
            code_text,
        ));

        let _ = end_y;
    }
}

/// Compute pixel heights for all messages using real GDI text measurement.
pub fn compute_message_heights(
    hdc: HDC,
    fonts: &Fonts,
    chat: &mut ChatState,
    content_width: i32,
    line_height: i32,
    scale: f32,
) {
    let bubble_padding = (MESSAGE_INNER_PADDING as f32 * scale) as i32;
    let label_height = (SPACE_4 as f32 * scale) as i32;
    let spacing = (MESSAGE_SPACING as f32 * scale) as i32;
    let thinking_header_h = (THINKING_HEADER_HEIGHT as f32 * scale) as i32;
    let max_bubble_width = (content_width as f32 * MESSAGE_MAX_WIDTH_RATIO) as i32;
    let text_width = max_bubble_width - 2 * bubble_padding;

    let mut total = 0i32;

    unsafe {
        let old_font = SelectObject(hdc, fonts.normal as HGDIOBJ);

        for msg in &mut chat.messages {
            // Main text height
            let mut total_text_height = 0i32;
            for run in &msg.runs {
                let style_font = if run.style.code {
                    fonts.code
                } else if run.style.bold {
                    fonts.bold
                } else {
                    fonts.normal
                };
                SelectObject(hdc, style_font as HGDIOBJ);
                for line_text in run.text.split('\n') {
                    if line_text.is_empty() {
                        total_text_height += line_height;
                        continue;
                    }
                    let wrapped = wrap_text(hdc, line_text, text_width, &run.style, fonts);
                    total_text_height += wrapped.len() as i32 * line_height;
                }
            }
            if total_text_height == 0 {
                total_text_height = line_height;
            }

            // Thinking section height
            let has_thinking = !msg.thinking.is_empty();
            let mut thinking_section_h = 0i32;
            if has_thinking && msg.role == Role::Agent {
                thinking_section_h += thinking_header_h;
                if !msg.thinking_collapsed {
                    let mut th = 0i32;
                    for run in &msg.thinking_runs {
                        let style_font = if run.style.code {
                            fonts.code
                        } else if run.style.bold {
                            fonts.bold
                        } else {
                            fonts.normal
                        };
                        SelectObject(hdc, style_font as HGDIOBJ);
                        for line_text in run.text.split('\n') {
                            if line_text.is_empty() {
                                th += line_height;
                                continue;
                            }
                            let wrapped = wrap_text(hdc, line_text, text_width, &run.style, fonts);
                            th += wrapped.len() as i32 * line_height;
                        }
                    }
                    if th == 0 {
                        th = line_height;
                    }
                    thinking_section_h += th + bubble_padding;
                    msg.thinking_height = th + bubble_padding;
                } else {
                    msg.thinking_height = 0;
                }
            }

            let bubble_height = if msg.role == Role::System {
                total_text_height + 2 * bubble_padding
            } else {
                total_text_height
                    + 2 * bubble_padding
                    + label_height
                    + (SPACE_1 as f32 * scale) as i32
                    + thinking_section_h
            };

            msg.height = bubble_height + spacing;
            total += msg.height;
        }

        SelectObject(hdc, old_font as HGDIOBJ);
    }

    chat.total_content_height = total;
}

/// Draw a thin scrollbar on the right edge of the chat area.
pub fn draw_message_scrollbar(
    hdc: HDC,
    ctx: &PaintContext,
    _chat_left: i32,
    chat_right: i32,
    area_bottom: i32,
    area_top: i32,
    scale: f32,
) {
    let idx = ctx.active_session.min(ctx.sessions.len().saturating_sub(1));
    let chat = &ctx.sessions[idx].chat;
    let total = chat.total_content_height;
    let viewport = area_bottom - area_top;
    if total <= viewport {
        return;
    }

    unsafe {
        let sb_w = (4.0 * scale).max(3.0) as i32;
        let sb_x = chat_right - sb_w;
        let sb_track = RECT {
            left: sb_x,
            top: area_top,
            right: chat_right,
            bottom: area_bottom,
        };
        let track_brush = CreateSolidBrush(SCROLLBAR_TRACK);
        FillRect(hdc, &sb_track, track_brush);
        DeleteObject(track_brush as HGDIOBJ);

        let max_scroll = (total - viewport).max(0);
        let thumb_ratio = viewport as f32 / total as f32;
        let thumb_h = ((viewport as f32 * thumb_ratio).max(12.0 * scale)) as i32;
        let scroll_ratio = if max_scroll > 0 {
            chat.scroll_offset as f32 / max_scroll as f32
        } else {
            0.0
        };
        let thumb_y = area_top + (scroll_ratio * (viewport - thumb_h) as f32) as i32;
        let thumb_rect = RECT {
            left: sb_x,
            top: thumb_y,
            right: chat_right,
            bottom: (thumb_y + thumb_h).min(area_bottom),
        };
        let thumb_color = if ctx.hover_scrollbar {
            SCROLLBAR_THUMB_HOVER
        } else {
            SCROLLBAR_THUMB
        };
        let thumb_brush = CreateSolidBrush(thumb_color);
        FillRect(hdc, &thumb_rect, thumb_brush);
        DeleteObject(thumb_brush as HGDIOBJ);
    }
}
