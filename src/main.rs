// vibe-chat - Cross-platform ACP Chat Client
// Entry point: dispatches to the platform backend.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod acp;
mod app;
mod chat;
mod input;
mod interaction;
mod json;
mod markdown;
mod platform;
mod session;
mod text;
mod theme;
mod ui;

fn main() {
    platform::run();
}

/// Application entry point called by the platform backend after initialization.
/// On Windows this is called from platform::windows::run().
#[cfg(windows)]
pub fn app_entry() {
    use crate::app::{AppState, FontsHandle};
    use crate::platform::windows::{ffi::*, fonts::Fonts, window};
    use std::ptr;

    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

        let hinstance = GetModuleHandleW(ptr::null());

        if !window::register_class(hinstance) {
            MessageBoxW(
                ptr::null_mut(),
                to_wide("Failed to register window class").as_ptr(),
                to_wide("Error").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        let caret_blink = GetCaretBlinkTime();
        let mut app_state = Box::new(AppState::new(caret_blink));

        app_state.as_mut().create_local_session_default();

        let hwnd = window::create_window(hinstance, &mut *app_state, 1100, 700);

        if hwnd.is_null() {
            MessageBoxW(
                ptr::null_mut(),
                to_wide("Failed to create window").as_ptr(),
                to_wide("Error").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        // Initialize fonts and store them in the platform-agnostic handle
        let fonts = Fonts::new(app_state.dpi);
        app_state.fonts = Some(FontsHandle::new(fonts));

        app_state.as_mut().connect_acp();

        app_state.as_mut().sessions[0]
            .chat
            .add_system_message("ACP Chat started. Connecting to agent...");

        let mut msg = MSG {
            hwnd: ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT { x: 0, y: 0 },
        };

        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);

            let state_ptr = GetWindowLongPtrW(msg.hwnd, -21) as *mut AppState;
            if !state_ptr.is_null() && (*state_ptr).should_quit {
                break;
            }
        }

        if let Some(a) = app_state.acp.take() {
            a.shutdown();
        }
    }
}
