// Font management: creation, selection, and cleanup of GDI fonts.
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::theme::*;

pub struct Fonts {
    pub generation: u64,
    pub normal: HFONT,
    pub bold: HFONT,
    pub italic: HFONT,
    pub code: HFONT,
    pub label: HFONT,
    pub caption: HFONT,
    pub sidebar: HFONT,
    pub sidebar_medium: HFONT,
    pub title: HFONT,
    pub empty_title: HFONT,
    pub timestamp: HFONT,
    pub session_preview: HFONT,
    pub hint: HFONT,
    pub group_header: HFONT,
    pub session_header_name: HFONT,
}

impl Fonts {
    pub fn new(dpi: u32) -> Self {
        let scale = dpi as f32 / 96.0;
        let body_h = (-((TEXT_BODY_SIZE * scale) as i32)) as LONG;
        let label_h = (-((12.0 * scale) as i32)) as LONG;
        let caption_h = (-((TEXT_CAPTION_SIZE * scale) as i32)) as LONG;
        let code_h = (-((TEXT_CODE_SIZE * scale) as i32)) as LONG;
        let ui_h = (-((TEXT_UI_SIZE * scale) as i32)) as LONG;
        let title_h = (-((TEXT_HEADING_SIZE * scale) as i32)) as LONG;
        let timestamp_h = (-((TEXT_TIMESTAMP_SIZE * scale) as i32)) as LONG;
        let preview_h = (-((TEXT_SESSION_PREVIEW_SIZE * scale) as i32)) as LONG;
        let hint_h = (-((TEXT_HINT_SIZE * scale) as i32)) as LONG;
        let group_h = (-((TEXT_GROUP_HEADER_SIZE * scale) as i32)) as LONG;
        let sess_hdr_h = (-((TEXT_BODY_SIZE * scale) as i32)) as LONG;

        let normal = create_font(body_h, FW_NORMAL, false, "Segoe UI");
        let bold = create_font(body_h, FW_BOLD, false, "Segoe UI");
        let italic = create_font(body_h, FW_NORMAL, true, "Segoe UI");
        let code = create_font(code_h, FW_NORMAL, false, "Consolas");
        let label = create_font(label_h, FW_BOLD, false, "Segoe UI");
        let caption = create_font(caption_h, FW_SEMIBOLD, false, "Segoe UI");
        let sidebar = create_font(ui_h, FW_NORMAL, false, "Segoe UI");
        let sidebar_medium = create_font(ui_h, FW_SEMIBOLD, false, "Segoe UI");
        let title = create_font(title_h, FW_BOLD, false, "Segoe UI");
        let empty_title = create_font(title_h, FW_NORMAL, false, "Segoe UI");
        let timestamp = create_font(timestamp_h, FW_NORMAL, false, "Segoe UI");
        let session_preview = create_font(preview_h, FW_NORMAL, false, "Segoe UI");
        let hint = create_font(hint_h, FW_NORMAL, false, "Segoe UI");
        let group_header = create_font(group_h, FW_SEMIBOLD, false, "Segoe UI");
        let session_header_name = create_font(sess_hdr_h, FW_SEMIBOLD, false, "Segoe UI");

        static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            generation: NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            normal,
            bold,
            italic,
            code,
            label,
            caption,
            sidebar,
            sidebar_medium,
            title,
            empty_title,
            timestamp,
            session_preview,
            hint,
            group_header,
            session_header_name,
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        // SAFETY: each non-stock handle is a distinct CreateFontIndirectW
        // acquisition. Fonts are UI-thread owned and outlive paint selections.
        unsafe {
            let stock = GetStockObject(DEFAULT_GUI_FONT as i32);
            for font in [
                self.normal,
                self.bold,
                self.italic,
                self.code,
                self.label,
                self.caption,
                self.sidebar,
                self.sidebar_medium,
                self.title,
                self.empty_title,
                self.timestamp,
                self.session_preview,
                self.hint,
                self.group_header,
                self.session_header_name,
            ] {
                if !font.is_null() && !std::ptr::eq(font, stock) {
                    DeleteObject(font as HGDIOBJ);
                }
            }
        }
    }
}

pub fn create_font(height: LONG, weight: LONG, italic: bool, face: &str) -> HFONT {
    let mut lf = LOGFONTW::new();
    lf.lfHeight = height;
    lf.lfWeight = weight;
    lf.lfItalic = if italic { 1 } else { 0 };
    lf.lfCharSet = DEFAULT_CHARSET;
    lf.lfQuality = CLEARTYPE_QUALITY;
    lf.lfPitchAndFamily = 0;

    let face_wide: Vec<u16> = face.encode_utf16().collect();
    let len = face_wide.len().min(LF_FACESIZE - 1);
    lf.lfFaceName[..len].copy_from_slice(&face_wide[..len]);

    // SAFETY: lf is initialized. On allocation failure borrow the stock font;
    // Fonts::drop explicitly excludes that borrowed handle from deletion.
    unsafe {
        let font = CreateFontIndirectW(&lf);
        if font.is_null() {
            GetStockObject(DEFAULT_GUI_FONT as i32) as HFONT
        } else {
            font
        }
    }
}

/// Select the appropriate font for a markdown style and return the previously selected font.
pub fn select_font_for_style(hdc: HDC, style: &crate::markdown::Style, fonts: &Fonts) -> HGDIOBJ {
    unsafe {
        let font = if style.code {
            fonts.code
        } else if style.bold {
            fonts.bold
        } else if style.italic {
            fonts.italic
        } else {
            fonts.normal
        };
        SelectObject(hdc, font as HGDIOBJ)
    }
}
