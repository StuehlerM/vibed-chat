// Windows text measurement and word wrapping using GDI.
#![allow(dead_code)]

use crate::markdown::Style;
use crate::platform::windows::ffi::*;
use crate::platform::windows::fonts::Fonts;

// ── Text measurement ────────────────────────────────────────────────────────

pub fn measure_text_width(hdc: HDC, text: &str, fonts: &Fonts) -> i32 {
    if text.is_empty() {
        return 0;
    }
    unsafe {
        let old_font = SelectObject(hdc, fonts.normal as HGDIOBJ);
        let wide = to_wide(text);
        let mut size = POINT { x: 0, y: 0 };
        GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut size);
        SelectObject(hdc, old_font as HGDIOBJ);
        size.x
    }
}

/// Truncate a string to fit within max_width pixels, adding ellipsis if needed.
pub fn truncate_to_width(hdc: HDC, text: &str, max_width: i32, font: HFONT) -> String {
    if text.is_empty() {
        return String::new();
    }
    unsafe {
        let old_font = SelectObject(hdc, font as HGDIOBJ);
        let wide = to_wide(text);
        let mut size = POINT { x: 0, y: 0 };
        GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut size);

        if size.x <= max_width {
            SelectObject(hdc, old_font as HGDIOBJ);
            return text.to_string();
        }

        let ellipsis = "...";
        let ell_wide = to_wide(ellipsis);
        let mut ell_size = POINT { x: 0, y: 0 };
        GetTextExtentPoint32W(
            hdc,
            ell_wide.as_ptr(),
            ell_wide.len() as i32 - 1,
            &mut ell_size,
        );

        let chars: Vec<char> = text.chars().collect();
        let mut lo = 0;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect::<String>() + ellipsis;
            let cand_wide = to_wide(&candidate);
            let mut cand_size = POINT { x: 0, y: 0 };
            GetTextExtentPoint32W(
                hdc,
                cand_wide.as_ptr(),
                cand_wide.len() as i32 - 1,
                &mut cand_size,
            );
            if cand_size.x <= max_width {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }

        SelectObject(hdc, old_font as HGDIOBJ);
        if lo == 0 {
            return ellipsis.to_string();
        }
        chars[..lo].iter().collect::<String>() + ellipsis
    }
}

// ── Word wrapping ───────────────────────────────────────────────────────────

pub fn wrap_text(
    hdc: HDC,
    text: &str,
    max_width: i32,
    style: &Style,
    fonts: &Fonts,
) -> Vec<(String, Style, i32)> {
    let font = if style.code {
        fonts.code
    } else if style.bold {
        fonts.bold
    } else {
        fonts.normal
    };
    let old_font = unsafe { SelectObject(hdc, font as HGDIOBJ) };

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        unsafe { SelectObject(hdc, old_font) };
        return vec![(text.to_string(), style.clone(), 0)];
    }

    // Measure the space glyph once; word widths are measured separately so we
    // only build a `String` for a candidate line when it actually fits.
    let space_wide = to_wide(" ");
    let mut space_size = POINT { x: 0, y: 0 };
    unsafe {
        GetTextExtentPoint32W(
            hdc,
            space_wide.as_ptr(),
            space_wide.len() as i32 - 1,
            &mut space_size,
        );
    }
    let space_width = space_size.x;

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0i32;

    for word in words {
        let word_wide = to_wide(word);
        let mut word_size = POINT { x: 0, y: 0 };
        unsafe {
            GetTextExtentPoint32W(
                hdc,
                word_wide.as_ptr(),
                word_wide.len() as i32 - 1,
                &mut word_size,
            );
        }

        if current_line.is_empty() {
            // First word on the line always goes in, even if it overflows.
            current_line = word.to_string();
            current_width = word_size.x;
        } else if current_width + space_width + word_size.x <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += space_width + word_size.x;
        } else {
            lines.push((
                std::mem::take(&mut current_line),
                style.clone(),
                current_width,
            ));
            current_line = word.to_string();
            current_width = word_size.x;
        }
    }

    if !current_line.is_empty() {
        lines.push((current_line, style.clone(), current_width));
    }
    if lines.is_empty() {
        lines.push((text.to_string(), style.clone(), 0));
    }

    unsafe { SelectObject(hdc, old_font) };
    lines
}
