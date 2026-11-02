// Direct Responses API client. Bounded queues, turn identity, and one terminal.
#![allow(dead_code)]

use crate::http::SseParser;
use crate::json::{self, Json};
use crate::session::{HistoryItem, ReasoningEffort, TurnKey};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug)]
pub enum StreamOutcome {
    Completed,
    Incomplete(String),
    Cancelled,
    Failed(String),
}
#[derive(Debug)]
pub enum ResponsesEvent {
    TextDelta {
        key: TurnKey,
        text: String,
    },
    ReasoningDelta {
        key: TurnKey,
        text: String,
    },
    Terminal {
        key: TurnKey,
        outcome: StreamOutcome,
    },
}
struct Request {
    key: TurnKey,
    history: Vec<HistoryItem>,
    reasoning: ReasoningEffort,
    cancelled: Arc<AtomicBool>,
}
pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
}
/// Transport delivers decoded entity bytes (never HTTP transfer framing).
/// Metadata must be delivered and accepted before any body bytes.
pub trait HttpTransport {
    fn post_stream(
        &self,
        host: &str,
        path: &str,
        headers: &[(String, String)],
        body: &str,
        cancelled: &AtomicBool,
        on_response: &mut dyn FnMut(&HttpResponse) -> Result<(), String>,
        on_chunk: &mut dyn FnMut(&[u8]),
    ) -> Result<(), String>;
}
pub const API_HOST: &str = "api.mistral.ai";
pub const RESPONSES_PATH: &str = "/v1/responses";
pub const DEFAULT_MODEL: &str = "mistral-large-latest";
pub const MAX_OUTPUT_TOKENS: i64 = 4096;

pub struct ResponsesClient {
    commands: SyncSender<Request>,
    events: Receiver<ResponsesEvent>,
    cancellations: Arc<Mutex<HashMap<TurnKey, Arc<AtomicBool>>>>,
}
impl ResponsesClient {
    pub fn new(transport: Box<dyn HttpTransport + Send>) -> Self {
        let (commands, requests) = sync_channel::<Request>(8);
        let (event_tx, events) = sync_channel(512);
        let cancellations = Arc::new(Mutex::new(HashMap::new()));
        let worker_cancellations = Arc::clone(&cancellations);
        thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let body = build_request_body(&request.history, request.reasoning, true);
                let outcome = do_stream(
                    transport.as_ref(),
                    &body,
                    &request.key,
                    &request.cancelled,
                    &event_tx,
                );
                if let Ok(mut map) = worker_cancellations.lock() {
                    map.remove(&request.key);
                }
                if event_tx
                    .send(ResponsesEvent::Terminal {
                        key: request.key,
                        outcome,
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        Self {
            commands,
            events,
            cancellations,
        }
    }
    pub fn send(
        &self,
        key: TurnKey,
        history: Vec<HistoryItem>,
        reasoning: ReasoningEffort,
    ) -> Result<(), String> {
        auth_headers()?; // fail locally before queueing an unauthenticated request
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut map = self
            .cancellations
            .lock()
            .map_err(|_| "Direct cancellation state poisoned")?;
        map.insert(key.clone(), cancelled.clone());
        if let Err(error) = self.commands.try_send(Request {
            key: key.clone(),
            history,
            reasoning,
            cancelled,
        }) {
            map.remove(&key);
            return Err(format!("Direct request queue unavailable: {error}"));
        }
        Ok(())
    }
    pub fn abort(&self, key: &TurnKey) {
        if let Ok(map) = self.cancellations.lock() {
            if let Some(cancelled) = map.get(key) {
                cancelled.store(true, Ordering::Release);
            }
        }
    }
    pub fn try_recv(&self) -> Option<ResponsesEvent> {
        self.events.try_recv().ok()
    }
}
impl Drop for ResponsesClient {
    fn drop(&mut self) {
        if let Ok(map) = self.cancellations.lock() {
            for flag in map.values() {
                flag.store(true, Ordering::Release);
            }
        }
    }
}
fn api_key() -> Option<String> {
    ["API_KEY", "MISTRAL_API_KEY"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.trim().is_empty()))
}
fn auth_headers() -> Result<Vec<(String, String)>, String> {
    let key = api_key().ok_or("Set API_KEY or MISTRAL_API_KEY before using Direct mode")?;
    if key.contains(['\r', '\n', '\0']) {
        return Err("Invalid API key header value".to_string());
    }
    Ok(vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Authorization".to_string(), format!("Bearer {key}")),
    ])
}
fn build_request_body(history: &[HistoryItem], reasoning: ReasoningEffort, stream: bool) -> String {
    let mut fields = vec![
        ("model", Json::String(DEFAULT_MODEL.to_string())),
        ("input", history_to_json(history)),
    ];
    if reasoning != ReasoningEffort::None {
        fields.push((
            "reasoning",
            Json::object(vec![(
                "effort",
                Json::String(reasoning.as_str().to_string()),
            )]),
        ));
    }
    fields.push((
        "max_output_tokens",
        Json::Integer(i128::from(MAX_OUTPUT_TOKENS)),
    ));
    if stream {
        fields.push(("stream", Json::Bool(true)));
    }
    Json::object(fields).to_string()
}
fn history_to_json(history: &[HistoryItem]) -> Json {
    Json::Array(
        history
            .iter()
            .filter_map(|item| match item {
                HistoryItem::UserText { text } | HistoryItem::ToolSummary { text } => {
                    Some(Json::object(vec![
                        ("role", Json::String("user".to_string())),
                        ("content", Json::String(text.clone())),
                    ]))
                }
                HistoryItem::ModelMessage { text } => Some(Json::object(vec![
                    ("role", Json::String("assistant".to_string())),
                    ("content", Json::String(text.clone())),
                ])),
                // Displayed reasoning is not an authenticated provider reasoning item.
                HistoryItem::ModelReasoning { .. } => None,
            })
            .collect(),
    )
}
fn do_stream(
    transport: &dyn HttpTransport,
    body: &str,
    key: &TurnKey,
    cancelled: &AtomicBool,
    events: &SyncSender<ResponsesEvent>,
) -> StreamOutcome {
    if cancelled.load(Ordering::Acquire) {
        return StreamOutcome::Cancelled;
    }
    let headers = match auth_headers() {
        Ok(h) => h,
        Err(e) => return StreamOutcome::Failed(e),
    };
    let mut sse = SseParser::new();
    let mut terminal = None;
    let mut status = 0;
    let mut error_body = Vec::new();
    // Metadata and body callbacks share only a Cell, not overlapping mutable borrows.
    let accepted_status = std::cell::Cell::new(0);
    let result = transport.post_stream(
        API_HOST,
        RESPONSES_PATH,
        &headers,
        body,
        cancelled,
        &mut |metadata| {
            status = metadata.status;
            accepted_status.set(status);
            if status == 200
                && !metadata
                    .content_type
                    .split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
            {
                return Err("Expected text/event-stream response".to_string());
            }
            Ok(())
        },
        &mut |bytes| {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            if accepted_status.get() != 200 {
                let remaining = 8192usize.saturating_sub(error_body.len());
                error_body.extend_from_slice(&bytes[..remaining.min(bytes.len())]);
                return;
            }
            for (_, data) in sse.extend(bytes) {
                if terminal.is_some() {
                    continue;
                }
                if data == "[DONE]" {
                    continue;
                }
                let event = match json::parse(&data) {
                    Ok(event) => event,
                    Err(e) => {
                        terminal = Some(StreamOutcome::Failed(format!("Malformed SSE JSON: {e}")));
                        break;
                    }
                };
                let event_type = event.get("type").and_then(Json::as_str).unwrap_or("");
                let delta = event.get("delta").and_then(Json::as_str);
                let event = match event_type {
                    "response.output_text.delta" => delta.map(|text| ResponsesEvent::TextDelta {
                        key: key.clone(),
                        text: text.to_string(),
                    }),
                    "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                        delta.map(|text| ResponsesEvent::ReasoningDelta {
                            key: key.clone(),
                            text: text.to_string(),
                        })
                    }
                    "response.completed" => {
                        terminal = Some(StreamOutcome::Completed);
                        None
                    }
                    "response.incomplete" => {
                        terminal = Some(StreamOutcome::Incomplete(
                            event
                                .get("response")
                                .and_then(|r| r.get("incomplete_details"))
                                .map(ToString::to_string)
                                .unwrap_or_else(|| "Provider stopped early".to_string()),
                        ));
                        None
                    }
                    "response.failed" | "error" => {
                        let error = event
                            .get("response")
                            .and_then(|r| r.get("error"))
                            .or_else(|| event.get("error"))
                            .unwrap_or(&event);
                        terminal = Some(StreamOutcome::Failed(
                            error
                                .get("message")
                                .and_then(Json::as_str)
                                .unwrap_or("Provider stream failed")
                                .to_string(),
                        ));
                        None
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    let _ = events.send(event);
                }
            }
        },
    );
    if cancelled.load(Ordering::Acquire) {
        return StreamOutcome::Cancelled;
    }
    if let Err(error) = result {
        return StreamOutcome::Failed(error);
    }
    if status != 200 {
        let detail = std::str::from_utf8(&error_body)
            .ok()
            .and_then(|body| json::parse(body).ok())
            .and_then(|j| {
                j.get("message")
                    .or_else(|| j.get("error").and_then(|e| e.get("message")))
                    .and_then(Json::as_str)
                    .map(str::to_string)
            });
        return StreamOutcome::Failed(format!(
            "HTTP {status}: {}",
            detail.unwrap_or_else(|| "Request rejected".to_string())
        ));
    }
    if let Some(error) = sse.error() {
        return StreamOutcome::Failed(error.to_string());
    }
    terminal.unwrap_or_else(|| {
        StreamOutcome::Incomplete("Stream ended before a completion event".to_string())
    })
}

#[cfg(test)]
mod tests {
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
        let j = json::parse(&build_request_body(&hist(), ReasoningEffort::High, true))
            .expect("valid json");
        assert_eq!(
            j.get("model").and_then(Json::as_str),
            Some("mistral-large-latest")
        );
        assert_eq!(j.get("stream").and_then(Json::as_bool), Some(true));
        let input = j.get("input").and_then(Json::as_array).unwrap();
        assert_eq!(input.len(), 3);
        assert_eq!(input[0].get("role").and_then(Json::as_str), Some("user"));
    }
    #[test]
    fn reasoning_none_omitted() {
        let j = json::parse(&build_request_body(&hist(), ReasoningEffort::None, false))
            .expect("valid json");
        assert!(j.get("reasoning").is_none());
        assert!(j.get("stream").is_none());
    }
    #[test]
    fn history_serialization() {
        let j = history_to_json(&hist());
        let array = j.as_array().unwrap();
        assert_eq!(
            array[1].get("role").and_then(Json::as_str),
            Some("assistant")
        );
    }
}
