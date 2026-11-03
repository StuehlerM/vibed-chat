// Text utilities: date/time helpers, string truncation, message previews.
// Platform-independent text logic that does not require a graphics context.
// Platform-specific text measurement (GDI, CoreText, Cairo, etc.) lives in
// each platform backend (e.g., platform/windows/text.rs).
#![allow(dead_code)]

use crate::chat::ChatState;
use std::borrow::Cow;

// ── Date/time helpers ───────────────────────────────────────────────────────

/// Returns a relative time string like "2m ago", "1h ago", "3d ago".
/// The common "now" case is borrowed (no allocation); only older timestamps
/// allocate.
pub fn relative_time(created_at: &std::time::SystemTime) -> Cow<'static, str> {
    let secs = created_at.elapsed().unwrap_or_default().as_secs();
    if secs < 60 {
        Cow::Borrowed("now")
    } else if secs < 3600 {
        Cow::Owned(format!("{}m ago", secs / 60))
    } else if secs < 86400 {
        Cow::Owned(format!("{}h ago", secs / 3600))
    } else {
        Cow::Owned(format!("{}d ago", secs / 86400))
    }
}

/// Get a preview of the last message in a chat (first 60 chars, single line).
pub fn last_message_preview(chat: &ChatState) -> String {
    for msg in chat.messages.iter().rev() {
        if msg.role == crate::chat::Role::System {
            continue;
        }
        let text = msg.text.trim();
        if !text.is_empty() {
            let first_line: String = text.chars().take_while(|&c| c != '\n').collect();
            let preview = if first_line.chars().count() > 60 {
                let truncated: String = first_line.chars().take(60).collect();
                format!("{truncated}...")
            } else {
                first_line
            };
            let prefix = match msg.role {
                crate::chat::Role::User => "You: ",
                _ => "",
            };
            return format!("{prefix}{preview}");
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::Role;

    #[test]
    fn preview_skips_notices_and_empty_assistant_placeholder() {
        let mut chat = ChatState::new();
        chat.add_message(Role::User, "A meaningful question");
        chat.add_system_message("Starting transport");
        chat.add_message(Role::Agent, "");
        chat.add_system_message("Tool activity");
        assert_eq!(last_message_preview(&chat), "You: A meaningful question");
        chat.add_message(Role::Agent, "The answer\nMore details");
        chat.add_system_message("Disconnected");
        assert_eq!(last_message_preview(&chat), "The answer");
    }

    #[test]
    fn notice_only_chat_has_no_preview() {
        let mut chat = ChatState::new();
        chat.add_system_message("vibe-chat started");
        assert!(last_message_preview(&chat).is_empty());
    }
}
