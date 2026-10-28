# Responses API Integration — Design Doc

> Status: proposal. No code yet. This document defines the architecture for
> integrating the [Mistral Responses API](https://docs.mistral.ai/studio/conversations/responses)
> (public preview) alongside the existing vibe-cli ACP agent path.

## 1. Motivation

The app currently talks to a single backend: `vibe-acp` over ACP (JSON-RPC on
stdio). The Responses API becomes the **default backend** — plain chat with
reasoning, no agent process required — while vibe-cli becomes **Code mode**,
 opt-in for tool-using project work. The user switches between the two with
a radio button in the UI (see §3.3). The Responses feature set maps almost
1:1 onto the app's existing message model:

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

Why make Responses the default and vibe-cli opt-in?

1. **No agent process required for the common case.** Default mode works
   without `vibe-acp` on PATH. The app becomes a standalone Mistral chat
   client out of the box; Code mode is an explicit opt-in for agentic work.
2. **Reasoning control.** `reasoning.effort` is exposed per-request; the ACP
   path hides it. A settings toggle (low/medium/high) is meaningful here.
3. **Token accounting.** `/v1/responses/input_tokens` lets us show a live
   context-usage estimate next to the input box — impossible via ACP today.

## 2. Non-goals (for this phase)

- Removing the ACP path. Code mode stays first-class for project sessions.
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

### 3.3 Session modes + radio switch

`Session` gains a mode field, default `Direct` ("Default mode"):

```rust
pub enum SessionMode {
    /// Default mode: plain chat via the Responses API. No agent process.
    Direct,
    /// Code mode: agentic project work via vibe-acp (tools, permissions).
    Code,
}
```
- Per-session, persisted (additive field; `persistence.rs` readers default
  a missing field to `Direct` for backward compatibility).
- **Radio-button switch** in the header bar (rendered by `render/header.rs`,
  hit-tested by `interaction.rs`): two options side by side, `Default` and
  `Code`. Exactly one is selected at a time — radio semantics, not toggles.
  Selection state lives in the per-session `Settings`-independent layout
  cache so paint and hit-testing stay in sync (same pattern as
  `SettingsLayout` in `theme.rs`).
- Switching is allowed mid-session. Both modes read from the same
  `Session.chat` message list, so history carries over. Only the *next*
  turn uses the new backend; the current turn is unaffected.
  - `Direct → Code`: the next prompt opens/uses an ACP session
    (`session/new` on first Code turn; cached `acp_session_id` after).
    ACP does not receive the pre-switch Direct history — the agent's
    context starts at switch time (keeping ACP's working-dir semantics
    intact).
  - `Code → Direct`: the next prompt ships the full message history
    (including tool calls/results rendered as text summaries) as
    `HistoryItem`s.
- Mode is enforced at send time in `app.rs`: `Code` → `AcpCommand::Prompt`;
  `Direct` → build `Vec<HistoryItem>` from the session's messages, push
  `ResponsesCommand::Send`.
- Header also shows the active mode as a small badge next to the session
  name so the mode is visible without looking at the radio control.
- `Direct` sessions never touch ACP: no `session/new`, no `session/prompt`,
  no permission requests. When a session has never used Code mode, the
  `AcpClient` need not be spawned at all — the app only spawns `vibe-acp`
  lazily, on the first Code-mode turn (or new Code session).


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
| **P0** | `http.rs` core + WinHTTP transport, `ResponsesClient` non-streaming, `SessionMode` with `Direct` default, header radio switch (Default/Code), lazy ACP spawn, send-path branch | New session opens in Default mode; radio switches a follow-up turn to Code mode and back; full reply renders as one bubble |
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
| `src/session.rs` | `SessionMode` enum (`Direct` default) + `Session.mode` |
| `src/app.rs` | mode branch on send, lazy ACP spawn, `ResponsesClient` ownership |
| `src/persistence.rs` | serialize/deserialize `mode` (default `Direct`) |
| `src/platform/windows/render/header.rs` | radio switch (Default/Code) + mode badge |
| `src/interaction.rs` | hit-testing for the radio control |
| `src/theme.rs` | radio layout constants (paint/hit-test shared source) |
