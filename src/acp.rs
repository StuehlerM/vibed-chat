// ACP client: bounded JSON-RPC over stdio, typed request and local-turn correlation.
#![allow(dead_code)]

use crate::json::{self, Json};
use crate::session::TurnKey;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug)]
pub enum AcpEvent {
    StreamChunk {
        key: TurnKey,
        text: String,
    },
    ThoughtChunk {
        key: TurnKey,
        text: String,
    },
    TurnComplete {
        key: TurnKey,
        stop_reason: String,
    },
    ToolCall {
        key: TurnKey,
        name: String,
        input: String,
    },
    ToolResult {
        key: TurnKey,
        call_id: String,
        output: String,
    },
    PermissionRequest {
        id: Json,
        key: TurnKey,
        method: String,
        params: Json,
    },
    ProcessDied {
        code: Option<i32>,
    },
    Error(String),
    RequestFailed {
        key: TurnKey,
        message: String,
    },
    Connected,
    SessionReady {
        key: TurnKey,
        session_id: String,
    },
}
#[derive(Debug)]
enum AcpCommand {
    Prompt {
        key: TurnKey,
        session_id: String,
        text: String,
    },
    Cancel {
        session_id: String,
    },
    Reply {
        id: Json,
        result: Option<Json>,
    },
    Shutdown,
    Initialize,
    NewSession {
        key: TurnKey,
        cwd: String,
    },
}
#[derive(Debug)]
enum PendingRequest {
    Initialize,
    NewSession(TurnKey),
    Prompt { key: TurnKey, remote: String },
}
struct AcpState {
    next_id: i64,
    requests: HashMap<i64, PendingRequest>,
    turns: HashMap<String, TurnKey>,
    retired: HashMap<String, u64>,
}
pub struct AcpClient {
    commands: SyncSender<AcpCommand>,
    events: Receiver<AcpEvent>,
    child: Arc<Mutex<Option<Child>>>,
    state: Arc<Mutex<AcpState>>,
    intentional_shutdown: Arc<AtomicBool>,
    diagnostics: Arc<Mutex<std::collections::VecDeque<String>>>,
}
impl AcpClient {
    pub fn spawn(configure: fn(&mut Command)) -> Result<Self, String> {
        Self::spawn_configured("vibe-acp", &[], configure)
    }
    pub fn spawn_with_command(program: &str, args: &[&str]) -> Result<Self, String> {
        Self::spawn_configured(program, args, |_| {})
    }
    fn spawn_configured(
        program: &str,
        args: &[&str],
        configure: fn(&mut Command),
    ) -> Result<Self, String> {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure(&mut command); // OS process setup belongs to the backend.
        let mut child = command
            .spawn()
            .map_err(|e| format!("Spawn {program}: {e}"))?;
        let Some(stdin) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Missing agent stdin".to_string());
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Missing agent stdout".to_string());
        };
        let (commands, command_rx) = sync_channel(64);
        let (event_tx, events) = sync_channel(512);
        let state = Arc::new(Mutex::new(AcpState {
            next_id: 1,
            requests: HashMap::new(),
            turns: HashMap::new(),
            retired: HashMap::new(),
        }));
        let stderr = child.stderr.take();
        let child = Arc::new(Mutex::new(Some(child)));
        let diagnostics = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let intentional_shutdown = Arc::new(AtomicBool::new(false));
        let died = Arc::new(AtomicBool::new(false));
        let client = Self {
            commands,
            events,
            child: child.clone(),
            state: state.clone(),
            intentional_shutdown: intentional_shutdown.clone(),
            diagnostics: diagnostics.clone(),
        };
        if let Some(stderr) = stderr {
            thread::Builder::new()
                .name("acp-stderr".to_string())
                .spawn(move || {
                    let mut reader = BufReader::new(stderr);
                    loop {
                        let mut line = String::new();
                        match reader.by_ref().take(65_537).read_line(&mut line) {
                            Ok(0) | Err(_) => return,
                            Ok(_) if line.len() <= 65_536 => {}
                            Ok(_) => return,
                        }
                        let lower = line.to_ascii_lowercase();
                        let sensitive = [
                            "authorization",
                            "bearer",
                            "token",
                            "api_key",
                            "api key",
                            "secret",
                        ]
                        .iter()
                        .any(|word| lower.contains(word))
                            || ["API_KEY", "MISTRAL_API_KEY"]
                                .iter()
                                .filter_map(|key| std::env::var(key).ok())
                                .any(|key| !key.is_empty() && line.contains(&key));
                        let line = if sensitive {
                            "[redacted agent diagnostic]".to_string()
                        } else {
                            line.trim().chars().take(512).collect()
                        };
                        let Ok(mut tail) = diagnostics.lock() else {
                            return;
                        };
                        if tail.len() >= 16 {
                            tail.pop_front();
                        }
                        tail.push_back(line);
                    }
                })
                .map_err(|e| format!("Create ACP diagnostics reader: {e}"))?;
        }
        let writer_state = state.clone();
        let writer_events = event_tx.clone();
        thread::Builder::new()
            .name("acp-writer".to_string())
            .spawn(move || writer(stdin, command_rx, writer_state, writer_events))
            .map_err(|e| format!("Create ACP writer: {e}"))?;
        let reader_state = state;
        let reader_events = event_tx.clone();
        let reader_died = died.clone();
        let reader_shutdown = intentional_shutdown.clone();
        let replies = client.commands.clone();
        thread::Builder::new()
            .name("acp-reader".to_string())
            .spawn(move || {
                reader(stdout, reader_state, &reader_events, replies);
                if !reader_shutdown.load(Ordering::Acquire)
                    && !reader_died.swap(true, Ordering::AcqRel)
                {
                    let _ = reader_events.send(AcpEvent::ProcessDied { code: None });
                }
            })
            .map_err(|e| format!("Create ACP reader: {e}"))?;
        thread::Builder::new()
            .name("acp-monitor".to_string())
            .spawn(move || loop {
                thread::sleep(std::time::Duration::from_millis(250));
                let status = {
                    let Ok(mut guard) = child.lock() else {
                        return;
                    };
                    let Some(child) = guard.as_mut() else {
                        return;
                    };
                    child.try_wait()
                };
                if !matches!(status, Ok(None)) {
                    if !intentional_shutdown.load(Ordering::Acquire)
                        && !died.swap(true, Ordering::AcqRel)
                    {
                        let _ = event_tx.send(AcpEvent::ProcessDied {
                            code: status.ok().flatten().and_then(|s| s.code()),
                        });
                    }
                    return;
                }
            })
            .map_err(|e| format!("Create ACP monitor: {e}"))?;
        client.submit(AcpCommand::Initialize)?;
        Ok(client)
    }
    fn submit(&self, command: AcpCommand) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|e| format!("ACP command queue unavailable: {e}"))
    }
    pub fn send_prompt(&self, key: TurnKey, remote: &str, text: &str) -> Result<(), String> {
        if remote.is_empty() {
            return Err("Remote session is not ready".to_string());
        }
        self.submit(AcpCommand::Prompt {
            key,
            session_id: remote.to_string(),
            text: text.to_string(),
        })
    }
    pub fn request_session(&self, key: TurnKey, cwd: &str) -> Result<(), String> {
        self.submit(AcpCommand::NewSession {
            key,
            cwd: cwd.to_string(),
        })
    }
    pub fn try_recv(&self) -> Option<AcpEvent> {
        self.events.try_recv().ok()
    }
    pub fn retire_turn(&self, key: &TurnKey) {
        if let Ok(mut state) = self.state.lock() {
            state
                .retired
                .entry(key.local_id.clone())
                .and_modify(|id| *id = (*id).max(key.turn_id))
                .or_insert(key.turn_id);
            state.turns.retain(|_, turn| turn != key);
            state.requests.retain(|_, request| match request {
                PendingRequest::Initialize => true,
                PendingRequest::NewSession(turn) | PendingRequest::Prompt { key: turn, .. } => {
                    turn != key
                }
            });
        }
    }

    pub fn cancel(&self, remote: &str) -> Result<(), String> {
        // Retire before sending: late notifications cannot resurrect a cancelled turn.
        if let Ok(mut state) = self.state.lock() {
            state.turns.remove(remote);
            state.requests.retain(|_, request| !matches!(request, PendingRequest::Prompt { remote: id, .. } if id == remote));
        }
        self.submit(AcpCommand::Cancel {
            session_id: remote.to_string(),
        })
    }
    pub fn respond_permission(&self, id: Json, option: Option<String>) -> Result<(), String> {
        let outcome = match option {
            Some(id) => Json::object(vec![
                ("outcome", Json::String("selected".to_string())),
                ("optionId", Json::String(id)),
            ]),
            None => Json::object(vec![("outcome", Json::String("cancelled".to_string()))]),
        };
        self.submit(AcpCommand::Reply {
            id,
            result: Some(Json::object(vec![("outcome", outcome)])),
        })
    }
    pub fn diagnostic_tail(&self) -> String {
        self.diagnostics
            .lock()
            .map(|tail| tail.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }

    pub fn shutdown(&self) {
        self.intentional_shutdown.store(true, Ordering::Release);
        let _ = self.commands.try_send(AcpCommand::Shutdown);
        // Child is taken once: explicit shutdown and Drop are idempotent.
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
impl Drop for AcpClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn rpc(method: &str, params: Json) -> Json {
    Json::object(vec![
        ("jsonrpc", Json::String("2.0".to_string())),
        ("method", Json::String(method.to_string())),
        ("params", params),
    ])
}
fn writer(
    mut stdin: ChildStdin,
    commands: Receiver<AcpCommand>,
    state: Arc<Mutex<AcpState>>,
    events: SyncSender<AcpEvent>,
) {
    while let Ok(command) = commands.recv() {
        let (mut message, pending) = match command {
            AcpCommand::Initialize => (
                rpc(
                    "initialize",
                    Json::object(vec![
                        ("protocolVersion", Json::Integer(1)),
                        ("clientCapabilities", Json::object(vec![])),
                        (
                            "clientInfo",
                            Json::object(vec![
                                ("name", Json::String("vibe-chat".to_string())),
                                (
                                    "version",
                                    Json::String(env!("CARGO_PKG_VERSION").to_string()),
                                ),
                            ]),
                        ),
                    ]),
                ),
                Some(PendingRequest::Initialize),
            ),
            AcpCommand::NewSession { key, cwd } => (
                rpc(
                    "session/new",
                    Json::object(vec![
                        ("cwd", Json::String(cwd)),
                        ("mcpServers", Json::Array(vec![])),
                    ]),
                ),
                Some(PendingRequest::NewSession(key)),
            ),
            AcpCommand::Prompt {
                key,
                session_id,
                text,
            } => {
                match state.lock() {
                    Ok(mut s) => {
                        s.turns.insert(session_id.clone(), key.clone());
                    }
                    Err(_) => {
                        let _ = events.send(AcpEvent::Error("ACP state lock poisoned".to_string()));
                        return;
                    }
                }
                (
                    rpc(
                        "session/prompt",
                        Json::object(vec![
                            ("sessionId", Json::String(session_id.clone())),
                            (
                                "prompt",
                                Json::Array(vec![Json::object(vec![
                                    ("type", Json::String("text".to_string())),
                                    ("text", Json::String(text)),
                                ])]),
                            ),
                        ]),
                    ),
                    Some(PendingRequest::Prompt {
                        key,
                        remote: session_id,
                    }),
                )
            }
            AcpCommand::Cancel { session_id } => (
                rpc(
                    "session/cancel",
                    Json::object(vec![("sessionId", Json::String(session_id))]),
                ),
                None,
            ),
            AcpCommand::Reply { id, result } => (
                match result {
                    Some(result) => Json::object(vec![
                        ("jsonrpc", Json::String("2.0".to_string())),
                        ("id", id),
                        ("result", result),
                    ]),
                    None => Json::object(vec![
                        ("jsonrpc", Json::String("2.0".to_string())),
                        ("id", id),
                        (
                            "error",
                            Json::object(vec![
                                ("code", Json::Integer(-32601)),
                                ("message", Json::String("Method not supported".to_string())),
                            ]),
                        ),
                    ]),
                },
                None,
            ),
            AcpCommand::Shutdown => return,
        };
        if let Some(pending) = pending {
            let Ok(mut s) = state.lock() else {
                let _ = events.send(AcpEvent::Error("ACP state lock poisoned".to_string()));
                return;
            };
            let key = match &pending {
                PendingRequest::Initialize => None,
                PendingRequest::NewSession(key) | PendingRequest::Prompt { key, .. } => Some(key),
            };
            if key.is_some_and(|key| {
                s.retired
                    .get(&key.local_id)
                    .is_some_and(|id| *id >= key.turn_id)
            }) {
                continue;
            }
            let id = s.next_id;
            let Some(next) = id.checked_add(1) else {
                let _ = events.send(AcpEvent::Error("ACP request ID exhausted".to_string()));
                return;
            };
            s.next_id = next;
            s.requests.insert(id, pending);
            if let Json::Object(fields) = &mut message {
                fields.push(("id".to_string(), Json::Integer(i128::from(id))));
            }
        }
        if let Err(e) = writeln!(stdin, "{message}").and_then(|()| stdin.flush()) {
            let _ = events.send(AcpEvent::Error(format!("Write/flush agent stdin: {e}")));
            return;
        }
    }
}
fn reader(
    stdout: ChildStdout,
    state: Arc<Mutex<AcpState>>,
    events: &SyncSender<AcpEvent>,
    replies: SyncSender<AcpCommand>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = String::new();
        match reader
            .by_ref()
            .take((json::MAX_INPUT_BYTES + 1) as u64)
            .read_line(&mut line)
        {
            Ok(0) => return,
            Ok(_) if line.len() <= json::MAX_INPUT_BYTES => {}
            Ok(_) => {
                let _ = events.send(AcpEvent::Error("Agent line exceeded 16 MiB".to_string()));
                return;
            }
            Err(e) => {
                let _ = events.send(AcpEvent::Error(format!("Read agent stdout: {e}")));
                return;
            }
        }
        if line.trim().is_empty() {
            continue;
        }
        let message = match json::parse(line.trim()) {
            Ok(m) => m,
            Err(e) => {
                let _ = events.send(AcpEvent::Error(format!("Invalid agent JSON: {e}")));
                return;
            }
        };
        if let Some(method) = message.get("method").and_then(Json::as_str) {
            let params = message.get("params").cloned().unwrap_or(Json::Null);
            let id = message.get("id").cloned();
            let remote = params.get("sessionId").and_then(Json::as_str).unwrap_or("");
            let key = match state.lock() {
                Ok(s) => s.turns.get(remote).cloned(),
                Err(_) => return,
            };
            if method == "session/request_permission" {
                if let Some(id) = id {
                    if let Some(key) = key {
                        let title = params
                            .get("toolCall")
                            .and_then(|t| t.get("title"))
                            .and_then(Json::as_str)
                            .unwrap_or("Tool permission")
                            .to_string();
                        if events
                            .send(AcpEvent::PermissionRequest {
                                id,
                                key,
                                method: title,
                                params,
                            })
                            .is_err()
                        {
                            return;
                        }
                    } else {
                        let outcome = Json::object(vec![(
                            "outcome",
                            Json::object(vec![("outcome", Json::String("cancelled".to_string()))]),
                        )]);
                        if replies
                            .try_send(AcpCommand::Reply {
                                id,
                                result: Some(outcome),
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            } else if id.is_some() {
                if replies
                    .try_send(AcpCommand::Reply {
                        id: id.unwrap_or(Json::Null),
                        result: None,
                    })
                    .is_err()
                {
                    return;
                }
            } else if method == "session/update" {
                if let (Some(key), Some(update)) = (key, params.get("update")) {
                    let kind = update
                        .get("sessionUpdate")
                        .and_then(Json::as_str)
                        .unwrap_or("");
                    let text = update
                        .get("content")
                        .and_then(|v| v.get("text"))
                        .and_then(Json::as_str)
                        .unwrap_or("")
                        .to_string();
                    let event = match kind {
                        "agent_message_chunk" => Some(AcpEvent::StreamChunk { key, text }),
                        "agent_thought_chunk" => Some(AcpEvent::ThoughtChunk { key, text }),
                        "tool_call" => Some(AcpEvent::ToolCall {
                            key,
                            name: update
                                .get("title")
                                .and_then(Json::as_str)
                                .unwrap_or("Tool")
                                .to_string(),
                            input: update
                                .get("rawInput")
                                .map(ToString::to_string)
                                .unwrap_or_default(),
                        }),
                        "tool_call_update" => Some(AcpEvent::ToolResult {
                            key,
                            call_id: update
                                .get("toolCallId")
                                .and_then(Json::as_str)
                                .unwrap_or("")
                                .to_string(),
                            output: update
                                .get("rawOutput")
                                .or_else(|| update.get("content"))
                                .map(ToString::to_string)
                                .unwrap_or_default(),
                        }),
                        _ => None,
                    };
                    if let Some(event) = event {
                        if events.send(event).is_err() {
                            return;
                        }
                    }
                }
            }
            continue;
        }
        let Some(id) = message.get("id").and_then(Json::as_i64) else {
            continue;
        };
        let pending = match state.lock() {
            Ok(mut s) => {
                let pending = s.requests.remove(&id);
                if let Some(PendingRequest::Prompt { remote, .. }) = &pending {
                    s.turns.remove(remote);
                }
                pending
            }
            Err(_) => return,
        };
        let Some(pending) = pending else {
            continue;
        };
        let result = message.get("result");
        let error = message
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Json::as_str)
            .unwrap_or("Invalid ACP response");
        let event = match pending {
            PendingRequest::Initialize => {
                if result
                    .and_then(|r| r.get("protocolVersion"))
                    .and_then(Json::as_i64)
                    == Some(1)
                {
                    AcpEvent::Connected
                } else {
                    AcpEvent::Error(error.to_string())
                }
            }
            PendingRequest::NewSession(key) => match result
                .and_then(|r| r.get("sessionId"))
                .and_then(Json::as_str)
                .filter(|s| !s.is_empty())
            {
                Some(id) => AcpEvent::SessionReady {
                    key,
                    session_id: id.to_string(),
                },
                None => AcpEvent::RequestFailed {
                    key,
                    message: error.to_string(),
                },
            },
            PendingRequest::Prompt { key, .. } => match result
                .and_then(|r| r.get("stopReason"))
                .and_then(Json::as_str)
            {
                Some(reason) => AcpEvent::TurnComplete {
                    key,
                    stop_reason: reason.to_string(),
                },
                None => AcpEvent::RequestFailed {
                    key,
                    message: error.to_string(),
                },
            },
        };
        if events.send(event).is_err() {
            return;
        }
    }
}
