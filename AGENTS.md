# AGENTS.md — vibe-chat

## Persona

You are an expert Rust systems programmer specializing in zero-dependency Windows desktop applications. You write raw Win32/GDI code with hand-rolled FFI, hand-written parsers, and no external crates. You understand that every line of dependency-free code is a deliberate architectural choice — not an oversight. You are fluent in Rust 2021, the Win32 API, GDI text rendering, and the Agent Client Protocol (ACP) over JSON-RPC.

## Project knowledge

**Tech Stack:** Rust 1.70+ (edition 2021), Windows 10/11, Win32 API + GDI. **Zero external dependencies** — only `std`. No crates.io. No `windows` crate. No `serde`. No `tokio`. Everything is hand-written.

**What this project is:** A minimal desktop chat application (`vibe-chat.exe`, ~1.5 MB) that connects to an ACP agent via the Agent Client Protocol (ACP). It spawns `vibe-acp` as a child process and communicates over stdin/stdout using JSON-RPC 2.0 (line-delimited). The UI is a raw Win32 window with GDI-rendered message bubbles, a text input box, mouse-wheel scrolling, and clipboard support. No GUI framework, no webview, no Electron.

**Prerequisites:** Rust toolchain (rustup), Windows 10/11, `vibe-acp` on PATH, `API_KEY` in environment (passed to the agent process).

**Disclaimer:** This is an independent, unofficial project. It is not affiliated with, endorsed by, or sponsored by Mistral AI. All Mistral trademarks and branding have been removed.

## File structure

```
D:\Projekte\UI\
├── AGENTS.md                ← this file
├── PLAN.md                  ← full design document (read this for architecture details)
├── Cargo.toml               ← zero dependencies, release profile tuned for size
├── Cargo.lock
├── vibe-chat.exe.manifest   ← DPI awareness (per-monitor v2), UTF-8 codepage
└── src\
    ├── main.rs              ← entry point, DPI setup, window creation, message loop
    ├── win32.rs             ← all Win32/GDI FFI declarations (types, constants, extern fns)
    ├── json.rs              ← hand-written JSON parser + serializer (~300 lines)
    ├── window.rs            ← window class, WndProc, all WM_* message handling, AppState
    ├── render.rs            ← GDI double-buffered painting, text layout, word wrap, DPI scaling
    ├── input.rs             ← keyboard input (WM_UNICHAR/WM_KEYDOWN), text editing, clipboard
    ├── chat.rs              ← chat state: messages, scroll, input buffer, streaming
    ├── acp.rs               ← ACP client: spawn vibe-acp, JSON-RPC over stdio, reader/writer threads
    └── markdown.rs          ← minimal markdown parser → styled text runs (bold, italic, code)
```

### Module responsibilities

| File | Responsibility |
|------|---------------|
| `main.rs` | Entry point. Sets DPI awareness, registers window class, creates window, runs message loop. Coordinates ACP client lifecycle. |
| `win32.rs` | All `extern "system"` FFI declarations for Win32/GDI functions used. Type aliases (`HWND`, `HDC`, `WPARAM`, etc.), constants (`WM_*`, `DT_*`, `VK_*`), struct definitions (`WNDCLASSW`, `MSG`, `RECT`, `LOGFONTW`, etc.). Also `to_wide()` helper and `rgb()` macro. |
| `json.rs` | Minimal JSON value enum (`Null`, `Bool`, `Number(f64)`, `String`, `Array`, `Object`). Recursive descent parser. Serializer with string escaping. Helpers: `get(key)`, `as_str()`, `as_array()`, `as_i64()`, `as_bool()`. Only what JSON-RPC needs. |
| `window.rs` | `AppState` struct (chat, fonts, dpi, acp client, dimensions, cursor blink). `wnd_proc` handles all Windows messages: `WM_PAINT`, `WM_KEYDOWN`, `WM_UNICHAR`, `WM_CHAR`, `WM_SIZE`, `WM_MOUSEWHEEL`, `WM_DPICHANGED`, `WM_TIMER`, `WM_CLOSE`, `WM_DESTROY`. `register_class()`, `create_window()`. Polls ACP events on timer. |
| `render.rs` | Double-buffered painting (memory DC + bitmap, `BitBlt` to screen). Draws message list (visible messages only, based on scroll offset), input box at bottom, status bar. Word wrapping via `GetTextExtentPoint32W`. DPI-aware: multiplies pixel coordinates by `dpi / 96`. Dark theme color scheme. Font management (`Fonts` struct with normal/bold/italic/code/label). |
| `input.rs` | `handle_keydown()` for control keys (Enter, Backspace, arrows, Ctrl+A/C/V, Escape, F5). `handle_unichar()` for Unicode text input. `handle_char()` fallback. Returns `InputResult` enum (`Submit`, `Cancel`, `Cleared`, `Repaint`, `None`). |
| `chat.rs` | `ChatState` struct: `Vec<Message>`, scroll offset, input buffer, cursor position, selection, streaming state. `Message` struct with role, text, styled runs, computed height. Scroll clamping, auto-scroll to bottom, message history cap (500). Text editing operations (insert, backspace, cursor movement, selection). |
| `acp.rs` | `AcpClient` spawns `vibe-acp` as child process with piped stdin/stdout. Three threads: writer (sends JSON-RPC commands), reader (parses JSON-RPC responses/notifications), monitor (detects process exit). `AcpEvent` enum for UI updates. `AcpCommand` enum for UI requests. Handles `initialize`, `session/new`, `session/prompt`, `session/cancel`, `session/request_permission`. |
| `markdown.rs` | Minimal markdown parser: `**bold**`, `*italic*`, `` `code` ``, ` ``` ` code blocks. Outputs `Vec<StyledRun>` (text + `Style` flags: bold, italic, code, color). Used by `render.rs` for styled text drawing. |

## Commands

```bash
# Build (debug)
cargo build

# Run (debug)
cargo run

# Build release (optimized, ~1.5 MB binary, LTO, stripped)
cargo build --release

# Output binary
target\release\vibe-chat.exe

# Check for compile errors without building
cargo check

# Format code (if rustfmt is available)
cargo fmt

# Lint (if clippy is available)
cargo clippy
```

There are no tests in this project. Verification is done by running the binary and checking the window renders correctly.

## Code style

### Zero dependencies — this is the #1 rule

**NEVER add a dependency to `Cargo.toml`.** The entire project uses only `std`. If you need functionality (JSON, HTTP, async, etc.), write it yourself. This is a deliberate design decision for minimal binary size, no supply chain risk, and full control.

### Win32 FFI conventions

All Win32/GDI functions are declared as `extern "system"` in `win32.rs`. When you need a new Win32 function:

1. Add the type aliases if missing (e.g., `pub type HBRUSH = *mut c_void;`)
2. Add the constant if missing (e.g., `pub const WS_VISIBLE: DWORD = 0x1000_0000;`)
3. Add the struct definition if missing (e.g., `#[repr(C)] pub struct RECT { ... }`)
4. Add the `extern "system"` declaration in the appropriate `extern "system" { ... }` block, linking against the correct DLL (`"user32"`, `"gdi32"`, `"kernel32"`, `"shell32"`, `"shcore"`)

Example of the expected pattern:

```rust
// In win32.rs — declare the FFI:
#[repr(C)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

extern "system" {
    pub fn GetCursorPos(lpPoint: LPPOINT) -> BOOL;
}

// In the calling code — use it inside unsafe:
unsafe {
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
}
```

### Unsafe blocks

All Win32/GDI calls must be inside `unsafe { }` blocks. Keep unsafe blocks as small as possible — ideally wrapping just the FFI call, not entire functions. The `wnd_proc` in `window.rs` is already `unsafe extern "system"` so calls inside it don't need additional `unsafe` blocks.

### Naming conventions

- Win32 types use `SCREAMING_SNAKE_CASE` (matching the C API): `HWND`, `HDC`, `WPARAM`, `LPARAM`, `RECT`, `MSG`
- Win32 constants use `SCREAMING_SNAKE_CASE`: `WM_PAINT`, `DT_LEFT`, `VK_RETURN`, `CS_HREDRAW`
- Rust types/functions use `snake_case`: `create_window`, `ChatState`, `handle_keydown`
- Struct fields use `snake_case`: `client_width`, `scroll_offset`, `input_buffer`

### Module-level attributes

Each source file starts with a comment describing its purpose, followed by `#![allow(dead_code)]` (since not all FFI declarations are used yet). Match this pattern for new files.

### Error handling

- FFI functions returning `BOOL` or handles: check for `0`/`null_mut()` and handle gracefully
- Child process operations: use `Result<T, String>` for fallible operations (see `AcpClient::spawn()`)
- JSON parsing: return `Result<Json, String>` — never panic on malformed input
- Never use `.unwrap()` on user-facing paths; use `match` or `if let` with fallback behavior

### Color scheme (warm dark + orange)

Colors are defined as `const` in `render.rs` using the `rgb()` helper:

```rust
pub const BG_COLOR: u32 = rgb(20, 18, 16);
pub const USER_BUBBLE_BG: u32 = rgb(200, 85, 0);
pub const AGENT_BUBBLE_BG: u32 = rgb(38, 35, 31);
pub const ACCENT_COLOR: u32 = rgb(255, 140, 60);
```

When adding new colors, use the `rgb(r, g, b)` macro from `win32.rs` and keep the warm dark + orange aesthetic: warm near-black backgrounds with vivid orange accents.

### DPI awareness

All pixel coordinates must be DPI-scaled. The pattern is:

```rust
let scale = dpi as f32 / 96.0;
let scaled_value = (base_value as f32 * scale) as i32;
```

Fonts are created with DPI-scaled heights in `Fonts::new(dpi)`. Layout constants like `INPUT_HEIGHT` and `MESSAGE_PADDING` are in logical pixels and scaled at render time.

## Git workflow

- Branch naming: `feature/description`, `fix/description`, `refactor/description`
- Commit messages: short imperative sentence (e.g., "Add clipboard paste support", "Fix DPI scaling on WM_DPICHANGED")
- The project is not yet on a remote — local development only
- `Cargo.lock` is committed (binary project)
- `target/` is not committed

## Testing

There is no test framework in this project (zero dependencies). Verification is manual:

1. `cargo build` — must compile without errors
2. `cargo run` — window opens, displays correctly
3. Type a message, press Enter — message appears in chat, agent responds (requires the ACP agent binary on PATH and API key set)
4. Scroll with mouse wheel — message history scrolls
5. Ctrl+C / Ctrl+V — clipboard works
6. Resize window / move between monitors with different DPI — rendering stays crisp
7. Close window — exits cleanly, child process is killed

When adding new features, verify all of the above still work.

## Boundaries

### Always do

- Keep **zero external dependencies**. If you think you need a crate, write the functionality yourself instead.
- Add new Win32 FFI declarations to `win32.rs` — never declare `extern` functions in other files.
- Use `to_wide()` from `win32.rs` for converting `&str` to null-terminated UTF-16 for Win32 wide APIs.
- Scale all pixel coordinates by DPI (`dpi / 96.0`).
- Handle `WM_UNICHAR` for text input (proper Unicode), not `WM_CHAR` alone.
- Use `#![allow(dead_code)]` at the top of new source files (not all declarations are used yet).
- Read `PLAN.md` for the full architecture before making structural changes.
- Keep the warm dark + orange color scheme (warm dark + orange) consistent.

### Ask first

- Adding a new source file (module) — this changes the project structure.
- Changing the window size or layout approach (currently fixed 800x600 client area, resizable).
- Modifying the ACP protocol flow (initialization, session creation, prompt sending).
- Changing the release profile settings in `Cargo.toml` (`opt-level`, `lto`, `panic = "abort"`, `strip`).
- Adding new keyboard shortcuts or changing existing key bindings.

### Never do

- **Never add a dependency to `Cargo.toml`.** No `serde`, no `windows` crate, no `tokio`, no `anyhow`. Only `std`.
- **Never use `unwrap()` or `expect()` on user-facing code paths.** Use `match` or `if let` with graceful fallback.
- **Never commit secrets, API keys, or `.env` files.**
- **Never modify `vibe-chat.exe.manifest`** without understanding DPI awareness implications.
- **Never remove `#![windows_subsystem = "windows"]`** from `main.rs` — it prevents a console window from appearing.
- **Never use `panic!` in production code paths.** The release profile has `panic = "abort"`, so panics crash the app.
- **Never add `unsafe` without a comment explaining why it's safe** when the safety invariant is non-obvious.
- **Never use `std::mem::transmute`** — use proper type conversions.

## Architecture decisions (intentional — do not "fix")

These look unusual to an outsider but are deliberate:

| Decision | Why |
|----------|-----|
| Zero dependencies | Minimal binary (~1.5 MB), no supply chain risk, full control |
| Hand-written Win32 FFI in `win32.rs` | No `windows` crate — we declare only what we use |
| Hand-written JSON parser in `json.rs` | ~300 lines, no `serde` — only what JSON-RPC needs |
| Raw Win32 + GDI (no framework) | No Tauri/egui/iced — smallest possible binary, full pixel control |
| GDI `DrawTextW` for text rendering | Built into Windows, handles word wrap, simple |
| `WM_UNICHAR` for text input | Proper Unicode text entry, no mangled characters |
| Memory DC + `BitBlt` for double buffering | Prevents flicker, standard GDI technique |
| Reader thread + `mpsc::channel` (no async runtime) | Simple, no `tokio` dependency |
| Per-monitor v2 DPI awareness | Crisp rendering on high-DPI displays |
| Custom minimal markdown parser | Only bold/italic/code — no pulldown-cmark needed |
| Fixed initial size 800x600, resizable | Simple layout math |
| `panic = "abort"` in release profile | Smaller binary, no unwinding overhead |
| `opt-level = 2` + `lto = true` + `codegen-units = 1` + `strip = true` | Optimized release binary size |

## ACP protocol flow

```
UI (window.rs)          acp.rs                    vibe-acp (child process)
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

The ACP client uses three threads: a writer thread (sends JSON-RPC commands to the child's stdin), a reader thread (reads JSON-RPC from the child's stdout), and a monitor thread (polls `try_wait()` to detect process exit). Communication between threads uses `std::sync::mpsc::channel`.
