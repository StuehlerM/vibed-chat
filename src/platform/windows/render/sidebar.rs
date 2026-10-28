// Sidebar rendering: session list with date grouping, new chat button, settings button.
#![allow(dead_code)]

use crate::interaction::session_action_rects;
use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::platform::windows::text::truncate_to_width;
use crate::text::{last_message_preview, relative_time};
use crate::theme::*;
use crate::ui::{FocusedElement, PressedElement, RenameTarget};

pub fn draw_sidebar(hdc: HDC, ctx: &mut PaintContext, scale: f32) {
    let sidebar_w = ctx.sidebar_w();
    let height = ctx.height;

    unsafe {
        // Sidebar background
        let sidebar_rect = RECT {
            left: 0,
            top: 0,
            right: sidebar_w,
            bottom: height,
        };
        let bg_brush = CreateSolidBrush(SIDEBAR_BG);
        FillRect(hdc, &sidebar_rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        // Right border of sidebar
        let border_rect = RECT {
            left: sidebar_w - 1,
            top: 0,
            right: sidebar_w,
            bottom: height,
        };
        let border_brush = CreateSolidBrush(SIDEBAR_DIVIDER);
        FillRect(hdc, &border_rect, border_brush);
        DeleteObject(border_brush as HGDIOBJ);

        let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
        let header_h = (SIDEBAR_HEADER_HEIGHT as f32 * scale) as i32;
        let item_h = (SIDEBAR_ITEM_HEIGHT as f32 * scale) as i32;
        let item_gap = (SIDEBAR_ITEM_MARGIN as f32 * scale) as i32;
        let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;

        // ── "+ New Chat" button ──
        let btn_margin = pad;
        let btn_left = btn_margin;
        let btn_right = sidebar_w - btn_margin;
        let btn_top = pad;
        let btn_bottom = btn_top + header_h - pad * 2;
        let btn_rect = RECT {
            left: btn_left,
            top: btn_top,
            right: btn_right,
            bottom: btn_bottom,
        };

        let btn_color = if ctx.pressed_element == Some(PressedElement::NewChat) {
            NEW_CHAT_BTN_PRESSED
        } else if ctx.hover_new_chat {
            NEW_CHAT_BTN_HOVER
        } else {
            NEW_CHAT_BTN_BG
        };
        let radius = (RADIUS_MD as f32 * scale) as i32;
        let gfx = crate::platform::windows::gfx::Gfx::new(hdc);
        match gfx {
            Some(ref g) => {
                if ctx.hover_new_chat && ctx.pressed_element != Some(PressedElement::NewChat) {
                    g.button_with_ring(btn_rect, radius, btn_color, btn_color);
                } else {
                    g.fill_round_rect(btn_rect, radius, btn_color);
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

        // Focus ring for New Chat button
        if ctx.focused_element == Some(FocusedElement::NewChat) {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            let fr = (RADIUS_MD as f32 * scale) as i32;
            RoundRect(
                hdc,
                btn_rect.left - 1,
                btn_rect.top - 1,
                btn_rect.right + 1,
                btn_rect.bottom + 1,
                (fr + 1) * 2,
                (fr + 1) * 2,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        // Button text
        let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
        SetTextColor(hdc, NEW_CHAT_BTN_TEXT);
        let btn_text = to_wide("+ New Chat");
        let mut text_rect = RECT {
            left: btn_rect.left,
            top: btn_rect.top,
            right: btn_rect.right,
            bottom: btn_rect.bottom,
        };
        DrawTextW(
            hdc,
            btn_text.as_ptr(),
            btn_text.len() as i32 - 1,
            &mut text_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // ── "+ New Project" button (below the New Chat button) ──
        let new_proj_h = (28.0 * scale) as i32;
        let new_proj_top = btn_bottom + (SPACE_1 as f32 * scale) as i32;
        let new_proj_bottom = new_proj_top + new_proj_h;
        let new_proj_rect = RECT {
            left: pad,
            top: new_proj_top,
            right: sidebar_w - pad,
            bottom: new_proj_bottom,
        };
        let new_proj_color = if ctx.pressed_element == Some(PressedElement::NewProject) {
            NEW_PROJECT_BTN_PRESSED
        } else if ctx.hover_new_project {
            NEW_PROJECT_BTN_HOVER
        } else {
            NEW_PROJECT_BTN_BG
        };
        let np_radius = (RADIUS_SM as f32 * scale) as i32;
        let np_gfx = crate::platform::windows::gfx::Gfx::new(hdc);
        match np_gfx {
            Some(ref g) => {
                if ctx.hover_new_project && ctx.pressed_element != Some(PressedElement::NewProject)
                {
                    g.button_with_ring(new_proj_rect, np_radius, new_proj_color, new_proj_color);
                } else {
                    g.fill_round_rect(new_proj_rect, np_radius, new_proj_color);
                }
            }
            None => {
                let np_brush = CreateSolidBrush(new_proj_color);
                let np_pen = CreatePen(PS_SOLID, 1, new_proj_color);
                let old_pen = SelectObject(hdc, np_pen as HGDIOBJ);
                let old_brush = SelectObject(hdc, np_brush as HGDIOBJ);
                RoundRect(
                    hdc,
                    new_proj_rect.left,
                    new_proj_rect.top,
                    new_proj_rect.right,
                    new_proj_rect.bottom,
                    np_radius * 2,
                    np_radius * 2,
                );
                SelectObject(hdc, old_pen as HGDIOBJ);
                SelectObject(hdc, old_brush as HGDIOBJ);
                DeleteObject(np_pen as HGDIOBJ);
                DeleteObject(np_brush as HGDIOBJ);
            }
        }

        // Focus ring for New Project button
        if ctx.focused_element == Some(FocusedElement::NewProject) {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            RoundRect(
                hdc,
                new_proj_rect.left - 1,
                new_proj_rect.top - 1,
                new_proj_rect.right + 1,
                new_proj_rect.bottom + 1,
                (np_radius + 1) * 2,
                (np_radius + 1) * 2,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        // Button text
        let old_font = SelectObject(hdc, ctx.fonts.sidebar as HGDIOBJ);
        SetTextColor(hdc, NEW_PROJECT_BTN_TEXT);
        let np_text = to_wide("+ New Project");
        let mut np_text_rect = RECT {
            left: new_proj_rect.left,
            top: new_proj_rect.top,
            right: new_proj_rect.right,
            bottom: new_proj_rect.bottom,
        };
        DrawTextW(
            hdc,
            np_text.as_ptr(),
            np_text.len() as i32 - 1,
            &mut np_text_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // ── Project-grouped session list (with scroll) ──
        let list_top = new_proj_bottom + (SPACE_1 as f32 * scale) as i32;
        let list_bottom = height - footer_h - pad;
        let avail_height = list_bottom - list_top;

        // Project-aware layout matching interaction.rs::sidebar_layout
        let proj_h = (PROJECT_HEADER_HEIGHT as f32 * scale) as i32;
        let indent = (SESSION_INDENT as f32 * scale) as i32;
        let menu_btn_size = (PROJECT_MENU_BTN_SIZE as f32 * scale) as i32;
        let menu_btn_right = sidebar_w - pad - (SPACE_1 as f32 * scale) as i32;
        let menu_btn_left = menu_btn_right - menu_btn_size;
        let chevron_w = (PROJECT_CHEVRON_SIZE as f32 * scale) as i32;

        let other_idx = ctx.projects.len();

        let mut grouped: Vec<(usize, bool, String, Vec<usize>)> = Vec::new();
        for (pidx, project) in ctx.projects.iter().enumerate() {
            let visible: Vec<usize> = project
                .session_indices
                .iter()
                .copied()
                .filter(|&i| {
                    ctx.sessions
                        .get(i)
                        .map(|s| ctx.settings.show_archived || !s.archived)
                        .unwrap_or(false)
                })
                .collect();
            grouped.push((pidx, project.expanded, project.name.clone(), visible));
        }
        let show_archived = ctx.settings.show_archived;
        let other_sessions: Vec<usize> = ctx
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                (show_archived || !s.archived)
                    && !ctx.projects.iter().any(|p| p.path == s.working_dir)
            })
            .map(|(i, _)| i)
            .collect();
        if !other_sessions.is_empty() {
            grouped.push((other_idx, true, "Other".to_string(), other_sessions));
        }

        // Calculate total height (New Project button + headers + items)
        let mut total_list_height = new_proj_h + (SPACE_1 as f32 * scale) as i32;
        for (_, expanded, _, indices) in &grouped {
            total_list_height += proj_h;
            if *expanded {
                total_list_height += indices.len() as i32 * (item_h + item_gap);
            }
            total_list_height += (SPACE_1 as f32 * scale) as i32;
        }
        if !grouped.is_empty() {
            total_list_height -= item_gap;
        }

        let needs_scroll = total_list_height > avail_height;
        let max_sidebar_scroll = (total_list_height - avail_height).max(0);
        let sidebar_scroll = ctx.sidebar_scroll_offset.min(max_sidebar_scroll).max(0);

        // Clip session list to its area
        let list_clip = RECT {
            left: 0,
            top: list_top,
            right: sidebar_w,
            bottom: list_bottom,
        };
        let clip_rgn = CreateRectRgn(
            list_clip.left,
            list_clip.top,
            list_clip.right,
            list_clip.bottom,
        );
        SelectClipRgn(hdc, clip_rgn);

        let accent_bar_w = (SIDEBAR_ACCENT_BAR_W as f32 * scale).max(2.0) as i32;
        let group_gap = (SPACE_1 as f32 * scale) as i32;

        let mut y = list_top - sidebar_scroll;
        for (group_pos, &(pidx, expanded, ref group_name, ref indices)) in
            grouped.iter().enumerate()
        {
            // ── Project header ──
            if y + proj_h > list_top - proj_h && y < list_bottom + proj_h {
                let is_hover = ctx.hover_project == Some(pidx);
                let is_pressed = ctx.pressed_element == Some(PressedElement::ProjectHeader(pidx));
                let is_menu_hover = ctx.hover_project_menu == Some(pidx);
                let header_color = if is_pressed {
                    PROJECT_HEADER_HOVER_BG
                } else if is_hover {
                    PROJECT_HEADER_HOVER_BG
                } else {
                    PROJECT_HEADER_BG
                };
                let header_rect = RECT {
                    left: 0,
                    top: y,
                    right: sidebar_w,
                    bottom: y + proj_h,
                };
                let header_brush = CreateSolidBrush(header_color);
                FillRect(hdc, &header_rect, header_brush);
                DeleteObject(header_brush as HGDIOBJ);

                // Chevron (expand/collapse)
                let chevron_cx = pad + chevron_w / 2;
                let chevron_cy = y + proj_h / 2;
                let ch = (chevron_w as f32 * 0.6) as i32;
                let old_font = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
                SetTextColor(hdc, PROJECT_CHEVRON_COLOR);
                let chevron = to_wide(if expanded { "\u{25BE}" } else { "\u{25B8}" });
                let mut chev_rect = RECT {
                    left: chevron_cx - ch,
                    top: chevron_cy - ch,
                    right: chevron_cx + ch,
                    bottom: chevron_cy + ch,
                };
                DrawTextW(
                    hdc,
                    chevron.as_ptr(),
                    chevron.len() as i32 - 1,
                    &mut chev_rect as LPRECT,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                );
                SelectObject(hdc, old_font as HGDIOBJ);

                // Project name (+ rename field when renaming)
                let name_left = pad + chevron_w + (SPACE_1 as f32 * scale) as i32;
                let count_text = format!("{}", indices.len());
                let old_font = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
                let count_wide = to_wide(&count_text);
                let mut count_sz = POINT { x: 0, y: 0 };
                GetTextExtentPoint32W(
                    hdc,
                    count_wide.as_ptr(),
                    count_wide.len() as i32 - 1,
                    &mut count_sz,
                );
                SelectObject(hdc, old_font as HGDIOBJ);

                let name_right = if pidx == other_idx {
                    sidebar_w - pad
                } else {
                    menu_btn_left
                        - (SPACE_1 as f32 * scale) as i32
                        - count_sz.x
                        - (SPACE_2 as f32 * scale) as i32
                };

                if ctx.rename_target == Some(RenameTarget::Project(pidx)) {
                    let edit_bg = SURFACE_2;
                    let edit_rect = RECT {
                        left: name_left - (SPACE_1 as f32 * scale) as i32,
                        top: y + (SPACE_1 as f32 * scale) as i32,
                        right: (name_right + count_sz.x + (SPACE_2 as f32 * scale) as i32)
                            .min(sidebar_w - pad),
                        bottom: y + proj_h - (SPACE_1 as f32 * scale) as i32,
                    };
                    let edit_brush = CreateSolidBrush(edit_bg);
                    FillRect(hdc, &edit_rect, edit_brush);
                    DeleteObject(edit_brush as HGDIOBJ);
                    let edit_pen = CreatePen(PS_SOLID, (1.0 * scale).max(1.0) as i32, ACCENT_HOVER);
                    let old_ep = SelectObject(hdc, edit_pen as HGDIOBJ);
                    let old_eb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
                    Rectangle(
                        hdc,
                        edit_rect.left,
                        edit_rect.top,
                        edit_rect.right,
                        edit_rect.bottom,
                    );
                    SelectObject(hdc, old_ep as HGDIOBJ);
                    SelectObject(hdc, old_eb as HGDIOBJ);
                    DeleteObject(edit_pen as HGDIOBJ);

                    let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
                    SetTextColor(hdc, TEXT_PRIMARY);
                    let buf_wide = to_wide(ctx.rename_buffer);
                    let mut edit_text_rect = RECT {
                        left: edit_rect.left + (SPACE_1 as f32 * scale) as i32,
                        top: edit_rect.top,
                        right: edit_rect.right - (SPACE_1 as f32 * scale) as i32,
                        bottom: edit_rect.bottom,
                    };
                    DrawTextW(
                        hdc,
                        buf_wide.as_ptr(),
                        buf_wide.len() as i32 - 1,
                        &mut edit_text_rect as LPRECT,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                    );

                    // Draw cursor
                    if ctx.cursor_visible {
                        let cursor_byte_pos = ctx.rename_cursor.min(ctx.rename_buffer.len());
                        let before_cursor = &ctx.rename_buffer[..cursor_byte_pos];
                        let before_wide = to_wide(before_cursor);
                        let mut cursor_sz = POINT { x: 0, y: 0 };
                        GetTextExtentPoint32W(
                            hdc,
                            before_wide.as_ptr(),
                            before_wide.len() as i32 - 1,
                            &mut cursor_sz,
                        );
                        let cursor_x = edit_text_rect.left + cursor_sz.x;
                        let cursor_w = (1.0 * scale).max(1.0) as i32;
                        let cursor_rect = RECT {
                            left: cursor_x,
                            top: edit_rect.top + (SPACE_1 as f32 * scale) as i32,
                            right: cursor_x + cursor_w,
                            bottom: edit_rect.bottom - (SPACE_1 as f32 * scale) as i32,
                        };
                        let cursor_brush = CreateSolidBrush(CURSOR_COLOR);
                        FillRect(hdc, &cursor_rect, cursor_brush);
                        DeleteObject(cursor_brush as HGDIOBJ);
                    }
                    SelectObject(hdc, old_font as HGDIOBJ);
                } else {
                    let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
                    SetTextColor(hdc, PROJECT_HEADER_TEXT);
                    let display_name = truncate_to_width(
                        hdc,
                        group_name,
                        name_right - name_left,
                        ctx.fonts.sidebar_medium,
                    );
                    let name_wide = to_wide(&display_name);
                    let mut name_rect = RECT {
                        left: name_left,
                        top: y,
                        right: name_right,
                        bottom: y + proj_h,
                    };
                    DrawTextW(
                        hdc,
                        name_wide.as_ptr(),
                        name_wide.len() as i32 - 1,
                        &mut name_rect as LPRECT,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP | DT_END_ELLIPSIS,
                    );
                    SelectObject(hdc, old_font as HGDIOBJ);

                    // Session count
                    let old_font = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
                    SetTextColor(hdc, PROJECT_COUNT_TEXT);
                    let mut count_rect = RECT {
                        left: name_right + (SPACE_2 as f32 * scale) as i32,
                        top: y,
                        right: name_right + (SPACE_2 as f32 * scale) as i32 + count_sz.x,
                        bottom: y + proj_h,
                    };
                    DrawTextW(
                        hdc,
                        count_wide.as_ptr(),
                        count_wide.len() as i32 - 1,
                        &mut count_rect as LPRECT,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                    );
                    SelectObject(hdc, old_font as HGDIOBJ);
                }

                // Menu button (⋯) - only for real projects
                if pidx != other_idx {
                    let menu_color = if is_menu_hover || is_pressed {
                        PROJECT_MENU_BTN_HOVER
                    } else {
                        PROJECT_MENU_BTN_COLOR
                    };
                    let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
                    SetTextColor(hdc, menu_color);
                    let menu_icon = to_wide("\u{22EF}");
                    let mut menu_rect = RECT {
                        left: menu_btn_left,
                        top: y + (proj_h - menu_btn_size) / 2,
                        right: menu_btn_right,
                        bottom: y + (proj_h + menu_btn_size) / 2,
                    };
                    DrawTextW(
                        hdc,
                        menu_icon.as_ptr(),
                        menu_icon.len() as i32 - 1,
                        &mut menu_rect as LPRECT,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                    );
                    SelectObject(hdc, old_font as HGDIOBJ);
                }

                // Focus ring for project header
                if ctx.focused_element == Some(FocusedElement::ProjectHeader(pidx)) {
                    let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
                    let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
                    let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
                    Rectangle(hdc, pad - 1, y - 1, sidebar_w - pad + 1, y + proj_h + 1);
                    SelectObject(hdc, old_fp as HGDIOBJ);
                    SelectObject(hdc, old_fb as HGDIOBJ);
                    DeleteObject(focus_pen as HGDIOBJ);
                }
            }
            y += proj_h;

            // ── Session items in this project ──
            if expanded {
                for (item_idx_in_group, &i) in indices.iter().enumerate() {
                    if y + item_h > list_bottom + item_h {
                        break;
                    }
                    if y + item_h < list_top - item_h {
                        y += item_h + item_gap;
                        continue;
                    }

                    let session = &ctx.sessions[i];
                    let is_active = i == ctx.active_session;
                    let is_hover = ctx.hover_sidebar_item == Some(i) && !is_active;
                    let is_pressed = ctx.pressed_element == Some(PressedElement::SidebarItem(i));
                    let is_archived = session.archived;
                    let is_last_in_group = item_idx_in_group == indices.len() - 1;

                    let item_rect = RECT {
                        left: pad + indent,
                        top: y,
                        right: sidebar_w - pad,
                        bottom: y + item_h,
                    };

                    // Item background - archived sessions get a dimmer background
                    let item_color = if is_active {
                        SIDEBAR_ITEM_ACTIVE_BG
                    } else if is_pressed {
                        PRESSED_SURFACE
                    } else if is_archived {
                        SESSION_ARCHIVED_BG
                    } else if is_hover {
                        SIDEBAR_ITEM_HOVER_BG
                    } else {
                        SIDEBAR_BG
                    };
                    let item_brush = CreateSolidBrush(item_color);
                    let item_radius = (RADIUS_SM as f32 * scale) as i32;
                    let item_pen = CreatePen(PS_SOLID, 1, item_color);
                    let old_pen = SelectObject(hdc, item_pen as HGDIOBJ);
                    let old_brush = SelectObject(hdc, item_brush as HGDIOBJ);
                    RoundRect(
                        hdc,
                        item_rect.left,
                        item_rect.top,
                        item_rect.right,
                        item_rect.bottom,
                        item_radius * 2,
                        item_radius * 2,
                    );
                    SelectObject(hdc, old_pen as HGDIOBJ);
                    SelectObject(hdc, old_brush as HGDIOBJ);
                    DeleteObject(item_pen as HGDIOBJ);
                    DeleteObject(item_brush as HGDIOBJ);

                    // Active accent bar (3px left)
                    if is_active {
                        let bar_rect = RECT {
                            left: item_rect.left,
                            top: item_rect.top + (SPACE_1 as f32 * scale) as i32,
                            right: item_rect.left + accent_bar_w,
                            bottom: item_rect.bottom - (SPACE_1 as f32 * scale) as i32,
                        };
                        let bar_brush = CreateSolidBrush(SIDEBAR_ITEM_ACTIVE_BAR);
                        FillRect(hdc, &bar_rect, bar_brush);
                        DeleteObject(bar_brush as HGDIOBJ);
                    }

                    // Focus ring for sidebar item
                    if ctx.focused_element == Some(FocusedElement::SidebarItem(i)) {
                        let focus_pen =
                            CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
                        let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
                        let old_fb =
                            SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
                        RoundRect(
                            hdc,
                            item_rect.left - 1,
                            item_rect.top - 1,
                            item_rect.right + 1,
                            item_rect.bottom + 1,
                            (item_radius + 1) * 2,
                            (item_radius + 1) * 2,
                        );
                        SelectObject(hdc, old_fp as HGDIOBJ);
                        SelectObject(hdc, old_fb as HGDIOBJ);
                        DeleteObject(focus_pen as HGDIOBJ);
                    }

                    let text_pad = pad + accent_bar_w + (SPACE_1 as f32 * scale) as i32;
                    let name_left = item_rect.left + text_pad;
                    let name_right = item_rect.right - pad;

                    // Line 1: Session name + status dot + timestamp or action buttons
                    let name = if session.name.is_empty() {
                        format!("Chat {}", i + 1)
                    } else {
                        session.name.clone()
                    };

                    let is_streaming = session.chat.is_waiting_for_agent
                        || session.chat.streaming_message_idx.is_some();

                    // Compute action button rects (used when hovering)
                    let (archive_rect, delete_rect) = session_action_rects(
                        item_rect.left,
                        item_rect.right,
                        item_rect.top,
                        item_h,
                        scale,
                    );
                    let show_actions = is_hover && !is_active;

                    let ts_text = relative_time(&session.created_at);
                    let ts_font = ctx.fonts.timestamp;
                    let old_ts_font = SelectObject(hdc, ts_font as HGDIOBJ);
                    let ts_wide = to_wide(&ts_text);
                    let mut ts_sz = POINT { x: 0, y: 0 };
                    GetTextExtentPoint32W(
                        hdc,
                        ts_wide.as_ptr(),
                        ts_wide.len() as i32 - 1,
                        &mut ts_sz,
                    );
                    SelectObject(hdc, old_ts_font as HGDIOBJ);

                    // When hovering, reserve space for action buttons instead of timestamp
                    let right_reserve = if show_actions {
                        let btn_size = (SESSION_ACTION_BTN_SIZE as f32 * scale) as i32;
                        let btn_gap = (SESSION_ACTION_BTN_GAP as f32 * scale) as i32;
                        2 * btn_size + btn_gap + (SPACE_1 as f32 * scale) as i32
                    } else {
                        ts_sz.x + (SPACE_2 as f32 * scale) as i32
                    };

                    let name_font = if is_active {
                        ctx.fonts.sidebar_medium
                    } else {
                        ctx.fonts.sidebar
                    };
                    let max_name_w = name_right - name_left - right_reserve;
                    let display_name = truncate_to_width(hdc, &name, max_name_w, name_font);

                    let name_color = if is_archived {
                        SESSION_ARCHIVED_TEXT
                    } else if is_active {
                        SIDEBAR_ITEM_ACTIVE_TEXT
                    } else {
                        SIDEBAR_TEXT
                    };

                    let name_wide = to_wide(&display_name);
                    let name_line_h = (item_h / 2) - (SPACE_1 as f32 * scale) as i32;
                    let mut name_rect = RECT {
                        left: name_left,
                        top: item_rect.top + (SPACE_1 as f32 * scale) as i32,
                        right: name_left + max_name_w,
                        bottom: item_rect.top + name_line_h,
                    };

                    // Inline rename: draw editable text field instead of static name
                    if ctx.rename_target == Some(RenameTarget::Session(i)) {
                        let edit_bg = SURFACE_2;
                        let edit_rect = RECT {
                            left: name_left - (SPACE_1 as f32 * scale) as i32,
                            top: item_rect.top + (SPACE_1 as f32 * scale) as i32,
                            right: item_rect.right - pad,
                            bottom: item_rect.top + (item_h / 2) - (SPACE_1 as f32 * scale) as i32,
                        };
                        let edit_brush = CreateSolidBrush(edit_bg);
                        FillRect(hdc, &edit_rect, edit_brush);
                        DeleteObject(edit_brush as HGDIOBJ);
                        let edit_pen =
                            CreatePen(PS_SOLID, (1.0 * scale).max(1.0) as i32, ACCENT_HOVER);
                        let old_ep = SelectObject(hdc, edit_pen as HGDIOBJ);
                        let old_eb =
                            SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
                        Rectangle(
                            hdc,
                            edit_rect.left,
                            edit_rect.top,
                            edit_rect.right,
                            edit_rect.bottom,
                        );
                        SelectObject(hdc, old_ep as HGDIOBJ);
                        SelectObject(hdc, old_eb as HGDIOBJ);
                        DeleteObject(edit_pen as HGDIOBJ);

                        let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
                        SetTextColor(hdc, TEXT_PRIMARY);
                        let buf_wide = to_wide(ctx.rename_buffer);
                        let mut edit_text_rect = RECT {
                            left: edit_rect.left + (SPACE_1 as f32 * scale) as i32,
                            top: edit_rect.top,
                            right: edit_rect.right - (SPACE_1 as f32 * scale) as i32,
                            bottom: edit_rect.bottom,
                        };
                        DrawTextW(
                            hdc,
                            buf_wide.as_ptr(),
                            buf_wide.len() as i32 - 1,
                            &mut edit_text_rect as LPRECT,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                        );

                        // Draw cursor
                        if ctx.cursor_visible {
                            let cursor_byte_pos = ctx.rename_cursor.min(ctx.rename_buffer.len());
                            let before_cursor = &ctx.rename_buffer[..cursor_byte_pos];
                            let before_wide = to_wide(before_cursor);
                            let mut cursor_sz = POINT { x: 0, y: 0 };
                            GetTextExtentPoint32W(
                                hdc,
                                before_wide.as_ptr(),
                                before_wide.len() as i32 - 1,
                                &mut cursor_sz,
                            );
                            let cursor_x = edit_text_rect.left + cursor_sz.x;
                            let cursor_w = (1.0 * scale).max(1.0) as i32;
                            let cursor_rect = RECT {
                                left: cursor_x,
                                top: edit_rect.top + (SPACE_1 as f32 * scale) as i32,
                                right: cursor_x + cursor_w,
                                bottom: edit_rect.bottom - (SPACE_1 as f32 * scale) as i32,
                            };
                            let cursor_brush = CreateSolidBrush(CURSOR_COLOR);
                            FillRect(hdc, &cursor_rect, cursor_brush);
                            DeleteObject(cursor_brush as HGDIOBJ);
                        }
                        SelectObject(hdc, old_font as HGDIOBJ);
                    } else {
                        let old_font = SelectObject(hdc, name_font as HGDIOBJ);
                        SetTextColor(hdc, name_color);
                        DrawTextW(
                            hdc,
                            name_wide.as_ptr(),
                            name_wide.len() as i32 - 1,
                            &mut name_rect as LPRECT,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP | DT_END_ELLIPSIS,
                        );
                        SelectObject(hdc, old_font as HGDIOBJ);
                    }

                    // Status dot (streaming) - next to name
                    if is_streaming && !show_actions {
                        let dot_r = (3.0 * scale).max(2.0) as i32;
                        let pulse = (ctx.streaming_dots % 30) < 15;
                        let dot_color = if pulse { ACCENT } else { ACCENT_DIM };
                        let mut name_w = POINT { x: 0, y: 0 };
                        let old_f = SelectObject(hdc, name_font as HGDIOBJ);
                        GetTextExtentPoint32W(
                            hdc,
                            name_wide.as_ptr(),
                            name_wide.len() as i32 - 1,
                            &mut name_w,
                        );
                        SelectObject(hdc, old_f as HGDIOBJ);
                        let dot_cx = name_left + name_w.x + dot_r + (3.0 * scale) as i32;
                        let dot_cy = name_rect.top + name_rect.height() / 2;
                        if dot_cx + dot_r < name_right - ts_sz.x {
                            let dot_brush = CreateSolidBrush(dot_color);
                            let dot_pen = CreatePen(PS_SOLID, 1, dot_color);
                            let old_dp = SelectObject(hdc, dot_pen as HGDIOBJ);
                            let old_db = SelectObject(hdc, dot_brush as HGDIOBJ);
                            Ellipse(
                                hdc,
                                dot_cx - dot_r,
                                dot_cy - dot_r,
                                dot_cx + dot_r,
                                dot_cy + dot_r,
                            );
                            SelectObject(hdc, old_dp as HGDIOBJ);
                            SelectObject(hdc, old_db as HGDIOBJ);
                            DeleteObject(dot_pen as HGDIOBJ);
                            DeleteObject(dot_brush as HGDIOBJ);
                        }
                    }

                    // Right side of line 1: action buttons on hover, timestamp otherwise
                    if show_actions {
                        // Archive/unarchive button
                        let is_archive_hover = ctx.hover_session_archive == Some(i);
                        let is_archive_pressed =
                            ctx.pressed_element == Some(PressedElement::SessionArchive(i));
                        let archive_color = if is_archive_pressed {
                            ACCENT_PRESSED
                        } else if is_archive_hover {
                            SESSION_ACTION_ARCHIVE_HOVER
                        } else {
                            SESSION_ACTION_ARCHIVE
                        };
                        let archive_icon = if is_archived { "↩" } else { "📦" };
                        let old_font = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
                        SetTextColor(hdc, archive_color);
                        let icon_wide = to_wide(archive_icon);
                        let mut icon_rect = RECT {
                            left: archive_rect.0,
                            top: archive_rect.1,
                            right: archive_rect.2,
                            bottom: archive_rect.3,
                        };
                        DrawTextW(
                            hdc,
                            icon_wide.as_ptr(),
                            icon_wide.len() as i32 - 1,
                            &mut icon_rect as LPRECT,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                        );
                        SelectObject(hdc, old_font as HGDIOBJ);

                        // Delete button
                        let is_delete_hover = ctx.hover_session_delete == Some(i);
                        let is_delete_pressed =
                            ctx.pressed_element == Some(PressedElement::SessionDelete(i));
                        let delete_color = if is_delete_pressed {
                            ERROR_COLOR
                        } else if is_delete_hover {
                            SESSION_ACTION_DELETE_HOVER
                        } else {
                            SESSION_ACTION_DELETE
                        };
                        let old_font = SelectObject(hdc, ctx.fonts.caption as HGDIOBJ);
                        SetTextColor(hdc, delete_color);
                        let delete_icon = to_wide("×");
                        let mut del_rect = RECT {
                            left: delete_rect.0,
                            top: delete_rect.1,
                            right: delete_rect.2,
                            bottom: delete_rect.3,
                        };
                        DrawTextW(
                            hdc,
                            delete_icon.as_ptr(),
                            delete_icon.len() as i32 - 1,
                            &mut del_rect as LPRECT,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                        );
                        SelectObject(hdc, old_font as HGDIOBJ);
                    } else {
                        // Archived label or timestamp (right-aligned on line 1)
                        let old_font = SelectObject(hdc, ctx.fonts.timestamp as HGDIOBJ);
                        if is_archived {
                            SetTextColor(hdc, SESSION_ARCHIVED_LABEL);
                            let arch_wide = to_wide("Archived");
                            let mut arch_rect = RECT {
                                left: name_right - ts_sz.x,
                                top: item_rect.top + (SPACE_1 as f32 * scale) as i32,
                                right: name_right,
                                bottom: item_rect.top + name_line_h,
                            };
                            DrawTextW(
                                hdc,
                                arch_wide.as_ptr(),
                                arch_wide.len() as i32 - 1,
                                &mut arch_rect as LPRECT,
                                DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                            );
                        } else {
                            SetTextColor(hdc, SIDEBAR_TIMESTAMP);
                            let mut ts_rect = RECT {
                                left: name_right - ts_sz.x,
                                top: item_rect.top + (SPACE_1 as f32 * scale) as i32,
                                right: name_right,
                                bottom: item_rect.top + name_line_h,
                            };
                            DrawTextW(
                                hdc,
                                ts_wide.as_ptr(),
                                ts_wide.len() as i32 - 1,
                                &mut ts_rect as LPRECT,
                                DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                            );
                        }
                        SelectObject(hdc, old_font as HGDIOBJ);
                    }

                    // Line 2: Last message preview
                    let preview = last_message_preview(&session.chat);
                    let preview_color = if is_archived {
                        SESSION_ARCHIVED_TEXT
                    } else {
                        SIDEBAR_PREVIEW_TEXT
                    };
                    if !preview.is_empty() {
                        let old_font = SelectObject(hdc, ctx.fonts.session_preview as HGDIOBJ);
                        SetTextColor(hdc, preview_color);
                        let preview_display = truncate_to_width(
                            hdc,
                            &preview,
                            name_right - name_left,
                            ctx.fonts.session_preview,
                        );
                        let preview_wide = to_wide(&preview_display);
                        let preview_line_top =
                            item_rect.top + name_line_h + (SPACE_1 as f32 * scale) as i32;
                        let mut preview_rect = RECT {
                            left: name_left,
                            top: preview_line_top,
                            right: name_right,
                            bottom: item_rect.bottom - (SPACE_1 as f32 * scale) as i32,
                        };
                        DrawTextW(
                            hdc,
                            preview_wide.as_ptr(),
                            preview_wide.len() as i32 - 1,
                            &mut preview_rect as LPRECT,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP | DT_END_ELLIPSIS,
                        );
                        SelectObject(hdc, old_font as HGDIOBJ);
                    } else {
                        let msg_count = session.chat.messages.len();
                        let subtitle = if msg_count == 0 {
                            "No messages yet".to_string()
                        } else {
                            format!("{} messages", msg_count)
                        };
                        let old_font = SelectObject(hdc, ctx.fonts.session_preview as HGDIOBJ);
                        SetTextColor(hdc, preview_color);
                        let sub_wide = to_wide(&subtitle);
                        let preview_line_top =
                            item_rect.top + name_line_h + (SPACE_1 as f32 * scale) as i32;
                        let mut sub_rect = RECT {
                            left: name_left,
                            top: preview_line_top,
                            right: name_right,
                            bottom: item_rect.bottom - (SPACE_1 as f32 * scale) as i32,
                        };
                        DrawTextW(
                            hdc,
                            sub_wide.as_ptr(),
                            sub_wide.len() as i32 - 1,
                            &mut sub_rect as LPRECT,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
                        );
                        SelectObject(hdc, old_font as HGDIOBJ);
                    }

                    y += item_h + item_gap;

                    if is_last_in_group && group_pos + 1 < grouped.len() {
                        y += group_gap;
                    }
                }
            }
        }

        // Remove clip
        SelectClipRgn(hdc, std::ptr::null_mut());
        DeleteObject(clip_rgn as HGDIOBJ);

        // ── Sidebar scrollbar ──
        if needs_scroll {
            let sb_w = (4.0 * scale).max(3.0) as i32;
            let sb_x = sidebar_w - sb_w - 1;
            let sb_track = RECT {
                left: sb_x,
                top: list_top,
                right: sb_x + sb_w,
                bottom: list_bottom,
            };
            let track_brush = CreateSolidBrush(SCROLLBAR_TRACK);
            FillRect(hdc, &sb_track, track_brush);
            DeleteObject(track_brush as HGDIOBJ);

            let thumb_ratio = avail_height as f32 / total_list_height as f32;
            let thumb_h = ((avail_height as f32 * thumb_ratio).max(8.0 * scale)) as i32;
            let thumb_y = list_top + (sidebar_scroll as f32 * thumb_ratio) as i32;
            let thumb_rect = RECT {
                left: sb_x,
                top: thumb_y,
                right: sb_x + sb_w,
                bottom: (thumb_y + thumb_h).min(list_bottom),
            };
            let thumb_color = if ctx.hover_sidebar_scrollbar {
                SCROLLBAR_THUMB_HOVER
            } else {
                SCROLLBAR_THUMB
            };
            let thumb_brush = CreateSolidBrush(thumb_color);
            FillRect(hdc, &thumb_rect, thumb_brush);
            DeleteObject(thumb_brush as HGDIOBJ);
        }

        // ── Settings button at bottom ──
        let settings_btn_rect = RECT {
            left: pad,
            top: height - footer_h + pad / 2,
            right: sidebar_w - pad,
            bottom: height - pad / 2,
        };
        let settings_color = if ctx.pressed_element == Some(PressedElement::SettingsBtn) {
            SETTINGS_BTN_PRESSED
        } else if ctx.hover_settings_btn {
            SETTINGS_BTN_HOVER
        } else {
            SETTINGS_BTN_BG
        };
        let settings_radius = (RADIUS_SM as f32 * scale) as i32;
        let gfx = crate::platform::windows::gfx::Gfx::new(hdc);
        match gfx {
            Some(ref g) => {
                if ctx.hover_settings_btn
                    && ctx.pressed_element != Some(PressedElement::SettingsBtn)
                {
                    g.button_with_ring(settings_btn_rect, settings_radius, settings_color, ACCENT);
                } else {
                    g.fill_round_rect(settings_btn_rect, settings_radius, settings_color);
                }
            }
            None => {
                let settings_brush = CreateSolidBrush(settings_color);
                let settings_pen = CreatePen(PS_SOLID, 1, settings_color);
                let old_pen = SelectObject(hdc, settings_pen as HGDIOBJ);
                let old_brush = SelectObject(hdc, settings_brush as HGDIOBJ);
                RoundRect(
                    hdc,
                    settings_btn_rect.left,
                    settings_btn_rect.top,
                    settings_btn_rect.right,
                    settings_btn_rect.bottom,
                    settings_radius * 2,
                    settings_radius * 2,
                );
                SelectObject(hdc, old_pen as HGDIOBJ);
                SelectObject(hdc, old_brush as HGDIOBJ);
                DeleteObject(settings_pen as HGDIOBJ);
                DeleteObject(settings_brush as HGDIOBJ);
            }
        }

        // Focus ring for settings button
        if ctx.focused_element == Some(FocusedElement::SettingsBtn) {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            RoundRect(
                hdc,
                settings_btn_rect.left - 1,
                settings_btn_rect.top - 1,
                settings_btn_rect.right + 1,
                settings_btn_rect.bottom + 1,
                (settings_radius + 1) * 2,
                (settings_radius + 1) * 2,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        // Settings button text
        let old_font = SelectObject(hdc, ctx.fonts.sidebar as HGDIOBJ);
        SetTextColor(hdc, SETTINGS_BTN_TEXT);
        let settings_text = to_wide("Settings");
        let mut settings_text_rect = RECT {
            left: settings_btn_rect.left + pad,
            top: settings_btn_rect.top,
            right: settings_btn_rect.right,
            bottom: settings_btn_rect.bottom,
        };
        DrawTextW(
            hdc,
            settings_text.as_ptr(),
            settings_text.len() as i32 - 1,
            &mut settings_text_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // Connection status dot
        let dot_color = if ctx.acp_connected {
            SUCCESS
        } else {
            ERROR_COLOR
        };
        let dot_x = settings_btn_rect.right - pad - (6.0 * scale) as i32;
        let dot_y = settings_btn_rect.top + settings_btn_rect.height() / 2;
        let dot_r = (4.0 * scale) as i32;
        let dot_brush = CreateSolidBrush(dot_color);
        let dot_pen = CreatePen(PS_SOLID, 1, dot_color);
        let old_pen = SelectObject(hdc, dot_pen as HGDIOBJ);
        let old_brush = SelectObject(hdc, dot_brush as HGDIOBJ);
        Ellipse(
            hdc,
            dot_x - dot_r,
            dot_y - dot_r,
            dot_x + dot_r,
            dot_y + dot_r,
        );
        SelectObject(hdc, old_pen as HGDIOBJ);
        SelectObject(hdc, old_brush as HGDIOBJ);
        DeleteObject(dot_pen as HGDIOBJ);
        DeleteObject(dot_brush as HGDIOBJ);
    }
}
