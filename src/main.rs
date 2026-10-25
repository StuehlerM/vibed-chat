// ACP Chat — Barebone Windows ACP Client
// Entry point: register window class, create window, run message loop.

#![windows_subsystem = "windows"]

mod acp;
mod app;
mod chat;
mod events;
mod fonts;
mod input;
mod interaction;
mod json;
mod markdown;
mod render;
mod session;
mod text;
mod theme;
mod ui;
mod win32;

use std::ptr;
use win32::*;
use app::AppState;

fn main() {
    unsafe {
        // Set DPI awareness
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

        // Get module handle
        let hinstance = GetModuleHandleW(ptr::null());

        // Register window class
        if !events::register_class(hinstance) {
            MessageBoxW(
                ptr::null_mut(),
                to_wide("Failed to register window class").as_ptr(),
                to_wide("Error").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        // Create application state
        let mut app_state = Box::new(AppState::new());

        // Create a placeholder session in the default working directory
        app_state.as_mut().create_local_session_default();

        // Create window (larger default for sidebar layout)
        let hwnd = events::create_window(hinstance, &mut *app_state, 1100, 700);

        if hwnd.is_null() {
            MessageBoxW(
                ptr::null_mut(),
                to_wide("Failed to create window").as_ptr(),
                to_wide("Error").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        // Auto-connect to agent
        app_state.as_mut().connect_acp();

        // Add welcome message
        app_state
            .as_mut()
            .sessions[0]
            .chat
            .add_system_message("ACP Chat started. Connecting to agent...");

        // Message loop
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

            // Check if we should quit
            let state_ptr = GetWindowLongPtrW(msg.hwnd, -21) as *mut AppState;
            if !state_ptr.is_null() && (*state_ptr).should_quit {
                break;
            }
        }

        // Cleanup
        if let Some(a) = app_state.acp.take() {
            a.shutdown();
        }
    }
}
