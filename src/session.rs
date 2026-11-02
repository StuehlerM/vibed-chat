// Session: a single chat conversation with its own ChatState.
// Project: a working directory that groups related sessions (Passeo-style).
#![allow(dead_code)]

use crate::chat::{ChatState, Role};

/// Which backend a session uses for its turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    /// Default mode: plain chat via the Mistral Responses API.
    /// No agent process required.
    Direct,
    /// Code mode: agentic work via vibe-acp (tools, permissions).
    Code,
}

impl SessionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionMode::Direct => "direct",
            SessionMode::Code => "code",
        }
    }

    pub fn from_str(s: &str) -> SessionMode {
        match s {
            "code" => SessionMode::Code,
            _ => SessionMode::Direct,
        }
    }
}

/// Reasoning effort for Responses API requests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReasoningEffort {
    None,
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReasoningEffort::None => "none",
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
        }
    }
}

/// One item of conversation history for a Responses API request.
#[derive(Debug, Clone)]
pub enum HistoryItem {
    UserText {
        text: String,
    },
    ModelMessage {
        text: String,
    },
    ModelReasoning {
        text: String,
    },
    /// Tool calls/results from Code mode, summarized as user text so the
    /// conversation stays coherent after a mode switch.
    ToolSummary {
        text: String,
    },
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TurnKey {
    pub local_id: String,
    pub turn_id: u64,
}

#[derive(Debug, Clone)]
pub struct ActiveTurn {
    pub key: TurnKey,
    pub backend: SessionMode,
    pub text: String,
    pub submitted: bool,
    pub session_requested: bool,
    pub cancelling: bool,
    pub started: std::time::Instant,
}

#[derive(Debug)]
pub struct Session {
    pub local_id: String,
    pub acp_session_id: String,
    pub name: String,
    pub working_dir: String,
    pub chat: ChatState,
    pub created_at: std::time::SystemTime,
    pub last_activity: std::time::SystemTime,
    pub archived: bool,
    pub mode: SessionMode,
    pub active_turn: Option<ActiveTurn>,
    /// Failed submission can be retried without duplicating its user entry.
    pub failed_turn: Option<ActiveTurn>,
    pub title_is_automatic: bool,
}

impl Session {
    pub fn new(acp_session_id: &str, name: &str, working_dir: &str) -> Self {
        Self {
            local_id: generate_local_id(),
            acp_session_id: acp_session_id.to_string(),
            name: name.to_string(),
            working_dir: working_dir.to_string(),
            chat: ChatState::new(),
            created_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            archived: false,
            mode: SessionMode::Direct,
            active_turn: None,
            failed_turn: None,
            title_is_automatic: true,
        }
    }

    pub fn start_turn_streaming(&mut self) {
        let Some(turn) = self.active_turn.as_ref() else {
            return;
        };
        self.chat.start_streaming();
        if let Some(message) = self.chat.messages.back_mut() {
            message.turn_id = Some(turn.key.turn_id);
            message.backend = Some(turn.backend);
            message.model = (turn.backend == SessionMode::Direct)
                .then(|| crate::responses::DEFAULT_MODEL.to_string());
            message.outcome = Some(crate::chat::TurnStatus::Streaming);
        }
        for message in &mut self.chat.messages {
            if message.turn_id == Some(turn.key.turn_id) {
                message.outcome = Some(crate::chat::TurnStatus::Streaming);
            }
        }
    }

    /// Build the Responses API history for the next Direct-mode request.
    /// System messages are skipped (UI-only); the new user message is
    /// included by the caller. Thinking text travels as reasoning items.
    pub fn responses_history(&self) -> Vec<HistoryItem> {
        let mut items = Vec::new();
        for msg in &self.chat.messages {
            match msg.role {
                Role::User => items.push(HistoryItem::UserText {
                    text: msg.text.clone(),
                }),
                Role::Agent => {
                    if !msg.text.is_empty() {
                        items.push(HistoryItem::ModelMessage {
                            text: msg.text.clone(),
                        });
                    }
                }
                Role::System => {}
            }
        }
        items
    }

    pub fn auto_name(&mut self) {
        if !self.title_is_automatic {
            return;
        }
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
                    self.title_is_automatic = false;
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
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    // Monotonic process component prevents same-clock-tick collisions.
    let counter = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let component = (u128::from(std::process::id()) << 64) | u128::from(counter);
    format!("sess_{nanos}_{component}")
}
