// Window events: WndProc message handling, window class registration, window creation.
#![allow(dead_code)]

use crate::app::{AppState, FontsHandle};
use crate::input::{self, InputResult, Key, Modifiers};
use crate::platform::windows::ffi::*;
use crate::platform::windows::fonts::Fonts;
use crate::platform::windows::paint::PaintContext;
use crate::platform::windows::render;
use crate::platform::windows::{clipboard::WinClipboard, dialogs::WinDialogs};
use crate::theme::*;

pub const WINDOW_CLASS_NAME: &str = "AcpChatWindow";
pub const WINDOW_TITLE: &str = "ACP Chat";
pub const TIMER_ID_REPAINT: UINT_PTR = 1;
pub const TIMER_INTERVAL_MS: UINT = 16; // ~60fps

/// Minimum window size (logical px, scaled by DPI)
pub const MIN_WINDOW_WIDTH: i32 = 720;
pub const MIN_WINDOW_HEIGHT: i32 = 480;

pub const GWLP_USERDATA: i32 = -21;

// ── Window procedure ─────────────────────────────────────────

pub unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Modal effects allow paint/timer re-entry, but not nested UI commands.
    let pointer = get_app_state(hwnd);
    if !pointer.is_null()
        && (*pointer).modal_active
        && matches!(
            msg,
            WM_KEYDOWN | WM_CHAR | WM_UNICHAR | WM_LBUTTONDOWN | WM_LBUTTONDBLCLK
        )
    {
        return 0;
    }
    match msg {
        WM_GETMINMAXINFO => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let scale = (*app_state).dpi as f32 / 96.0;
                // SAFETY: for WM_GETMINMAXINFO, `lparam` points to a valid
                // MINMAXINFO struct owned by the system for the duration of
                // this message; we only write the ptMinTrackSize field.
                let mmi = lparam as *mut MINMAXINFO;
                (*mmi).ptMinTrackSize.x = (MIN_WINDOW_WIDTH as f32 * scale) as i32;
                (*mmi).ptMinTrackSize.y = (MIN_WINDOW_HEIGHT as f32 * scale) as i32;
            }
            return 0;
        }
        WM_CREATE => {
            // SAFETY: on WM_CREATE, `lparam` points to the CREATESTRUCTW that
            // CreateWindowExW received; its lpCreateParams is the AppState
            // pointer we passed, valid for the lifetime of the window.
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
                (*app_state).invalidate_sidebar_cache();
                let fonts = Fonts::new(new_dpi);
                (*app_state).fonts = Some(FontsHandle::new(fonts));

                // SAFETY: for WM_DPICHANGED, `lparam` points to a valid RECT
                // owned by the system for the duration of this message.
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
                (*app_state).invalidate_sidebar_cache();

                // Save window size to state (only when not minimized and has real size)
                let state = &mut *app_state;
                if wparam != SIZE_MINIMIZED && state.client_width > 0 && state.client_height > 0 {
                    let mut rect = RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    GetWindowRect(hwnd, &mut rect);
                    state.saved_window = Some(crate::persistence::WindowState {
                        width: state.client_width,
                        height: state.client_height,
                        x: rect.left,
                        y: rect.top,
                    });
                }
            }
            return 0;
        }
        WM_MOVE => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                if let Some(ref mut w) = state.saved_window {
                    w.x = x;
                    w.y = y;
                } else if state.client_width > 0 && state.client_height > 0 {
                    state.saved_window = Some(crate::persistence::WindowState {
                        width: state.client_width,
                        height: state.client_height,
                        x,
                        y,
                    });
                }
            }
            return 0;
        }
        WM_TIMER => {
            if wparam == TIMER_ID_REPAINT {
                let app_state = get_app_state(hwnd);
                if !app_state.is_null() {
                    (*app_state).poll_acp();
                    (*app_state).poll_responses();
                    (*app_state).try_auto_reconnect();

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
                    if !(*app_state).modal_active {
                        let permission = (*app_state).pending_permissions.pop_front();
                        if let Some(permission) = permission {
                            execute_effect(hwnd, NativeEffect::Permission(permission));
                        }
                    }
                }
            }
            return 0;
        }
        WM_PAINT => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;

                if state.fonts.is_none() {
                    let fonts = Fonts::new(state.dpi);
                    state.fonts = Some(FontsHandle::new(fonts));
                }

                // A separate resource owner, not a reference into mutable AppState.
                let fonts_handle = match state.fonts.clone() {
                    Some(fonts) => fonts,
                    None => return 0,
                };
                let fonts_ref = match fonts_handle.downcast_ref::<Fonts>() {
                    Some(fonts) => fonts,
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
                    app: state,
                    fonts: fonts_ref,
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

                // Handle rename mode keys
                if state.rename_target.is_some() {
                    match wparam as u8 {
                        VK_RETURN => {
                            state.commit_rename();
                            InvalidateRect(hwnd, std::ptr::null(), 0);
                            return 0;
                        }
                        VK_ESCAPE => {
                            state.cancel_rename();
                            InvalidateRect(hwnd, std::ptr::null(), 0);
                            return 0;
                        }
                        VK_BACK => {
                            state.rename_backspace();
                            InvalidateRect(hwnd, std::ptr::null(), 0);
                            return 0;
                        }
                        VK_LEFT => {
                            state.rename_cursor_left();
                            InvalidateRect(hwnd, std::ptr::null(), 0);
                            return 0;
                        }
                        VK_RIGHT => {
                            state.rename_cursor_right();
                            InvalidateRect(hwnd, std::ptr::null(), 0);
                            return 0;
                        }
                        VK_HOME => {
                            if state.rename_target.is_some() {
                                state.rename_cursor = 0;
                                InvalidateRect(hwnd, std::ptr::null(), 0);
                            }
                            return 0;
                        }
                        VK_END => {
                            if state.rename_target.is_some() {
                                state.rename_cursor = state.rename_buffer.len();
                                InvalidateRect(hwnd, std::ptr::null(), 0);
                            }
                            return 0;
                        }
                        _ => {}
                    }
                    return 0;
                }

                // F5 to reconnect
                if wparam as u8 == VK_F5 {
                    state.connect_acp();
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Ctrl+N to open folder picker and create a new chat
                if wparam as u8 == VK_N
                    && (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0
                {
                    execute_effect(hwnd, NativeEffect::PickProjectFolder);
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Ctrl+Shift+A to toggle archive on the active session
                if wparam as u8 == b'A'
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
                if wparam as u8 == b'D'
                    && (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0
                    && (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0
                {
                    if !state.sessions.is_empty() {
                        let idx = state.active_session.min(state.sessions.len() - 1);
                        let effect = NativeEffect::ConfirmDelete {
                            local_id: state.sessions[idx].local_id.clone(),
                            name: state.sessions[idx].name.clone(),
                        };
                        execute_effect(hwnd, effect);
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

                // Enter/Space activates the focused element
                if (wparam as u8 == VK_RETURN || wparam as u8 == VK_SPACE)
                    && state.activate_focused_element()
                {
                    // Handle deferred platform requests from the activation
                    if state.new_project_requested {
                        state.new_project_requested = false;
                        execute_effect(hwnd, NativeEffect::PickProjectFolder);
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                if state.sessions.is_empty() {
                    return 0;
                }

                if let Some(chat) = state.active_chat() {
                    let key = wparam_to_key(wparam as u8);
                    let mods = Modifiers {
                        ctrl: (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0,
                        shift: (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0,
                    };
                    let clipboard = WinClipboard(hwnd);
                    let result = match key {
                        Some(Key::Char(b'C')) if mods.ctrl => {
                            if input::copy_selection(chat, &clipboard).is_err() {
                                state.show_toast(
                                    "Copy failed: clipboard unavailable",
                                    crate::ui::ToastKind::Error,
                                );
                            }
                            InputResult::None
                        }
                        Some(Key::Char(b'V')) if mods.ctrl => {
                            input::paste_from_clipboard(chat, &clipboard);
                            InputResult::Repaint
                        }
                        Some(Key::Char(b'X')) if mods.ctrl => {
                            if input::copy_selection(chat, &clipboard).is_ok() {
                                chat.delete_selection();
                            } else {
                                state.show_toast(
                                    "Cut failed: selection preserved",
                                    crate::ui::ToastKind::Error,
                                );
                            }
                            InputResult::Repaint
                        }
                        Some(k) => input::handle_key(chat, k, mods),
                        None => InputResult::None,
                    };
                    match result {
                        InputResult::Submit(text) => {
                            state.send_message(&text);
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
                if state.rename_target.is_some() {
                    if let Some(c) = char::from_u32(wparam as u32) {
                        state.rename_insert_char(c);
                    }
                } else if !state.sessions.is_empty() {
                    if let Some(chat) = state.active_chat() {
                        if let Some(c) = char::from_u32(wparam as u32) {
                            let _ = input::handle_char_input(chat, c);
                        }
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
                        if let Some(c) = char::from_u32(wparam as u32) {
                            if c >= ' ' && c != '\u{7F}' {
                                let _ = input::handle_char_input(chat, c);
                            }
                        }
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
                let effect = if let Some(url) = state.click_link(x, y) {
                    Some(NativeEffect::OpenLink(url))
                } else if let Some(code) = state.click_code_copy(x, y) {
                    Some(NativeEffect::CopyCode(code))
                } else {
                    state.handle_click(x, y);
                    if std::mem::take(&mut state.new_project_requested) {
                        Some(NativeEffect::PickProjectFolder)
                    } else {
                        state
                            .project_menu_requested
                            .take()
                            .and_then(|idx| state.projects.get(idx))
                            .map(|project| NativeEffect::ProjectMenu {
                                path: project.path.clone(),
                                name: project.name.clone(),
                                session_count: project.session_indices.len(),
                            })
                    }
                };
                // All application borrows end before a native modal call.
                if let Some(effect) = effect {
                    execute_effect(hwnd, effect);
                }

                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_LBUTTONDBLCLK => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                state.handle_double_click(x, y);
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

                let old_hover = state.hover.clone();
                state.update_hover(x, y);
                state.update_link_hover(x, y);
                if state.hover != old_hover {
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
                    let client_height = state.client_height;
                    let max_scroll = state.with_sidebar_layout(|list_top, total_list_height, _| {
                        let list_bottom = client_height - footer_h - pad;
                        let avail_height = list_bottom - list_top;
                        (total_list_height - avail_height).max(0)
                    });
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
                    && (state.hover.new_chat
                        || state.hover.new_project
                        || state.hover.project.is_some()
                        || state.hover.sidebar_item.is_some()
                        || state.hover.settings_btn
                        || state.hover.sidebar_scrollbar
                        || state.hover.session_archive.is_some()
                        || state.hover.session_delete.is_some())
                {
                    let hcursor = LoadCursorW(std::ptr::null_mut(), IDC_HAND);
                    SetCursor(hcursor);
                    return 1;
                }

                if state.hover.thinking_toggle.is_some()
                    || state.hover.settings_toggle.is_some()
                    || state.hover.scrollbar
                    || state.hover.send_btn
                    || state.hover.link.is_some()
                    || state.hover.code_copy.is_some()
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
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ── Helpers ──────────────────────────────────────────────────

fn get_app_state(hwnd: HWND) -> *mut AppState {
    // SAFETY: GWLP_USERDATA was set in WM_CREATE to the `*mut AppState` passed
    // via CREATESTRUCTW::lpCreateParams, and the AppState outlives the window
    // (it is owned by the message loop in `app_entry`). The pointer is only
    // dereferenced on the UI thread that owns the window.
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState }
}

/// Owned data only: no application borrow may cross execution of an effect.
enum NativeEffect {
    PickProjectFolder,
    ConfirmDelete {
        local_id: String,
        name: String,
    },
    ProjectMenu {
        path: String,
        name: String,
        session_count: usize,
    },
    OpenLink(String),
    CopyCode(String),
    Permission(crate::app::PendingPermission),
}

fn execute_effect(hwnd: HWND, effect: NativeEffect) {
    use crate::platform::backend::{Clipboard, Dialogs};
    // SAFETY: UI-thread-only pointer is resolved anew after every modal call.
    // The boxed application outlives window destruction and nested dispatch.
    unsafe {
        let pointer = get_app_state(hwnd);
        if pointer.is_null() || (*pointer).modal_active {
            return;
        }
        (*pointer).modal_active = true;
        let dialogs = WinDialogs(hwnd);
        match effect {
            NativeEffect::Permission(permission) => {
                let menu = CreatePopupMenu();
                let mut selected = None;
                if !menu.is_null() {
                    AppendMenuW(
                        menu,
                        MF_STRING | 0x0002,
                        0,
                        to_wide(&permission.title).as_ptr(),
                    );
                    for (index, (_, label)) in permission.options.iter().enumerate() {
                        AppendMenuW(menu, MF_STRING, index + 1, to_wide(label).as_ptr());
                    }
                    AppendMenuW(
                        menu,
                        MF_STRING,
                        permission.options.len() + 1,
                        to_wide("Cancel request").as_ptr(),
                    );
                    let mut point = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut point);
                    let command = TrackPopupMenu(
                        menu,
                        TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        std::ptr::null(),
                    ) as usize;
                    DestroyMenu(menu);
                    selected = command
                        .checked_sub(1)
                        .and_then(|index| permission.options.get(index))
                        .map(|(id, _)| id.clone());
                }
                let pointer = get_app_state(hwnd);
                if !pointer.is_null() {
                    (*pointer).resolve_permission(permission, selected);
                }
            }
            NativeEffect::PickProjectFolder => {
                let result = dialogs.pick_folder("Select a project folder");
                let pointer = get_app_state(hwnd);
                if !pointer.is_null() {
                    if let Some(path) = result {
                        (*pointer).new_chat_in_dir(&path);
                    }
                }
            }
            NativeEffect::ConfirmDelete { local_id, name } => {
                let confirmed = dialogs.confirm(
                    "Delete Session",
                    &format!("Delete session \"{name}\"? This cannot be undone."),
                );
                let pointer = get_app_state(hwnd);
                if confirmed && !pointer.is_null() {
                    let state = &mut *pointer;
                    if let Some(idx) = state.sessions.iter().position(|s| s.local_id == local_id) {
                        state.delete_session(idx);
                    }
                }
            }
            NativeEffect::ProjectMenu {
                path,
                name,
                session_count,
            } => {
                let cmd = show_project_menu(hwnd);
                let confirmed = cmd == PROJECT_MENU_DELETE && dialogs.confirm("Delete Project",
                    &format!("Delete project \"{name}\" and its {session_count} chat(s)? Workspace files are not deleted. This cannot be undone."));
                let pointer = get_app_state(hwnd);
                if !pointer.is_null() {
                    let state = &mut *pointer;
                    if let Some(idx) = state.projects.iter().position(|p| p.path == path) {
                        if cmd == PROJECT_MENU_RENAME {
                            state.start_rename_project(idx);
                        } else if confirmed {
                            state.remove_project(idx);
                        }
                    }
                }
            }
            NativeEffect::OpenLink(url) => {
                let allowed = !url.contains('\0')
                    && (url.starts_with("https://") || url.starts_with("http://"));
                let result = if allowed {
                    ShellExecuteW(
                        hwnd,
                        to_wide("open").as_ptr(),
                        to_wide(&url).as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        SW_SHOW,
                    ) as isize
                } else {
                    0
                };
                let pointer = get_app_state(hwnd);
                if result <= 32 && !pointer.is_null() {
                    (*pointer).show_toast("Unable to open link", crate::ui::ToastKind::Error);
                }
            }
            NativeEffect::CopyCode(code) => {
                let result = WinClipboard(hwnd).set_text(&code);
                let pointer = get_app_state(hwnd);
                if !pointer.is_null() {
                    match result {
                        Ok(()) => {
                            (*pointer).show_toast("Code copied", crate::ui::ToastKind::Success)
                        }
                        Err(e) => (*pointer).show_toast(&e, crate::ui::ToastKind::Error),
                    }
                }
            }
        }
        let pointer = get_app_state(hwnd);
        if !pointer.is_null() {
            (*pointer).modal_active = false;
        }
    }
}

const PROJECT_MENU_RENAME: usize = 1;
const PROJECT_MENU_DELETE: usize = 2;

/// Show the project context menu (Rename / Delete) at the cursor position.
fn show_project_menu(hwnd: HWND) -> usize {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return 0;
        }
        let rename_wide = to_wide("Rename project");
        let delete_wide = to_wide("Delete project");
        AppendMenuW(menu, MF_STRING, PROJECT_MENU_RENAME, rename_wide.as_ptr());
        AppendMenuW(menu, MF_STRING, PROJECT_MENU_DELETE, delete_wide.as_ptr());

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);

        cmd as usize
    }
}

/// Translate a Win32 virtual key code to the platform-independent Key enum.
fn wparam_to_key(vk: u8) -> Option<Key> {
    match vk {
        VK_RETURN => Some(Key::Return),
        VK_BACK => Some(Key::Backspace),
        VK_DELETE => Some(Key::Delete),
        VK_LEFT => Some(Key::Left),
        VK_RIGHT => Some(Key::Right),
        VK_UP => Some(Key::Up),
        VK_DOWN => Some(Key::Down),
        VK_HOME => Some(Key::Home),
        VK_END => Some(Key::End),
        VK_ESCAPE => Some(Key::Escape),
        VK_TAB => Some(Key::Tab),
        VK_F5 => Some(Key::F5),
        c if c.is_ascii_alphabetic() => Some(Key::Char(c.to_ascii_uppercase())),
        c if c.is_ascii_digit() => Some(Key::Char(c)),
        _ => None,
    }
}

// ── Window creation ──────────────────────────────────────────

pub fn register_class(hinstance: HINSTANCE) -> bool {
    unsafe {
        let class_name = to_wide(WINDOW_CLASS_NAME);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
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

        // Use saved window position if available
        let (pos_x, pos_y) = if let Some(ref w) = (*app_state).saved_window {
            (w.x, w.y)
        } else {
            (CW_USEDEFAULT, CW_USEDEFAULT)
        };

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            pos_x,
            pos_y,
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
        let fonts = Fonts::new(dpi);
        (*app_state).fonts = Some(FontsHandle::new(fonts));

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
