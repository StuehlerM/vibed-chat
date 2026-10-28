// Keyboard input: platform-independent key mapping and text editing.
// Platform-specific concerns (clipboard, modifier key state) are abstracted
// through the KeyEvent type and platform callbacks.
#![allow(dead_code)]

use crate::chat::ChatState;

/// Virtual key codes used by the input handler.
/// These mirror the common subset of key codes across platforms.
/// Platform backends translate their native key events into these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Key {
    Return,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Escape,
    Tab,
    F5,
    Char(u8),
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
}

/// Result of handling a key event.
pub enum InputResult {
    /// No action needed
    None,
    /// A message was submitted (Enter pressed with non-empty input)
    Submit(String),
    /// Input was cleared (Escape pressed)
    Cleared,
    /// Request to cancel the current agent turn
    Cancel,
    /// Need to repaint
    Repaint,
}

/// Handle a key press. Returns an `InputResult` if something happened.
pub fn handle_key(chat: &mut ChatState, key: Key, mods: Modifiers) -> InputResult {
    match key {
        Key::Return => {
            if !mods.shift && !chat.input_buffer.trim().is_empty() && !chat.is_waiting_for_agent {
                let text = chat.take_input();
                return InputResult::Submit(text);
            }
            if !chat.is_waiting_for_agent {
                chat.insert_char('\n');
                return InputResult::Repaint;
            }
            InputResult::None
        }
        Key::Backspace => {
            chat.backspace();
            InputResult::Repaint
        }
        Key::Delete => {
            chat.delete_forward();
            InputResult::Repaint
        }
        Key::Left => {
            if mods.shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_left();
            InputResult::Repaint
        }
        Key::Right => {
            if mods.shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_right();
            InputResult::Repaint
        }
        Key::Up => {
            if mods.shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_up();
            InputResult::Repaint
        }
        Key::Down => {
            if mods.shift && chat.selection_start.is_none() {
                chat.selection_start = Some(chat.cursor_pos);
            }
            chat.move_cursor_down();
            InputResult::Repaint
        }
        Key::Home => {
            if mods.ctrl {
                chat.selection_start = None;
                chat.cursor_pos = 0;
            } else {
                if mods.shift && chat.selection_start.is_none() {
                    chat.selection_start = Some(chat.cursor_pos);
                }
                chat.move_cursor_home();
            }
            InputResult::Repaint
        }
        Key::End => {
            if mods.ctrl {
                chat.selection_start = None;
                chat.cursor_pos = chat.input_buffer.len();
            } else {
                if mods.shift && chat.selection_start.is_none() {
                    chat.selection_start = Some(chat.cursor_pos);
                }
                chat.move_cursor_end();
            }
            InputResult::Repaint
        }
        Key::Escape => {
            if chat.is_waiting_for_agent {
                InputResult::Cancel
            } else {
                chat.clear_input();
                InputResult::Cleared
            }
        }
        Key::Char(b'A') if mods.ctrl => {
            chat.select_all();
            InputResult::Repaint
        }
        // Cut/copy are handled by the platform layer (they need clipboard access)
        Key::Char(b'X' | b'C') if mods.ctrl => InputResult::None,
        Key::Char(b'V') if mods.ctrl => InputResult::Repaint, // paste handled by platform
        _ => InputResult::None,
    }
}

/// Handle a Unicode character input.
pub fn handle_char_input(chat: &mut ChatState, c: char) -> InputResult {
    if chat.is_waiting_for_agent {
        return InputResult::None;
    }
    if !c.is_control() {
        chat.insert_char(c);
        return InputResult::Repaint;
    }
    InputResult::None
}

// ── Clipboard operations (delegated to platform) ────────────────────────────

/// Copy the current selection to the clipboard.
/// The platform backend provides the actual clipboard implementation.
pub fn copy_selection(chat: &ChatState, clipboard: &dyn crate::platform::backend::Clipboard) {
    if let Some((start, end)) = chat.selection_range() {
        if start < end {
            let text = &chat.input_buffer[start..end];
            clipboard.set_text(text);
        }
    }
}

/// Paste text from the clipboard into the chat input.
pub fn paste_from_clipboard(
    chat: &mut ChatState,
    clipboard: &dyn crate::platform::backend::Clipboard,
) {
    if let Some(text) = clipboard.get_text() {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if !normalized.is_empty() {
            chat.insert_str(&normalized);
        }
    }
}
