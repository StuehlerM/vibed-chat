// Responses API client: direct chat with the Mistral Responses API.
// Mirrors AcpClient's shape: worker thread, command channel in, event
// channel out. All response parsing goes through json.rs; the HTTP/TLS
// transport is injected via the HttpTransport trait (platform backend).
#![allow(dead_code)]

use crate::http::SseParser;
use crate::json::{self, Json};
use crate::session::{HistoryItem, ReasoningEffort};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Events sent from the Responses worker thread to the UI.
#[derive(Debug)]
pub enum ResponsesEvent {
    /// A delta of answer text (response.output_text.delta).
    TextDelta { session_id: String, text: String },
    /// A delta of reasoning text.
    ReasoningDelta { session_id: String, text: String },
    /// The response finished successfully.
    Completed { session_id: String },
    /// The response failed (transport error, HTTP error, response.failed).
    Failed { session_id: String, message: String },
    /// The response stopped early (response.incomplete).
    Incomplete { session_id: String },
    /// Preflight input-token count arrived.
    InputTokens { session_id: String, tokens: u64 },
}

/// Commands sent from the UI to the Responses worker.
#[derive(Debug)]
pub enum ResponsesCommand {
    /// Send the history (including the new user message) and stream a reply.
    Send {
        session_id: String,
        history: Vec<HistoryItem>,
        reasoning: ReasoningEffort,
    },
    /// Count input tokens for the history without generating.
    CountInputTokens {
        session_id: String,
        history: Vec<HistoryItem>,
    },
    /// Abort the in-flight request (worker closes transport handles).
    Abort { session_id: String },
}

/// Result of one HTTP exchange: status code plus body bytes. The transport
/// decodes chunked transfer coding before invoking callbacks, so the core
/// only ever sees plain body bytes.
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Pluggable HTTPS transport, implemented by the platform backend.
pub trait HttpTransport {
    /// POST `body` to `https://host/path` with `headers`.
    /// Calls `on_body_bytes` for each group of decoded body bytes as it
    /// arrives. Returns the HTTP status code. Chunked transfer coding is
    /// decoded by the transport (it owns the response headers), so the
    /// callback only ever sees plain body bytes.
    fn post_stream(
        &self,
        host: &str,
        path: &str,
        headers: &[(String, String)],
        body: &str,
        on_body_bytes: &mut dyn FnMut(&[u8]),
    ) -> Result<u16, String>;

    /// POST `body` and return the complete decoded response. Default
    /// implementation buffers `post_stream`.
    fn post(
        &self,
        host: &str,
        path: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<HttpResponse, String> {
        let mut buf = Vec::new();
        let status =
            self.post_stream(host, path, headers, body, &mut |b| buf.extend_from_slice(b))?;
        Ok(HttpResponse { status, body: buf })
    }
}

pub const API_HOST: &str = "api.mistral.ai";
pub const RESPONSES_PATH: &str = "/v1/responses";
pub const INPUT_TOKENS_PATH: &str = "/v1/responses/input_tokens";
pub const DEFAULT_MODEL: &str = "mistral-large-latest";
pub const MAX_OUTPUT_TOKENS: i64 = 4096;

pub struct ResponsesClient {
    cmd_tx: Sender<ResponsesCommand>,
    event_rx: Receiver<ResponsesEvent>,
}

impl ResponsesClient {
    /// Create the client. `transport` moves to the worker thread.
    pub fn new(transport: Box<dyn HttpTransport + Send>) -> Self {
        let (cmd_tx, cmd_rx) = channel::<ResponsesCommand>();
        let (event_tx, event_rx) = channel::<ResponsesEvent>();
        thread::spawn(move || worker(transport, cmd_rx, event_tx));
        Self { cmd_tx, event_rx }
    }

    pub fn send(&self, session_id: &str, history: Vec<HistoryItem>, reasoning: ReasoningEffort) {
        let _ = self.cmd_tx.send(ResponsesCommand::Send {
            session_id: session_id.to_string(),
            history,
            reasoning,
        });
    }

    pub fn count_input_tokens(&self, session_id: &str, history: Vec<HistoryItem>) {
        let _ = self.cmd_tx.send(ResponsesCommand::CountInputTokens {
            session_id: session_id.to_string(),
            history,
        });
    }

    pub fn abort(&self, session_id: &str) {
        let _ = self.cmd_tx.send(ResponsesCommand::Abort {
            session_id: session_id.to_string(),
        });
    }

    pub fn try_recv(&self) -> Option<ResponsesEvent> {
        self.event_rx.try_recv().ok()
    }
}

fn worker(
    transport: Box<dyn HttpTransport + Send>,
    cmd_rx: Receiver<ResponsesCommand>,
    event_tx: Sender<ResponsesEvent>,
) {
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            ResponsesCommand::Send {
                session_id,
                history,
                reasoning,
            } => {
                if api_key().is_none() {
                    let _ = event_tx.send(ResponsesEvent::Failed {
                        session_id,
                        message: "No API key found. Set API_KEY or MISTRAL_API_KEY and restart."
                            .to_string(),
                    });
                    continue;
                }
                let body = build_request_body(&history, reasoning, true);
                let result = do_stream(transport.as_ref(), &body, &session_id, &event_tx);
                match result {
                    Ok(()) => {
                        let _ = event_tx.send(ResponsesEvent::Completed { session_id });
                    }
                    Err(msg) => {
                        let _ = event_tx.send(ResponsesEvent::Failed {
                            session_id,
                            message: msg,
                        });
                    }
                }
            }
            ResponsesCommand::CountInputTokens {
                session_id,
                history,
            } => {
                let body = build_request_body(&history, ReasoningEffort::Low, false);
                let res = transport.post(API_HOST, INPUT_TOKENS_PATH, &auth_headers(), &body);
                match res {
                    Ok(resp) if resp.status == 200 => {
                        match json::parse(&String::from_utf8_lossy(&resp.body)) {
                            Ok(j) => {
                                if let Some(n) = j.get("input_tokens").and_then(|v| v.as_i64()) {
                                    let _ = event_tx.send(ResponsesEvent::InputTokens {
                                        session_id,
                                        tokens: n.max(0) as u64,
                                    });
                                } else {
                                    let _ = event_tx.send(ResponsesEvent::Failed {
                                        session_id,
                                        message: "input_tokens response missing count".to_string(),
                                    });
                                }
                            }
                            Err(e) => {
                                let _ = event_tx.send(ResponsesEvent::Failed {
                                    session_id,
                                    message: format!("Bad input_tokens JSON: {}", e),
                                });
                            }
                        }
                    }
                    Ok(resp) => {
                        let msg = error_message_from_body(resp.status, &resp.body);
                        let _ = event_tx.send(ResponsesEvent::Failed {
                            session_id,
                            message: msg,
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(ResponsesEvent::Failed {
                            session_id,
                            message: e,
                        });
                    }
                }
            }
            ResponsesCommand::Abort { session_id: _ } => {
                // The transport closes handles on abort; nothing to do here
                // until the worker owns per-request handle state (P0 keeps
                // requests serialized, so abort applies to the next request).
            }
        }
    }
}

fn api_key() -> Option<String> {
    for var in ["API_KEY", "MISTRAL_API_KEY"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn auth_headers() -> Vec<(String, String)> {
    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    if let Some(key) = api_key() {
        headers.push(("Authorization".to_string(), format!("Bearer {}", key)));
    }
    headers
}

fn build_request_body(history: &[HistoryItem], reasoning: ReasoningEffort, stream: bool) -> String {
    let mut pairs: Vec<(&str, Json)> = vec![
        ("model", Json::String(DEFAULT_MODEL.to_string())),
        ("input", history_to_json(history)),
    ];
    match reasoning {
        ReasoningEffort::None => {}
        other => {
            pairs.push((
                "reasoning",
                Json::object(vec![("effort", Json::String(other.as_str().to_string()))]),
            ));
        }
    }
    pairs.push(("max_output_tokens", Json::Number(MAX_OUTPUT_TOKENS as f64)));
    if stream {
        pairs.push(("stream", Json::Bool(true)));
    }
    Json::object(pairs).to_string()
}

fn history_to_json(history: &[HistoryItem]) -> Json {
    let items: Vec<Json> = history.iter().map(history_item_to_json).collect();
    Json::Array(items)
}

fn history_item_to_json(item: &HistoryItem) -> Json {
    match item {
        HistoryItem::UserText { text } => Json::object(vec![
            ("role", Json::String("user".to_string())),
            ("content", Json::String(text.clone())),
        ]),
        HistoryItem::ModelMessage { text } => Json::object(vec![
            ("role", Json::String("assistant".to_string())),
            ("content", Json::String(text.clone())),
        ]),
        HistoryItem::ModelReasoning { text } => Json::object(vec![
            ("type", Json::String("reasoning".to_string())),
            ("content", Json::String(text.clone())),
        ]),
        HistoryItem::ToolSummary { text } => Json::object(vec![
            ("role", Json::String("user".to_string())),
            ("content", Json::String(text.clone())),
        ]),
    }
}

/// Perform one streaming request; emit TextDelta/ReasoningDelta/Incomplete
/// events as they arrive. Returns Ok(()) if the stream completed, Err on
/// transport/HTTP failure. `response.failed` / `error` events map to Err.
fn do_stream(
    transport: &dyn HttpTransport,
    body: &str,
    session_id: &str,
    event_tx: &Sender<ResponsesEvent>,
) -> Result<(), String> {
    let mut sse = SseParser::new();
    // Raw body copy: non-200 responses carry a JSON error document instead
    // of an SSE stream, so keep the bytes for error_message_from_body().
    let mut raw_body: Vec<u8> = Vec::new();
    let mut saw_completed = false;
    let mut incomplete = false;
    let mut failure: Option<String> = None;

    let mut on_chunk = |chunk: &[u8]| {
        // The transport has already decoded chunked transfer coding; these
        // bytes are the plain SSE body.
        raw_body.extend_from_slice(chunk);
        for (_name, data) in sse.extend(chunk) {
            if data == "[DONE]" {
                continue;
            }
            let ev = match json::parse(&data) {
                Ok(j) => j,
                Err(_) => continue,
            };
            let ev_type = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match ev_type {
                "response.output_text.delta" => {
                    if let Some(d) = ev.get("delta").and_then(|d| d.as_str()) {
                        let _ = event_tx.send(ResponsesEvent::TextDelta {
                            session_id: session_id.to_string(),
                            text: d.to_string(),
                        });
                    }
                }
                "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                    if let Some(d) = ev.get("delta").and_then(|d| d.as_str()) {
                        let _ = event_tx.send(ResponsesEvent::ReasoningDelta {
                            session_id: session_id.to_string(),
                            text: d.to_string(),
                        });
                    }
                }
                "response.completed" => {
                    saw_completed = true;
                }
                "response.incomplete" => {
                    incomplete = true;
                }
                "response.failed" => {
                    let msg = ev
                        .get("response")
                        .and_then(|r| r.get("error"))
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("Response failed")
                        .to_string();
                    failure = Some(msg);
                }
                "error" => {
                    let msg = ev
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Stream error")
                        .to_string();
                    failure = Some(msg);
                }
                _ => {}
            }
        }
    };

    let status = transport.post_stream(
        API_HOST,
        RESPONSES_PATH,
        &auth_headers(),
        body,
        &mut on_chunk,
    )?;

    if status != 200 {
        return Err(error_message_from_body(status, &raw_body));
    }
    if let Some(msg) = failure {
        return Err(msg);
    }
    if incomplete {
        let _ = event_tx.send(ResponsesEvent::Incomplete {
            session_id: session_id.to_string(),
        });
        return Ok(());
    }
    if !saw_completed {
        // Stream ended without a completion event: treat as truncation,
        // not a hard failure — partial text is already delivered.
        return Ok(());
    }
    Ok(())
}

/// Build a human-readable error from a non-200 body. The API returns JSON
/// like {"message": "..."} or {"error": {"message": "..."}} - surface that
/// instead of a bare status code so failures are diagnosable.
pub fn error_message_from_body(status: u16, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    if let Ok(j) = json::parse(&text) {
        if let Some(m) = j.get("message").and_then(|m| m.as_str()) {
            return format!("HTTP {}: {}", status, m);
        }
        if let Some(m) = j
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return format!("HTTP {}: {}", status, m);
        }
        if let Some(m) = j.get("detail").and_then(|m| m.as_str()) {
            return format!("HTTP {}: {}", status, m);
        }
    }
    let snippet: String = text.chars().take(200).collect();
    format!("HTTP {}: {}", status, snippet)
}

#[cfg(test)]
mod tests {
    struct MockTransport {
        status: u16,
        body: Vec<u8>,
    }
    impl HttpTransport for MockTransport {
        fn post_stream(
            &self,
            _host: &str,
            _path: &str,
            _headers: &[(String, String)],
            _body: &str,
            on_body_bytes: &mut dyn FnMut(&[u8]),
        ) -> Result<u16, String> {
            on_body_bytes(&self.body);
            Ok(self.status)
        }
    }

    #[test]
    fn error_body_becomes_readable_message() {
        let msg = error_message_from_body(
            401,
            b"{\"message\": \"Unauthorized\", \"request_id\": \"abc123\"}",
        );
        assert_eq!(msg, "HTTP 401: Unauthorized");
    }

    #[test]
    fn sse_stream_produces_text_deltas() {
        // Simulates the full Direct-mode request path with a mocked wire.
        use std::sync::mpsc::channel;
        let (tx, rx) = channel::<ResponsesEvent>();
        let body = b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\n\n\ndata: {\"type\":\"response.completed\"}\n\n".to_vec();
        let transport = MockTransport { status: 200, body };
        let event_tx = tx.clone();
        std::thread::spawn(move || {
            let result = do_stream(&transport, "{}", "sess", &event_tx);
            match result {
                Ok(()) => {
                    let _ = event_tx.send(ResponsesEvent::Completed {
                        session_id: "sess".to_string(),
                    });
                }
                Err(m) => {
                    let _ = event_tx.send(ResponsesEvent::Failed {
                        session_id: "sess".to_string(),
                        message: m,
                    });
                }
            }
        });
        let mut deltas = 0;
        let mut completed = false;
        for ev in rx
            .try_recv()
            .into_iter()
            .chain(std::iter::from_fn(|| rx.recv().ok()))
        {
            match ev {
                ResponsesEvent::TextDelta { text, .. } => {
                    assert_eq!(text, "Hi");
                    deltas += 1;
                }
                ResponsesEvent::Completed { .. } => {
                    completed = true;
                    break;
                }
                ResponsesEvent::Failed { message, .. } => {
                    panic!("stream failed: {}", message);
                }
                _ => {}
            }
        }
        assert_eq!(deltas, 1);
        assert!(completed);
    }

    #[test]
    fn non_200_stream_surfaces_api_message() {
        use std::sync::mpsc::channel;
        let (tx, rx) = channel::<ResponsesEvent>();
        let body = b"{\"message\": \"model not found\"}".to_vec();
        let transport = MockTransport { status: 404, body };
        let event_tx = tx.clone();
        std::thread::spawn(move || {
            let result = do_stream(&transport, "{}", "sess", &event_tx);
            let _ = event_tx.send(match result {
                Ok(()) => ResponsesEvent::Completed {
                    session_id: "sess".to_string(),
                },
                Err(m) => ResponsesEvent::Failed {
                    session_id: "sess".to_string(),
                    message: m,
                },
            });
        });
        let ev = rx.recv().expect("event");
        match ev {
            ResponsesEvent::Failed { message, .. } => {
                assert!(message.contains("model not found"), "got: {}", message);
                assert!(message.starts_with("HTTP 404"), "got: {}", message);
            }
            other => panic!("expected failure, got {:?}", other),
        }
    }

    use super::*;

    fn hist() -> Vec<HistoryItem> {
        vec![
            HistoryItem::UserText {
                text: "Hello".to_string(),
            },
            HistoryItem::ModelMessage {
                text: "Hi!".to_string(),
            },
            HistoryItem::UserText {
                text: "How are you?".to_string(),
            },
        ]
    }

    #[test]
    fn request_body_shape() {
        let body = build_request_body(&hist(), ReasoningEffort::High, true);
        let j = json::parse(&body).expect("valid json");
        assert_eq!(
            j.get("model").and_then(|m| m.as_str()),
            Some("mistral-large-latest")
        );
        assert!(j.get("stream").and_then(|s| s.as_bool()).unwrap_or(false));
        let input = j.get("input").and_then(|i| i.as_array()).unwrap();
        assert_eq!(input.len(), 3);
        assert_eq!(input[0].get("role").and_then(|r| r.as_str()), Some("user"));
    }

    #[test]
    fn reasoning_none_omitted() {
        let body = build_request_body(&hist(), ReasoningEffort::None, false);
        let j = json::parse(&body).expect("valid json");
        assert!(j.get("reasoning").is_none());
        assert!(j.get("stream").is_none());
    }

    #[test]
    fn history_serialization() {
        let j = history_to_json(&hist());
        let arr = j.as_array().unwrap();
        assert_eq!(
            arr[1].get("role").and_then(|r| r.as_str()),
            Some("assistant")
        );
    }
}
