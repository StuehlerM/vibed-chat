# vibed-chat

> **Disclaimer:** This is an independent, unofficial project. It is built for use with [vibe-cli](https://github.com/mistralai/vibe) but is **not** affiliated with, endorsed by, or sponsored by Mistral AI. This project does not contain any official Mistral trademarks or branding.

A minimal zero-dependency desktop chat application that connects to a [vibe-cli](https://github.com/mistralai/vibe) ACP agent. No GUI framework, no webview, no Electron. Just a small native binary.

## Supported Platforms

| Platform | Status |
|----------|--------|
| Windows  | Fully functional (Win32/GDI backend) |
| Linux    | Skeleton in place (`src/platform/linux/`) - not yet implemented |
| macOS    | Skeleton in place (`src/platform/macos/`) - not yet implemented |

The core application logic is platform-independent. Each platform has its own backend under `src/platform/<os>/` that implements the traits defined in `src/platform/backend.rs`.

## Features

- Connects to `vibe-acp` (the vibe-cli agent) over the Agent Client Protocol (ACP) via JSON-RPC 2.0 on stdio
- GDI-rendered message bubbles with markdown support (bold, italic, code, code blocks)
- Streaming responses with live text updates
- Sidebar with session list
- Dark theme with warm orange accents
- Per-monitor v2 DPI awareness (Windows)
- Mouse-wheel scrolling, clipboard support (Ctrl+C / Ctrl+V)
- Thinking/reasoning chunks (collapsible)
- Tool call and tool result display
- Permission request handling
- Folder picker for session working directory

## Prerequisites

- Rust toolchain (rustup)
- `vibe-acp` on PATH (the vibe-cli agent binary)
- `API_KEY` in environment (passed to the agent process)

On Windows, you also need Windows 10/11.

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
| `interaction.rs` | Mouse click handling, hover tracking, layout computation |
| `text.rs` | Date/time helpers, message previews (platform-independent text logic) |
| `ui.rs` | UI element state enums (PressedElement, FocusedElement, Toast) |

### Platform backends

| Backend | Path | Status |
|---------|------|--------|
| Windows | `src/platform/windows/` | Fully functional |
| Linux | `src/platform/linux/` | Skeleton (stub) |
| macOS | `src/platform/macos/` | Skeleton (stub) |

#### Windows backend (`src/platform/windows/`)

| Module | Responsibility |
|--------|---------------|
| `ffi.rs` | All Win32/GDI FFI declarations (types, constants, extern fns) |
| `window.rs` | Window class, WndProc, all WM_* message handling, window creation |
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
