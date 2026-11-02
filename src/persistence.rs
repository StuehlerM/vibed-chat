// Versioned persistence: immutable session snapshots, atomic index publication,
// last-known-good backup, bounded validation, and non-destructive recovery.
#![allow(dead_code)]

use crate::chat::{Message, Role};
use crate::json::{self, Json};
use crate::session::{Project, Session, SessionMode};
use crate::theme::Settings;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const STATE_FILE: &str = "state.json";
pub const SESSIONS_DIR: &str = "sessions";
const VERSION: i64 = 2;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// The only accepted persisted identity grammar is sess_<digits>_<digits>.
/// Separators, drive prefixes, NULs and traversal can never reach path creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalSessionId(String);
impl LocalSessionId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut parts = value.split('_');
        if value.len() > 100 || parts.next() != Some("sess") {
            return Err("Invalid local session ID".to_string());
        }
        for _ in 0..2 {
            if !parts
                .next()
                .is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err("Invalid local session ID".to_string());
            }
        }
        if parts.next().is_some() {
            return Err("Invalid local session ID".to_string());
        }
        Ok(Self(value.to_string()))
    }
}

#[derive(Debug)]
pub enum LoadError {
    Io { path: PathBuf, message: String },
    Malformed { path: PathBuf, message: String },
    UnsupportedVersion(i64),
}
impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Malformed { path, message } => write!(f, "{}: {message}", path.display()),
            Self::UnsupportedVersion(v) => {
                write!(f, "Unsupported storage version {v}; originals preserved")
            }
        }
    }
}

pub struct LoadedState {
    pub settings: Settings,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub window: Option<WindowState>,
    /// Partial recovery is read-only until the damaged originals are repaired.
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

pub fn default_config_dir() -> String {
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata)
            .join("vibe-chat")
            .to_string_lossy()
            .into_owned();
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library/Application Support/vibe-chat")
            .to_string_lossy()
            .into_owned();
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg)
                .join("vibe-chat")
                .to_string_lossy()
                .into_owned();
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".config/vibe-chat")
                .to_string_lossy()
                .into_owned();
        }
    }
    ".vibe-chat".to_string()
}

fn generation() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!(
        "{nanos}_{}_{}",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    )
}
fn valid_generation(value: &str) -> bool {
    value.len() <= 100
        && value.split('_').count() == 3
        && value
            .split('_')
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}
fn session_path(config: &str, id: &LocalSessionId, generation: Option<&str>) -> PathBuf {
    let name = generation.map_or_else(
        || format!("{}.json", id.0),
        |g| format!("{}_{g}.json", id.0),
    );
    Path::new(config).join(SESSIONS_DIR).join(name)
}

/// std::fs::rename replaces existing files atomically (MoveFileExW with
/// REPLACE_EXISTING on Windows). Never remove the destination before replacement.
/// A same-directory create_new temporary is flushed before publication.
fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err(format!(
            "{} exceeds the 16 MiB storage limit; previous snapshot preserved",
            path.display()
        ));
    }
    let temporary = path.with_extension(format!("{}.tmp", generation()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| format!("Create {}: {e}", temporary.display()))?;
        file.write_all(content)
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("Flush {}: {e}", temporary.display()))?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| format!("Publish {}: {e}", path.display()))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn message_to_json(m: &Message) -> Json {
    Json::object(vec![
        (
            "role",
            Json::String(
                match m.role {
                    Role::User => "user",
                    Role::Agent => "agent",
                    Role::System => "system",
                }
                .to_string(),
            ),
        ),
        ("text", Json::String(m.text.clone())),
        ("thinking", Json::String(m.thinking.clone())),
        ("thinking_collapsed", Json::Bool(m.thinking_collapsed)),
        (
            "turn_id",
            m.turn_id
                .map_or(Json::Null, |id| Json::Integer(i128::from(id))),
        ),
        (
            "backend",
            m.backend.map_or(Json::Null, |backend| {
                Json::String(backend.as_str().to_string())
            }),
        ),
        (
            "model",
            m.model
                .as_ref()
                .map_or(Json::Null, |model| Json::String(model.clone())),
        ),
        (
            "outcome",
            m.outcome.map_or(Json::Null, |outcome| {
                Json::String(outcome.as_str().to_string())
            }),
        ),
    ])
}
fn required_str<'a>(value: &'a Json, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("Missing/invalid {key}"))
}
fn required_array<'a>(value: &'a Json, key: &str) -> Result<&'a [Json], String> {
    value
        .get(key)
        .and_then(Json::as_array)
        .ok_or_else(|| format!("Missing/invalid {key}"))
}
fn message_from_json(j: &Json) -> Result<Message, String> {
    let role = match required_str(j, "role")? {
        "user" => Role::User,
        "agent" => Role::Agent,
        "system" => Role::System,
        _ => return Err("Invalid role".to_string()),
    };
    let mut msg = Message::new(role, required_str(j, "text")?);
    msg.turn_id = j
        .get("turn_id")
        .filter(|value| !value.is_null())
        .map(|value| value.as_u64().ok_or("Invalid turn ID"))
        .transpose()?;
    msg.backend = match j.get("backend").and_then(Json::as_str) {
        None => None,
        Some("direct") => Some(SessionMode::Direct),
        Some("code") => Some(SessionMode::Code),
        _ => return Err("Invalid message backend".to_string()),
    };
    msg.model = j.get("model").and_then(Json::as_str).map(str::to_string);
    msg.outcome = j
        .get("outcome")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_str()
                .and_then(crate::chat::TurnStatus::parse)
                .ok_or("Invalid turn outcome")
        })
        .transpose()?;
    if matches!(
        msg.outcome,
        Some(crate::chat::TurnStatus::Queued | crate::chat::TurnStatus::Streaming)
    ) {
        msg.outcome = Some(crate::chat::TurnStatus::Interrupted);
    }
    msg.thinking = required_str(j, "thinking")?.to_string();
    msg.thinking_runs = crate::markdown::parse(&msg.thinking);
    msg.thinking_collapsed = j
        .get("thinking_collapsed")
        .and_then(Json::as_bool)
        .unwrap_or(true);
    Ok(msg)
}
fn settings_to_json(s: &Settings) -> Json {
    Json::object(vec![
        ("dark_theme", Json::Bool(s.dark_theme)),
        ("auto_scroll", Json::Bool(s.auto_scroll)),
        ("auto_approve", Json::Bool(s.auto_approve)),
        ("show_archived", Json::Bool(s.show_archived)),
    ])
}

/// Every save publishes a complete generation. Old snapshots remain available
/// through state.json.bak; orphan generations are deliberately retained for recovery.
pub fn save_all(
    config: &str,
    settings: &Settings,
    projects: &[Project],
    sessions: &[Session],
    window: Option<&WindowState>,
) -> Result<(), String> {
    let mut ids = HashSet::new();
    for session in sessions {
        let id = LocalSessionId::parse(&session.local_id)?;
        if !ids.insert(id) {
            return Err("Duplicate local session ID".to_string());
        }
    }
    std::fs::create_dir_all(Path::new(config).join(SESSIONS_DIR)).map_err(|e| e.to_string())?;
    let generation = generation();
    let mut metadata = Vec::new();
    // Publish immutable snapshots first. An interrupted save cannot alter any
    // snapshot referenced by either the current index or its previous backup.
    for session in sessions {
        let id = LocalSessionId::parse(&session.local_id)?;
        let snapshot = Json::object(vec![
            ("version", Json::Number(VERSION as f64)),
            ("local_id", Json::String(id.0.clone())),
            ("draft", Json::String(session.chat.input_buffer.clone())),
            ("interrupted", Json::Bool(session.chat.is_waiting_for_agent)),
            (
                "messages",
                Json::Array(session.chat.messages.iter().map(message_to_json).collect()),
            ),
        ]);
        atomic_write(
            &session_path(config, &id, Some(&generation)),
            snapshot.to_string().as_bytes(),
        )?;
        metadata.push(Json::object(vec![
            ("local_id", Json::String(id.0)),
            ("name", Json::String(session.name.clone())),
            ("title_is_automatic", Json::Bool(session.title_is_automatic)),
            (
                "created_at",
                Json::Integer(i128::from(
                    session
                        .created_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                )),
            ),
            (
                "last_activity",
                Json::Integer(i128::from(
                    session
                        .last_activity
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                )),
            ),
            ("working_dir", Json::String(session.working_dir.clone())),
            ("archived", Json::Bool(session.archived)),
            ("mode", Json::String(session.mode.as_str().to_string())),
        ]));
    }
    let mut root = vec![
        ("version", Json::Number(VERSION as f64)),
        ("generation", Json::String(generation)),
        ("settings", settings_to_json(settings)),
        (
            "projects",
            Json::Array(
                projects
                    .iter()
                    .map(|p| {
                        Json::object(vec![
                            ("path", Json::String(p.path.clone())),
                            ("name", Json::String(p.name.clone())),
                            ("expanded", Json::Bool(p.expanded)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("sessions", Json::Array(metadata)),
    ];
    if let Some(w) = window {
        root.push((
            "window",
            Json::object(vec![
                ("width", Json::Number(f64::from(w.width))),
                ("height", Json::Number(f64::from(w.height))),
                ("x", Json::Number(f64::from(w.x))),
                ("y", Json::Number(f64::from(w.y))),
            ]),
        ));
    }
    let index = Path::new(config).join(STATE_FILE);
    match std::fs::metadata(&index) {
        Ok(_) => {
            // Revalidate at the write boundary too: another process/user may
            // have changed the index after startup. Never overwrite future or
            // damaged data just because this process still has an older model.
            let old = read_json(&index).map_err(|e| e.to_string())?;
            if !matches!(old.get("version").and_then(Json::as_i64), Some(1 | VERSION))
                || old.get("sessions").and_then(Json::as_array).is_none()
                || old.get("projects").and_then(Json::as_array).is_none()
            {
                return Err(
                    "Existing index is damaged or unsupported; refusing replacement".to_string(),
                );
            }
            atomic_write(
                &index.with_extension("json.bak"),
                old.to_string().as_bytes(),
            )?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Read previous index: {e}")),
    }
    atomic_write(&index, Json::object(root).to_string().as_bytes())
}
/// Reserve neither files nor directories, but reject identities already present
/// in any retained generation (including an orphan from an interrupted save).
pub fn session_id_in_storage(config: &str, local_id: &str) -> Result<bool, String> {
    let id = LocalSessionId::parse(local_id)?;
    let entries = match std::fs::read_dir(Path::new(config).join(SESSIONS_DIR)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("Check session identity: {e}")),
    };
    let legacy = format!("{}.json", id.0);
    let prefix = format!("{}_", id.0);
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Some(name) = entry.file_name().to_str() {
            if name == legacy || name.starts_with(&prefix) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn save_index(
    config: &str,
    settings: &Settings,
    projects: &[Project],
    sessions: &[Session],
    window: Option<&WindowState>,
) -> Result<(), String> {
    save_all(config, settings, projects, sessions, window)
}

/// Delete only validated filenames belonging to this identity. Caller publishes
/// metadata removal first, then removes recovery copies intentionally as well.
pub fn delete_session_file(config: &str, local_id: &str) -> Result<(), String> {
    let id = LocalSessionId::parse(local_id)?;
    let entries = match std::fs::read_dir(Path::new(config).join(SESSIONS_DIR)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let legacy = format!("{}.json", id.0);
        let prefix = format!("{}_", id.0);
        let snapshot = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".json"));
        if name == legacy || snapshot.is_some_and(valid_generation) {
            std::fs::remove_file(entry.path())
                .map_err(|e| format!("Delete {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Json, LoadError> {
    let malformed = |message| LoadError::Malformed {
        path: path.to_path_buf(),
        message,
    };
    let file = std::fs::File::open(path).map_err(|e| LoadError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let mut content = String::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|e| LoadError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err(malformed("File exceeds storage size limit".to_string()));
    }
    json::parse(&content).map_err(malformed)
}

pub fn load_all(config: &str) -> Result<Option<LoadedState>, LoadError> {
    load_index(config, false)
}
pub fn load_backup(config: &str) -> Result<Option<LoadedState>, LoadError> {
    load_index(config, true)
}
fn load_index(config: &str, backup: bool) -> Result<Option<LoadedState>, LoadError> {
    let path = Path::new(config).join(if backup { "state.json.bak" } else { STATE_FILE });
    match std::fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if !backup
                && (Path::new(config).join("state.json.bak").exists()
                    || std::fs::read_dir(Path::new(config).join(SESSIONS_DIR))
                        .is_ok_and(|mut entries| entries.next().is_some()))
            {
                return Err(LoadError::Malformed {
                    path,
                    message: "Index missing but recovery files exist".to_string(),
                });
            }
            return Ok(None);
        }
        Err(e) => {
            return Err(LoadError::Io {
                path,
                message: e.to_string(),
            })
        }
        Ok(_) => {}
    }
    let root = read_json(&path)?;
    let malformed = |message| LoadError::Malformed {
        path: path.clone(),
        message,
    };
    let version = root
        .get("version")
        .and_then(Json::as_i64)
        .ok_or_else(|| malformed("Missing/invalid version".to_string()))?;
    if version != 1 && version != VERSION {
        return Err(LoadError::UnsupportedVersion(version));
    }
    let generation = if version == VERSION {
        let g = required_str(&root, "generation").map_err(malformed)?;
        if !valid_generation(g) {
            return Err(malformed("Invalid generation".to_string()));
        }
        Some(g)
    } else {
        None
    };
    let settings_json = root
        .get("settings")
        .filter(|v| v.as_object().is_some())
        .ok_or_else(|| malformed("Missing settings object".to_string()))?;
    let mut settings = Settings::default();
    if let Some(v) = settings_json.get("dark_theme").and_then(Json::as_bool) {
        settings.dark_theme = v;
    }
    if let Some(v) = settings_json.get("auto_scroll").and_then(Json::as_bool) {
        settings.auto_scroll = v;
    }
    // Never restore a global automatic permission policy from disk.
    settings.auto_approve = false;
    if let Some(v) = settings_json.get("show_archived").and_then(Json::as_bool) {
        settings.show_archived = v;
    }
    let mut projects = Vec::new();
    let mut project_paths = HashSet::new();
    for item in required_array(&root, "projects").map_err(malformed)? {
        let location = required_str(item, "path").map_err(malformed)?;
        if location.is_empty() || !project_paths.insert(location) {
            return Err(malformed("Empty/duplicate project path".to_string()));
        }
        let mut project = Project::new(location);
        project.name = required_str(item, "name").map_err(malformed)?.to_string();
        project.expanded = item.get("expanded").and_then(Json::as_bool).unwrap_or(true);
        projects.push(project);
    }
    let mut ids = HashSet::new();
    let mut sessions = Vec::new();
    let mut warnings = Vec::new();
    // Validate every identity before doing any session-file I/O.
    let metadata = required_array(&root, "sessions").map_err(malformed)?;
    for item in metadata {
        let id = LocalSessionId::parse(required_str(item, "local_id").map_err(malformed)?)
            .map_err(malformed)?;
        if !ids.insert(id) {
            return Err(malformed("Duplicate session ID".to_string()));
        }
    }
    for item in metadata {
        let id = LocalSessionId::parse(required_str(item, "local_id").map_err(malformed)?)
            .map_err(malformed)?;
        let mode = match item.get("mode").and_then(Json::as_str) {
            Some("direct") => SessionMode::Direct,
            Some("code") => SessionMode::Code,
            None if version == 1 => SessionMode::Code,
            _ => return Err(malformed("Unknown/missing session mode".to_string())),
        };
        let mut session = Session::new(
            "",
            required_str(item, "name").map_err(malformed)?,
            required_str(item, "working_dir").map_err(malformed)?,
        );
        session.local_id = id.0.clone();
        session.title_is_automatic = item
            .get("title_is_automatic")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        let stored_time = |key| {
            item.get(key)
                .and_then(Json::as_i64)
                .and_then(|v| u64::try_from(v).ok())
                .and_then(|secs| {
                    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
                })
        };
        session.created_at = stored_time("created_at").unwrap_or(session.created_at);
        session.last_activity = stored_time("last_activity").unwrap_or(session.created_at);
        session.mode = mode;
        session.archived = item
            .get("archived")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        let snapshot_path = session_path(config, &id, generation);
        let loaded = (|| -> Result<(), String> {
            let value = read_json(&snapshot_path).map_err(|e| e.to_string())?;
            if required_str(&value, "local_id")? != id.0 {
                return Err("Snapshot identity mismatch".to_string());
            }
            if version == VERSION && value.get("version").and_then(Json::as_i64) != Some(VERSION) {
                return Err("Unsupported snapshot version".to_string());
            }
            let messages = required_array(&value, "messages")?
                .iter()
                .map(message_from_json)
                .collect::<Result<_, _>>()?;
            session.chat.messages = messages;
            if let Some(draft) = value.get("draft").and_then(Json::as_str) {
                session.chat.input_buffer = draft.to_string();
                session.chat.cursor_pos = draft.len();
            }
            if let Some(message) = session
                .chat
                .messages
                .iter()
                .rev()
                .find(|message| message.role == Role::User)
            {
                if message.outcome == Some(crate::chat::TurnStatus::Failed)
                    && message.text == session.chat.input_buffer
                {
                    session.failed_turn = Some(crate::session::ActiveTurn {
                        key: crate::session::TurnKey {
                            local_id: session.local_id.clone(),
                            turn_id: message.turn_id.unwrap_or(0),
                        },
                        backend: message.backend.unwrap_or(mode),
                        text: message.text.clone(),
                        submitted: false,
                        session_requested: false,
                        cancelling: false,
                        started: std::time::Instant::now(),
                    });
                }
            }
            if value.get("interrupted").and_then(Json::as_bool) == Some(true) {
                session.chat.add_system_message(
                    "Previous turn was interrupted. Partial output has been preserved.",
                );
            }
            Ok(())
        })();
        if let Err(e) = loaded {
            warnings.push(format!("{} ({}): {e}", session.name, id.0));
            session.chat.add_system_message(
                "History could not be recovered. Storage is read-only; originals are preserved.",
            );
        }
        sessions.push(session);
    }
    let window = root.get("window").and_then(|w| {
        Some(WindowState {
            width: i32::try_from(w.get("width")?.as_i64()?)
                .ok()?
                .clamp(720, 16384),
            height: i32::try_from(w.get("height")?.as_i64()?)
                .ok()?
                .clamp(480, 16384),
            x: i32::try_from(w.get("x")?.as_i64()?).ok()?,
            y: i32::try_from(w.get("y")?.as_i64()?).ok()?,
        })
    });
    Ok(Some(LoadedState {
        settings,
        projects,
        sessions,
        window,
        warnings,
    }))
}
