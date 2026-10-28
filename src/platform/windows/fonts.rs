// Font management: creation, selection, and cleanup of GDI fonts.
#![allow(dead_code)]

use crate::platform::windows::ffi::*;
use crate::theme::*;

// SAFETY: `Fonts` holds GDI HFONT handles, which are raw pointers and thus
// neither `Send` nor `Sync`. The handles are created on the UI thread and are
// only ever selected into a DC on that same UI thread; they are never used
// from another thread, so transferring ownership across threads is sound.
unsafe impl Send for Fonts {}

pub struct Fonts {
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

        Self {
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
        unsafe {
            DeleteObject(self.normal as HGDIOBJ);
            DeleteObject(self.bold as HGDIOBJ);
            DeleteObject(self.italic as HGDIOBJ);
            DeleteObject(self.code as HGDIOBJ);
            DeleteObject(self.label as HGDIOBJ);
            DeleteObject(self.caption as HGDIOBJ);
            DeleteObject(self.sidebar as HGDIOBJ);
            DeleteObject(self.sidebar_medium as HGDIOBJ);
            DeleteObject(self.title as HGDIOBJ);
            DeleteObject(self.empty_title as HGDIOBJ);
            DeleteObject(self.timestamp as HGDIOBJ);
            DeleteObject(self.session_preview as HGDIOBJ);
            DeleteObject(self.hint as HGDIOBJ);
            DeleteObject(self.group_header as HGDIOBJ);
            DeleteObject(self.session_header_name as HGDIOBJ);
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

    unsafe { CreateFontIndirectW(&lf) }
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
