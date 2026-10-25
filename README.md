# vibed-chat

> **Disclaimer:** This is an independent, unofficial project. It is built for use with [vibe-cli](https://github.com/mistralai/vibe) but is **not** affiliated with, endorsed by, or sponsored by Mistral AI. All Mistral trademarks and branding have been removed.

A minimal zero-dependency Windows desktop chat application that connects to a [vibe-cli](https://github.com/mistralai/vibe) ACP agent. Raw Win32/GDI — no GUI framework, no webview, no Electron. Just a ~1.5 MB binary.

## Features

- Connects to `vibe-acp` (the vibe-cli agent) over the Agent Client Protocol (ACP) via JSON-RPC 2.0 on stdio
- GDI-rendered message bubbles with markdown support (bold, italic, code, code blocks)
- Streaming responses with live text updates
- Sidebar with session list
- Dark theme with warm orange accents
- Per-monitor v2 DPI awareness
- Mouse-wheel scrolling, clipboard support (Ctrl+C / Ctrl+V)
- Thinking/reasoning chunks (collapsible)
- Tool call and tool result display
- Permission request handling
- Folder picker for session working directory

## Prerequisites

- Windows 10/11
- Rust toolchain (rustup)
- `vibe-acp` on PATH (the vibe-cli agent binary)
- `API_KEY` in environment (passed to the agent process)

## Build

```bash
# Debug build
cargo build

# Release build (optimized, ~1.5 MB, LTO, stripped)
cargo build --release
```

Output binary: `target\release\vibe-chat.exe`

## Run

```bash
cargo run --release
```

## Architecture

Zero external dependencies — only `std`. Everything is hand-written:

| Module | Responsibility |
|--------|---------------|
| `main.rs` | Entry point, DPI setup, window creation, message loop |
| `win32.rs` | All Win32/GDI FFI declarations (types, constants, extern fns) |
| `json.rs` | Hand-written JSON parser + serializer |
| `app.rs` | Application state, session management, ACP coordination |
| `events.rs` | Window class, WndProc, all WM_* message handling |
| `render/` | GDI double-buffered painting, text layout, word wrap, DPI scaling |
| `input.rs` | Keyboard input (WM_UNICHAR/WM_KEYDOWN), text editing, clipboard |
| `chat.rs` | Chat state: messages, scroll, input buffer, streaming |
| `acp.rs` | ACP client: spawn vibe-acp, JSON-RPC over stdio, reader/writer threads |
| `markdown.rs` | Minimal markdown parser to styled text runs |
| `theme.rs` | Color scheme and styling constants |
| `fonts.rs` | Font management (normal/bold/italic/code/label) |
| `interaction.rs` | Mouse interaction, hit testing, resizing |
| `session.rs` | Session data model |
| `text.rs` | Text measurement and layout helpers |
| `ui.rs` | UI component helpers |

## ACP Protocol Flow

```
UI (events.rs)         acp.rs                    vibe-acp (child process)
  │                       │                           │
  ├── spawn ─────────────►│── std::process::Command ─►│
  │                       │◄── stdout reader thread ──│
  │                       │                           │
  ├── initialize() ──────►│── JSON-RPC ──────────────►│
  │                       │◄── response ──────────────│
  │                       │                           │
  ├── session/new() ─────►│── JSON-RPC ──────────────►│
  │                       │◄── response ──────────────│
  │                       │                           │
  ├── session/prompt() ──►│── JSON-RPC ──────────────►│
  │                       │◄── session/update ────────│  (streaming text chunks)
  │◄── display chunk ────│                           │
  │                       │◄── prompt response ───────│  (stop reason)
  │◄── turn complete ────│                           │
```

## License

MIT
