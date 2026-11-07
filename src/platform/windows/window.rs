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
pub const WINDOW_TITLE: &str = "vibe-chat";
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
    // Native synchronization can dispatch messages. Never keep an application
    // borrow across it; the dispatcher and each sync resolve HWND state anew.
    if matches!(msg, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK) {
        super::native::checkpoint_selection(hwnd);
    }
    if msg == WM_PAINT {
        super::native::sync(hwnd);
    }
    let result = dispatch(hwnd, msg, wparam, lparam);
    if matches!(
        msg,
        WM_COMMAND | WM_KEYDOWN | WM_LBUTTONUP | WM_SETFOCUS | WM_SIZE | WM_DPICHANGED
    ) {
        super::native::sync(hwnd);
    }
    result
}

unsafe fn dispatch(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Modal effects allow paint/timer re-entry, but not nested UI commands.
    let pointer = get_app_state(hwnd);
    if !pointer.is_null()
        && (*pointer).modal_active
        && matches!(
            msg,
            WM_KEYDOWN | WM_CHAR | WM_UNICHAR | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK
        )
    {
        return 0;
    }
    match msg {
        WM_DRAWITEM => {
            if super::native::draw_button(lparam) {
                return 1;
            }
        }
        WM_NOTIFY => {
            // RichEdit supplies its actual formatted text height.
            if super::native::notify(wparam, lparam) {
                return 0;
            }
        }
        WM_COMMAND => {
            if super::native::command(hwnd, wparam, lparam) {
                return 0;
            }
        }
        WM_VSCROLL => {
            if super::native::scroll(hwnd, wparam, lparam) {
                return 0;
            }
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            if let Some(brush) = super::native::control_color(wparam as HDC) {
                return brush as LRESULT;
            }
        }
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
            cancel_pointer_capture(hwnd);
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
            cancel_pointer_capture(hwnd);
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                (*app_state).window_minimized = wparam == SIZE_MINIMIZED;
                (*app_state).client_width = (lparam & 0xFFFF) as i32;
                (*app_state).client_height = ((lparam >> 16) & 0xFFFF) as i32;
                (*app_state).invalidate_sidebar_cache();
                (*app_state).settings_scroll = (*app_state)
                    .settings_scroll
                    .min((*app_state).settings_max_scroll());

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
                    if !(*app_state).native_editor_active
                        && now
                            .duration_since((*app_state).last_cursor_blink)
                            .as_millis()
                            > blink_ms
                    {
                        (*app_state).cursor_visible = !(*app_state).cursor_visible;
                        (*app_state).last_cursor_blink = now;
                        if (*app_state).window_focused
                            && matches!(
                                (*app_state).focused_element,
                                Some(
                                    crate::ui::FocusedElement::Input
                                        | crate::ui::FocusedElement::Rename
                                )
                            )
                        {
                            (*app_state).repaint_needed = true;
                        }
                    }
                    if (*app_state)
                        .sessions
                        .iter()
                        .any(|s| s.active_turn.is_some())
                    {
                        (*app_state).streaming_dots = (*app_state).streaming_dots.wrapping_add(1);
                        if (*app_state).streaming_dots % 8 == 0 {
                            (*app_state).repaint_needed = true;
                        }
                    }
                    if (*app_state).toast.as_ref().is_some_and(|toast| {
                        toast.shown_at.elapsed().as_millis() >= u128::from(toast.duration_ms)
                    }) {
                        (*app_state).toast = None;
                        (*app_state).repaint_needed = true;
                    }
                    if (*app_state).repaint_needed && !(*app_state).window_minimized {
                        (*app_state).repaint_needed = false;
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
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
            let paint = WindowPaint::begin(hwnd);
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

                let mut ctx = PaintContext {
                    app: state,
                    fonts: fonts_ref,
                };
                render::paint(paint.state.hdc, &mut ctx);
            }
            return 0;
        }
        WM_ERASEBKGND => {
            return 1;
        }
        WM_KEYDOWN => {
            cancel_pointer_capture(hwnd);
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
                        VK_TAB => {
                            state.cancel_rename();
                            state.cycle_focus(
                                (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0,
                            );
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
                        if let Some(effect) =
                            delete_session_effect(state, &state.sessions[idx].local_id)
                        {
                            execute_effect(hwnd, effect);
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

                // Escape closes transient panels before cancelling a turn.
                if wparam as u8 == VK_ESCAPE && (state.show_settings || state.sidebar_drawer_open) {
                    if state.show_settings {
                        state.show_settings = false;
                        state.focused_element = Some(crate::ui::FocusedElement::SettingsBtn);
                    } else {
                        state.sidebar_drawer_open = false;
                        state.focused_element = Some(crate::ui::FocusedElement::History);
                    }
                    state.invalidate_sidebar_cache();
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                // Enter used to commit an IME composition must not submit a turn.
                if wparam as u8 == VK_RETURN && state.ime_composing {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                // Consume the WM_CHAR queued by TranslateMessage for activation.
                if (wparam as u8 == VK_RETURN || wparam as u8 == VK_SPACE)
                    && state.activate_focused_element()
                {
                    state.consumed_character = Some(wparam as u32);
                    // Handle deferred platform requests from the activation
                    let effect = take_requested_effect(state);
                    if let Some(effect) = effect {
                        execute_effect(hwnd, effect);
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }

                if state.sessions.is_empty()
                    || state.focused_element != Some(crate::ui::FocusedElement::Input)
                {
                    return 0;
                }

                if let Some(chat) = state.active_chat() {
                    let key = wparam_to_key(wparam as u8);
                    let mods = Modifiers {
                        ctrl: (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0,
                        shift: (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0,
                    };
                    let clipboard = WinClipboard(hwnd);
                    let previous_draft = chat.input_buffer.clone();
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
                    if state
                        .sessions
                        .get(state.active_session)
                        .is_some_and(|s| s.chat.input_buffer != previous_draft)
                    {
                        state.draft_save_due = Some(std::time::Instant::now());
                    }
                    match result {
                        InputResult::Submit(text) => {
                            state.send_message(&text);
                        }
                        InputResult::Cancel => {
                            state.cancel_turn();
                        }
                        InputResult::Repaint => {}
                        InputResult::None => {}
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            if msg == WM_KILLFOCUS {
                cancel_pointer_capture(hwnd);
            }
            let pointer = get_app_state(hwnd);
            if !pointer.is_null() {
                (*pointer).window_focused =
                    msg == WM_SETFOCUS || IsChild(hwnd, wparam as HWND) != 0;
                (*pointer).pending_high_surrogate = None;
                (*pointer).cursor_visible = true;
                (*pointer).last_cursor_blink = std::time::Instant::now();
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_IME_STARTCOMPOSITION | WM_IME_ENDCOMPOSITION => {
            let pointer = get_app_state(hwnd);
            if !pointer.is_null() {
                (*pointer).ime_composing = msg == WM_IME_STARTCOMPOSITION;
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        WM_UNICHAR | WM_CHAR => {
            if msg == WM_UNICHAR && wparam == 0xffff {
                return 1;
            } // UNICODE_NOCHAR probe
            let pointer = get_app_state(hwnd);
            if !pointer.is_null() {
                let state = &mut *pointer;
                if state.consumed_character.take() == Some(wparam as u32) {
                    return 0;
                }
                let scalar = if msg == WM_UNICHAR {
                    state.pending_high_surrogate = None;
                    u32::try_from(wparam).ok()
                } else {
                    let unit = wparam as u16;
                    if (0xd800..=0xdbff).contains(&unit) {
                        state.pending_high_surrogate = Some(unit);
                        return 0;
                    }
                    let high = state.pending_high_surrogate.take();
                    if (0xdc00..=0xdfff).contains(&unit) {
                        high.map(|high| {
                            0x10000 + ((u32::from(high) - 0xd800) << 10) + u32::from(unit) - 0xdc00
                        })
                    } else {
                        Some(u32::from(unit))
                    }
                };
                if let Some(c) = scalar.and_then(char::from_u32) {
                    route_character(state, c);
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return if msg == WM_UNICHAR { 1 } else { 0 };
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            cancel_pointer_capture(hwnd);
            let pointer = get_app_state(hwnd);
            if !pointer.is_null() {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let capture = (*pointer).begin_pointer_press(x, y, msg == WM_LBUTTONDBLCLK);
                // SetCapture can dispatch messages. No application borrow crosses it.
                if capture {
                    SetCapture(hwnd);
                    if GetCapture() != hwnd {
                        let pointer = get_app_state(hwnd);
                        if !pointer.is_null() {
                            (*pointer).cancel_pointer_press();
                        }
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_LBUTTONUP => {
            let pointer = get_app_state(hwnd);
            let effect = if !pointer.is_null() && GetCapture() == hwnd {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let state = &mut *pointer;
                let effect = state.finish_pointer_press(x, y).and_then(|effect| {
                    use crate::interaction::PointerEffect;
                    match effect {
                        PointerEffect::DeleteSession(id) => delete_session_effect(state, &id),
                        PointerEffect::OpenLink(url) => Some(NativeEffect::OpenLink(url)),
                        PointerEffect::CopyCode(code) => Some(NativeEffect::CopyCode(code)),
                    }
                });
                effect.or_else(|| take_requested_effect(state))
            } else {
                None
            };
            // ReleaseCapture may re-enter WndProc; intent/effects are owned here.
            cancel_pointer_capture(hwnd);
            if let Some(effect) = effect {
                execute_effect(hwnd, effect);
            }
            InvalidateRect(hwnd, std::ptr::null(), 0);
            return 0;
        }
        WM_CAPTURECHANGED => {
            let pointer = get_app_state(hwnd);
            if !pointer.is_null() {
                (*pointer).cancel_pointer_press();
            }
            InvalidateRect(hwnd, std::ptr::null(), 0);
            return 0;
        }
        WM_CANCELMODE => {
            cancel_pointer_capture(hwnd);
            InvalidateRect(hwnd, std::ptr::null(), 0);
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
                let old_pressed = state.pressed_element;
                state.update_pointer_press(x, y);
                state.update_hover(x, y);
                state.update_link_hover(x, y);
                if state.hover != old_hover || state.pressed_element != old_pressed {
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            return 0;
        }
        WM_MOUSELEAVE => {
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &mut *app_state;
                state.cancel_pointer_press();
                state.clear_hover();
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_MOUSEWHEEL => {
            cancel_pointer_capture(hwnd);
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let delta = ((wparam >> 16) as i16) as i32;
                let mut point = POINT {
                    x: (lparam & 0xffff) as i16 as i32,
                    y: ((lparam >> 16) & 0xffff) as i16 as i32,
                };
                if ScreenToClient(hwnd, &mut point) == 0 {
                    return 0;
                }
                let state = &mut *app_state;
                state.link_rects.clear();
                state.code_block_rects.clear();
                if state.sessions.is_empty() {
                    return 0;
                }
                state.wheel_remainder += delta;
                let scale = state.dpi as f32 / 96.0;
                let scroll_amount =
                    ((state.wheel_remainder as f32 / WHEEL_DELTA as f32) * 40.0 * scale) as i32;
                if scroll_amount == 0 {
                    return 0;
                }
                state.wheel_remainder = 0;
                let x = point.x;

                if state.show_settings && state.layout().settings.contains(point.x, point.y) {
                    state.scroll_settings(-scroll_amount);
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }
                let sidebar_w = state.layout().sidebar.width();
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
            // Let native child classes choose their own cursor (especially the
            // RichEdit I-beam); a parent arrow must not override editor behavior.
            if wparam as HWND != hwnd && IsChild(hwnd, wparam as HWND) != 0 {
                return 0;
            }
            if (lparam & 0xffff) != 1 {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) == 0 || ScreenToClient(hwnd, &mut point) == 0 {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let app_state = get_app_state(hwnd);
            if !app_state.is_null() {
                let state = &*app_state;
                let scale = state.dpi as f32 / 96.0;
                let sidebar_w = (SIDEBAR_WIDTH as f32 * scale) as i32;
                let x = point.x;
                let y = point.y;

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
            let is_input = !app_state.is_null()
                && !(*app_state).native_editor_active
                && (*app_state).layout().content_visible
                && point.x >= (*app_state).chat_left()
                && point.x < (*app_state).chat_right()
                && point.y >= (*app_state).input_box_top();
            let hcursor = LoadCursorW(
                std::ptr::null_mut(),
                if is_input { IDC_IBEAM } else { IDC_ARROW },
            );
            SetCursor(hcursor);
            return 1;
        }
        WM_CLOSE => {
            cancel_pointer_capture(hwnd);
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
            super::native::destroy();
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            render::release_backbuffer();
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ── Helpers ──────────────────────────────────────────────────

/// Cancel state before releasing native capture, which may synchronously re-enter.
fn cancel_pointer_capture(hwnd: HWND) {
    // SAFETY: this helper runs on the owning UI thread. No AppState reference
    // survives ReleaseCapture, and state is always resolved from the live HWND.
    unsafe {
        let pointer = get_app_state(hwnd);
        if !pointer.is_null() {
            (*pointer).cancel_pointer_press();
        }
        if GetCapture() == hwnd {
            ReleaseCapture();
        }
    }
}

fn delete_session_effect(state: &AppState, local_id: &str) -> Option<NativeEffect> {
    state
        .sessions
        .iter()
        .find(|session| session.local_id == local_id)
        .map(|session| NativeEffect::ConfirmDelete {
            local_id: session.local_id.clone(),
            name: session.name.clone(),
        })
}

struct WindowPaint {
    hwnd: HWND,
    state: PAINTSTRUCT,
}
impl WindowPaint {
    fn begin(hwnd: HWND) -> Self {
        // SAFETY: PAINTSTRUCT contains only integers, pointers and byte arrays;
        // BeginPaint initializes it, and the guard always pairs EndPaint.
        unsafe {
            let mut state: PAINTSTRUCT = std::mem::zeroed();
            BeginPaint(hwnd, &mut state);
            Self { hwnd, state }
        }
    }
}
impl Drop for WindowPaint {
    fn drop(&mut self) {
        // SAFETY: this is the one guard corresponding to BeginPaint above.
        unsafe {
            EndPaint(self.hwnd, &self.state);
        }
    }
}

fn route_character(state: &mut AppState, c: char) {
    if !state.window_focused || c.is_control() {
        return;
    }
    if state.rename_target.is_some() {
        state.rename_insert_char(c);
    } else if state.focused_element == Some(crate::ui::FocusedElement::Input) {
        if let Some(chat) = state.active_chat() {
            let _ = input::handle_char_input(chat, c);
        }
    }
    state.draft_save_due = Some(std::time::Instant::now());
    state.cursor_visible = true;
    state.last_cursor_blink = std::time::Instant::now();
}

pub(super) fn get_app_state(hwnd: HWND) -> *mut AppState {
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
    SetupHelp {
        mode: crate::session::SessionMode,
        storage: String,
    },
    ModeChange {
        local_id: String,
        mode: crate::session::SessionMode,
    },
    Permission(crate::app::PendingPermission),
}

pub(super) fn run_requested_effect(hwnd: HWND) {
    let pointer = get_app_state(hwnd);
    // SAFETY: UI-thread-only, effect owns all data before any native operation.
    let effect = if pointer.is_null() {
        None
    } else {
        unsafe { take_requested_effect(&mut *pointer) }
    };
    if let Some(effect) = effect {
        execute_effect(hwnd, effect);
    }
}

fn take_requested_effect(state: &mut AppState) -> Option<NativeEffect> {
    if let Some(mode) = state.setup_help_requested.take() {
        return Some(NativeEffect::SetupHelp {
            mode,
            storage: state.config_dir.clone(),
        });
    }
    if let Some((local_id, mode)) = state.mode_change_requested.take() {
        return Some(NativeEffect::ModeChange { local_id, mode });
    }
    if std::mem::take(&mut state.new_project_requested) {
        return Some(NativeEffect::PickProjectFolder);
    }
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

fn execute_effect(hwnd: HWND, effect: NativeEffect) {
    use crate::platform::backend::{Clipboard, Dialogs};
    // SAFETY: UI-thread-only pointer is resolved anew after every modal call.
    // The boxed application outlives window destruction and nested dispatch.
    unsafe {
        cancel_pointer_capture(hwnd);
        let pointer = get_app_state(hwnd);
        if pointer.is_null() || (*pointer).modal_active {
            return;
        }
        (*pointer).modal_active = true;
        let dialogs = WinDialogs(hwnd);
        match effect {
            NativeEffect::SetupHelp { mode, storage } => {
                let title = if mode == crate::session::SessionMode::Direct {
                    "Set up Chat"
                } else {
                    "Set up Code"
                };
                let help = format!("Chat: chat with Mistral using the Responses API. No project or agent is required.\n\nSet API_KEY in the environment before launching vibe-chat. MISTRAL_API_KEY is used only when API_KEY is absent or blank. Restart the app after changing environment variables. The app reports local key presence, not provider verification. Never paste the key into a conversation.\n\nCode: install vibe-acp, put its executable on PATH, and restart vibe-chat. The agent inherits your environment; use the credentials its documentation requires. Tool permissions are always requested explicitly.\n\nFollow new replies applies only while you are at the bottom. Escape never clears a draft.\n\nPrivacy: histories, drafts, reasoning and tool output are unencrypted JSON in:\n{storage}\nProtect this directory and keep secrets out of prompts/tool output. Chat sends user/assistant history to api.mistral.ai; tool/system text and displayed reasoning are excluded.");
                MessageBoxW(
                    hwnd,
                    to_wide(&help).as_ptr(),
                    to_wide(title).as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            NativeEffect::ModeChange { local_id, mode } => {
                let policy = match mode {
                    crate::session::SessionMode::Direct => "Direct uploads this chat's user and assistant text to api.mistral.ai, including earlier Code assistant text. Tool/system activity and displayed thinking are excluded. No tool summary is supplied.",
                    crate::session::SessionMode::Code => "Code uses the agent's context. A new/reconnected agent session does not receive this Direct transcript. Earlier Direct messages are not automatically transferred.",
                };
                let accepted = MessageBoxW(hwnd, to_wide(&format!("{policy}\n\nChange the backend for future turns? A running turn is unchanged.")).as_ptr(),
                    to_wide("Change chat backend").as_ptr(), MB_YESNO | MB_ICONWARNING | 0x100) == IDYES as i32;
                let pointer = get_app_state(hwnd);
                if accepted && !pointer.is_null() {
                    (*pointer).apply_session_mode(&local_id, mode);
                }
            }
            NativeEffect::Permission(permission) => {
                // Details are owned, so both nested modal loops can repaint and
                // settle/disconnect the original turn without aliasing AppState.
                let details: String = permission.details.chars().take(8000).collect();
                let review = MessageBoxW(hwnd,
                    to_wide(&format!("{}\n\n{details}\n\nReview offered permission choices? No cancels this request.", permission.title)).as_ptr(),
                    to_wide("Tool permission required").as_ptr(), MB_YESNO | MB_ICONWARNING | 0x100) == IDYES as i32;
                let menu = if review {
                    CreatePopupMenu()
                } else {
                    std::ptr::null_mut()
                };
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
                        (*pointer).sidebar_drawer_open = false;
                        (*pointer).focused_element = Some(crate::ui::FocusedElement::Input);
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
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
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

        if !super::native::create(hwnd) {
            (*app_state).show_toast(
                "Native Unicode editor unavailable; using basic input",
                crate::ui::ToastKind::Error,
            );
        }
        SetTimer(hwnd, TIMER_ID_REPAINT, TIMER_INTERVAL_MS, None);

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        hwnd
    }
}
