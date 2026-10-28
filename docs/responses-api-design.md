# Responses API Integration — Design Doc

> Status: proposal. No code yet. This document defines the architecture for
> integrating the [Mistral Responses API](https://docs.mistral.ai/studio/conversations/responses)
> (public preview) alongside the existing vibe-cli ACP agent path.

## 1. Motivation

The app currently talks to a single backend: `vibe-acp` over ACP (JSON-RPC on
stdio). The Responses API gives us a second, complementary backend — direct
model access over HTTPS — with a feature set that maps almost 1:1 onto the
app's existing message model:

| Responses API feature | Existing vibed-chat concept |
|---|---|
| `response.output_text.delta` streaming events | `AcpEvent::StreamChunk` |
| Reasoning output items | `Message.thinking` / `ThoughtChunk` |
| `function_call` / `function_call_output` items | `ToolCall` / `ToolResult` |
| Statelesssness (client keeps history, sends it in `input`) | `Session` + persisted messages |
| `text.format` (JSON / JSON Schema) | new: structured output |
| `POST /v1/responses/input_tokens` | new: context-usage indicator |
| `input_image` / `input_file` content items | new: attachment support |

The API being **stateless** is the key architectural fit: we already own the
conversation history client-side (`persistence.rs`), so a direct-mode client
needs no server-side session state, no additional protocol machinery — just an
HTTP request per turn carrying the full history.

Why bother when vibe-cli already wraps the model? Three concrete gains:

1. **No agent process required.** Direct mode works when `vibe-acp` is not on
   PATH, is crashing, or when the user only wants a plain chat without
   tools/permissions. It makes the app a standalone Mistral chat client.
2. **Reasoning control.** `reasoning.effort` is exposed per-request; the ACP
   path hides it. A settings toggle (low/medium/high) is meaningful here.
3. **Token accounting.** `/v1/responses/input_tokens` lets us show a live
   context-usage estimate next to the input box — impossible via ACP today.

## 2. Non-goals (for this phase)

- Replacing the ACP path. Agent mode stays the default for project sessions.
- Tool use in direct mode (`tools` / function calling). Direct mode is plain
  chat + reasoning first; client-executed function calling is a later phase
  (it requires a permission UI equivalent and function-registry design).
- Linux/macOS backends for HTTP. Phase targets Windows (`winhttp`); the core
  client code is platform-independent so ports only need a TLS transport.

## 3. Architecture

### 3.1 New module: `src/responses.rs` (core)

A `ResponsesClient` mirroring `AcpClient`'s shape (worker thread(s), command
channel in, event channel out) so `AppState` can hold either behind a common
interface without caring which is live:

```
UI thread                     worker thread
──────────                     ─────────────
AppState ──AcpCommand──▶ AcpClient        (unchanged)
AppState ──ResponsesCommand──▶ ResponsesClient ──▶ transport (WinHTTP)
AppState ◀──ResponsesEvent──── ResponsesClient ◀── SSE / JSON body
```

Core types (all in `src/responses.rs`, platform-independent; JSON via
existing `src/json.rs`):

```rust
pub enum ResponsesCommand {
    Send { history: Vec<HistoryItem>, reasoning: ReasoningEffort },
    CountInputTokens { history: Vec<HistoryItem> },
    Abort,
}

pub enum ResponsesEvent {
    TextDelta { text: String },          // response.output_text.delta
    ReasoningDelta { text: String },      // reasoning deltas
    Completed { usage: Option<Usage> },   // response.completed
    Failed { message: String },           // response.failed / error
    Incomplete,                            // response.incomplete
    InputTokens { tokens: u64 },
}

pub enum ReasoningEffort { Low, Medium, High }
```

`HistoryItem` is a small enum (`UserText`, `ModelMessage`, `ModelReasoning`)
built from `Session.chat.messages`. We do **not** persist raw Responses
`output` items; we reconstruct `input` from our own `Message` model, which the
API accepts (stateless: `input` is a message array). This keeps
`persistence.rs` format unchanged and avoids serializing preview-only output
items back into requests.

### 3.2 New module: `src/http.rs` (core) + `platform/windows/http.rs` (FFI)

Zero-dependency HTTP is split the same way every other OS capability is:

- `src/http.rs` — pure: request serialization (build headers/body from
  `Json`), response parsing (chunked-transfer dechunking, SSE frame parsing,
  status-line/headers). Fully unit-testable with string fixtures.
- `platform/windows/http.rs` — the `TlsTransport` trait implementation using
  `WinHTTP` (`winhttp.dll`): `WinHttpOpen`, `WinHttpConnect`,
  `WinHttpOpenRequest`, `WinHttpSendRequest`, `WinHttpReceiveResponse`,
  `WinHttpReadData`. WinHTTP owns TLS (schannel), certificate validation, and
  proxy resolution — no SSPI code, no certificate pinning logic for us.
  New FFI declarations go in `platform/windows/ffi.rs` per repo convention
  (`extern "system"`, linking `"winhttp"`).

Trait in `platform/backend.rs` alongside `Clipboard`/`Dialogs`:

```rust
pub trait HttpTransport {
    /// POST `body` to `https://host/path`, invoke `on_chunk` per body chunk.
    /// Returns final status code. `on_chunk` may be called from this call.
    fn post_stream(&self, host: &str, path: &str, headers: &[(String, String)],
                   body: &str, on_chunk: &mut dyn FnMut(&[u8])) -> Result<u16, String>;
}
```

Streaming with `WinHttpReadData` in a loop naturally delivers SSE frames to
`on_chunk`; the core `src/http.rs` SSE parser reassembles `response.*` events.
Chunked-transfer decoding is handled there too (the API streams SSE).

### 3.3 Session mode

`Session` gains a mode field (default `Agent`):

```rust
pub enum SessionMode { Agent, Direct }
```

- Per-session, persisted (additive field; `persistence.rs` readers default
  missing field to `Agent` for backward compatibility).
- Set at session creation; visible as a small badge in the header bar
  ("Agent" / "Direct API") rendered by `render/header.rs`.
- Direct sessions skip ACP entirely: no `session/new`, no `session/prompt`,
  no permission requests. The send path in `app.rs` branches on mode:
  `Agent` → `AcpCommand::Prompt`; `Direct` → build `Vec<HistoryItem>` from
  the session's messages, push `ResponsesCommand::Send`.

### 3.4 Event mapping into the UI

Direct-mode events reuse the same `ChatState` mutation paths as ACP events,
so `render/messages.rs`, thinking sections, and streaming repaint need no
changes:

| Responses event | UI action |
|---|---|
| `TextDelta` | append to streaming `Message.text`, set `runs_dirty` |
| `ReasoningDelta` | append to `Message.thinking`, `thinking_runs_dirty` |
| `Completed` | finalize bubble, update usage counter |
| `Failed` / `Incomplete` | error toast (`ToastKind::Error`), finalize bubble |
| `InputTokens` | hint row next to send button |

### 3.5 API key

Reuse the existing `API_KEY` env var (already the ACP convention here) as the
bearer token: `Authorization: Bearer <API_KEY>`. Never rendered, never
logged, never persisted. `MISTRAL_API_KEY` is accepted as a fallback if
`API_KEY` is unset, matching the SDK convention.

## 4. Phased delivery

| Phase | Scope | Definition of done |
|---|---|---|
| **P0** | `http.rs` core + WinHTTP transport, `ResponsesClient` non-streaming, `SessionMode::Direct`, header badge, send-path branch | Type a prompt in a Direct session → full reply renders as one bubble |
| **P1** | Streaming: `stream=true`, SSE parser, `TextDelta` → live bubble updates | Reply appears progressively, indistinguishable UX from agent streaming |
| **P2** | `ReasoningDelta` → thinking sections; `reasoning.effort` toggle in settings; `input_tokens` counter in hint row | Thinking section collapses/expands; counter updates per turn |
| **P3** | Attachments (`input_image`/`input_file` via base64 data URLs) and `text.format` structured output | Image question answered; JSON-mode request returns validated JSON |

Each phase is one PR. P0 is the only phase that touches FFI/transport; P1–P3
build on its interfaces.

## 5. Risks / open questions

- **SSE framing correctness.** The SSE parser must handle events split
  across `WinHttpReadData` chunks and multi-line `data:` payloads. Mitigate
  with pure-`src/http.rs` unit tests on string fixtures (allowed: tests in
  `src/` with `#[cfg(test)]`, no new deps).
- **Preview API churn.** Public preview; expect field changes. Keep all
  response-shape parsing isolated in `src/responses.rs` behind small parse
  functions returning `Option` — never `.unwrap()` on wire data.
- **History size.** Stateless API means full history per turn; long sessions
  grow request size. The P2 token counter surfaces this to the user; a later
  truncation policy can follow the same `HistoryItem` seam.
- **Cancellation.** ACP has abort; direct mode needs `WinHttpCloseHandle`
  from the worker thread on `ResponsesCommand::Abort`. Designed in from P0
  (the transport owns its handles; abort closes them).

## 6. Files touched (P0 summary)

| File | Change |
|---|---|
| `src/responses.rs` | new: client, command/event enums, history mapping, request/response JSON |
| `src/http.rs` | new: header/body building, chunked decode, SSE parse (pure) |
| `src/platform/backend.rs` | add `HttpTransport` trait |
| `src/platform/windows/http.rs` | new: WinHTTP transport impl |
| `src/platform/windows/ffi.rs` | WinHTTP declarations, `link winhttp` |
| `src/session.rs` | `SessionMode` enum + `Session.mode` |
| `src/app.rs` | mode branch on send, `ResponsesClient` ownership |
| `src/persistence.rs` | serialize/deserialize `mode` (default `Agent`) |
| `src/platform/windows/render/header.rs` | mode badge |
