// Window events: WndProc message handling, window class registration, window creation.
#![allow(dead_code)]

use crate::app::AppState;
use crate::input::{self, InputResult};
use crate::render;
use crate::theme::*;
use crate::ui::{FocusedElement, PaintContext};
use crate::win32::*;

pub const WINDOW_CLASS_NAME: &str = "AcpChatWindow";
pub const WINDOW_TITLE: &str = "ACP Chat";
pub const TIMER_ID_REPAINT: UINT_PTR = 1;
pub const TIMER_INTERVAL_MS: UINT = 16; // ~60fps

/// Minimum window size (logical px, scaled by DPI)
pub const MIN_WINDOW_WIDTH: i32 = 720;
pub const MIN_WINDOW_HEIGHT: i32 = 480;

const GWLP_USERDATA: i32 = -21;

// ── Window procedure ─────────────────────────────────────────

pub unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_GETMINMAXINFO => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let scale = (*app_state).dpi as f32 / 96.0;
                let mmi = lparam as *mut MINMAXINFO;
                (*mmi).ptMinTrackSize.x = (MIN_WINDOW_WIDTH as f32 * scale) as i32;
                (*mmi).ptMinTrackSize.y = (MIN_WINDOW_HEIGHT as f32 * scale) as i32;
            }
            return 0;
        }
        WM_CREATE => {
            let create_struct = lparam as *const CREATESTRUCTW;
            let app_state = (*create_struct).lpCreateParams as *mut AppState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_state as LONG_PTR);
            return 0;
        }
        WM_DPICHANGED => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let new_dpi = (wparam & 0xFFFF) as u32;
                (*app_state).dpi = new_dpi;
                (*app_state).scale_fonts();

                let rect = lparam as *const RECT;
                SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    (*rect).left,
                    (*rect).top,
                    (*rect).right - (*rect).left,
                    (*rect).bottom - (*rect).top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_SIZE => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                (*app_state).client_width = (lparam & 0xFFFF) as i32;
                (*app_state).client_height = ((lparam >> 16) & 0xFFFF) as i32;
            }
            return 0;
        }
        WM_TIMER => {
            if wparam == TIMER_ID_REPAINT {
                let app_state = get_app_state(hwnd);
                if !app_state.is_null() {
                    (*app_state).poll_acp();

                    // Cursor blink
                    let now = std::time::Instant::now();
                    let blink_ms = (*app_state).caret_blink_ms as u128;
                    if now
                        .duration_since((*app_state).last_cursor_blink)
                        .as_millis()
                        > blink_ms
                    {
                        (*app_state).cursor_visible = !(*app_state).cursor_visible;
                        (*app_state).last_cursor_blink = now;
                    }

                    // Streaming animation dots
                    (*app_state).streaming_dots = (*app_state).streaming_dots.wrapping_add(1);

                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            return 0;
        }
        WM_PAINT => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;

                if state.fonts.is_none() {
                    state.init_fonts();
                }

                let fonts_ref = match state.fonts.as_ref() {
                    Some(f) => f,
                    None => return 0,
                };

                let mut ps = PAINTSTRUCT {
                    hdc: std::ptr::null_mut(),
                    fErase: 0,
                    rcPaint: RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    },
                    fRestore: 0,
                    fIncUpdate: 0,
                    rgbReserved: [0; 32],
                };
                BeginPaint(hwnd, &mut ps);

                let mut ctx = PaintContext {
                    toast: state.toast.clone(),
                    projects: &state.projects,
                    sessions: &mut state.sessions,
                    active_session: state.active_session,
                    show_settings: state.show_settings,
                    settings: &state.settings,
                    fonts: fonts_ref,
                    dpi: state.dpi,
                    width: state.client_width,
                    height: state.client_height,
                    cursor_visible: state.cursor_visible,
                    acp_connected: state.acp_connected,
                    hover_sidebar_item: state.hover_sidebar_item,
                    hover_new_chat: state.hover_new_chat,
                    hover_new_project: state.hover_new_project,
                    hover_project: state.hover_project,
                    hover_project_menu: state.hover_project_menu,
                    hover_settings_btn: state.hover_settings_btn,
                    hover_settings_close: state.hover_settings_close,
                    hover_thinking_toggle: state.hover_thinking_toggle,
                    hover_settings_toggle: state.hover_settings_toggle,
                    pressed_element: state.pressed_element,
                    focused_element: state.focused_element,
                    sidebar_scroll_offset: state.sidebar_scroll_offset,
                    hover_scrollbar: state.hover_scrollbar,
                    hover_sidebar_scrollbar: state.hover_sidebar_scrollbar,
                    streaming_dots: state.streaming_dots,
                    hover_send_btn: state.hover_send_btn,
                    hover_session_header: state.hover_session_header,
                    hover_session_archive: state.hover_session_archive,
                    hover_session_delete: state.hover_session_delete,
                };
                render::paint(hwnd, &mut ctx);

                EndPaint(hwnd, &ps);
            }
            return 0;
        }
        WM_ERASEBKGND => {
            return 1;
        }
        WM_KEYDOWN => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;

                // F5 to reconnect
                if wparam as u8 == VK_F5 {
                    state.connect_acp();
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Ctrl+N to open folder picker and create a new chat
                if wparam as u8 == VK_N as u8
                    && (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0
                {
                    if let Some(path) = crate::interaction::pick_folder_dialog() {
                        state.new_chat_in_dir(&path);
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Ctrl+Shift+A to toggle archive on the active session
                if wparam as u8 == 'A' as u8
                    && (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0
                    && (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0
                {
                    if !state.sessions.is_empty() {
                        let idx = state.active_session.min(state.sessions.len() - 1);
                        state.toggle_archive_session(idx);
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Ctrl+Shift+D to delete the active session (with confirmation)
                if wparam as u8 == 'D' as u8
                    && (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0
                    && (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0
                {
                    if !state.sessions.is_empty() {
                        let idx = state.active_session.min(state.sessions.len() - 1);
                        let session_name = state.sessions[idx].name.clone();
                        let msg = to_wide(&format!(
                            "Delete session \"{}\"? This cannot be undone.",
                            session_name
                        ));
                        let title = to_wide("Delete Session");
                        let result = MessageBoxW(
                            hwnd,
                            msg.as_ptr(),
                            title.as_ptr(),
                            MB_YESNO | MB_ICONWARNING,
                        );
                        if result == IDYES as i32 {
                            state.delete_session(idx);
                        }
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Tab / Shift+Tab for keyboard navigation
                if wparam as u8 == VK_TAB {
                    let shift = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
                    state.cycle_focus(shift);
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Escape to close settings
                if wparam as u8 == VK_ESCAPE && state.show_settings {
                    state.show_settings = false;
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Send button keyboard activation
                if state.focused_element == Some(FocusedElement::SendButton)
                    && (wparam as u8 == VK_RETURN || wparam as u8 == VK_SPACE)
                {
                    if !state.sessions.is_empty() {
                        let idx = state.active_session.min(state.sessions.len() - 1);
                        let text = state.sessions[idx].chat.input_buffer.trim().to_string();
                        if !text.is_empty() && !state.sessions[idx].chat.is_waiting_for_agent {
                            let text = state.sessions[idx].chat.take_input();
                            state.send_message(text);
                        }
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                if state.sessions.is_empty() {
                    return 0;
                }

                if let Some(chat) = state.active_chat() {
                    let result = input::handle_keydown(chat, wparam, lparam);
                    match result {
                        InputResult::Submit(text) => {
                            state.send_message(text);
                        }
                        InputResult::Cancel => {
                            state.cancel_turn();
                        }
                        InputResult::Cleared => {}
                        InputResult::Repaint => {}
                        InputResult::None => {}
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_UNICHAR => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                if wparam == VK_BACK as WPARAM
                    || wparam == VK_RETURN as WPARAM
                    || wparam == VK_ESCAPE as WPARAM
                {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                if !state.sessions.is_empty() {
                    if let Some(chat) = state.active_chat() {
                        let _ = input::handle_unichar(chat, wparam, lparam);
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 1;
        }
        WM_CHAR => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                if !state.sessions.is_empty() {
                    if let Some(chat) = state.active_chat() {
                        let _ = input::handle_char(chat, wparam);
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_LBUTTONDOWN => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                state.handle_click(x, y);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_LBUTTONUP => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                state.pressed_element = None;
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_MOUSEMOVE => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                // Request WM_MOUSELEAVE notification
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as DWORD,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut tme);

                let old_hover_item = state.hover_sidebar_item;
                let old_hover_new = state.hover_new_chat;
                let old_hover_new_proj = state.hover_new_project;
                let old_hover_proj = state.hover_project;
                let old_hover_settings = state.hover_settings_btn;
                let old_hover_close = state.hover_settings_close;
                let old_hover_thinking = state.hover_thinking_toggle;
                let old_hover_toggle = state.hover_settings_toggle;
                let old_hover_sb = state.hover_scrollbar;
                let old_hover_sb_sidebar = state.hover_sidebar_scrollbar;
                let old_hover_send = state.hover_send_btn;
                let old_hover_sh = state.hover_session_header;
                let old_hover_arch = state.hover_session_archive;
                let old_hover_del = state.hover_session_delete;
                state.update_hover(x, y);
                if state.hover_sidebar_item != old_hover_item
                    || state.hover_new_chat != old_hover_new
                    || state.hover_new_project != old_hover_new_proj
                    || state.hover_project != old_hover_proj
                    || state.hover_settings_btn != old_hover_settings
                    || state.hover_settings_close != old_hover_close
                    || state.hover_thinking_toggle != old_hover_thinking
                    || state.hover_settings_toggle != old_hover_toggle
                    || state.hover_scrollbar != old_hover_sb
                    || state.hover_sidebar_scrollbar != old_hover_sb_sidebar
                    || state.hover_send_btn != old_hover_send
                    || state.hover_session_header != old_hover_sh
                    || state.hover_session_archive != old_hover_arch
                    || state.hover_session_delete != old_hover_del
                {
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            return 0;
        }
        WM_MOUSELEAVE => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                state.clear_hover();
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_MOUSEWHEEL => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let delta = ((wparam >> 16) as i16) as i32;
                let state = &mut *app_state;
                if state.sessions.is_empty() {
                    return 0;
                }
                let scroll_amount = (delta / WHEEL_DELTA) * 40;
                let scale = state.dpi as f32 / 96.0;
                let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
                let x = (lparam & 0xFFFF) as i16 as i32;

                // Sidebar scroll
                if x < sidebar_w {
                    let pad = (SIDEBAR_PADDING as f32 * scale) as i32;
                    let footer_h = (SIDEBAR_FOOTER_HEIGHT as f32 * scale) as i32;
                    let (list_top, total_list_height, _) = state.sidebar_layout();
                    let list_bottom = state.client_height - footer_h - pad;
                    let avail_height = list_bottom - list_top;
                    let max_scroll = (total_list_height - avail_height).max(0);
                    if scroll_amount > 0 {
                        state.sidebar_scroll_offset =
                            (state.sidebar_scroll_offset - scroll_amount).max(0);
                    } else {
                        state.sidebar_scroll_offset =
                            (state.sidebar_scroll_offset + (-scroll_amount)).min(max_scroll);
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Message area scroll
                let viewport_top = state.messages_viewport_top();
                let viewport_bottom = state.messages_viewport_bottom();
                let viewport = viewport_bottom - viewport_top;

                if let Some(chat) = state.active_chat() {
                    if scroll_amount > 0 {
                        chat.scroll_up(scroll_amount, viewport);
                    } else {
                        chat.scroll_down(-scroll_amount, viewport);
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_SETCURSOR => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &*app_state;
                let scale = state.dpi as f32 / 96.0;
                let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                if x < sidebar_w
                    && (state.hover_new_chat
                        || state.hover_new_project
                        || state.hover_project.is_some()
                        || state.hover_sidebar_item.is_some()
                        || state.hover_settings_btn
                        || state.hover_sidebar_scrollbar
                        || state.hover_session_archive.is_some()
                        || state.hover_session_delete.is_some())
                {
                    let hcursor = LoadCursorW(std::ptr::null_mut(), IDC_HAND);
                    SetCursor(hcursor);
                    return 1;
                }

                if state.hover_thinking_toggle.is_some()
                    || state.hover_settings_toggle.is_some()
                    || state.hover_scrollbar
                    || state.hover_send_btn
                {
                    let hcursor = LoadCursorW(std::ptr::null_mut(), IDC_HAND);
                    SetCursor(hcursor);
                    return 1;
                }

                if state.show_settings {
                    let spad = (SPACE_5 as f32 * scale) as i32;
                    let close_size = (SPACE_5 as f32 * scale) as i32;
                    if x >= state.client_width - spad - close_size
                        && x <= state.client_width - spad
                        && y >= spad
                        && y <= spad + close_size
                    {
                        let hcursor = LoadCursorW(std::ptr::null_mut(), IDC_HAND);
                        SetCursor(hcursor);
                        return 1;
                    }
                }
            }
            let hcursor = LoadCursorW(std::ptr::null_mut(), IDC_IBEAM);
            SetCursor(hcursor);
            return 1;
        }
        WM_CLOSE => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                (*app_state).should_quit = true;
                if let Some(ref acp) = (*app_state).acp {
                    acp.shutdown();
                }
            }
            DestroyWindow(hwnd);
            return 0;
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ── Helpers ──────────────────────────────────────────────────

fn get_app_state(hwnd: HWND) -> *mut AppState {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState }
}

// ── Window creation ──────────────────────────────────────────

pub fn register_class(hinstance: HINSTANCE) -> bool {
    unsafe {
        let class_name = to_wide(WINDOW_CLASS_NAME);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: LoadIconW(std::ptr::null_mut(), IDI_APPLICATION),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        RegisterClassExW(&wc) != 0
    }
}

pub fn create_window(
    hinstance: HINSTANCE,
    app_state: *mut AppState,
    width: i32,
    height: i32,
) -> HWND {
    unsafe {
        let class_name = to_wide(WINDOW_CLASS_NAME);
        let title = to_wide(WINDOW_TITLE);

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, 0, 0);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            app_state as LPVOID,
        );

        if hwnd.is_null() {
            return std::ptr::null_mut();
        }

        // Enable dark title bar (immersive dark mode via DWM)
        {
            let use_dark: BOOL = 1;
            let hr = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &use_dark as *const BOOL as *const std::ffi::c_void,
                std::mem::size_of::<BOOL>() as DWORD,
            );
            if hr != 0 {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE_BEFORE_20H1,
                    &use_dark as *const BOOL as *const std::ffi::c_void,
                    std::mem::size_of::<BOOL>() as DWORD,
                );
            }
        }

        let dpi = GetDpiForWindow(hwnd);
        (*app_state).dpi = dpi;
        (*app_state).init_fonts();

        let mut client_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(hwnd, &mut client_rect);
        (*app_state).client_width = client_rect.width();
        (*app_state).client_height = client_rect.height();

        SetTimer(hwnd, TIMER_ID_REPAINT, TIMER_INTERVAL_MS, None);

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        hwnd
    }
}
