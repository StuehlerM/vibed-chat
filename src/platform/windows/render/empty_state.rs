// Empty state rendering: shown when a chat has no messages.
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::platform::windows::paint::PaintContext;
use crate::theme::*;

pub fn draw_empty_state(
    hdc: HDC,
    ctx: &PaintContext,
    chat_left: i32,
    chat_right: i32,
    area_bottom: i32,
    area_top: i32,
    scale: f32,
) {
    unsafe {
        let center_x = (chat_left + chat_right) / 2;
        let center_y = (area_top + area_bottom) / 2;

        // Signature gradient bar (yellow → amber → orange → red-orange → red)
        let stripe_w = (120.0 * scale) as i32;
        let stripe_h = (3.0 * scale).max(2.0) as i32;
        let stripe_y = center_y - (56.0 * scale) as i32;
        let stripe_x = center_x - stripe_w / 2;
        let seg_w = stripe_w / 5;
        let gradient_colors = [
            GRADIENT_YELLOW,
            GRADIENT_AMBER,
            GRADIENT_ORANGE,
            GRADIENT_RED_ORANGE,
            GRADIENT_RED,
        ];
        for (i, &color) in gradient_colors.iter().enumerate() {
            let seg_rect = RECT {
                left: stripe_x + i as i32 * seg_w,
                top: stripe_y,
                right: stripe_x + (i as i32 + 1) * seg_w,
                bottom: stripe_y + stripe_h,
            };
            let brush = CreateSolidBrush(color);
            FillRect(hdc, &seg_rect, brush);
            DeleteObject(brush as HGDIOBJ);
        }

        // Title - larger
        let old_font = SelectObject(hdc, ctx.fonts.title as HGDIOBJ);
        SetTextColor(hdc, TEXT_PRIMARY);
        let title = to_wide("ACP Chat");
        let mut title_rect = RECT {
            left: chat_left,
            top: center_y - (32.0 * scale) as i32,
            right: chat_right,
            bottom: center_y - (4.0 * scale) as i32,
        };
        DrawTextW(
            hdc,
            title.as_ptr(),
            title.len() as i32 - 1,
            &mut title_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // Subtitle
        let old_font = SelectObject(hdc, ctx.fonts.sidebar as HGDIOBJ);
        SetTextColor(hdc, EMPTY_STATE_SUB);
        let sub = to_wide("Start a conversation with your agent");
        let mut sub_rect = RECT {
            left: chat_left,
            top: center_y + (4.0 * scale) as i32,
            right: chat_right,
            bottom: center_y + (28.0 * scale) as i32,
        };
        DrawTextW(
            hdc,
            sub.as_ptr(),
            sub.len() as i32 - 1,
            &mut sub_rect as LPRECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);

        // Keyboard shortcut cards
        let card_w = (220.0 * scale) as i32;
        let card_h = (36.0 * scale) as i32;
        let card_gap = (SPACE_3 as f32 * scale) as i32;
        let total_w = card_w * 2 + card_gap;
        let card_start_x = (chat_left + chat_right) / 2 - total_w / 2;
        let card_y = center_y + (44.0 * scale) as i32;

        draw_shortcut_card(
            hdc,
            ctx,
            card_start_x,
            card_y,
            card_w,
            card_h,
            scale,
            "Enter",
            "Send message",
        );
        draw_shortcut_card(
            hdc,
            ctx,
            card_start_x + card_w + card_gap,
            card_y,
            card_w,
            card_h,
            scale,
            "Shift+Enter",
            "New line",
        );
    }
}

/// Draw a keyboard shortcut card in the empty state.
#[allow(clippy::too_many_arguments)]
fn draw_shortcut_card(
    hdc: HDC,
    ctx: &PaintContext,
    left: i32,
    top: i32,
    w: i32,
    h: i32,
    scale: f32,
    key: &str,
    desc: &str,
) {
    unsafe {
        let card_rect = RECT {
            left,
            top,
            right: left + w,
            bottom: top + h,
        };
        let card_brush = CreateSolidBrush(CARD_BG);
        let card_radius = (RADIUS_MD as f32 * scale) as i32;
        let card_pen = CreatePen(PS_SOLID, 1, CARD_BORDER);
        let old_pen = SelectObject(hdc, card_pen as HGDIOBJ);
        let old_brush = SelectObject(hdc, card_brush as HGDIOBJ);
        RoundRect(
            hdc,
            card_rect.left,
            card_rect.top,
            card_rect.right,
            card_rect.bottom,
            card_radius * 2,
            card_radius * 2,
        );
        SelectObject(hdc, old_pen as HGDIOBJ);
        SelectObject(hdc, old_brush as HGDIOBJ);
        DeleteObject(card_pen as HGDIOBJ);
        DeleteObject(card_brush as HGDIOBJ);

        // Key label (bold)
        let old_font = SelectObject(hdc, ctx.fonts.sidebar_medium as HGDIOBJ);
        SetTextColor(hdc, ACCENT);
        let key_wide = to_wide(key);
        let mut key_rect = RECT {
            left: card_rect.left + (SPACE_3 as f32 * scale) as i32,
            top: card_rect.top,
            right: card_rect.right,
            bottom: card_rect.top + h / 2,
        };
        DrawTextW(
            hdc,
            key_wide.as_ptr(),
            key_wide.len() as i32 - 1,
            &mut key_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );

        // Description
        SetTextColor(hdc, EMPTY_STATE_SUB);
        let desc_wide = to_wide(desc);
        let mut desc_rect = RECT {
            left: card_rect.left + (SPACE_3 as f32 * scale) as i32,
            top: card_rect.top + h / 2,
            right: card_rect.right,
            bottom: card_rect.bottom - (SPACE_1 as f32 * scale) as i32,
        };
        DrawTextW(
            hdc,
            desc_wide.as_ptr(),
            desc_wide.len() as i32 - 1,
            &mut desc_rect as LPRECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOCLIP,
        );
        SelectObject(hdc, old_font as HGDIOBJ);
    }
}
