// vibe-chat - Cross-platform ACP Chat Client
// Entry point: dispatches to the platform backend.

#![cfg_attr(windows, windows_subsystem = "windows")]
// Every DPI-scaled pixel value is computed as `(base as f32 * scale) as i32`
// (see AGENTS.md). These casts are intentional and saturate at the f32->i32
// boundary; allowing them crate-wide is clearer than 160+ inline allows.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod acp;
mod app;
mod chat;
mod http;
mod input;
mod interaction;
mod json;
mod layout;
mod markdown;
mod persistence;
mod platform;
mod responses;
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
    use crate::app::AppState;
    use crate::platform::windows::{ffi::*, window};
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

        // OLE is UI-thread-affine and must be balanced, including early returns.
        struct OleGuard;
        impl Drop for OleGuard {
            fn drop(&mut self) {
                // SAFETY: created only after successful initialization on this thread.
                unsafe {
                    OleUninitialize();
                }
            }
        }
        let ole_result = OleInitialize(ptr::null_mut());
        let _ole = if ole_result >= 0 {
            Some(OleGuard)
        } else {
            None
        };
        if ole_result < 0 {
            MessageBoxW(
                ptr::null_mut(),
                to_wide("Failed to initialize OLE").as_ptr(),
                to_wide("Error").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }
        let caret_blink = GetCaretBlinkTime();
        let mut app_state = Box::new(AppState::new(caret_blink));
        app_state.configure_agent_process = |command| {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        };

        app_state.load_state();
        if app_state.persistence_blocked {
            let error = app_state.persistence_error.clone().unwrap_or_default();
            let use_backup = MessageBoxW(ptr::null_mut(),
                to_wide(&format!("{error}\n\nOriginal files will not be overwritten. Open the last saved backup read-only?\nStorage: {}", app_state.config_dir)).as_ptr(),
                to_wide("History recovery required").as_ptr(), MB_YESNO | MB_ICONWARNING) == IDYES as i32;
            if use_backup {
                app_state.load_backup();
            }
        }
        if app_state.sessions.is_empty() {
            app_state.create_local_session_default();
        }

        let (win_w, win_h) = if let Some(ref w) = app_state.saved_window {
            (w.width.max(720), w.height.max(480))
        } else {
            (1100, 700)
        };
        let hwnd = window::create_window(hinstance, &mut *app_state, win_w, win_h);

        if hwnd.is_null() {
            MessageBoxW(
                ptr::null_mut(),
                to_wide("Failed to create window").as_ptr(),
                to_wide("Error").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        // Direct mode is the default backend; the agent is spawned lazily on
        // the first Code-mode turn (or Code session). Install the WinHTTP
        // transport for the Responses API client.
        app_state.as_mut().install_responses_transport(Box::new(
            crate::platform::windows::http::WinHttpTransport::new(),
        ));

        let mut msg = MSG {
            hwnd: ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT { x: 0, y: 0 },
        };

        loop {
            let status = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if status == 0 {
                break;
            }
            if status == -1 {
                let error = std::io::Error::last_os_error();
                MessageBoxW(
                    hwnd,
                    to_wide(&format!("GetMessageW: {error}")).as_ptr(),
                    to_wide("Window message loop failed").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);

            let state_ptr = GetWindowLongPtrW(hwnd, window::GWLP_USERDATA) as *mut AppState;
            if !state_ptr.is_null() && (*state_ptr).should_quit {
                break;
            }
        }

        // Also retire native callbacks when GetMessage failed before WM_CLOSE.
        if GetWindowLongPtrW(hwnd, window::GWLP_USERDATA) != 0 {
            DestroyWindow(hwnd);
        }
        crate::platform::windows::native::finish_resources();
        // Cancel independently of queue order, release backpressure, and wait
        // only after native callbacks can no longer borrow application state.
        let direct_shutdown = app_state
            .responses
            .take()
            .map(|client| client.finish(std::time::Duration::from_secs(2)));
        app_state.finish_saves();
        if let Some(Err(error)) = direct_shutdown {
            MessageBoxW(
                ptr::null_mut(),
                to_wide(&error).as_ptr(),
                to_wide("Direct shutdown").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
        if let Some(error) = app_state.persistence_error.clone() {
            MessageBoxW(
                ptr::null_mut(),
                to_wide(&error).as_ptr(),
                to_wide("Changes were not saved").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }

        if let Some(a) = app_state.acp.take() {
            a.shutdown();
        }
    }
}
