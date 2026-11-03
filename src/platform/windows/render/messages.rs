// Message rendering: bubbles, thinking sections, text layout, scrollbar.
#![allow(dead_code)]

use crate::chat::{ChatState, Message, Role};
use crate::markdown::Style;
use crate::platform::windows::ffi::*;
use crate::platform::windows::fonts::{select_font_for_style, Fonts};
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;

#[derive(Default, Clone)]
struct VisualLine {
    spans: Vec<(String, Style, i32)>,
    code_header: Option<Style>,
}

type LayoutKey = (u64, u64, u64, i32, bool);
thread_local! {
    static MESSAGE_LAYOUTS: std::cell::RefCell<std::collections::HashMap<LayoutKey, std::rc::Rc<Vec<VisualLine>>>> = std::cell::RefCell::new(std::collections::HashMap::new());
}
fn message_lines(
    hdc: HDC,
    msg: &Message,
    width: i32,
    fonts: &Fonts,
    thinking: bool,
) -> std::rc::Rc<Vec<VisualLine>> {
    let key = (msg.id, msg.revision, fonts.generation, width, thinking);
    if let Some(layout) = MESSAGE_LAYOUTS.with(|cache| cache.borrow().get(&key).cloned()) {
        return layout;
    }
    let runs = if thinking {
        &msg.thinking_runs
    } else {
        &msg.runs
    };
    let lines = std::rc::Rc::new(wrap_runs(hdc, runs, width, fonts));
    // Bound cache retention separately from saved transcript retention.
    if msg.text.len() + msg.thinking.len() < 100_000 {
        MESSAGE_LAYOUTS.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.retain(|(id, revision, ..), _| *id != msg.id || *revision == msg.revision);
            if cache.len() >= 512 {
                cache.clear();
            }
            cache.insert(key, lines.clone());
        });
    }
    lines
}

/// Inline spans share a measured visual line. Explicit breaks emit exactly one
/// line. Code retains its original source independently of its clipped display.
fn wrap_runs(
    hdc: HDC,
    runs: &[crate::markdown::StyledRun],
    text_width: i32,
    fonts: &Fonts,
) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut line = VisualLine::default();
    let mut width = 0;
    for run in runs {
        let old = select_font_for_style(hdc, &run.style, fonts);
        if run.style.is_code_block {
            if !line.spans.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            lines.push(VisualLine {
                spans: Vec::new(),
                code_header: Some(run.style.clone()),
            });
            for text in run.text.split_terminator('\n') {
                let display = text.trim_end_matches('\r').replace('\t', "    ");
                lines.push(VisualLine {
                    spans: vec![(display, run.style.clone(), 0)],
                    code_header: None,
                });
            }
            width = 0;
        } else {
            for c in run.text.chars() {
                if c == '\n' {
                    lines.push(std::mem::take(&mut line));
                    width = 0;
                    continue;
                }
                let same_style = line
                    .spans
                    .last()
                    .is_some_and(|(_, style, _)| *style == run.style);
                let mut candidate = if same_style {
                    line.spans.last().map(|s| s.0.clone()).unwrap_or_default()
                } else {
                    String::new()
                };
                candidate.push(c);
                let previous_width = if same_style {
                    line.spans.last().map_or(0, |s| s.2)
                } else {
                    0
                };
                let wide = to_wide(&candidate);
                let mut size = POINT { x: 0, y: 0 };
                // SAFETY: selected font matches drawing; UTF-16 buffer remains live.
                unsafe {
                    GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut size);
                }
                if width - previous_width + size.x > text_width.max(1) && width > 0 {
                    lines.push(std::mem::take(&mut line));
                    width = 0;
                    candidate = c.to_string();
                    let wide = to_wide(&candidate);
                    unsafe {
                        GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut size);
                    }
                } else if same_style {
                    line.spans.pop();
                    width -= previous_width;
                }
                width += size.x;
                line.spans.push((candidate, run.style.clone(), size.x));
            }
        }
        // SAFETY: restore the previous selected GDI object before returning.
        unsafe {
            SelectObject(hdc, old);
        }
    }
    if !line.spans.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

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

    let session_idx = ctx
        .app
        .active_session
        .min(ctx.app.sessions.len().saturating_sub(1));
    let scroll_offset = ctx.app.sessions[session_idx].chat.scroll_offset;
    let streaming_idx = ctx.app.sessions[session_idx].chat.streaming_message_idx;
    let messages_len = ctx.app.sessions[session_idx].chat.messages.len();
    let hover_thinking = ctx.app.hover.thinking_toggle;
    let viewport = area_bottom - area_top;

    let visible = {
        let chat = &ctx.app.sessions[session_idx].chat;
        chat.visible_messages(viewport, scroll_offset)
    };

    // Collect clickable rects in local buffers and swap them into AppState at
    // the end, so a re-entrant borrow of the shared Vecs can never panic and
    // the hit-test state is only updated after a complete paint pass.
    let mut link_rects: Vec<crate::app::ClickableRect> = Vec::new();
    let mut code_block_rects: Vec<crate::app::ClickableRect> = Vec::new();

    for (msg_idx, y_top) in visible {
        if msg_idx >= messages_len {
            break;
        }
        let msg = &ctx.app.sessions[session_idx].chat.messages[msg_idx];
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
            ctx.app.hover.link,
            ctx.app.hover.code_copy,
            &mut link_rects,
            &mut code_block_rects,
        );
    }

    // Pixels and interactive regions share the same half-open viewport clip.
    let clip = |rect: &mut crate::app::ClickableRect| {
        rect.0 = rect.0.max(chat_left);
        rect.1 = rect.1.max(area_top);
        rect.2 = rect.2.min(chat_right);
        rect.3 = rect.3.min(area_bottom);
    };
    for rect in &mut link_rects {
        clip(rect);
    }
    for rect in &mut code_block_rects {
        clip(rect);
    }
    link_rects.retain(|r| r.0 < r.2 && r.1 < r.3);
    code_block_rects.retain(|r| r.0 < r.2 && r.1 < r.3);
    ctx.app.link_rects = link_rects;
    ctx.app.code_block_rects = code_block_rects;
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
    link_rects: &mut Vec<crate::app::ClickableRect>,
    code_block_rects: &mut Vec<crate::app::ClickableRect>,
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
            let wrapped = message_lines(hdc, msg, text_width, fonts, true);
            let mut th = wrapped.len() as i32 * line_height;
            if th == 0 {
                th = line_height;
            }
            th
        } else {
            0
        };

        // Calculate main text height
        let lines = message_lines(hdc, msg, text_width, fonts, false);
        let mut total_text_height = lines.len() as i32 * line_height;
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
            let thinking_now = is_streaming && msg.text.is_empty();
            let label = if thinking_now {
                "Thinking..."
            } else {
                "Reasoning"
            };
            let header_text = format!("{arrow} {label}");
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
                for line in message_lines(hdc, msg, text_width, fonts, true).iter() {
                    let mut x = bubble_left + bubble_padding;
                    SetTextColor(hdc, THINKING_TEXT_COLOR);
                    for (text, style, width) in &line.spans {
                        let previous = select_font_for_style(hdc, style, fonts);
                        let wide = to_wide(text);
                        let mut rect = RECT {
                            left: x,
                            top: think_y,
                            right: bubble_left + bubble_width - bubble_padding,
                            bottom: think_y + line_height,
                        };
                        DrawTextW(
                            hdc,
                            wide.as_ptr(),
                            wide.len() as i32 - 1,
                            &mut rect,
                            DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOPREFIX,
                        );
                        SelectObject(hdc, previous);
                        x += width;
                    }
                    think_y += line_height;
                }
                SelectObject(hdc, fonts.italic as HGDIOBJ);
                content_y += thinking_text_height + bubble_padding;
            }
        }

        let mut text_y = content_y;
        let left = bubble_left + bubble_padding;
        let right = bubble_left + bubble_width - bubble_padding;
        for line in lines.iter() {
            if let Some(style) = &line.code_header {
                finish_code_block(
                    hdc,
                    fonts,
                    text_y,
                    left,
                    right,
                    &style.code_lang,
                    style.code_source.as_deref().unwrap_or(""),
                    hover_code_copy == Some(code_block_rects.len()),
                    code_block_rects,
                    line_height,
                    scale,
                );
            }
            let mut x = left;
            for (text, style, width) in &line.spans {
                let previous = select_font_for_style(hdc, style, fonts);
                let color = if style.is_link {
                    if hover_link == Some(link_rects.len()) {
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
                SetTextColor(hdc, color);
                let wide = to_wide(text);
                let mut rect = RECT {
                    left: x,
                    top: text_y,
                    right,
                    bottom: text_y + line_height,
                };
                DrawTextW(
                    hdc,
                    wide.as_ptr(),
                    wide.len() as i32 - 1,
                    &mut rect,
                    DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOPREFIX,
                );
                if style.is_link {
                    let end = (x + width).min(right);
                    let underline = RECT {
                        left: x,
                        top: text_y + line_height - 1,
                        right: end,
                        bottom: text_y + line_height,
                    };
                    let brush = CreateSolidBrush(color);
                    FillRect(hdc, &underline, brush);
                    DeleteObject(brush as HGDIOBJ);
                    if let Some(target) = &style.link_target {
                        link_rects.push((x, text_y, end, text_y + line_height, target.clone()));
                    }
                }
                SelectObject(hdc, previous);
                x += width;
            }
            text_y += line_height;
        }

        SelectObject(hdc, old_font as HGDIOBJ);
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_code_block(
    hdc: HDC,
    fonts: &Fonts,
    start_y: i32,
    left: i32,
    right: i32,
    lang: &Option<String>,
    code_source: &str,
    is_hover: bool,
    code_block_rects: &mut Vec<crate::app::ClickableRect>,
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
                right: right - (32.0 * scale) as i32,
                bottom: start_y + line_height,
            };
            DrawTextW(
                hdc,
                label_wide.as_ptr(),
                label_wide.len() as i32 - 1,
                &mut label_rect as LPRECT,
                DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOPREFIX,
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
        let code_text = code_source.to_string();
        code_block_rects.push((
            btn_rect.left,
            btn_rect.top,
            btn_rect.right,
            btn_rect.bottom,
            code_text,
        ));
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
            // Materialize deferred markdown parses (streaming marks runs dirty
            // per chunk; parsing here keeps the per-chunk cost at O(chunk)).
            msg.ensure_runs_parsed();

            // Main text height
            let wrapped = message_lines(hdc, msg, text_width, fonts, false);
            let mut total_text_height = wrapped.len() as i32 * line_height;
            if total_text_height == 0 {
                total_text_height = line_height;
            }

            // Thinking section height
            let has_thinking = !msg.thinking.is_empty();
            let mut thinking_section_h = 0i32;
            if has_thinking && msg.role == Role::Agent {
                thinking_section_h += thinking_header_h;
                if !msg.thinking_collapsed {
                    let wrapped = message_lines(hdc, msg, text_width, fonts, true);
                    let mut th = wrapped.len() as i32 * line_height;
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
    let idx = ctx
        .app
        .active_session
        .min(ctx.app.sessions.len().saturating_sub(1));
    let chat = &ctx.app.sessions[idx].chat;
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
        let thumb_color = if ctx.app.hover.scrollbar {
            SCROLLBAR_THUMB_HOVER
        } else {
            SCROLLBAR_THUMB
        };
        let thumb_brush = CreateSolidBrush(thumb_color);
        FillRect(hdc, &thumb_rect, thumb_brush);
        DeleteObject(thumb_brush as HGDIOBJ);
    }
}
