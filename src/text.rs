// Text utilities: measurement, word wrapping, truncation, date/time helpers.
#![allow(dead_code)]

use crate::chat::ChatState;
use crate::fonts::Fonts;
use crate::markdown::Style;
use crate::win32::*;

// ── Text measurement ─────────────────────────────────────────

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

        // Binary search for the right truncation point
        let ellipsis = "...";
        let ell_wide = to_wide(ellipsis);
        let mut ell_size = POINT { x: 0, y: 0 };
        GetTextExtentPoint32W(hdc, ell_wide.as_ptr(), ell_wide.len() as i32 - 1, &mut ell_size);

        let chars: Vec<char> = text.chars().collect();
        let mut lo = 0;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect::<String>() + ellipsis;
            let cand_wide = to_wide(&candidate);
            let mut cand_size = POINT { x: 0, y: 0 };
            GetTextExtentPoint32W(hdc, cand_wide.as_ptr(), cand_wide.len() as i32 - 1, &mut cand_size);
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

// ── Word wrapping ────────────────────────────────────────────

pub fn wrap_text(hdc: HDC, text: &str, max_width: i32, style: &Style, fonts: &Fonts) -> Vec<(String, Style, i32)> {
    let font = if style.code { fonts.code } else if style.bold { fonts.bold } else { fonts.normal };
    let old_font = unsafe { SelectObject(hdc, font as HGDIOBJ) };

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        unsafe { SelectObject(hdc, old_font) };
        return vec![(text.to_string(), *style, 0)];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0i32;

    for word in words {
        let test_line = if current_line.is_empty() { word.to_string() } else { format!("{} {}", current_line, word) };
        let wide = to_wide(&test_line);
        let mut size = POINT { x: 0, y: 0 };
        unsafe { GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32 - 1, &mut size); }

        if size.x <= max_width || current_line.is_empty() {
            current_line = test_line;
            current_width = size.x;
        } else {
            lines.push((current_line.clone(), *style, current_width));
            current_line = word.to_string();
            let word_wide = to_wide(word);
            unsafe { GetTextExtentPoint32W(hdc, word_wide.as_ptr(), word_wide.len() as i32 - 1, &mut size); }
            current_width = size.x;
        }
    }

    if !current_line.is_empty() {
        lines.push((current_line, *style, current_width));
    }
    if lines.is_empty() {
        lines.push((text.to_string(), *style, 0));
    }

    unsafe { SelectObject(hdc, old_font) };
    lines
}

// ── Date/time helpers ────────────────────────────────────────

/// Convert Unix timestamp (seconds) to civil date (year, month, day).
/// Uses the algorithm from Howard Hinnant's date library.
pub fn unix_to_civil(unix_secs: u64) -> (i64, u32, u32) {
    let days = (unix_secs / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Returns a date group label for a session based on its creation time.
/// Groups: "Today", "Yesterday", or "Mon DD" (e.g., "Oct 23").
pub fn date_group_label(created_at: &std::time::Instant) -> String {
    let now = std::time::SystemTime::now();
    let elapsed = created_at.elapsed();
    let created_sys = now.checked_sub(elapsed).unwrap_or(now);

    let created = match created_sys.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return "Unknown".to_string(),
    };
    let now_secs = match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return "Unknown".to_string(),
    };

    let (cy, cm, cd) = unix_to_civil(created);
    let (ny, nm, nd) = unix_to_civil(now_secs);

    if cy == ny && cm == nm && cd == nd {
        "Today".to_string()
    } else {
        let yesterday = now_secs.saturating_sub(86400);
        let (yy, ym, yd) = unix_to_civil(yesterday);
        if cy == yy && cm == ym && cd == yd {
            "Yesterday".to_string()
        } else {
            let month_names = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                               "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            let m_idx = (cm.saturating_sub(1)) as usize % 12;
            format!("{} {}", month_names[m_idx], cd)
        }
    }
}

/// Returns a relative time string like "2m ago", "1h ago", "3d ago".
pub fn relative_time(created_at: &std::time::Instant) -> String {
    let secs = created_at.elapsed().as_secs();
    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Get a preview of the last message in a chat (first 60 chars, single line).
pub fn last_message_preview(chat: &ChatState) -> String {
    for msg in chat.messages.iter().rev() {
        let text = msg.text.trim();
        if !text.is_empty() {
            let first_line: String = text.chars().take_while(|&c| c != '\n').collect();
            let preview = if first_line.chars().count() > 60 {
                let truncated: String = first_line.chars().take(60).collect();
                format!("{}...", truncated)
            } else {
                first_line
            };
            let prefix = match msg.role {
                crate::chat::Role::User => "You: ",
                crate::chat::Role::Agent => "",
                crate::chat::Role::System => "",
            };
            return format!("{}{}", prefix, preview);
        }
    }
    String::new()
}
