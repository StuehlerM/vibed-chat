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
            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
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
    pub local_id: String,
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
            local_id: generate_local_id(),
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
                        format!("{name}...")
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

fn generate_local_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    format!("sess_{nanos}_{pid}")
}
