# AGENTS.md - vibe-chat

## Persona

You are an expert Rust systems programmer specializing in zero-dependency cross-platform desktop applications. You write raw OS API code with hand-rolled FFI, hand-written parsers, and no external crates. You understand that every line of dependency-free code is a deliberate architectural choice - not an oversight. You are fluent in Rust 2021, the Win32 API, GDI text rendering, and the Agent Client Protocol (ACP) over JSON-RPC.

## Project knowledge

**Tech Stack:** Rust 1.70+ (edition 2021), **Zero external dependencies** - only `std`. No crates.io. No `windows` crate. No `serde`. No `tokio`. Everything is hand-written.

**Supported platforms:** Windows (fully functional), Linux (skeleton), macOS (skeleton).

**What this project is:** A minimal desktop chat application that connects to an ACP agent via the Agent Client Protocol (ACP). It spawns `vibe-acp` as a child process and communicates over stdin/stdout using JSON-RPC 2.0 (line-delimited). The Windows UI is a raw Win32 window with GDI-rendered message bubbles. No GUI framework, no webview, no Electron.

**Prerequisites:** Rust toolchain (rustup), `vibe-acp` on PATH, `API_KEY` in environment (passed to the agent process).

**Disclaimer:** This is an independent, unofficial project. It is not affiliated with, endorsed by, or sponsored by Mistral AI. All Mistral trademarks and branding have been removed.

## Architecture: Core + Platform Backends

The codebase is split into a **platform-independent core** and **platform-specific backends**:

```
src/
├── main.rs                  ← thin entry point, calls platform::run()
├── platform/
│   ├── mod.rs               ← cfg-gated backend selection
│   ├── backend.rs           ← traits: Platform, Clipboard, Dialogs
│   ├── windows/             ← Windows backend (fully functional)
│   │   ├── mod.rs           ← WindowsPlatform impl, run() entry
│   │   ├── ffi.rs           ← all Win32/GDI FFI declarations
│   │   ├── window.rs        ← WndProc, message loop, window creation
│   │   ├── render/          ← GDI painting (sidebar, messages, input, etc.)
│   │   ├── fonts.rs         ← GDI font management
│   │   ├── gfx.rs           ← GDI+ anti-aliased shapes
│   │   ├── paint.rs         ← PaintContext (render data bundle)
│   │   ├── text.rs          ← GDI text measurement/wrapping
│   │   ├── clipboard.rs     ← Win32 clipboard
│   │   └── dialogs.rs       ← Win32 dialogs (folder picker, confirm, error)
│   ├── linux/
│   │   └── mod.rs           ← Linux skeleton (stub, not yet implemented)
│   └── macos/
│       └── mod.rs           ← macOS skeleton (stub, not yet implemented)
├── app.rs                   ← AppState: sessions, projects, ACP coordination
├── chat.rs                  ← ChatState: messages, scroll, input buffer
├── session.rs               ← Session + Project data models
├── acp.rs                   ← ACP client (spawn, JSON-RPC, threads)
├── json.rs                  ← hand-written JSON parser + serializer
├── markdown.rs              ← minimal markdown → styled runs
├── theme.rs                 ← colors, spacing, layout constants, Settings
├── input.rs                 ← platform-independent key mapping + text editing
├── interaction.rs           ← mouse/hover/focus/layout (platform-independent)
├── text.rs                  ← date helpers, previews (platform-independent)
└── ui.rs                    ← UI enums: PressedElement, FocusedElement, Toast
```

### Platform abstraction

The core talks to the OS through traits in `platform/backend.rs`:

- **`Platform`** — provides `PlatformInfo` (caret blink, font names), `Clipboard`, `Dialogs`
- **`Clipboard`** — `set_text()`, `get_text()`
- **`Dialogs`** — `pick_folder()`, `confirm()`, `show_error()`

Each platform backend implements these traits. The core never calls OS APIs directly.

### Module responsibilities (core)

| File | Responsibility |
|------|---------------|
| `main.rs` | Entry point. Dispatches to `platform::run()`. On Windows, `app_entry()` is called by the backend. |
| `platform/mod.rs` | Selects the backend module via `cfg(windows)`, `cfg(target_os = "linux")`, `cfg(target_os = "macos")`. |
| `platform/backend.rs` | Trait definitions (`Platform`, `Clipboard`, `Dialogs`) and `run_platform()` dispatcher. |
| `json.rs` | Minimal JSON value enum. Recursive descent parser. Serializer. Only what JSON-RPC needs. |
| `app.rs` | `AppState` struct: projects, sessions, ACP client, DPI, hover/focus state, toasts. Fonts stored as opaque `FontsHandle`. |
| `chat.rs` | `ChatState` struct: messages, scroll, input buffer, cursor, selection, streaming. Text editing operations. |
| `session.rs` | `Session` (chat conversation) and `Project` (working directory grouping) data models. |
| `acp.rs` | `AcpClient` spawns `vibe-acp` as child process with piped stdin/stdout. Three threads: writer, reader, monitor. |
| `markdown.rs` | Minimal markdown parser: `**bold**`, `*italic*`, `` `code` ``, ` ``` ` code blocks. Outputs `Vec<StyledRun>`. |
| `theme.rs` | Colors (as `rgb()` COLORREF u32), spacing tokens, typography scale, layout constants, `Settings`. Fully platform-independent. |
| `input.rs` | Platform-independent key mapping (`Key` enum, `Modifiers`), text editing delegation to `ChatState`, clipboard delegation to platform trait. |
| `interaction.rs` | Mouse click handling, hover tracking, keyboard focus cycling, sidebar layout computation. Pure coordinate math. |
| `text.rs` | Date/time helpers (unix_to_civil, date_group_label, relative_time), message previews. No graphics dependency. |
| `ui.rs` | `PressedElement`, `FocusedElement`, `Toast`, `ToastKind` enums. Shared between core and backends. |

### Module responsibilities (Windows backend)

| File | Responsibility |
|------|---------------|
| `platform/windows/ffi.rs` | All `extern "system"` FFI declarations for Win32/GDI. Type aliases, constants, struct definitions. `to_wide()`, `from_wide()`, `rgb()`, `argb_from_rgb()`. |
| `platform/windows/window.rs` | `wnd_proc` handles all Windows messages. `register_class()`, `create_window()`. Translates Win32 key codes to platform-independent `Key` enum. |
| `platform/windows/render/mod.rs` | Double-buffered painting entry point. Creates memory DC, draws all regions, BitBlts to screen. |
| `platform/windows/render/sidebar.rs` | Sidebar rendering: session list, date grouping, new chat button, settings button. |
| `platform/windows/render/messages.rs` | Message bubbles, thinking sections, text layout, scrollbar. |
| `platform/windows/render/input_box.rs` | Input box with cursor, selection, placeholder, send button, hint row. |
| `platform/windows/render/header.rs` | Session header bar with name and connection status. |
| `platform/windows/render/settings.rs` | Settings panel with toggle switches, info rows, reconnect button. |
| `platform/windows/render/empty_state.rs` | Empty state with gradient stripe. |
| `platform/windows/render/toast.rs` | Toast notification popup. |
| `platform/windows/paint.rs` | `PaintContext` struct: bundles all data needed for a GDI paint pass. |
| `platform/windows/fonts.rs` | `Fonts` struct with GDI HFONT handles. `create_font()`, `select_font_for_style()`. |
| `platform/windows/gfx.rs` | GDI+ helpers loaded dynamically via LoadLibrary. Anti-aliased rounded rectangles. |
| `platform/windows/text.rs` | GDI text measurement (`GetTextExtentPoint32W`), word wrapping, truncation. |
| `platform/windows/clipboard.rs` | Win32 clipboard (`OpenClipboard`, `SetClipboardData`, `GetClipboardData`). |
| `platform/windows/dialogs.rs` | Win32 dialogs (`SHBrowseForFolderW`, `MessageBoxW`). |

## Commands

```bash
# Build (debug, current platform)
cargo build

# Run (debug)
cargo run

# Build release (optimized, LTO, stripped)
cargo build --release

# Check compilation for Windows from any platform
cargo check --target x86_64-pc-windows-msvc

# Check compilation for current platform
cargo check

# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Lint (if clippy is available)
cargo clippy
```

There are no tests in this project. Verification is done by running the binary and checking the window renders correctly.

## Code style

### Zero dependencies - this is the #1 rule

**NEVER add a dependency to `Cargo.toml`.** The entire project uses only `std`. If you need functionality (JSON, HTTP, async, etc.), write it yourself. This is a deliberate design decision for minimal binary size, no supply chain risk, and full control.

### Platform separation rules

1. **Core modules** (`app.rs`, `chat.rs`, `session.rs`, `acp.rs`, `json.rs`, `markdown.rs`, `theme.rs`, `input.rs`, `interaction.rs`, `text.rs`, `ui.rs`) must **never** import from `platform::` or use any OS-specific APIs.
2. **Platform backends** (`platform/windows/`, `platform/linux/`, `platform/macos/`) implement the traits in `platform/backend.rs` and can use OS-specific APIs freely.
3. **Colors** in `theme.rs` are stored as `rgb()` COLORREF u32 values. Each backend converts to its native format.
4. **Fonts** in the core are stored as opaque `FontsHandle` (type-erased). The backend downcasts to its native font type.
5. **Clipboard and dialogs** are accessed through the `Clipboard` and `Dialogs` traits, not directly.
6. **Key input** uses the platform-independent `Key` enum and `Modifiers` struct from `input.rs`. Backends translate native key events.

### Win32 FFI conventions

All Win32/GDI functions are declared as `extern "system"` in `platform/windows/ffi.rs`. When you need a new Win32 function:

1. Add the type aliases if missing (e.g., `pub type HBRUSH = *mut c_void;`)
2. Add the constant if missing (e.g., `pub const WS_VISIBLE: DWORD = 0x1000_0000;`)
3. Add the struct definition if missing (e.g., `#[repr(C)] pub struct RECT { ... }`)
4. Add the `extern "system"` declaration in the appropriate `extern "system" { ... }` block, linking against the correct DLL (`"user32"`, `"gdi32"`, `"kernel32"`, `"shell32"`, `"shcore"`)

### Unsafe blocks

All Win32/GDI calls must be inside `unsafe { }` blocks. Keep unsafe blocks as small as possible - ideally wrapping just the FFI call, not entire functions. The `wnd_proc` in `platform/windows/window.rs` is already `unsafe extern "system"` so calls inside it don't need additional `unsafe` blocks.

### Naming conventions

- Win32 types use `SCREAMING_SNAKE_CASE` (matching the C API): `HWND`, `HDC`, `WPARAM`, `LPARAM`, `RECT`, `MSG`
- Win32 constants use `SCREAMING_SNAKE_CASE`: `WM_PAINT`, `DT_LEFT`, `VK_RETURN`, `CS_HREDRAW`
- Rust types/functions use `snake_case`: `create_window`, `ChatState`, `handle_key`
- Struct fields use `snake_case`: `client_width`, `scroll_offset`, `input_buffer`

### Module-level attributes

Each source file starts with a comment describing its purpose, followed by `#![allow(dead_code)]` (since not all FFI declarations are used yet). Match this pattern for new files.

### Error handling

- FFI functions returning `BOOL` or handles: check for `0`/`null_mut()` and handle gracefully
- Child process operations: use `Result<T, String>` for fallible operations (see `AcpClient::spawn()`)
- JSON parsing: return `Result<Json, String>` - never panic on malformed input
- Never use `.unwrap()` on user-facing paths; use `match` or `if let` with fallback behavior

### Color scheme (warm dark + orange)

Colors are defined as `const` in `theme.rs` using the `rgb()` helper (defined in `theme.rs` itself, platform-independent):

```rust
pub const BG_COLOR: u32 = rgb(20, 18, 16);
pub const USER_BUBBLE_BG: u32 = rgb(200, 85, 0);
```

The `rgb()` function packs into COLORREF format (0x00BBGGRR). Platform backends convert as needed (e.g., `argb_from_rgb()` in `ffi.rs` for GDI+).

### DPI awareness

All pixel coordinates must be DPI-scaled. The pattern is:

```rust
let scale = dpi as f32 / 96.0;
let scaled_value = (base_value as f32 * scale) as i32;
```

## Git workflow

- Branch naming: `feature/description`, `fix/description`, `refactor/description`, or `vibe/<slug>-02a0b6` for agent tasks
- Commit messages: short imperative sentence (e.g., "Add clipboard paste support", "Fix DPI scaling on WM_DPICHANGED")
- `Cargo.lock` is committed (binary project)
- `target/` is not committed

## Testing

There is no test framework in this project (zero dependencies). Verification is manual:

1. `cargo check` - must compile without errors on the current platform
2. `cargo check --target x86_64-pc-windows-msvc` - must compile for Windows
3. `cargo run` - window opens, displays correctly (Windows only for now)
4. Type a message, press Enter - message appears in chat, agent responds (requires the ACP agent binary on PATH and API key set)
5. Scroll with mouse wheel - message history scrolls
6. Ctrl+C / Ctrl+V - clipboard works
7. Resize window / move between monitors with different DPI - rendering stays crisp
8. Close window - exits cleanly, child process is killed

When adding new features, verify all of the above still work.

## Boundaries

### Always do

- Keep **zero external dependencies**. If you think you need a crate, write the functionality yourself instead.
- Keep core modules free of platform-specific imports. Core must compile on all platforms.
- Add new Win32 FFI declarations to `platform/windows/ffi.rs` - never declare `extern` functions in other files.
- Use `to_wide()` from `platform/windows/ffi.rs` for converting `&str` to null-terminated UTF-16 for Win32 wide APIs.
- Scale all pixel coordinates by DPI (`dpi / 96.0`).
- Handle `WM_UNICHAR` for text input (proper Unicode), not `WM_CHAR` alone.
- Use `#![allow(dead_code)]` at the top of new source files (not all declarations are used yet).
- Keep the warm dark + orange color scheme consistent.
- Access clipboard and dialogs through the platform traits, not directly.

### Ask first

- Adding a new source file (module) - this changes the project structure.
- Changing the window size or layout approach.
- Modifying the ACP protocol flow (initialization, session creation, prompt sending).
- Changing the release profile settings in `Cargo.toml`.
- Adding new keyboard shortcuts or changing existing key bindings.
- Implementing a Linux or macOS backend (major undertaking).

### Never do

- **Never add a dependency to `Cargo.toml`.** No `serde`, no `windows` crate, no `tokio`, no `anyhow`. Only `std`.
- **Never use `unwrap()` or `expect()` on user-facing code paths.** Use `match` or `if let` with graceful fallback.
- **Never commit secrets, API keys, or `.env` files.**
- **Never modify `vibe-chat.exe.manifest`** without understanding DPI awareness implications.
- **Never remove `#![windows_subsystem = "windows"]`** from `main.rs` (use `#![cfg_attr(windows, windows_subsystem = "windows")]`).
- **Never use `panic!` in production code paths.** The release profile has `panic = "abort"`, so panics crash the app.
- **Never add `unsafe` without a comment explaining why it's safe** when the safety invariant is non-obvious.
- **Never use `std::mem::transmute`** - use proper type conversions.
- **Never import platform-specific code from core modules.** Core modules must compile on all platforms.

## Architecture decisions (intentional - do not "fix")

| Decision | Why |
|----------|-----|
| Zero dependencies | Minimal binary, no supply chain risk, full control |
| Core + platform backend split | Clean separation, easy to add new platforms |
| Hand-written Win32 FFI in `platform/windows/ffi.rs` | No `windows` crate - we declare only what we use |
| Hand-written JSON parser in `json.rs` | No `serde` - only what JSON-RPC needs |
| Raw Win32 + GDI (no framework) on Windows | No Tauri/egui/iced - smallest possible binary, full pixel control |
| GDI `DrawTextW` for text rendering | Built into Windows, handles word wrap, simple |
| `WM_UNICHAR` for text input | Proper Unicode text entry, no mangled characters |
| Memory DC + `BitBlt` for double buffering | Prevents flicker, standard GDI technique |
| Reader thread + `mpsc::channel` (no async runtime) | Simple, no `tokio` dependency |
| Per-monitor v2 DPI awareness | Crisp rendering on high-DPI displays |
| Custom minimal markdown parser | Only bold/italic/code - no pulldown-cmark needed |
| Fixed initial size 1100x700, resizable | Simple layout math |
| `panic = "abort"` in release profile | Smaller binary, no unwinding overhead |
| `opt-level = 2` + `lto = true` + `codegen-units = 1` + `strip = true` | Optimized release binary size |
| Platform traits for clipboard/dialogs | Core never calls OS APIs directly |
| Opaque `FontsHandle` in core | Core doesn't know about GDI HFONT or platform font types |
| `Key` enum for keyboard input | Core doesn't know about Win32 VK_* codes |

## ACP protocol flow

```
UI (window.rs)          acp.rs                    vibe-acp (child process)
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

The ACP client uses three threads: a writer thread (sends JSON-RPC commands to the child's stdin), a reader thread (reads JSON-RPC from the child's stdout), and a monitor thread (polls `try_wait()` to detect process exit). Communication between threads uses `std::sync::mpsc::channel`.

## Adding a new platform (Linux/macOS)

The skeletons are already in place at `src/platform/linux/mod.rs` and `src/platform/macos/mod.rs`. To implement a backend:

1. Implement the `Platform`, `Clipboard`, and `Dialogs` traits in the backend module.
2. Implement a `run()` function that creates a window, runs an event loop, and renders the UI.
3. Create platform-specific render/text/fonts modules (analogous to `platform/windows/`).
4. The core app logic (`AppState`, `ChatState`, `AcpClient`, etc.) should work unchanged.
5. The rendering approach should match the zero-dependency philosophy: raw OS APIs (X11/Wayland + Cairo on Linux, AppKit/CoreGraphics on macOS), no GUI framework crates.
