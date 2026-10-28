// Persistence: save/load sessions, projects, and settings to JSON files.
// Uses the hand-written json.rs module — no external dependencies.
// Layout:
//   <config_dir>/state.json           — settings, projects, session index (metadata)
//   <config_dir>/sessions/<id>.json   — full message history per session (includes thinking)
#![allow(dead_code)]

use crate::chat::{Message, Role};
use crate::json::{self, Json};
use crate::session::{Project, Session};
use crate::theme::Settings;

pub const STATE_FILE: &str = "state.json";
pub const SESSIONS_DIR: &str = "sessions";

// ── Serialization ──────────────────────────────────────────────────────────

fn role_to_str(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Agent => "agent",
        Role::System => "system",
    }
}

fn str_to_role(s: &str) -> Role {
    match s {
        "user" => Role::User,
        "agent" => Role::Agent,
        _ => Role::System,
    }
}

fn message_to_json(m: &Message) -> Json {
    Json::object(vec![
        ("role", Json::String(role_to_str(m.role).to_string())),
        ("text", Json::String(m.text.clone())),
        ("thinking", Json::String(m.thinking.clone())),
        ("thinking_collapsed", Json::Bool(m.thinking_collapsed)),
    ])
}

fn message_from_json(j: &Json) -> Message {
    let role = j
        .get("role")
        .and_then(|v| v.as_str())
        .map(str_to_role)
        .unwrap_or(Role::System);
    let text = j
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let thinking = j
        .get("thinking")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let collapsed = j
        .get("thinking_collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut msg = Message::new(role, &text);
    msg.thinking = thinking.clone();
    msg.thinking_collapsed = collapsed;
    if !thinking.is_empty() {
        msg.thinking_runs = crate::markdown::parse(&thinking);
    }
    msg
}

fn settings_to_json(s: &Settings) -> Json {
    Json::object(vec![
        ("dark_theme", Json::Bool(s.dark_theme)),
        ("auto_scroll", Json::Bool(s.auto_scroll)),
        ("auto_approve", Json::Bool(s.auto_approve)),
        ("show_archived", Json::Bool(s.show_archived)),
    ])
}

fn settings_from_json(j: &Json) -> Settings {
    let mut s = Settings::default();
    if let Some(v) = j.get("dark_theme").and_then(|v| v.as_bool()) {
        s.dark_theme = v;
    }
    if let Some(v) = j.get("auto_scroll").and_then(|v| v.as_bool()) {
        s.auto_scroll = v;
    }
    if let Some(v) = j.get("auto_approve").and_then(|v| v.as_bool()) {
        s.auto_approve = v;
    }
    if let Some(v) = j.get("show_archived").and_then(|v| v.as_bool()) {
        s.show_archived = v;
    }
    s
}

fn project_to_json(p: &Project) -> Json {
    Json::object(vec![
        ("path", Json::String(p.path.clone())),
        ("name", Json::String(p.name.clone())),
        ("expanded", Json::Bool(p.expanded)),
    ])
}

fn project_from_json(j: &Json) -> Option<Project> {
    let path = j.get("path")?.as_str()?.to_string();
    let name = j
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let expanded = j.get("expanded").and_then(|v| v.as_bool()).unwrap_or(true);
    if path.is_empty() {
        return None;
    }
    let mut p = Project::new(&path);
    p.name = name;
    p.expanded = expanded;
    Some(p)
}

// ── State index (state.json) ───────────────────────────────────────────────

pub struct SessionMeta {
    pub local_id: String,
    pub acp_session_id: String,
    pub name: String,
    pub working_dir: String,
    pub archived: bool,
}

fn session_meta_to_json(m: &SessionMeta) -> Json {
    Json::object(vec![
        ("local_id", Json::String(m.local_id.clone())),
        ("acp_session_id", Json::String(m.acp_session_id.clone())),
        ("name", Json::String(m.name.clone())),
        ("working_dir", Json::String(m.working_dir.clone())),
        ("archived", Json::Bool(m.archived)),
        ("file", Json::String(format!("{}.json", m.local_id))),
    ])
}

fn session_meta_from_json(j: &Json) -> Option<SessionMeta> {
    let local_id = j.get("local_id")?.as_str()?.to_string();
    if local_id.is_empty() {
        return None;
    }
    Some(SessionMeta {
        local_id,
        acp_session_id: j
            .get("acp_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: j
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("New Chat")
            .to_string(),
        working_dir: j
            .get("working_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(".")
            .to_string(),
        archived: j.get("archived").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

// ── Public API ─────────────────────────────────────────────────────────────

pub struct LoadedState {
    pub settings: Settings,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub window: Option<WindowState>,
}

#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

/// Build the config directory path for the current platform.
pub fn default_config_dir() -> String {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return format!("{}\\vibe-chat", appdata);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/Library/Application Support/vibe-chat", home);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return format!("{}/vibe-chat", xdg);
        }
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/.config/vibe-chat", home);
        }
    }
    ".vibe-chat".to_string()
}

fn sessions_dir(config_dir: &str) -> String {
    format!("{}/{}", config_dir, SESSIONS_DIR)
}

fn session_file_path(config_dir: &str, local_id: &str) -> String {
    format!("{}/{}.json", sessions_dir(config_dir), local_id)
}

/// Save the state index (settings, projects, session metadata) to state.json.
pub fn save_index(
    config_dir: &str,
    settings: &Settings,
    projects: &[Project],
    sessions: &[Session],
    window: Option<&WindowState>,
) -> Result<(), String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let session_metas: Vec<Json> = sessions
        .iter()
        .map(|s| {
            session_meta_to_json(&SessionMeta {
                local_id: s.local_id.clone(),
                acp_session_id: s.acp_session_id.clone(),
                name: s.name.clone(),
                working_dir: s.working_dir.clone(),
                archived: s.archived,
            })
        })
        .collect();

    let projects_json: Vec<Json> = projects.iter().map(project_to_json).collect();

    let mut root_pairs: Vec<(&str, Json)> = vec![
        ("version", Json::Number(1.0)),
        ("settings", settings_to_json(settings)),
        ("projects", Json::Array(projects_json)),
        ("sessions", Json::Array(session_metas)),
    ];

    if let Some(w) = window {
        root_pairs.push((
            "window",
            Json::object(vec![
                ("width", Json::Number(w.width as f64)),
                ("height", Json::Number(w.height as f64)),
                ("x", Json::Number(w.x as f64)),
                ("y", Json::Number(w.y as f64)),
            ]),
        ));
    }

    let root = Json::Object(
        root_pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    );

    let path = format!("{}/{}", config_dir, STATE_FILE);
    std::fs::write(&path, root.to_string())
        .map_err(|e| format!("Failed to write state.json: {}", e))
}

/// Save a single session's message history to sessions/<local_id>.json.
pub fn save_session(config_dir: &str, session: &Session) -> Result<(), String> {
    let dir = sessions_dir(config_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create sessions dir: {}", e))?;

    let messages_json: Vec<Json> = session.chat.messages.iter().map(message_to_json).collect();

    let root = Json::object(vec![
        ("local_id", Json::String(session.local_id.clone())),
        (
            "acp_session_id",
            Json::String(session.acp_session_id.clone()),
        ),
        ("messages", Json::Array(messages_json)),
    ]);

    let path = session_file_path(config_dir, &session.local_id);
    std::fs::write(&path, root.to_string())
        .map_err(|e| format!("Failed to write session file: {}", e))
}

/// Save the full state: index + all session files.
pub fn save_all(
    config_dir: &str,
    settings: &Settings,
    projects: &[Project],
    sessions: &[Session],
    window: Option<&WindowState>,
) -> Result<(), String> {
    save_index(config_dir, settings, projects, sessions, window)?;
    for session in sessions {
        save_session(config_dir, session)?;
    }
    Ok(())
}

/// Delete a session file from disk.
pub fn delete_session_file(config_dir: &str, local_id: &str) {
    let path = session_file_path(config_dir, local_id);
    let _ = std::fs::remove_file(&path);
}

/// Load the state index and all session message files from disk.
/// Returns None if state.json doesn't exist or is corrupt.
pub fn load_all(config_dir: &str) -> Option<LoadedState> {
    let index_path = format!("{}/{}", config_dir, STATE_FILE);
    let content = std::fs::read_to_string(&index_path).ok()?;
    let root = json::parse(&content).ok()?;

    let settings = root
        .get("settings")
        .map(settings_from_json)
        .unwrap_or_default();

    let mut projects = Vec::new();
    if let Some(arr) = root.get("projects").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(p) = project_from_json(item) {
                projects.push(p);
            }
        }
    }

    let mut sessions = Vec::new();
    if let Some(arr) = root.get("sessions").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(meta) = session_meta_from_json(item) {
                let mut session = Session {
                    local_id: meta.local_id.clone(),
                    acp_session_id: meta.acp_session_id,
                    name: meta.name,
                    working_dir: meta.working_dir,
                    chat: crate::chat::ChatState::new(),
                    created_at: std::time::Instant::now(),
                    archived: meta.archived,
                };

                let session_path = session_file_path(config_dir, &meta.local_id);
                if let Ok(content) = std::fs::read_to_string(&session_path) {
                    if let Ok(session_json) = json::parse(&content) {
                        if let Some(msgs) = session_json.get("messages").and_then(|v| v.as_array())
                        {
                            for m in msgs {
                                session.chat.messages.push(message_from_json(m));
                            }
                        }
                    }
                }

                sessions.push(session);
            }
        }
    }

    let window = root.get("window").and_then(|w| {
        Some(WindowState {
            width: w.get("width").and_then(|v| v.as_i64())? as i32,
            height: w.get("height").and_then(|v| v.as_i64())? as i32,
            x: w.get("x").and_then(|v| v.as_i64())? as i32,
            y: w.get("y").and_then(|v| v.as_i64())? as i32,
        })
    });

    Some(LoadedState {
        settings,
        projects,
        sessions,
        window,
    })
}
