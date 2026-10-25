// GDI+ helpers: anti-aliased rounded rectangles and hover rings (smooth button rendering).
// GDI+ is loaded dynamically at runtime (LoadLibrary/GetProcAddress) to avoid
// toolchain import-library gaps; if loading fails, callers fall back to GDI.
#![allow(dead_code)]

use crate::win32::*;
use std::ffi::c_void;

/// Function pointers into gdiplus.dll, resolved once at startup.
struct Gdiplus {
    startup: GdiplusStartupFn,
    create_from_hdc: GdipCreateFromHDCFn,
    delete_graphics: GdipDeleteGraphicsFn,
    set_smoothing_mode: GdipSetSmoothingModeFn,
    create_solid_brush: GdipCreateSolidBrushFn,
    delete_brush: GdipDeleteBrushFn,
    create_pen1: GdipCreatePen1Fn,
    delete_pen: GdipDeletePenFn,
    create_path: GdipCreatePathFn,
    delete_path: GdipDeletePathFn,
    add_path_arc: GdipAddPathArcFn,
    close_path_figure: GdipClosePathFigureFn,
    fill_path: GdipFillPathFn,
    draw_path: GdipDrawPathFn,
}

/// Resolved GDI+ bindings plus the startup token (kept for process lifetime).
struct GdiplusState {
    fns: Gdiplus,
    token: SendSyncPtr,
}

/// Wrapper making a raw pointer usable inside a static OnceLock. The GDI+
/// handle is only ever dereferenced on the UI thread, after one-time init.
#[derive(Clone, Copy)]
struct SendSyncPtr(*mut c_void);
unsafe impl Send for SendSyncPtr {}
unsafe impl Sync for SendSyncPtr {}

static GDIPLUS: std::sync::OnceLock<GdiplusState> = std::sync::OnceLock::new();

/// Load gdiplus.dll and resolve all needed entry points. Runs once.
/// Resolve an export by name and reinterpret it as the function-pointer type T.
/// Uses a union instead of transmute (forbidden by project convention); T is
/// always a matching `extern "system"` fn-pointer type for the named export.
unsafe fn get_proc<T: Copy>(module: HMODULE, name: &[u8]) -> Option<T> {
    let ptr = GetProcAddress(module, name.as_ptr());
    if ptr.is_null() {
        return None;
    }
    union FnPtr<T: Copy> {
        raw: *mut c_void,
        typed: T,
    }
    let u = FnPtr { raw: ptr };
    Some(u.typed)
}

unsafe fn load_gdiplus() -> bool {
    if GDIPLUS.get().is_some() {
        return true;
    }

    let name: &[u8] = b"gdiplus.dll\0";
    let module = LoadLibraryA(name.as_ptr());
    if module.is_null() {
        return false;
    }

    macro_rules! sym {
        ($module:expr, $name:expr, $ty:ty) => {{
            let pname: &[u8] = concat!($name, "\0").as_bytes();
            match get_proc::<$ty>($module, pname) {
                Some(f) => f,
                None => return false,
            }
        }};
    }
    // Each $name string is a valid gdiplus.dll export.

    let fns = Gdiplus {
        startup: sym!(module, "GdiplusStartup", GdiplusStartupFn),
        create_from_hdc: sym!(module, "GdipCreateFromHDC", GdipCreateFromHDCFn),
        delete_graphics: sym!(module, "GdipDeleteGraphics", GdipDeleteGraphicsFn),
        set_smoothing_mode: sym!(module, "GdipSetSmoothingMode", GdipSetSmoothingModeFn),
        create_solid_brush: sym!(module, "GdipCreateSolidBrush", GdipCreateSolidBrushFn),
        delete_brush: sym!(module, "GdipDeleteBrush", GdipDeleteBrushFn),
        create_pen1: sym!(module, "GdipCreatePen1", GdipCreatePen1Fn),
        delete_pen: sym!(module, "GdipDeletePen", GdipDeletePenFn),
        create_path: sym!(module, "GdipCreatePath", GdipCreatePathFn),
        delete_path: sym!(module, "GdipDeletePath", GdipDeletePathFn),
        add_path_arc: sym!(module, "GdipAddPathArc", GdipAddPathArcFn),
        close_path_figure: sym!(module, "GdipClosePathFigure", GdipClosePathFigureFn),
        fill_path: sym!(module, "GdipFillPath", GdipFillPathFn),
        draw_path: sym!(module, "GdipDrawPath", GdipDrawPathFn),
    };

    let mut token: GpToken = std::ptr::null_mut();
    let input = GdiplusStartupInput {
        GdiplusVersion: GDIPLUS_VERSION_1,
        DebugEventCallback: std::ptr::null_mut(),
        SuppressBackgroundThread: 0,
        SuppressExternalCodecs: 0,
    };
    if (fns.startup)(&mut token, &input, std::ptr::null_mut()) != GDIPLUS_OK {
        return false;
    }

    let _ = GDIPLUS.set(GdiplusState {
        fns,
        token: SendSyncPtr(token),
    });
    true
}

/// RAII guard wrapping a GDI+ graphics object bound to a HDC.
pub struct Gfx {
    graphics: *mut GpGraphics,
}

impl Gfx {
    /// Create an anti-aliased GDI+ context for the given DC.
    /// Returns None if GDI+ is unavailable (caller falls back to GDI).
    pub fn new(hdc: HDC) -> Option<Gfx> {
        unsafe {
            if !load_gdiplus() {
                return None;
            }
            let state = GDIPLUS.get()?;
            let mut graphics: *mut GpGraphics = std::ptr::null_mut();
            if (state.fns.create_from_hdc)(hdc, &mut graphics) != GDIPLUS_OK || graphics.is_null() {
                return None;
            }
            (state.fns.set_smoothing_mode)(graphics, SMOOTHING_MODE_ANTI_ALIAS);
            Some(Gfx { graphics })
        }
    }

    /// Fill a rounded rectangle with a solid color, anti-aliased.
    pub fn fill_round_rect(&self, rect: RECT, radius: i32, color: u32) {
        unsafe {
            let state = match GDIPLUS.get() {
                Some(s) => s,
                None => return,
            };
            if let Some(path) = self.rounded_rect_path(&state.fns, rect, radius) {
                let mut brush: *mut GpBrush = std::ptr::null_mut();
                if (state.fns.create_solid_brush)(argb_from_rgb(color), &mut brush) == GDIPLUS_OK {
                    (state.fns.fill_path)(self.graphics, brush, path);
                    (state.fns.delete_brush)(brush);
                }
                (state.fns.delete_path)(path);
            }
        }
    }

    /// Stroke a rounded rectangle outline (1px), anti-aliased.
    pub fn stroke_round_rect(&self, rect: RECT, radius: i32, color: u32, width: f32) {
        unsafe {
            let state = match GDIPLUS.get() {
                Some(s) => s,
                None => return,
            };
            if let Some(path) = self.rounded_rect_path(&state.fns, rect, radius) {
                let mut pen: *mut GpPen = std::ptr::null_mut();
                if (state.fns.create_pen1)(argb_from_rgb(color), width, GDIP_UNIT_PIXEL, &mut pen)
                    == GDIPLUS_OK
                {
                    (state.fns.draw_path)(self.graphics, pen, path);
                    (state.fns.delete_pen)(pen);
                }
                (state.fns.delete_path)(path);
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

    /// Build a rounded-rect GDI+ path. Caller must delete the path.
    unsafe fn rounded_rect_path(
        &self,
        fns: &Gdiplus,
        rect: RECT,
        radius: i32,
    ) -> Option<*mut GpPath> {
        let r = radius.max(1) as f32;
        let x = rect.left as f32;
        let y = rect.top as f32;
        let w = (rect.right - rect.left) as f32;
        let h = (rect.bottom - rect.top) as f32;
        // Clamp radius to half the smaller dimension (pill shape at most)
        let r = r.min(w / 2.0).min(h / 2.0);

        let mut path: *mut GpPath = std::ptr::null_mut();
        if (fns.create_path)(PATH_FILL_MODE_ALTERNATE, &mut path) != GDIPLUS_OK || path.is_null() {
            return None;
        }
        // Four arcs: TR, BR, BL, TL (90 degrees each)
        (fns.add_path_arc)(path, x + w - 2.0 * r, y, 2.0 * r, 2.0 * r, 270.0, 90.0);
        (fns.add_path_arc)(
            path,
            x + w - 2.0 * r,
            y + h - 2.0 * r,
            2.0 * r,
            2.0 * r,
            0.0,
            90.0,
        );
        (fns.add_path_arc)(path, x, y + h - 2.0 * r, 2.0 * r, 2.0 * r, 90.0, 90.0);
        (fns.add_path_arc)(path, x, y, 2.0 * r, 2.0 * r, 180.0, 90.0);
        (fns.close_path_figure)(path);
        Some(path)
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        unsafe {
            if !self.graphics.is_null() {
                if let Some(state) = GDIPLUS.get() {
                    (state.fns.delete_graphics)(self.graphics);
                }
            }
        }
    }
}

// Silence unused-import warning for c_void (used in FFI type definitions).
#[allow(unused)]
fn _t(_: *mut c_void) {}
