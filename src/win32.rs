// Minimal Win32/GDI FFI declarations. No external crates.
#![allow(
    non_snake_case,
    non_camel_case_types,
    dead_code,
    clippy::upper_case_acronyms
)]

use std::ffi::c_void;

// ── Basic types ──────────────────────────────────────────────
pub type VOID = c_void;
pub type BOOL = i32;
pub type BYTE = u8;
pub type WORD = u16;
pub type DWORD = u32;
pub type UINT = u32;
pub type INT = i32;
pub type LONG = i32;
pub type ULONG = u32;
pub type LONGLONG = i64;
pub type UINT_PTR = usize;
pub type LONG_PTR = isize;
pub type DWORD_PTR = usize;
pub type SIZE_T = usize;
pub type WPARAM = UINT_PTR;
pub type LPARAM = LONG_PTR;
pub type LRESULT = LONG_PTR;
pub type ATOM = WORD;
pub type HANDLE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HICON = *mut c_void;
pub type HMENU = *mut c_void;
pub type HFONT = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type HPEN = *mut c_void;
pub type HRGN = *mut c_void;
pub type HMONITOR = *mut c_void;
pub type HKL = *mut c_void;
pub type HGLOBAL = *mut c_void;
pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;
pub type LPSTR = *mut u8;
pub type LPCSTR = *const u8;
pub type LPWSTR = *mut u16;
pub type LPCWSTR = *const u16;
pub type LPDWORD = *mut DWORD;
pub type LPRECT = *mut RECT;
pub type LPPOINT = *mut POINT;
pub type LPMSG = *mut MSG;
pub type LPPAINTSTRUCT = *mut PAINTSTRUCT;
pub type LPBITMAPINFO = *mut BITMAPINFO;
pub type LPLOGFONTW = *mut LOGFONTW;
pub type LPTEXTMETRICW = *mut TEXTMETRICW;
pub type LPCREATESTRUCTW = *const CREATESTRUCTW;
pub type WNDPROC = Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>;

pub const TRUE: BOOL = 1;
pub const FALSE: BOOL = 0;
pub const NULL: *mut c_void = std::ptr::null_mut();

// ── Window styles ────────────────────────────────────────────
pub const WS_OVERLAPPED: DWORD = 0x0000_0000;
pub const WS_CAPTION: DWORD = 0x00C0_0000;
pub const WS_SYSMENU: DWORD = 0x0008_0000;
pub const WS_THICKFRAME: DWORD = 0x0004_0000;
pub const WS_MINIMIZEBOX: DWORD = 0x0002_0000;
pub const WS_MAXIMIZEBOX: DWORD = 0x0001_0000;
pub const WS_OVERLAPPEDWINDOW: DWORD =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub const WS_VISIBLE: DWORD = 0x1000_0000;
pub const WS_CHILD: DWORD = 0x4000_0000;
pub const WS_CLIPSIBLINGS: DWORD = 0x0400_0000;
pub const WS_CLIPCHILDREN: DWORD = 0x0200_0000;

pub const CW_USEDEFAULT: i32 = -0x8000_0000i32;

// ── Class styles ─────────────────────────────────────────────
pub const CS_HREDRAW: UINT = 0x0002;
pub const CS_VREDRAW: UINT = 0x0001;
pub const CS_DBLCLKS: UINT = 0x0008;

// ── Show window ──────────────────────────────────────────────
pub const SW_SHOW: i32 = 5;
pub const SW_SHOWDEFAULT: i32 = 10;

// ── Window messages ──────────────────────────────────────────
pub const WM_NULL: UINT = 0x0000;
pub const WM_CREATE: UINT = 0x0001;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_PAINT: UINT = 0x000F;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_SIZE: UINT = 0x0005;
pub const WM_KEYDOWN: UINT = 0x0100;
pub const WM_KEYUP: UINT = 0x0101;
pub const WM_CHAR: UINT = 0x0102;
pub const WM_UNICHAR: UINT = 0x0109;
pub const WM_DEADCHAR: UINT = 0x0103;
pub const WM_MOUSEWHEEL: UINT = 0x020A;
pub const WM_MOUSEMOVE: UINT = 0x0200;
pub const WM_LBUTTONDOWN: UINT = 0x0201;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_LBUTTONDBLCLK: UINT = 0x0203;
pub const WM_RBUTTONDOWN: UINT = 0x0204;
pub const WM_SETFOCUS: UINT = 0x0007;
pub const WM_KILLFOCUS: UINT = 0x0008;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_ERASEBKGND: UINT = 0x0014;
pub const WM_DPICHANGED: UINT = 0x02E0;
pub const WM_SETCURSOR: UINT = 0x0020;
pub const WM_COPYDATA: UINT = 0x004A;
pub const WM_GETMINMAXINFO: UINT = 0x0024;

// ── Virtual key codes ────────────────────────────────────────
pub const VK_BACK: u8 = 0x08;
pub const VK_TAB: u8 = 0x09;
pub const VK_RETURN: u8 = 0x0D;
pub const VK_SHIFT: u8 = 0x10;
pub const VK_CONTROL: u8 = 0x11;
pub const VK_MENU: u8 = 0x12;
pub const VK_ESCAPE: u8 = 0x1B;
pub const VK_SPACE: u8 = 0x20;
pub const VK_PRIOR: u8 = 0x21;
pub const VK_NEXT: u8 = 0x22;
pub const VK_END: u8 = 0x23;
pub const VK_HOME: u8 = 0x24;
pub const VK_LEFT: u8 = 0x25;
pub const VK_UP: u8 = 0x26;
pub const VK_RIGHT: u8 = 0x27;
pub const VK_DOWN: u8 = 0x28;
pub const VK_DELETE: u8 = 0x2E;
pub const VK_A: u8 = 0x41;
pub const VK_C: u8 = 0x43;
pub const VK_V: u8 = 0x56;
pub const VK_X: u8 = 0x58;
pub const VK_Z: u8 = 0x5A;
pub const VK_Y: u8 = 0x59;
pub const VK_F5: u8 = 0x74;
pub const VK_N: u8 = 0x4E;

// ── DrawText flags ───────────────────────────────────────────
pub const DT_LEFT: UINT = 0x0000_0000;
pub const DT_TOP: UINT = 0x0000_0000;
pub const DT_CENTER: UINT = 0x0000_0001;
pub const DT_RIGHT: UINT = 0x0000_0002;
pub const DT_VCENTER: UINT = 0x0000_0004;
pub const DT_WORDBREAK: UINT = 0x0000_0010;
pub const DT_CALCRECT: UINT = 0x0000_0400;
pub const DT_NOCLIP: UINT = 0x0000_0100;
pub const DT_SINGLELINE: UINT = 0x0000_0020;
pub const DT_EDITCONTROL: UINT = 0x0000_2000;
pub const DT_END_ELLIPSIS: UINT = 0x0000_8000;
pub const DT_NOPREFIX: UINT = 0x0000_0800;
pub const DT_EXPANDTABS: UINT = 0x0000_0040;

// ── GDI constants ────────────────────────────────────────────
pub const BI_RGB: DWORD = 0;
pub const DIB_RGB_COLORS: UINT = 0;
pub const SRCCOPY: DWORD = 0x00CC_0020;
pub const TRANSPARENT: i32 = 1;
pub const OPAQUE: i32 = 2;
pub const LF_FACESIZE: usize = 32;

// ── Stock objects ────────────────────────────────────────────
pub const WHITE_BRUSH: usize = 0;
pub const BLACK_BRUSH: usize = 4;
pub const NULL_BRUSH: usize = 5;
pub const DEFAULT_GUI_FONT: usize = 17;
pub const SYSTEM_FONT: usize = 13;
pub const ANSI_FIXED_FONT: usize = 11;
pub const DEFAULT_PALETTE: usize = 15;

// ── Clipboard ────────────────────────────────────────────────
pub const CF_UNICODETEXT: UINT = 13;
pub const CF_TEXT: UINT = 1;
pub const GMEM_MOVEABLE: UINT = 0x0002;

// ── Mouse wheel ──────────────────────────────────────────────
pub const WHEEL_DELTA: i32 = 120;
pub const MK_CONTROL: WPARAM = 0x0008;
pub const MK_SHIFT: WPARAM = 0x0004;

// ── DPI ──────────────────────────────────────────────────────
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: *mut c_void = -4isize as *mut c_void;

// ── System metrics ───────────────────────────────────────────
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

// ── PeekMessage ──────────────────────────────────────────────
pub const PM_NOREMOVE: UINT = 0x0000;
pub const PM_REMOVE: UINT = 0x0001;
pub const PM_NOYIELD: UINT = 0x0002;

// ── MessageBox ───────────────────────────────────────────────
pub const MB_OK: UINT = 0x0000_0000;
pub const MB_ICONERROR: UINT = 0x0000_0010;
pub const MB_ICONWARNING: UINT = 0x0000_0030;
pub const MB_YESNO: UINT = 0x0000_0004;
pub const IDYES: UINT = 6;
pub const IDNO: UINT = 7;
pub const MB_ICONINFORMATION: UINT = 0x0000_0040;

// ── Cursor ───────────────────────────────────────────────────
pub const IDC_ARROW: LPCWSTR = 32512 as LPCWSTR;
pub const IDC_IBEAM: LPCWSTR = 32513 as LPCWSTR;

// ── Timer ────────────────────────────────────────────────────
pub const USER_TIMER_MINIMUM: UINT = 0x0000_000A;

// ── Structs ──────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

impl RECT {
    pub fn width(&self) -> LONG {
        self.right - self.left
    }
    pub fn height(&self) -> LONG {
        self.bottom - self.top
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [BYTE; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: LPCWSTR,
    pub lpszClassName: LPCWSTR,
    pub hIconSm: HICON,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CREATESTRUCTW {
    pub lpCreateParams: LPVOID,
    pub hInstance: HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: HWND,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: LONG,
    pub lpszName: LPCWSTR,
    pub lpszClass: LPCWSTR,
    pub dwExStyle: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BITMAPINFOHEADER {
    pub biSize: DWORD,
    pub biWidth: LONG,
    pub biHeight: LONG,
    pub biPlanes: WORD,
    pub biBitCount: WORD,
    pub biCompression: DWORD,
    pub biSizeImage: DWORD,
    pub biXPelsPerMeter: LONG,
    pub biYPelsPerMeter: LONG,
    pub biClrUsed: DWORD,
    pub biClrImportant: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LOGFONTW {
    pub lfHeight: LONG,
    pub lfWidth: LONG,
    pub lfEscapement: LONG,
    pub lfOrientation: LONG,
    pub lfWeight: LONG,
    pub lfItalic: BYTE,
    pub lfUnderline: BYTE,
    pub lfStrikeOut: BYTE,
    pub lfCharSet: BYTE,
    pub lfOutPrecision: BYTE,
    pub lfClipPrecision: BYTE,
    pub lfQuality: BYTE,
    pub lfPitchAndFamily: BYTE,
    pub lfFaceName: [u16; LF_FACESIZE],
}

impl LOGFONTW {
    pub fn new() -> Self {
        Self {
            lfHeight: 0,
            lfWidth: 0,
            lfEscapement: 0,
            lfOrientation: 0,
            lfWeight: 0,
            lfItalic: 0,
            lfUnderline: 0,
            lfStrikeOut: 0,
            lfCharSet: 0,
            lfOutPrecision: 0,
            lfClipPrecision: 0,
            lfQuality: 0,
            lfPitchAndFamily: 0,
            lfFaceName: [0; LF_FACESIZE],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TEXTMETRICW {
    pub tmHeight: LONG,
    pub tmAscent: LONG,
    pub tmDescent: LONG,
    pub tmInternalLeading: LONG,
    pub tmExternalLeading: LONG,
    pub tmAveCharWidth: LONG,
    pub tmMaxCharWidth: LONG,
    pub tmWeight: LONG,
    pub tmOverhang: LONG,
    pub tmDigitizedAspectX: LONG,
    pub tmDigitizedAspectY: LONG,
    pub tmFirstChar: u16,
    pub tmLastChar: u16,
    pub tmDefaultChar: u16,
    pub tmBreakChar: u16,
    pub tmItalic: BYTE,
    pub tmUnderlined: BYTE,
    pub tmStruckOut: BYTE,
    pub tmPitchAndFamily: BYTE,
    pub tmCharSet: BYTE,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

// ── user32.dll ───────────────────────────────────────────────
#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> ATOM;
    pub fn UnregisterClassW(lpClassName: LPCWSTR, hInstance: HINSTANCE) -> BOOL;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: LPCWSTR,
        lpWindowName: LPCWSTR,
        dwStyle: DWORD,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: LPVOID,
    ) -> HWND;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn GetMessageW(lpMsg: LPMSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> BOOL;
    pub fn PeekMessageW(
        lpMsg: LPMSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
        wRemoveMsg: UINT,
    ) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: LPRECT) -> BOOL;
    pub fn GetWindowRect(hWnd: HWND, lpRect: LPRECT) -> BOOL;
    pub fn BeginPaint(hWnd: HWND, lpPaint: LPPAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn SetTimer(
        hWnd: HWND,
        nIDEvent: UINT_PTR,
        uElapse: UINT,
        lpTimerFunc: Option<unsafe extern "system" fn(HWND, UINT, UINT_PTR, DWORD)>,
    ) -> UINT_PTR;
    pub fn KillTimer(hWnd: HWND, nIDEvent: UINT_PTR) -> BOOL;
    pub fn GetDC(hWnd: HWND) -> HDC;
    pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
    pub fn MessageBoxW(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: UINT) -> i32;
    pub fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> BOOL;
    pub fn GetWindowTextW(hWnd: HWND, lpString: LPWSTR, nMaxCount: i32) -> i32;
    pub fn SetFocus(hWnd: HWND) -> HWND;
    pub fn GetFocus() -> HWND;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: LPCWSTR) -> HCURSOR;
    pub fn SetCursor(hCursor: HCURSOR) -> HCURSOR;
    pub fn GetCursorPos(lpPoint: LPPOINT) -> BOOL;
    pub fn ScreenToClient(hWnd: HWND, lpPoint: LPPOINT) -> BOOL;
    pub fn SetCapture(hWnd: HWND) -> HWND;
    pub fn ReleaseCapture() -> BOOL;
    pub fn GetAsyncKeyState(vKey: i32) -> i16;
    pub fn GetKeyState(nVirtKey: i32) -> i16;
    pub fn ToUnicodeEx(
        wVirtKey: UINT,
        wScanCode: UINT,
        lpKeyState: *const BYTE,
        pwszBuff: LPWSTR,
        cchBuff: i32,
        wFlags: UINT,
        dwhkl: HKL,
    ) -> i32;
    pub fn GetKeyboardLayout(dwThreadId: DWORD) -> HKL;
    pub fn GetDpiForWindow(hWnd: HWND) -> UINT;
    pub fn SetProcessDpiAwarenessContext(dpiContext: *mut c_void) -> BOOL;
    pub fn OpenClipboard(hWndNewOwner: HWND) -> BOOL;
    pub fn CloseClipboard() -> BOOL;
    pub fn EmptyClipboard() -> BOOL;
    pub fn SetClipboardData(uFormat: UINT, hMem: HANDLE) -> HANDLE;
    pub fn GetClipboardData(uFormat: UINT) -> HANDLE;
    pub fn IsClipboardFormatAvailable(format: UINT) -> BOOL;
    pub fn GetSystemMetrics(nIndex: i32) -> i32;
    pub fn AdjustWindowRectEx(
        lpRect: LPRECT,
        dwStyle: DWORD,
        bMenu: BOOL,
        dwExStyle: DWORD,
    ) -> BOOL;
    pub fn MonitorFromWindow(hwnd: HWND, dwFlags: DWORD) -> HMONITOR;
    pub fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: UINT,
    ) -> BOOL;
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn SetRect(lprc: LPRECT, xLeft: i32, yTop: i32, xRight: i32, yBottom: i32) -> BOOL;
    pub fn InflateRect(lprc: LPRECT, dx: i32, dy: i32) -> BOOL;
    pub fn IntersectRect(lprcDst: LPRECT, lprcSrc1: *const RECT, lprcSrc2: *const RECT) -> BOOL;
    pub fn UnionRect(lprcDst: LPRECT, lprcSrc1: *const RECT, lprcSrc2: *const RECT) -> BOOL;
    pub fn PtInRect(lprc: *const RECT, pt: POINT) -> BOOL;
    pub fn SetRectEmpty(lprc: LPRECT) -> BOOL;
    pub fn CopyRect(lprcDst: LPRECT, lprcSrc: *const RECT) -> BOOL;
    pub fn OffsetRect(lprc: LPRECT, dx: i32, dy: i32) -> BOOL;
    pub fn DrawFocusRect(hDC: HDC, lprc: *const RECT) -> BOOL;
    pub fn GetSysColor(nIndex: i32) -> DWORD;
    pub fn GetStockObject(i: i32) -> HGDIOBJ;
    pub fn wsprintfW(lpOut: LPWSTR, lpFmt: LPCWSTR, ...) -> i32;
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> LONG_PTR;
    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: LONG_PTR) -> LONG_PTR;
    pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: LPCWSTR) -> HICON;
    pub fn SetScrollInfo(hWnd: HWND, nBar: i32, lpsi: *const SCROLLINFO, bRedraw: BOOL) -> i32;
    pub fn GetScrollInfo(hWnd: HWND, nBar: i32, lpsi: *mut SCROLLINFO) -> BOOL;
    pub fn SetScrollPos(hWnd: HWND, nBar: i32, nPos: i32, bRedraw: BOOL) -> i32;
    pub fn GetScrollPos(hWnd: HWND, nBar: i32) -> i32;
    pub fn ShowScrollBar(hWnd: HWND, wBar: i32, bShow: BOOL) -> BOOL;
    pub fn TrackMouseEvent(lpTrackMouseEvent: *mut TRACKMOUSEEVENT) -> BOOL;
    pub fn ClientToScreen(hWnd: HWND, lpPoint: LPPOINT) -> BOOL;
    pub fn GetSysColorBrush(nIndex: i32) -> HBRUSH;
    pub fn FrameRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn InvertRect(hDC: HDC, lprc: *const RECT) -> BOOL;
    pub fn EnableWindow(hWnd: HWND, bEnable: BOOL) -> BOOL;
    pub fn IsWindowEnabled(hWnd: HWND) -> BOOL;
    pub fn GetCapture() -> HWND;
    pub fn DragDetect(hwnd: HWND, pt: POINT) -> BOOL;
    pub fn GetCaretBlinkTime() -> UINT;
}

// ── gdi32.dll ────────────────────────────────────────────────
#[link(name = "gdi32")]
extern "system" {
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> HBITMAP;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn BitBlt(
        hdc: HDC,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        hdcSrc: HDC,
        x1: i32,
        y1: i32,
        rop: DWORD,
    ) -> BOOL;
    pub fn CreateSolidBrush(color: u32) -> HBRUSH;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn CreateFontIndirectW(lplf: *const LOGFONTW) -> HFONT;
    pub fn DrawTextW(hdc: HDC, lpchText: LPCWSTR, cchText: i32, lprc: LPRECT, format: UINT) -> i32;
    pub fn DrawTextExW(
        hdc: HDC,
        lpchText: LPWSTR,
        cchText: i32,
        lprc: LPRECT,
        dwDTFormat: UINT,
        lpDTParams: *const DRAWTEXTPARAMS,
    ) -> i32;
    pub fn GetTextExtentPoint32W(hdc: HDC, lpString: LPCWSTR, c: i32, lpszl: *mut POINT) -> BOOL;
    pub fn GetTextMetricsW(hdc: HDC, lptm: LPTEXTMETRICW) -> BOOL;
    pub fn Rectangle(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
    pub fn CreatePen(iStyle: i32, cWidth: i32, color: u32) -> HPEN;
    pub fn ExtTextOutW(
        hdc: HDC,
        x: i32,
        y: i32,
        options: UINT,
        lprect: *const RECT,
        lpString: LPCWSTR,
        c: UINT,
        lpDx: *const i32,
    ) -> BOOL;
    pub fn TextOutW(hdc: HDC, x: i32, y: i32, lpString: LPCWSTR, c: i32) -> BOOL;
    pub fn PatBlt(hdc: HDC, x: i32, y: i32, w: i32, h: i32, rop: DWORD) -> BOOL;
    pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
    pub fn FillRgn(hdc: HDC, hrgn: HRGN, hbr: HBRUSH) -> BOOL;
    pub fn RoundRect(
        hdc: HDC,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> BOOL;
    pub fn Ellipse(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
    pub fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> HRGN;
    pub fn SelectClipRgn(hdc: HDC, hrgn: HRGN) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DRAWTEXTPARAMS {
    pub cbSize: UINT,
    pub iTabLength: i32,
    pub iLeftMargin: i32,
    pub iRightMargin: i32,
    pub uiLengthDrawn: UINT,
}

// ── kernel32.dll ─────────────────────────────────────────────
#[link(name = "kernel32")]
extern "system" {
    pub fn GlobalAlloc(uFlags: UINT, dwBytes: SIZE_T) -> HGLOBAL;
    pub fn GlobalLock(hMem: HGLOBAL) -> LPVOID;
    pub fn GlobalUnlock(hMem: HGLOBAL) -> BOOL;
    pub fn GlobalFree(hMem: HGLOBAL) -> HGLOBAL;
    pub fn GlobalSize(hMem: HGLOBAL) -> SIZE_T;
    pub fn lstrlenW(lpString: LPCWSTR) -> i32;
    pub fn MultiByteToWideChar(
        CodePage: UINT,
        dwFlags: DWORD,
        lpMultiByteStr: LPCSTR,
        cbMultiByte: i32,
        lpWideCharStr: LPWSTR,
        cchWideChar: i32,
    ) -> i32;
    pub fn WideCharToMultiByte(
        CodePage: UINT,
        dwFlags: DWORD,
        lpWideCharStr: LPCWSTR,
        cchWideChar: i32,
        lpMultiByteStr: LPSTR,
        cbMultiByte: i32,
        lpDefaultChar: LPCSTR,
        lpUsedDefaultChar: *mut BOOL,
    ) -> i32;
    pub fn GetCurrentThreadId() -> DWORD;
    pub fn Sleep(dwMilliseconds: DWORD);
    pub fn QueryPerformanceCounter(lpPerformanceCount: *mut LONGLONG) -> BOOL;
    pub fn QueryPerformanceFrequency(lpFrequency: *mut LONGLONG) -> BOOL;
}

pub const CP_UTF8: UINT = 65001;
pub const MB_ERR_INVALID_CHARS: DWORD = 0x0000_0008;

// ── shcore.dll ───────────────────────────────────────────────
#[link(name = "shcore")]
extern "system" {
    pub fn SetProcessDpiAwareness(value: i32) -> i32;
}

pub const PROCESS_PER_MONITOR_DPI_AWARE: i32 = 2;

// ── Helper: convert Rust string to wide string ───────────────
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}

// ── Helper: RGB color ────────────────────────────────────────
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | (r as u32)
}

// ── Constants for window creation ────────────────────────────
pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOZORDER: UINT = 0x0004;
pub const SWP_NOACTIVATE: UINT = 0x0010;
pub const SWP_FRAMECHANGED: UINT = 0x0020;
pub const SWP_SHOWWINDOW: UINT = 0x0040;

pub const HWND_TOP: HWND = std::ptr::null_mut();
pub const HWND_TOPMOST: HWND = -1isize as HWND;
pub const HWND_NOTOPMOST: HWND = -2isize as HWND;

pub const MONITOR_DEFAULTTONEAREST: DWORD = 0x0000_0002;

pub const FW_NORMAL: LONG = 400;
pub const FW_BOLD: LONG = 700;
pub const FW_DONTCARE: LONG = 0;

pub const DEFAULT_CHARSET: BYTE = 1;
pub const OUT_DEFAULT_PRECIS: BYTE = 0;
pub const CLIP_DEFAULT_PRECIS: BYTE = 0;
pub const DEFAULT_QUALITY: BYTE = 0;
pub const CLEARTYPE_QUALITY: BYTE = 5;
pub const DEFAULT_PITCH: BYTE = 0;
pub const FF_DONTCARE: BYTE = 0;

pub const PS_SOLID: i32 = 0;

pub const ETO_OPAQUE: UINT = 0x0002;
pub const ETO_CLIPPED: UINT = 0x0004;

pub const COLOR_WINDOW: i32 = 5;
pub const COLOR_WINDOWTEXT: i32 = 8;
pub const COLOR_BTNFACE: i32 = 15;

pub const IDI_APPLICATION: LPCWSTR = 32512 as LPCWSTR;

pub const WM_SETREDRAW: UINT = 0x000B;

pub const HTCLIENT: LRESULT = 1;

pub const WM_NCHITTEST: UINT = 0x0084;

// ── Process creation flags ───────────────────────────────────
pub const CREATE_NO_WINDOW: DWORD = 0x0800_0000;
pub const CREATE_UNICODE_ENVIRONMENT: DWORD = 0x0000_0400;

// ── Mouse key state ──────────────────────────────────────────
pub const MK_LBUTTON: WPARAM = 0x0001;
pub const MK_RBUTTON: WPARAM = 0x0002;
pub const MK_MBUTTON: WPARAM = 0x0010;

// ── Hit test results ─────────────────────────────────────────
pub const HTCAPTION: LRESULT = 2;
pub const HTLEFT: LRESULT = 10;
pub const HTRIGHT: LRESULT = 11;
pub const HTTOP: LRESULT = 12;
pub const HTTOPLEFT: LRESULT = 13;
pub const HTTOPRIGHT: LRESULT = 14;
pub const HTBOTTOM: LRESULT = 15;
pub const HTBOTTOMLEFT: LRESULT = 16;
pub const HTBOTTOMRIGHT: LRESULT = 17;

// ── Cursors ──────────────────────────────────────────────────
pub const IDC_HAND: LPCWSTR = 32649 as LPCWSTR;

// ── Window sizing ────────────────────────────────────────────
pub const SWP_NOOWNERZORDER: UINT = 0x0200;

// ── Scroll info ──────────────────────────────────────────────
pub const SB_VERT: i32 = 1;
pub const SB_THUMBTRACK: i32 = 5;
pub const SB_THUMBPOSITION: i32 = 4;
pub const SB_LINEUP: i32 = 0;
pub const SB_LINEDOWN: i32 = 1;
pub const SB_PAGEUP: i32 = 2;
pub const SB_PAGEDOWN: i32 = 3;
pub const SIF_RANGE: UINT = 0x0001;
pub const SIF_PAGE: UINT = 0x0002;
pub const SIF_POS: UINT = 0x0004;
pub const SIF_ALL: UINT = SIF_RANGE | SIF_PAGE | SIF_POS;
pub const WM_VSCROLL: UINT = 0x0115;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SCROLLINFO {
    pub cbSize: UINT,
    pub fMask: UINT,
    pub nMin: i32,
    pub nMax: i32,
    pub nPage: UINT,
    pub nPos: i32,
    pub nTrackPos: i32,
}

// ── TrackMouseEvent for hover ─────────────────────────────────
pub const TME_LEAVE: DWORD = 0x0000_0002;
pub const WM_MOUSELEAVE: UINT = 0x02A3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: DWORD,
    pub dwFlags: DWORD,
    pub hwndTrack: HWND,
    pub dwHoverTime: DWORD,
}

// ── dwmapi.dll - dark title bar ──────────────────────────────
pub type HRESULT = i32;

pub const DWMWA_USE_IMMERSIVE_DARK_MODE: DWORD = 20;
// Fallback for Windows 10 builds before 20H1
pub const DWMWA_USE_IMMERSIVE_DARK_MODE_BEFORE_20H1: DWORD = 19;

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwAttribute: DWORD,
        pvAttribute: *const std::ffi::c_void,
        cbAttribute: DWORD,
    ) -> HRESULT;
}

// ── shell32.dll - folder picker (SHBrowseForFolderW) ─────────
pub type LPITEMIDLIST = *mut std::ffi::c_void;
pub type LPCITEMIDLIST = *const std::ffi::c_void;

pub const BIF_RETURNONLYFSDIRS: UINT = 0x0001;
pub const BIF_NEWDIALOGSTYLE: UINT = 0x0040;
pub const BIF_NONEWFOLDERBUTTON: UINT = 0x0200;

pub type BFFCALLBACK = Option<unsafe extern "system" fn(HWND, UINT, LPARAM, LPARAM) -> i32>;

#[repr(C)]
pub struct BROWSEINFOW {
    pub hwndOwner: HWND,
    pub pidlRoot: LPCITEMIDLIST,
    pub pszDisplayName: LPWSTR,
    pub lpszTitle: LPCWSTR,
    pub ulFlags: UINT,
    pub lpfn: BFFCALLBACK,
    pub lParam: LPARAM,
    pub iImage: i32,
}

pub type LPBROWSEINFOW = *mut BROWSEINFOW;

#[link(name = "shell32")]
extern "system" {
    pub fn SHBrowseForFolderW(lpbi: LPBROWSEINFOW) -> LPITEMIDLIST;
    pub fn SHGetPathFromIDListW(pidl: LPCITEMIDLIST, pszPath: LPWSTR) -> BOOL;
}

#[link(name = "ole32")]
extern "system" {
    pub fn CoTaskMemFree(pv: *mut std::ffi::c_void);
    pub fn OleInitialize(pvReserved: *mut std::ffi::c_void) -> HRESULT;
}
