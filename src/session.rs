// Session: a single chat conversation with its own ChatState.
// Project: a working directory that groups related sessions (Passeo-style).
#![allow(dead_code)]

use crate::chat::{ChatState, Role};

#[derive(Debug, Clone)]
pub struct Project {
    pub path: String,
    pub name: String,
    pub expanded: bool,
    pub session_indices: Vec<usize>,
}

impl Project {
    pub fn new(path: &str) -> Self {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        Self {
            path: path.to_string(),
            name,
            expanded: true,
            session_indices: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Session {
    pub acp_session_id: String,
    pub name: String,
    pub working_dir: String,
    pub chat: ChatState,
    pub created_at: std::time::Instant,
    pub archived: bool,
}

impl Session {
    pub fn new(acp_session_id: &str, name: &str, working_dir: &str) -> Self {
        Self {
            acp_session_id: acp_session_id.to_string(),
            name: name.to_string(),
            working_dir: working_dir.to_string(),
            chat: ChatState::new(),
            created_at: std::time::Instant::now(),
            archived: false,
        }
    }

    pub fn auto_name(&mut self) {
        for msg in &self.chat.messages {
            if msg.role == Role::User {
                let text = msg.text.trim();
                if !text.is_empty() {
                    let name: String = text.chars().take(30).collect();
                    let name = if text.chars().count() > 30 {
                        format!("{}...", name)
                    } else {
                        name
                    };
                    self.name = name;
                    return;
                }
            }
        }
    }
}
