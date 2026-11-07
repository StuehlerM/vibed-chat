# vibe-chat

> **Disclaimer:** This is an independent, unofficial project. It is built for use with [vibe-cli](https://github.com/mistralai/vibe) but is **not** affiliated with, endorsed by, or sponsored by Mistral AI. This project does not contain any official Mistral trademarks or branding.

A minimal zero-dependency desktop chat application with two backends: **Chat** (the Direct backend) sends text to the Mistral Responses API; **Code** connects to a [vibe-cli](https://github.com/mistralai/vibe) ACP agent. No GUI framework, no webview, no Electron. Just a small native binary.

## Supported Platforms

| Platform | Status |
|----------|--------|
| Windows  | Implemented Win32/GDI desktop backend; runtime acceptance still required |
| Linux    | Skeleton in place (`src/platform/linux/`) - not yet implemented |
| macOS    | Skeleton in place (`src/platform/macos/`) - not yet implemented |

The core application logic is platform-independent. Each platform has its own backend under `src/platform/<os>/` that implements the traits defined in `src/platform/backend.rs`.

## Features

- Direct mode (default): streamed Responses API requests over Windows WinHTTP
- Code mode: queued first send, agent initialization, per-chat remote session, explicit permission choices
- Turn-correlated streaming, cancellation signals, and persisted completed/failed/incomplete/interrupted outcomes
- Generation-based snapshots and read-only recovery for damaged or unsupported storage
- Connects to `vibe-acp` (the vibe-cli agent) over the Agent Client Protocol (ACP) via JSON-RPC 2.0 on stdio
- GDI-rendered message bubbles with markdown support (bold, italic, code, code blocks)
- Streaming responses with live text updates
- Adaptive history sidebar/drawer and docked settings or a dedicated settings page
- Windows RichEdit composer/rename fields with native selection, IME, clipboard and undo/redo
- Native primary controls with accessibility names; retained-canvas accessibility is still incomplete
- Dark theme with warm orange accents
- Per-monitor v2 DPI awareness (Windows)
- Mouse-wheel scrolling, clipboard support (Ctrl+C / Ctrl+V)
- Thinking/reasoning chunks (collapsible)
- Tool call and tool result display
- Permission request handling
- Folder picker for session working directory

## Prerequisites

- Windows 10/11 for the desktop application
- Rust **1.85 or newer**, edition 2021 (rustup)
- For the default MSVC target: Visual Studio Build Tools with **Desktop development with C++**, including the MSVC linker and Windows SDK
- **Direct:** `API_KEY`, falling back to `MISTRAL_API_KEY` when the former is absent/blank. Missing credentials fail locally before network submission.
- **Code:** `vibe-acp` on PATH and credentials supported by your installed agent. The child inherits the environment; its credential policy may differ from Direct.

Direct currently requests `mistral-large-latest` at `https://api.mistral.ai/v1/responses`, with a 4096-output-token limit and no requested reasoning effort by default. This endpoint/model contract still needs live provider verification; successful compilation does not establish API compatibility. Credentials are read from the environment rather than a settings file. Avoid including secrets in prompts/tool output: transcript storage is plaintext.

Start the application from an environment containing the key. For example, in PowerShell:

```powershell
$env:API_KEY = "your-key" # Do not commit this value or store it in repository files.
cargo run --release
```

Code starts the agent lazily when a Code turn needs it. The first Send is retained through initialization/session creation. Tool permission prompts show the operation and offered scopes; dismissal cancels the request. There is no global automatic approval.

## Build

```bash
# Debug build
cargo build

# Release build (optimized, LTO, stripped)
cargo build --release
```

On Windows, the output binary is `target\release\vibe-chat.exe`.

## Run

```bash
cargo run --release
```

## Windows editing and layout

The composer and inline rename use the OS Unicode RichEdit control from
`Msftedit.dll`, loaded from System32. Native editing provides mouse caret/selection,
word selection, clipboard commands, undo/redo, and IME/candidate-window behavior.
These behaviors still need runtime acceptance on Windows. If native initialization
or loading/reading a draft fails, the app warns and falls back to basic editing,
keeping the last synchronized draft rather than silently submitting unreadable
native text. The composer grows using native text layout, capped at
about eight lines and available height, then scrolls internally.

Rename selects the existing name. Enter commits; Escape, Tab, or clicking another
app control cancels. Navigating to another chat cancels unfinished IME composition
rather than transferring it to the next chat. Caret/selection are retained in memory per
chat; the native undo history resets when the editor is rebound or its document
is replaced (including submission).

Below 800 logical client pixels, **History** opens a drawer with a Close history
action. Settings docks only when a usable chat column remains; otherwise it opens
as a dedicated page. Its title/close area stays fixed, while the body supports
wheel/scrollbar input and scrolls focused settings into view.

Existing application shortcuts are unchanged:

| Binding | Action |
| --- | --- |
| Enter | Send once when idle; preserve the next draft while busy |
| Shift+Enter | Insert a newline in the composer |
| Escape | Cancel rename, close a transient panel, otherwise stop the active turn; never erase an idle draft |
| Tab / Shift+Tab | Move focus; rename follows the cancel-on-leave policy |
| Ctrl+N | Pick a project folder, **not** New chat |
| Ctrl+Shift+A / Ctrl+Shift+D | Archive / confirm deletion of the active chat |
| F5 | Start/restart the Code agent, even when Chat is selected |

Standard native editor commands (including Ctrl+A/C/X/V/Z/Y) and radio-arrow
navigation are control behavior, not remapped application shortcuts. The native
fields, primary buttons, mode radios, settings checkboxes/scrollbar and status use
Windows accessibility providers; names/descriptions are added through MSAA
annotations. Painted history, transcript and message actions still lack a complete
accessibility tree. Narrator, high-contrast and text-scaling acceptance is **not**
claimed.

## Context, cancellation, and storage

- Changing modes affects future turns, not the backend of a running turn. Code starts a fresh agent context after reconnect; it does **not** receive the prior Direct transcript. Direct sends visible user/assistant text, including Code assistant text, but excludes system/tool activity and displayed reasoning. There is no automatic tool-summary handoff.
- Stop signals the specific turn. Direct aborts its worker-owned WinHTTP request; Code sends agent cancellation. Deadlines settle nonresponsive work, and late events are ignored. Cancellation does not undo tool side effects or guarantee a provider stops billing immediately. Live cancellation acceptance remains outstanding.
- Windows storage defaults to `%APPDATA%\\vibe-chat`. `state.json` references immutable files under `sessions/`; `state.json.bak` retains the previous index. Chats, displayed thinking, tool notices, and drafts are **plaintext**, not encrypted. Protect this directory as sensitive data.
- Version-1 ACP histories load as Code chats. New saves publish version-2 generations; originals/recovery generations are retained rather than silently trimmed. This can consume disk space; automatic generation garbage collection is not implemented.
- Damaged/future-version data opens read-only, with an offer to inspect the backup. Copy the entire directory before manually repairing/migrating it. Recovery mode never replaces originals with an empty history. Save errors remain visible and are reported again at shutdown.
- Drafts are debounced and active partial output is checkpointed; an abrupt crash can still lose changes since the latest successful checkpoint. A single bounded worker serializes and publishes complete revisions in order, coalescing pending saves; the UI captures owned storage-only text/metadata. Shutdown waits up to five seconds and reports failure rather than racing a second writer. History cleanup runs only after its removal revision is published.

## Verification and implementation status

```bash
cargo fmt --check
cargo check --all-targets
cargo check --target x86_64-pc-windows-msvc
cargo clippy --all-targets
cargo +1.85.0 check --all-targets
cargo build --release
```

CI checks stable and MSRV builds, formatting, focused Clippy diagnostics, and Windows debug/release linking. Linux/macOS checks cover the core only; releases contain a Windows binary only. See [remediation status](docs/REMEDIATION_STATUS.md) for source changes, remaining review items, the Windows native acceptance matrix, and the distinction between compilation and manual/live verification.

## Architecture

Zero external dependencies - only `std`. Everything is hand-written.

The codebase is split into **platform-independent core** and **platform backends**:

### Core (platform-independent)

| Module | Responsibility |
|--------|---------------|
| `main.rs` | Thin entry point, dispatches to `platform::run()` |
| `platform/mod.rs` | Platform module selection via `cfg` |
| `platform/backend.rs` | Traits: `Platform`, `Clipboard`, `Dialogs` |
| `json.rs` | Hand-written JSON parser + serializer |
| `app.rs` | Application state, session management, ACP coordination |
| `chat.rs` | Chat state: messages, scroll, input buffer, streaming |
| `session.rs` | Session and Project data models |
| `acp.rs` | ACP client: spawn vibe-acp, JSON-RPC over stdio, reader/writer threads |
| `markdown.rs` | Minimal markdown parser to styled text runs |
| `theme.rs` | Color scheme, spacing tokens, layout constants, Settings |
| `input.rs` | Platform-independent keyboard input mapping and text editing |
| `interaction.rs` | Stable pointer commands, hover tracking, keyboard focus and sidebar geometry |
| `layout.rs` | Shared DPI-scaled adaptive panel and header-launcher geometry |
| `text.rs` | Date/time helpers, message previews (platform-independent text logic) |
| `ui.rs` | UI element state enums (PressedElement, FocusedElement, Toast) |

### Platform backends

| Backend | Path | Status |
|---------|------|--------|
| Windows | `src/platform/windows/` | Implemented; see verification status above |
| Linux | `src/platform/linux/` | Skeleton (stub) |
| macOS | `src/platform/macos/` | Skeleton (stub) |

#### Windows backend (`src/platform/windows/`)

| Module | Responsibility |
|--------|---------------|
| `ffi.rs` | All Win32/GDI FFI declarations (types, constants, extern fns) |
| `window.rs` | Window class, WndProc, native effect dispatch and window creation |
| `native.rs` | UI-thread native editors/primary controls, snapshot synchronization and resource lifetime |
| `accessibility.rs` | Native-control MSAA names/descriptions and discrete status notifications |
| `render/` | GDI double-buffered painting (sidebar, messages, input, header, settings, toast) |
| `fonts.rs` | Font management (normal/bold/italic/code/label) |
| `gfx.rs` | GDI+ anti-aliased rounded rectangles (loaded dynamically) |
| `paint.rs` | PaintContext: bundles all data for a GDI paint pass |
| `text.rs` | GDI text measurement and word wrapping |
| `clipboard.rs` | Win32 clipboard implementation |
| `dialogs.rs` | Win32 dialog implementations (folder picker, confirm, error) |

## ACP Protocol Flow

```
UI (window.rs)         acp.rs                    vibe-acp (child process)
  │                       │                           │
  ├── spawn ──────────────▶│── std::process::Command ──▶│
  │                       │◀── stdout reader thread ───│
  │                       │                           │
  ├── initialize() ───────▶│── JSON-RPC ───────────────▶│
  │                       │◀── response ───────────────│
  │                       │                           │
  ├── session/new() ──────▶│── JSON-RPC ───────────────▶│
  │                       │◀── response ───────────────│
  │                       │                           │
  ├── session/prompt() ───▶│── JSON-RPC ───────────────▶│
  │                       │◀── session/update ─────────│  (streaming text chunks)
  │◀── display chunk ─────│                           │
  │                       │◀── prompt response ────────│  (stop reason)
  │◀── turn complete ─────│                           │
```

## License

MIT
