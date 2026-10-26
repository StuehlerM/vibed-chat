// Settings panel rendering: toggle switches, info rows, reconnect button.
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;
use crate::ui::{FocusedElement, PressedElement};

pub fn draw_settings_panel(hdc: HDC, ctx: &PaintContext, scale: f32) {
    let panel_w = ctx.settings_w();
    let width = ctx.width;
    let height = ctx.height;
    let panel_left = width - panel_w;

    unsafe {
        // Panel background
        let panel_rect = RECT {
            left: panel_left,
            top: 0,
            right: width,
            bottom: height,
        };
        let bg_brush = CreateSolidBrush(SETTINGS_PANEL_BG);
        FillRect(hdc, &panel_rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        // Left border
        let border_rect = RECT {
            left: panel_left,
            top: 0,
            right: panel_left + 1,
            bottom: height,
        };
        let border_brush = CreateSolidBrush(SETTINGS_BORDER);
        FillRect(hdc, &border_rect, border_brush);
        DeleteObject(border_brush as HGDIOBJ);

        let pad = (SPACE_5 as f32 * scale) as i32;
        let mut y = pad;

        // Title
        let old_font = SelectObject(hdc, ctx.fonts.title as HGDIOBJ);
        SetTextColor(hdc, SETTINGS_TEXT);
        let title_wide = to_wide("Settings");
        let mut title_rect = RECT {
            left: panel_left + pad,
            top: y,
            right: width - pad,
            bottom: y + (SPACE_5 as f32 * scale) as i32,
        };
        DrawTextW(
            hdc,
            title_wide.as_ptr(),
            title_wide.len() as i32 - 1,
            &mut title_rect as LPRECT,
            DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP,
        );
        y += (SPACE_6 as f32 * scale) as i32;
        SelectObject(hdc, old_font as HGDIOBJ);

        // Close button (X)
        let close_size = (SPACE_5 as f32 * scale) as i32;
        let close_rect = RECT {
            left: width - pad - close_size,
            top: pad,
            right: width - pad,
            bottom: pad + close_size,
        };
        let close_color = if ctx.pressed_element == Some(PressedElement::SettingsClose) {
            SETTINGS_CLOSE_PRESSED
        } else if ctx.hover_settings_close {
            SETTINGS_CLOSE_HOVER
        } else {
            SETTINGS_CLOSE_BG
        };
        let close_brush = CreateSolidBrush(close_color);
        let close_radius = (RADIUS_SM as f32 * scale) as i32;
        let close_pen = CreatePen(PS_SOLID, 1, close_color);
        let old_pen = SelectObject(hdc, close_pen as HGDIOBJ);
        let old_brush = SelectObject(hdc, close_brush as HGDIOBJ);
        RoundRect(
            hdc,
            close_rect.left,
            close_rect.top,
            close_rect.right,
            close_rect.bottom,
            close_radius * 2,
            close_radius * 2,
        );

        // Focus ring for close button
        if ctx.focused_element == Some(FocusedElement::SettingsClose) {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            RoundRect(
                hdc,
                close_rect.left - 1,
                close_rect.top - 1,
                close_rect.right + 1,
                close_rect.bottom + 1,
                (close_radius + 1) * 2,
                (close_radius + 1) * 2,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        SelectObject(hdc, old_pen as HGDIOBJ);
        SelectObject(hdc, old_brush as HGDIOBJ);
        DeleteObject(close_pen as HGDIOBJ);
        DeleteObject(close_brush as HGDIOBJ);

        let old_font = SelectObject(hdc, ctx.fonts.normal as HGDIOBJ);
        SetTextColor(hdc, SETTINGS_TEXT);
        let x_wide = to_wide("X");
        let mut x_rect = close_rect;
        DrawTextW(
            hdc,
            x_wide.as_ptr(),
            x_wide.len() as i32 - 1,
            &mut x_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // Divider
        y += (SPACE_1 as f32 * scale) as i32;
        let div_rect = RECT {
            left: panel_left + pad,
            top: y,
            right: width - pad,
            bottom: y + 1,
        };
        let div_brush = CreateSolidBrush(SETTINGS_BORDER);
        FillRect(hdc, &div_rect, div_brush);
        DeleteObject(div_brush as HGDIOBJ);
        y += (SPACE_4 as f32 * scale) as i32;

        // Appearance section
        draw_settings_section_header(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Appearance",
        );
        draw_settings_switch(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Dark theme",
            ctx.settings.dark_theme,
            0,
        );
        draw_settings_switch(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Auto-scroll",
            ctx.settings.auto_scroll,
            1,
        );
        y += (SPACE_3 as f32 * scale) as i32;

        // Agent section
        draw_settings_section_header(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Agent",
        );
        draw_settings_switch(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Auto-approve",
            ctx.settings.auto_approve,
            2,
        );
        y += (SPACE_3 as f32 * scale) as i32;

        // Sessions section
        draw_settings_section_header(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Sessions",
        );
        draw_settings_switch(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Show archived",
            ctx.settings.show_archived,
            3,
        );
        y += (SPACE_3 as f32 * scale) as i32;

        // Connection section
        draw_settings_section_header(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Connection",
        );
        let conn_status = if ctx.acp_connected {
            "Connected"
        } else {
            "Disconnected"
        };
        let conn_color = if ctx.acp_connected {
            SUCCESS
        } else {
            ERROR_COLOR
        };
        draw_settings_info(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Status",
            conn_status,
            conn_color,
        );
        let archived = ctx.sessions.iter().filter(|s| s.archived).count();
        let sessions_label = if archived > 0 {
            format!("{} ({} archived)", ctx.sessions.len(), archived)
        } else {
            format!("{}", ctx.sessions.len())
        };
        draw_settings_info(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Sessions",
            &sessions_label,
            SETTINGS_TEXT,
        );
        y += (SPACE_3 as f32 * scale) as i32;

        // About section
        draw_settings_section_header(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "About",
        );
        draw_settings_info(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "App",
            "ACP Chat v0.1.0",
            SETTINGS_TEXT,
        );
        draw_settings_info(
            hdc,
            ctx,
            panel_left + pad,
            width - pad,
            &mut y,
            scale,
            "Protocol",
            "ACP (JSON-RPC 2.0)",
            SETTINGS_TEXT,
        );

        // Reconnect button
        y += (SPACE_4 as f32 * scale) as i32;
        let btn_h = (36.0 * scale) as i32;
        let btn_rect = RECT {
            left: panel_left + pad,
            top: y,
            right: width - pad,
            bottom: y + btn_h,
        };
        let btn_color = if ctx.pressed_element == Some(PressedElement::Reconnect) {
            NEW_CHAT_BTN_PRESSED
        } else if ctx.hover_reconnect {
            NEW_CHAT_BTN_HOVER
        } else {
            NEW_CHAT_BTN_BG
        };
        let btn_radius = (RADIUS_MD as f32 * scale) as i32;
        let gfx = crate::platform::windows::gfx::Gfx::new(hdc);
        match gfx {
            Some(ref g) => {
                if ctx.hover_reconnect && ctx.pressed_element != Some(PressedElement::Reconnect) {
                    g.button_with_ring(btn_rect, btn_radius, btn_color, btn_color);
                } else {
                    g.fill_round_rect(btn_rect, btn_radius, btn_color);
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
                    btn_radius * 2,
                    btn_radius * 2,
                );
                SelectObject(hdc, old_pen as HGDIOBJ);
                SelectObject(hdc, old_brush as HGDIOBJ);
                DeleteObject(btn_pen as HGDIOBJ);
                DeleteObject(btn_brush as HGDIOBJ);
            }
        }

        // Focus ring for reconnect button
        if ctx.focused_element == Some(FocusedElement::Reconnect) {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            RoundRect(
                hdc,
                btn_rect.left - 1,
                btn_rect.top - 1,
                btn_rect.right + 1,
                btn_rect.bottom + 1,
                (btn_radius + 1) * 2,
                (btn_radius + 1) * 2,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
        SetTextColor(hdc, NEW_CHAT_BTN_TEXT);
        let reconnect_wide = to_wide("Reconnect (F5)");
        let mut btn_text_rect = btn_rect;
        DrawTextW(
            hdc,
            reconnect_wide.as_ptr(),
            reconnect_wide.len() as i32 - 1,
            &mut btn_text_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);
    }
}

fn draw_settings_section_header(
    hdc: HDC,
    ctx: &PaintContext,
    left: i32,
    right: i32,
    y: &mut i32,
    scale: f32,
    title: &str,
) {
    unsafe {
        let old_font = SelectObject(hdc, ctx.fonts.label as HGDIOBJ);
        SetTextColor(hdc, SETTINGS_ACCENT);
        let title_wide = to_wide(title);
        let mut rect = RECT {
            left,
            top: *y,
            right,
            bottom: *y + (18.0 * scale) as i32,
        };
        DrawTextW(
            hdc,
            title_wide.as_ptr(),
            title_wide.len() as i32 - 1,
            &mut rect as LPRECT,
            DT_LEFT | DT_TOP | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);
        *y += (22.0 * scale) as i32;
    }
}

/// Draw a toggle switch component (pill with sliding knob).
#[allow(clippy::too_many_arguments)]
fn draw_settings_switch(
    hdc: HDC,
    ctx: &PaintContext,
    left: i32,
    right: i32,
    y: &mut i32,
    scale: f32,
    label: &str,
    value: bool,
    toggle_idx: usize,
) {
    unsafe {
        let row_h = (SPACE_5 as f32 * scale) as i32;
        let is_pressed = ctx.pressed_element == Some(PressedElement::SettingsToggle(toggle_idx));
        let is_focused = ctx.focused_element == Some(FocusedElement::SettingsToggle(toggle_idx));

        // Row background on hover/press
        if ctx.hover_settings_toggle == Some(toggle_idx) || is_pressed {
            let hover_rect = RECT {
                left: left - (SPACE_1 as f32 * scale) as i32,
                top: *y,
                right: right + (SPACE_1 as f32 * scale) as i32,
                bottom: *y + row_h,
            };
            let hover_color = if is_pressed {
                PRESSED_SURFACE
            } else {
                SURFACE_3
            };
            let hover_brush = CreateSolidBrush(hover_color);
            let hr = (RADIUS_SM as f32 * scale) as i32;
            let hover_pen = CreatePen(PS_SOLID, 1, hover_color);
            let old_hp = SelectObject(hdc, hover_pen as HGDIOBJ);
            let old_hb = SelectObject(hdc, hover_brush as HGDIOBJ);
            RoundRect(
                hdc,
                hover_rect.left,
                hover_rect.top,
                hover_rect.right,
                hover_rect.bottom,
                hr * 2,
                hr * 2,
            );
            SelectObject(hdc, old_hp as HGDIOBJ);
            SelectObject(hdc, old_hb as HGDIOBJ);
            DeleteObject(hover_pen as HGDIOBJ);
            DeleteObject(hover_brush as HGDIOBJ);
        }

        // Label
        let old_font = SelectObject(hdc, ctx.fonts.normal as HGDIOBJ);
        SetTextColor(hdc, SETTINGS_TEXT);
        let label_wide = to_wide(label);
        let switch_w = (36.0 * scale) as i32;
        let switch_h = (20.0 * scale) as i32;
        let label_right = right - switch_w - (SPACE_2 as f32 * scale) as i32;
        let mut label_rect = RECT {
            left,
            top: *y,
            right: label_right,
            bottom: *y + row_h,
        };
        DrawTextW(
            hdc,
            label_wide.as_ptr(),
            label_wide.len() as i32 - 1,
            &mut label_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );

        // Toggle switch (pill + knob)
        let switch_left = right - switch_w;
        let switch_top = *y + (row_h - switch_h) / 2;
        let switch_rect = RECT {
            left: switch_left,
            top: switch_top,
            right: switch_left + switch_w,
            bottom: switch_top + switch_h,
        };

        // Pill background
        let pill_color = if value { ACCENT } else { SETTINGS_BORDER };
        let pill_brush = CreateSolidBrush(pill_color);
        let pill_radius = switch_h;
        let pill_pen = CreatePen(PS_SOLID, 1, pill_color);
        let old_sp = SelectObject(hdc, pill_pen as HGDIOBJ);
        let old_sb = SelectObject(hdc, pill_brush as HGDIOBJ);
        RoundRect(
            hdc,
            switch_rect.left,
            switch_rect.top,
            switch_rect.right,
            switch_rect.bottom,
            pill_radius,
            pill_radius,
        );

        // Focus ring for toggle
        if is_focused {
            let focus_pen = CreatePen(PS_SOLID, (2.0 * scale).max(2.0) as i32, FOCUS_RING);
            let old_fp = SelectObject(hdc, focus_pen as HGDIOBJ);
            let old_fb = SelectObject(hdc, GetStockObject(NULL_BRUSH as i32) as HGDIOBJ);
            RoundRect(
                hdc,
                switch_rect.left - 2,
                switch_rect.top - 2,
                switch_rect.right + 2,
                switch_rect.bottom + 2,
                pill_radius + 4,
                pill_radius + 4,
            );
            SelectObject(hdc, old_fp as HGDIOBJ);
            SelectObject(hdc, old_fb as HGDIOBJ);
            DeleteObject(focus_pen as HGDIOBJ);
        }

        SelectObject(hdc, old_sp as HGDIOBJ);
        SelectObject(hdc, old_sb as HGDIOBJ);
        DeleteObject(pill_pen as HGDIOBJ);
        DeleteObject(pill_brush as HGDIOBJ);

        // Knob (circle inside pill)
        let knob_r = (switch_h / 2) - (3.0 * scale) as i32;
        let knob_cy = switch_top + switch_h / 2;
        let knob_cx = if value {
            switch_rect.right - (3.0 * scale) as i32 - knob_r
        } else {
            switch_rect.left + (3.0 * scale) as i32 + knob_r
        };
        let knob_color = if value { KNOB_ON } else { KNOB_OFF };
        let knob_brush = CreateSolidBrush(knob_color);
        let knob_pen = CreatePen(PS_SOLID, 1, knob_color);
        let old_kp = SelectObject(hdc, knob_pen as HGDIOBJ);
        let old_kb = SelectObject(hdc, knob_brush as HGDIOBJ);
        Ellipse(
            hdc,
            knob_cx - knob_r,
            knob_cy - knob_r,
            knob_cx + knob_r,
            knob_cy + knob_r,
        );
        SelectObject(hdc, old_kp as HGDIOBJ);
        SelectObject(hdc, old_kb as HGDIOBJ);
        DeleteObject(knob_pen as HGDIOBJ);
        DeleteObject(knob_brush as HGDIOBJ);

        SelectObject(hdc, old_font as HGDIOBJ);
        *y += row_h + (SPACE_1 as f32 * scale) as i32;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_settings_info(
    hdc: HDC,
    ctx: &PaintContext,
    left: i32,
    right: i32,
    y: &mut i32,
    scale: f32,
    label: &str,
    value: &str,
    value_color: u32,
) {
    unsafe {
        let row_h = (24.0 * scale) as i32;
        let old_font = SelectObject(hdc, ctx.fonts.normal as HGDIOBJ);
        SetTextColor(hdc, SETTINGS_LABEL);
        let label_wide = to_wide(label);
        let mut label_rect = RECT {
            left,
            top: *y,
            right: left + (100.0 * scale) as i32,
            bottom: *y + row_h,
        };
        DrawTextW(
            hdc,
            label_wide.as_ptr(),
            label_wide.len() as i32 - 1,
            &mut label_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );

        SetTextColor(hdc, value_color);
        let val_wide = to_wide(value);
        let mut val_rect = RECT {
            left: left + (100.0 * scale) as i32,
            top: *y,
            right,
            bottom: *y + row_h,
        };
        DrawTextW(
            hdc,
            val_wide.as_ptr(),
            val_wide.len() as i32 - 1,
            &mut val_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP | DT_END_ELLIPSIS,
        );
        SelectObject(hdc, old_font as HGDIOBJ);
        *y += row_h;
    }
}
