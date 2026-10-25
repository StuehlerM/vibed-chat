// GDI+ helpers: anti-aliased rounded rectangles and hover rings (smooth button rendering).
#![allow(dead_code)]

use crate::win32::*;
use std::ffi::c_void;

/// One-time GDI+ initialization token (created lazily on first use,
/// kept for the process lifetime; GDI+ is reference-counted per process).
static mut GDIPLUS_TOKEN: GpToken = std::ptr::null_mut();
static mut GDIPLUS_READY: bool = false;

/// Initialize GDI+. Safe to call multiple times.
pub fn ensure_gdiplus() -> bool {
    unsafe {
        if GDIPLUS_READY {
            return true;
        }
        let mut token: GpToken = std::ptr::null_mut();
        let input = GdiplusStartupInput {
            GdiplusVersion: GDIPLUS_VERSION_1,
            DebugEventCallback: std::ptr::null_mut(),
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        };
        if GdiplusStartup(&mut token, &input, std::ptr::null_mut()) == GDIPLUS_OK {
            GDIPLUS_TOKEN = token;
            GDIPLUS_READY = true;
            true
        } else {
            false
        }
    }
}

/// RAII guard wrapping a GDI+ graphics object bound to a HDC.
pub struct Gfx {
    graphics: *mut GpGraphics,
}

impl Gfx {
    /// Create an anti-aliased GDI+ context for the given DC.
    /// Returns None if GDI+ is unavailable (caller falls back to GDI).
    pub fn new(hdc: HDC) -> Option<Gfx> {
        if !ensure_gdiplus() {
            return None;
        }
        unsafe {
            let mut graphics: *mut GpGraphics = std::ptr::null_mut();
            if GdipCreateFromHDC(hdc, &mut graphics) != GDIPLUS_OK || graphics.is_null() {
                return None;
            }
            GdipSetSmoothingMode(graphics, SMOOTHING_MODE_ANTI_ALIAS);
            Some(Gfx { graphics })
        }
    }

    /// Fill a rounded rectangle with a solid color, anti-aliased.
    pub fn fill_round_rect(&self, rect: RECT, radius: i32, color: u32) {
        if let Some(path) = rounded_rect_path(rect, radius) {
            unsafe {
                let mut brush: *mut GpBrush = std::ptr::null_mut();
                if GdipCreateSolidBrush(argb_from_rgb(color), &mut brush) == GDIPLUS_OK {
                    GdipFillPath(self.graphics, brush, path);
                    GdipDeleteBrush(brush);
                }
                GdipDeletePath(path);
            }
        }
    }

    /// Stroke a rounded rectangle outline (1px), anti-aliased.
    pub fn stroke_round_rect(&self, rect: RECT, radius: i32, color: u32, width: f32) {
        if let Some(path) = rounded_rect_path(rect, radius) {
            unsafe {
                let mut pen: *mut GpPen = std::ptr::null_mut();
                if GdipCreatePen1(argb_from_rgb(color), width, GDIP_UNIT_PIXEL, &mut pen) == GDIPLUS_OK
                {
                    GdipDrawPath(self.graphics, pen, path);
                    GdipDeletePen(pen);
                }
                GdipDeletePath(path);
            }
        }
    }

    /// Draw a filled rounded rect plus a hover ring: a 1px outline in
    /// `ring_color`, offset outward by a 1px gap from the button edge.
    pub fn button_with_ring(&self, rect: RECT, radius: i32, fill: u32, ring_color: u32) {
        self.fill_round_rect(rect, radius, fill);
        let gap = 1;
        let ring = RECT {
            left: rect.left - gap - 1,
            top: rect.top - gap - 1,
            right: rect.right + gap + 1,
            bottom: rect.bottom + gap + 1,
        };
        self.stroke_round_rect(ring, radius + gap + 1, ring_color, 1.0);
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        unsafe {
            if !self.graphics.is_null() {
                GdipDeleteGraphics(self.graphics);
            }
        }
    }
}

/// Build a rounded-rect GDI+ path. Caller must GdipDeletePath it.
fn rounded_rect_path(rect: RECT, radius: i32) -> Option<*mut GpPath> {
    let r = radius.max(1) as f32;
    let x = rect.left as f32;
    let y = rect.top as f32;
    let w = (rect.right - rect.left) as f32;
    let h = (rect.bottom - rect.top) as f32;
    // Clamp radius to half the smaller dimension (pill shape at most)
    let r = r.min(w / 2.0).min(h / 2.0);

    unsafe {
        let mut path: *mut GpPath = std::ptr::null_mut();
        if GdipCreatePath(PATH_FILL_MODE_ALTERNATE, &mut path) != GDIPLUS_OK || path.is_null() {
            return None;
        }
        // Four arcs: TR, BR, BL, TL (90 degrees each)
        GdipAddPathArc(path, x + w - 2.0 * r, y, 2.0 * r, 2.0 * r, 270.0, 90.0);
        GdipAddPathArc(path, x + w - 2.0 * r, y + h - 2.0 * r, 2.0 * r, 2.0 * r, 0.0, 90.0);
        GdipAddPathArc(path, x, y + h - 2.0 * r, 2.0 * r, 2.0 * r, 90.0, 90.0);
        GdipAddPathArc(path, x, y, 2.0 * r, 2.0 * r, 180.0, 90.0);
        GdipClosePathFigure(path);
        Some(path)
    }
}

// Silence unused-parameter warnings for the c_void import kept for FFI types.
#[allow(unused)]
fn _t(_: *mut c_void) {}
