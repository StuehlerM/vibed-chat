// Keyboard input: WM_UNICHAR for Unicode, WM_KEYDOWN for control keys, clipboard.
#![allow(dead_code)]

use crate::chat::ChatState;
use crate::win32::*;
use std::ffi::c_void;

/// Result of handling a key event.
pub enum InputResult {
    /// No action needed
    None,
    /// A message was submitted (Ctrl+Enter or Shift+Enter pressed)
    Submit(String),
    /// Input was cleared (Escape pressed)
    Cleared,
    /// Request to cancel the current agent turn
    Cancel,
    /// Need to repaint
    Repaint,
}

/// Handle WM_KEYDOWN. Returns an InputResult if something happened.
pub fn handle_keydown(chat: &mut ChatState, wparam: WPARAM, _lparam: LPARAM) -> InputResult {
    let vk = wparam as u8;
    let (ctrl, shift) = unsafe {
        (
            (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0,
            (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0,
        )
    };

    match vk {
        VK_RETURN => {
            // Enter sends the message (standard chat convention)
            // Shift+Enter inserts a newline (multiline input)
            if !shift && !chat.input_buffer.trim().is_empty() && !chat.is_waiting_for_agent {
                let text = chat.take_input();
                return InputResult::Submit(text);
            }
            // Shift+Enter or empty input: insert a newline
            if !chat.is_waiting_for_agent {
                chat.insert_char('\n');
                return InputResult::Repaint;
            }
            InputResult::None
        }
        VK_BACK => {
            chat.backspace();
            InputResult::Repaint
        }
        VK_DELETE => {
            chat.delete_forward();
            InputResult::Repaint
        }
        VK_LEFT => {
            if shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_left();
            InputResult::Repaint
        }
        VK_RIGHT => {
            if shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_right();
            InputResult::Repaint
        }
        VK_UP => {
            if shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_up();
            InputResult::Repaint
        }
        VK_DOWN => {
            if shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_down();
            InputResult::Repaint
        }
        VK_HOME => {
            if ctrl {
                // Ctrl+Home: start of all text
                chat.selection_start = None;
                chat.cursor_pos = 0;
            } else {
                // Home: start of current line
                if shift && chat.selection_start.is_none() {
                    chat.selection_start = Some(chat.cursor_pos);
                }
                chat.move_cursor_home();
            }
            InputResult::Repaint
        }
        VK_END => {
            if ctrl {
                // Ctrl+End: end of all text
                chat.selection_start = None;
                chat.cursor_pos = chat.input_buffer.len();
            } else {
                // End: end of current line
                if shift && chat.selection_start.is_none() {
                    chat.selection_start = Some(chat.cursor_pos);
                }
                chat.move_cursor_end();
            }
            InputResult::Repaint
        }
        VK_ESCAPE => {
            if chat.is_waiting_for_agent {
                InputResult::Cancel
            } else {
                chat.clear_input();
                InputResult::Cleared
            }
        }
        VK_A if ctrl => {
            chat.select_all();
            InputResult::Repaint
        }
        VK_C if ctrl => {
            copy_selection(chat);
            InputResult::None
        }
        VK_V if ctrl => {
            paste_from_clipboard(chat);
            InputResult::Repaint
        }
        VK_X if ctrl => {
            copy_selection(chat);
            chat.delete_selection();
            InputResult::Repaint
        }
        _ => InputResult::None,
    }
}

/// Handle WM_UNICHAR for Unicode text input.
pub fn handle_unichar(chat: &mut ChatState, wparam: WPARAM, _lparam: LPARAM) -> InputResult {
    if chat.is_waiting_for_agent {
        return InputResult::None;
    }

    if let Some(c) = char::from_u32(wparam as u32) {
        // Don't accept control characters (except we handle them in WM_KEYDOWN)
        if !c.is_control() {
            chat.insert_char(c);
            return InputResult::Repaint;
        }
    }
    InputResult::None
}

/// Handle WM_CHAR as fallback for text input.
pub fn handle_char(chat: &mut ChatState, wparam: WPARAM) -> InputResult {
    if chat.is_waiting_for_agent {
        return InputResult::None;
    }

    if let Some(c) = char::from_u32(wparam as u32) {
        if c >= ' ' && c != '\u{7F}' {
            chat.insert_char(c);
            return InputResult::Repaint;
        }
    }
    InputResult::None
}

// ── Clipboard operations ─────────────────────────────────────

fn copy_selection(chat: &ChatState) {
    if let Some((start, end)) = chat.selection_range() {
        if start < end {
            let text = &chat.input_buffer[start..end];
            set_clipboard_text(text);
        }
    }
}

fn paste_from_clipboard(chat: &mut ChatState) {
    if let Some(text) = get_clipboard_text() {
        // Normalize line endings: convert \r\n and \r to \n
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if !normalized.is_empty() {
            chat.insert_str(&normalized);
        }
    }
}

pub fn set_clipboard_text(text: &str) {
    unsafe {
        if OpenClipboard(NULL) == 0 {
            return;
        }
        EmptyClipboard();

        let wide = to_wide(text);
        let size = wide.len() * 2; // bytes including null terminator

        let hglobal = GlobalAlloc(GMEM_MOVEABLE, size);
        if !hglobal.is_null() {
            let ptr = GlobalLock(hglobal);
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr() as *const c_void, ptr, wide.len() * 2);
                GlobalUnlock(hglobal);
                SetClipboardData(CF_UNICODETEXT, hglobal);
            } else {
                GlobalFree(hglobal);
            }
        }

        CloseClipboard();
    }
}

pub fn get_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(NULL) == 0 {
            return None;
        }

        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            CloseClipboard();
            return None;
        }

        let hglobal = GetClipboardData(CF_UNICODETEXT);
        if hglobal.is_null() {
            CloseClipboard();
            return None;
        }

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }

        let result = from_wide(ptr as *const u16);
        GlobalUnlock(hglobal);
        CloseClipboard();
        Some(result)
    }
}
