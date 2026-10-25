// ACP client: spawn vibe-acp, JSON-RPC over stdio.
#![allow(dead_code)]

use crate::json::{self, Json};
use crate::win32::CREATE_NO_WINDOW;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Events sent from the ACP reader thread to the UI.
#[derive(Debug)]
pub enum AcpEvent {
    /// A chunk of streaming text from the agent
    StreamChunk { session_id: String, text: String },
    /// A chunk of the agent's thinking/reasoning (collapsible in UI)
    ThoughtChunk { session_id: String, text: String },
    /// The agent's turn is complete
    TurnComplete { session_id: String, stop_reason: String },
    /// A tool call was made
    ToolCall { session_id: String, name: String, input: String },
    /// A tool result came back
    ToolResult { session_id: String, call_id: String, output: String },
    /// Permission request from the agent
    PermissionRequest {
        id: Json,
        session_id: String,
        method: String,
        params: Json,
    },
    /// The agent process died
    ProcessDied { code: Option<i32> },
    /// An error occurred
    Error(String),
    /// Connection established (initialize succeeded)
    Connected,
    /// Session created
    SessionReady { session_id: String },
}

/// Commands sent from the UI to the ACP client.
pub enum AcpCommand {
    /// Send a prompt to the agent
    Prompt { session_id: String, text: String },
    /// Cancel the current prompt
    Cancel { session_id: String },
    /// Respond to a permission request
    PermissionResponse { id: Json, approved: bool },
    /// Shutdown the client
    Shutdown,
    /// Internal: send initialize
    InitInternal,
    /// Internal: create new session
    NewSessionInternal { cwd: String },
}

pub struct AcpClient {
    /// Channel to send commands to the writer thread
    cmd_tx: Sender<AcpCommand>,
    /// Channel to receive events from the reader thread
    event_rx: Receiver<AcpEvent>,
    /// Shared state
    state: Arc<Mutex<AcpState>>,
    /// Child process handle
    child: Arc<Mutex<Option<Child>>>,
}

struct AcpState {
    next_id: i64,
    sessions: Vec<String>,
    active_session: Option<String>,
    pending_permissions: Vec<Json>,
    /// Maps JSON-RPC request IDs to session IDs for prompt responses
    /// (PromptResponse does not include sessionId)
    pending_prompts: std::collections::HashMap<i64, String>,
}

impl AcpClient {
    /// Spawn vibe-acp and start communication threads.
    pub fn spawn() -> Result<Self, String> {
        Self::spawn_with_command("vibe-acp", &[])
    }

    pub fn spawn_with_command(program: &str, args: &[&str]) -> Result<Self, String> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        // Hide the console window - spawn vibe-acp without a visible CLI
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", program, e))?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to open stdout")?;

        let (cmd_tx, cmd_rx) = channel::<AcpCommand>();
        let (event_tx, event_rx) = channel::<AcpEvent>();

        let state = Arc::new(Mutex::new(AcpState {
            next_id: 1,
            sessions: Vec::new(),
            active_session: None,
            pending_permissions: Vec::new(),
            pending_prompts: std::collections::HashMap::new(),
        }));

        let child_arc = Arc::new(Mutex::new(Some(child)));
        let died_sent = Arc::new(AtomicBool::new(false));

        // Writer thread: receives commands, sends JSON-RPC to agent stdin
        {
            let state = Arc::clone(&state);
            let event_tx = event_tx.clone();
            thread::spawn(move || {
                writer_thread(stdin, cmd_rx, state, event_tx);
            });
        }

        // Reader thread: reads JSON-RPC from agent stdout
        {
            let state = Arc::clone(&state);
            let event_tx_reader = event_tx.clone();
            let died_sent = Arc::clone(&died_sent);
            thread::spawn(move || {
                reader_thread(stdout, state, event_tx_reader, died_sent);
            });
        }

        // Monitor thread: watches for child process exit
        {
            let child_arc = Arc::clone(&child_arc);
            let died_sent = Arc::clone(&died_sent);
            thread::spawn(move || {
                monitor_thread(child_arc, event_tx, died_sent);
            });
        }

        // Send initialize
        let _ = cmd_tx.send(AcpCommand::InitInternal);

        Ok(Self {
            cmd_tx,
            event_rx,
            state,
            child: child_arc,
        })
    }

    pub fn send_prompt(&self, session_id: &str, text: &str) {
        let _ = self.cmd_tx.send(AcpCommand::Prompt {
            session_id: session_id.to_string(),
            text: text.to_string(),
        });
    }

    pub fn try_recv(&self) -> Option<AcpEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn cancel(&self, session_id: &str) {
        let _ = self.cmd_tx.send(AcpCommand::Cancel {
            session_id: session_id.to_string(),
        });
    }

    /// Request creation of a new session (call after initialize succeeds).
    pub fn request_session(&self, cwd: &str) {
        let _ = self.cmd_tx.send(AcpCommand::NewSessionInternal { cwd: cwd.to_string() });
    }

    pub fn respond_permission(&self, id: Json, approved: bool) {
        let _ = self
            .cmd_tx
            .send(AcpCommand::PermissionResponse { id, approved });
    }

    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(AcpCommand::Shutdown);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        if let Ok(state) = self.state.lock() {
            !state.sessions.is_empty()
        } else {
            false
        }
    }

    pub fn active_session(&self) -> Option<String> {
        if let Ok(state) = self.state.lock() {
            state.active_session.clone()
        } else {
            None
        }
    }
}

// ── Writer thread ────────────────────────────────────────────

fn writer_thread(
    mut stdin: ChildStdin,
    cmd_rx: Receiver<AcpCommand>,
    state: Arc<Mutex<AcpState>>,
    event_tx: Sender<AcpEvent>,
) {
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            AcpCommand::InitInternal => {
                let id = {
                    let mut s = state.lock().unwrap();
                    let id = s.next_id;
                    s.next_id += 1;
                    id
                };
                let msg = Json::object(vec![
                    ("jsonrpc", Json::String("2.0".to_string())),
                    ("id", Json::Number(id as f64)),
                    ("method", Json::String("initialize".to_string())),
                    (
                        "params",
                        Json::object(vec![
                            ("protocolVersion", Json::Number(1.0)),
                            ("clientCapabilities", Json::object(vec![])),
                            (
                                "clientInfo",
                                Json::object(vec![
                                    ("name", Json::String("vibe-chat".to_string())),
                                    ("version", Json::String("0.1.0".to_string())),
                                ]),
                            ),
                        ]),
                    ),
                ]);
                if send_jsonrpc(&mut stdin, &msg) {
                    // Wait for response in reader thread
                } else {
                    let _ = event_tx.send(AcpEvent::Error("Failed to send initialize".to_string()));
                    return;
                }
            }
            AcpCommand::NewSessionInternal { cwd } => {
                let id = {
                    let mut s = state.lock().unwrap();
                    let id = s.next_id;
                    s.next_id += 1;
                    id
                };
                let msg = Json::object(vec![
                    ("jsonrpc", Json::String("2.0".to_string())),
                    ("id", Json::Number(id as f64)),
                    ("method", Json::String("session/new".to_string())),
                    (
                        "params",
                        Json::object(vec![
                            ("cwd", Json::String(cwd)),
                            ("mcpServers", Json::Array(vec![])),
                        ]),
                    ),
                ]);
                let _ = send_jsonrpc(&mut stdin, &msg);
            }
            AcpCommand::Prompt { session_id, text } => {
                let sid = {
                    let s = state.lock().unwrap();
                    if session_id.is_empty() {
                        s.active_session.clone().unwrap_or_default()
                    } else {
                        session_id
                    }
                };

                if sid.is_empty() {
                    let _ = event_tx.send(AcpEvent::Error(
                        "Session not ready, prompt queued".to_string(),
                    ));
                    continue;
                }

                let id = {
                    let mut s = state.lock().unwrap();
                    let id = s.next_id;
                    s.next_id += 1;
                    s.pending_prompts.insert(id, sid.clone());
                    id
                };
                let msg = Json::object(vec![
                    ("jsonrpc", Json::String("2.0".to_string())),
                    ("id", Json::Number(id as f64)),
                    ("method", Json::String("session/prompt".to_string())),
                    (
                        "params",
                        Json::object(vec![
                            ("sessionId", Json::String(sid)),
                            (
                                "prompt",
                                Json::Array(vec![Json::object(vec![
                                    ("type", Json::String("text".to_string())),
                                    ("text", Json::String(text)),
                                ])]),
                            ),
                        ]),
                    ),
                ]);
                if !send_jsonrpc(&mut stdin, &msg) {
                    let _ = event_tx.send(AcpEvent::Error("Failed to send prompt".to_string()));
                    return;
                }
            }
            AcpCommand::Cancel { session_id } => {
                let msg = Json::object(vec![
                    ("jsonrpc", Json::String("2.0".to_string())),
                    ("method", Json::String("session/cancel".to_string())),
                    (
                        "params",
                        Json::object(vec![("sessionId", Json::String(session_id))]),
                    ),
                ]);
                let _ = send_jsonrpc(&mut stdin, &msg);
            }
            AcpCommand::PermissionResponse { id, approved } => {
                let outcome = if approved { "allow" } else { "deny" };
                let msg = Json::object(vec![
                    ("jsonrpc", Json::String("2.0".to_string())),
                    ("id", id),
                    (
                        "result",
                        Json::object(vec![(
                            "outcome",
                            Json::object(vec![(
                                "outcome",
                                Json::String(outcome.to_string()),
                            )]),
                        )]),
                    ),
                ]);
                let _ = send_jsonrpc(&mut stdin, &msg);
            }
            AcpCommand::Shutdown => {
                return;
            }
        }
    }
}

fn send_jsonrpc(stdin: &mut ChildStdin, msg: &Json) -> bool {
    let line = msg.to_string();
    match writeln!(stdin, "{}", line) {
        Ok(_) => {
            let _ = stdin.flush();
            true
        }
        Err(_) => false,
    }
}

// ── Reader thread ────────────────────────────────────────────

fn reader_thread(stdout: ChildStdout, state: Arc<Mutex<AcpState>>, event_tx: Sender<AcpEvent>, died_sent: Arc<AtomicBool>) {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg = match json::parse(trimmed) {
            Ok(m) => m,
            Err(e) => {
                let _ = event_tx.send(AcpEvent::Error(format!("JSON parse error: {}", e)));
                continue;
            }
        };

        handle_message(&msg, &state, &event_tx);
    }

    // EOF - process ended. Only send ProcessDied if the monitor thread hasn't already.
    if !died_sent.swap(true, Ordering::SeqCst) {
        let _ = event_tx.send(AcpEvent::ProcessDied { code: None });
    }
}

fn handle_message(msg: &Json, state: &Arc<Mutex<AcpState>>, event_tx: &Sender<AcpEvent>) {
    // Check for requests/notifications first (has "method") - agent→client requests
    // like session/request_permission have both "id" and "method"
    if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
        let params = msg.get("params").cloned().unwrap_or(Json::Null);
        let request_id = msg.get("id").cloned();
        handle_notification(method, &params, request_id, state, event_tx);
        return;
    }

    // Response (has "id" and "result" or "error")
    if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
        if let Some(result) = msg.get("result") {
            handle_result(id, result, state, event_tx);
        } else if let Some(error) = msg.get("error") {
            let err_msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            let _ = event_tx.send(AcpEvent::Error(format!("RPC error: {}", err_msg)));
        }
    }
}

fn handle_result(
    id: i64,
    result: &Json,
    state: &Arc<Mutex<AcpState>>,
    event_tx: &Sender<AcpEvent>,
) {
    // Check if this is a session/new response
    if let Some(session_id) = result.get("sessionId").and_then(|s| s.as_str()) {
        let mut s = state.lock().unwrap();
        s.sessions.push(session_id.to_string());
        s.active_session = Some(session_id.to_string());
        drop(s);
        let _ = event_tx.send(AcpEvent::SessionReady {
            session_id: session_id.to_string(),
        });
        return;
    }

    // Check for stopReason (prompt response) - PromptResponse has no sessionId,
    // so look up the session from the pending_prompts map
    if let Some(stop_reason) = result.get("stopReason").and_then(|s| s.as_str()) {
        let session_id = {
            let mut s = state.lock().unwrap();
            s.pending_prompts.remove(&id).unwrap_or_default()
        };
        let _ = event_tx.send(AcpEvent::TurnComplete {
            session_id,
            stop_reason: stop_reason.to_string(),
        });
        return;
    }

    // Initialize response
    if result.get("protocolVersion").is_some() {
        let _ = event_tx.send(AcpEvent::Connected);
    }
}

fn handle_notification(
    method: &str,
    params: &Json,
    request_id: Option<Json>,
    state: &Arc<Mutex<AcpState>>,
    event_tx: &Sender<AcpEvent>,
) {
    // Extract sessionId from params for routing
    let session_id = params
        .get("sessionId")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();

    match method {
        "session/update" => {
            // The update payload is nested under params.update, discriminated by sessionUpdate
            if let Some(update) = params.get("update") {
                let update_type = update
                    .get("sessionUpdate")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                match update_type {
                    "agent_message_chunk" => {
                        if let Some(content) = update.get("content") {
                            if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                                let _ = event_tx.send(AcpEvent::StreamChunk {
                                    session_id: session_id.clone(),
                                    text: text.to_string(),
                                });
                            }
                        }
                    }
                    "agent_thought_chunk" => {
                        if let Some(content) = update.get("content") {
                            if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                                let _ = event_tx.send(AcpEvent::ThoughtChunk {
                                    session_id: session_id.clone(),
                                    text: text.to_string(),
                                });
                            }
                        }
                    }
                    "tool_call" => {
                        let name = update
                            .get("title")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let input = update
                            .get("rawInput")
                            .map(|i| i.to_string())
                            .unwrap_or_default();
                        let _ = event_tx.send(AcpEvent::ToolCall {
                            session_id: session_id.clone(),
                            name,
                            input,
                        });
                    }
                    "tool_call_update" => {
                        let output = update
                            .get("rawOutput")
                            .or_else(|| update.get("content"))
                            .map(|o| o.to_string())
                            .unwrap_or_default();
                        let call_id = update
                            .get("toolCallId")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string();
                        let _ = event_tx.send(AcpEvent::ToolResult {
                            session_id: session_id.clone(),
                            call_id,
                            output,
                        });
                    }
                    _ => {
                        // Other update types (usage_update, available_commands_update, etc.) - ignore
                    }
                }
            }
        }
        "session/request_permission" => {
            let id = request_id.clone().unwrap_or(Json::Null);
            let tool_call = params.get("toolCall");
            let method_name = tool_call
                .and_then(|tc| tc.get("title"))
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string();
            state.lock().unwrap().pending_permissions.push(id.clone());
            let _ = event_tx.send(AcpEvent::PermissionRequest {
                id,
                session_id: session_id.clone(),
                method: method_name,
                params: params.clone(),
            });
        }
        "session/cancel" => {
            let _ = event_tx.send(AcpEvent::TurnComplete {
                session_id,
                stop_reason: "cancelled".to_string(),
            });
        }
        _ => {
            // Unknown notification, ignore
        }
    }
}

// ── Monitor thread ───────────────────────────────────────────

fn monitor_thread(child_arc: Arc<Mutex<Option<Child>>>, event_tx: Sender<AcpEvent>, died_sent: Arc<AtomicBool>) {
    loop {
        thread::sleep(std::time::Duration::from_millis(500));
        let mut child_lock = child_arc.lock().unwrap();
        if let Some(child) = child_lock.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !died_sent.swap(true, Ordering::SeqCst) {
                        let _ = event_tx.send(AcpEvent::ProcessDied {
                            code: status.code(),
                        });
                    }
                    return;
                }
                Ok(None) => continue,
                Err(_) => {
                    if !died_sent.swap(true, Ordering::SeqCst) {
                        let _ = event_tx.send(AcpEvent::ProcessDied { code: None });
                    }
                    return;
                }
            }
        } else {
            return;
        }
    }
}
