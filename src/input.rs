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
    /// Request to cancel the current agent turn
    Cancel,
    /// Need to repaint
    Repaint,
}

/// Handle a key press. Returns an `InputResult` if something happened.
pub fn handle_key(chat: &mut ChatState, key: Key, mods: Modifiers) -> InputResult {
    let anchor = chat.selection_start.unwrap_or(chat.cursor_pos);
    let motion = matches!(
        key,
        Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
    );
    let result = match key {
        Key::Return => {
            if chat.is_waiting_for_agent && !mods.shift {
                return InputResult::None;
            }
            if !mods.shift && chat.input_buffer.trim().is_empty() {
                return InputResult::None;
            }
            if !mods.shift {
                let text = chat.take_input();
                return InputResult::Submit(text);
            }
            chat.insert_char('\n');
            InputResult::Repaint
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
                InputResult::None
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
    };
    if motion {
        chat.selection_start = if mods.shift { Some(anchor) } else { None };
    }
    result
}

/// Handle a Unicode character input.
pub fn handle_char_input(chat: &mut ChatState, c: char) -> InputResult {
    if !c.is_control() {
        chat.insert_char(c);
        return InputResult::Repaint;
    }
    InputResult::None
}

// ── Clipboard operations (delegated to platform) ────────────────────────────

/// Copy the current selection to the clipboard.
/// The platform backend provides the actual clipboard implementation.
pub fn copy_selection(
    chat: &ChatState,
    clipboard: &dyn crate::platform::backend::Clipboard,
) -> Result<(), String> {
    if let Some((start, end)) = chat.selection_range() {
        if start < end {
            let text = &chat.input_buffer[start..end];
            return clipboard.set_text(text);
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(text: &str, busy: bool) -> ChatState {
        let mut chat = ChatState::new();
        chat.insert_str(text);
        chat.is_waiting_for_agent = busy;
        chat
    }

    #[test]
    fn busy_enter_preserves_draft_cursor_and_selection() {
        let mut chat = draft("next 🙂 question", true);
        chat.cursor_pos = "next 🙂".len();
        chat.selection_start = Some(0);
        assert!(matches!(
            handle_key(&mut chat, Key::Return, Modifiers::default()),
            InputResult::None
        ));
        assert_eq!(chat.input_buffer, "next 🙂 question");
        assert_eq!(chat.cursor_pos, "next 🙂".len());
        assert_eq!(chat.selection_start, Some(0));
    }

    #[test]
    fn busy_shift_enter_inserts_newline_at_caret() {
        let mut chat = draft("a🙂b", true);
        chat.cursor_pos = "a🙂".len();
        assert!(matches!(
            handle_key(
                &mut chat,
                Key::Return,
                Modifiers {
                    shift: true,
                    ctrl: false
                }
            ),
            InputResult::Repaint
        ));
        assert_eq!(chat.input_buffer, "a🙂\nb");
        assert_eq!(chat.cursor_pos, "a🙂\n".len());
    }

    #[test]
    fn busy_unicode_typing_and_editing_remain_available() {
        let mut chat = draft("next ", true);
        assert!(matches!(
            handle_char_input(&mut chat, '🙂'),
            InputResult::Repaint
        ));
        assert_eq!(chat.input_buffer, "next 🙂");
        handle_key(&mut chat, Key::Backspace, Modifiers::default());
        assert_eq!(chat.input_buffer, "next ");
        assert!(chat.is_waiting_for_agent);
    }

    #[test]
    fn escape_never_erases_draft_or_selection() {
        for busy in [false, true] {
            let mut chat = draft("keep me", busy);
            chat.select_all();
            let result = handle_key(&mut chat, Key::Escape, Modifiers::default());
            assert!(if busy {
                matches!(result, InputResult::Cancel)
            } else {
                matches!(result, InputResult::None)
            });
            assert_eq!(chat.input_buffer, "keep me");
            assert_eq!(chat.selection_range(), Some((0, 7)));
        }
    }

    #[test]
    fn idle_enter_takes_nonempty_draft_only_once() {
        let mut chat = draft("question", false);
        assert!(matches!(
            handle_key(&mut chat, Key::Return, Modifiers::default()),
            InputResult::Submit(text) if text == "question"
        ));
        assert!(chat.input_buffer.is_empty());
        assert!(matches!(
            handle_key(&mut chat, Key::Return, Modifiers::default()),
            InputResult::None
        ));
    }

    #[test]
    fn empty_enter_does_not_change_whitespace_draft() {
        let mut chat = draft(" \n\t", false);
        assert!(matches!(
            handle_key(&mut chat, Key::Return, Modifiers::default()),
            InputResult::None
        ));
        assert_eq!(chat.input_buffer, " \n\t");
    }
}
