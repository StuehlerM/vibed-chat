# vibed-chat — Review Findings, Fix Plan & Project Management Feature

Senior-engineer code review of the recent changes (persistence incl. thinking, settings,
toast, archive/delete, sidebar redesign, cross-platform skeleton).

Lens: correctness first, then DRY / KISS / YAGNI / clean code.
Constraints: std-only (no new dependencies), no unit tests, no `unwrap()`/`expect()`/
`panic!` on user-facing paths, DPI-scale pixels, core never imports platform code.

Verification for every fix: `cargo check`, `cargo check --target x86_64-pc-windows-msvc`,
`cargo fmt --check`, and a manual UI pass per `AGENTS.md`.

> **Status: ALL ITEMS DONE.** Every finding below is implemented on branch
> `vibe/plan-p0-fixes-47dbf5` (commits `91fe34c` through `3596539`).
> Verified with `cargo check`, `cargo check --target x86_64-pc-windows-msvc`,
> and `cargo fmt --check` (all clean, zero warnings). Manual UI pass on Windows
> still recommended per AGENTS.md.

---

## P0 — Critical bugs (fix first)

### 1. Compile error: `name_wide` / `name_rect` used out of scope
`src/platform/windows/render/sidebar.rs:445–478`

The rename branch of the sidebar item loop defines `name_wide` (line 445) and
`name_rect` (line 447) inside the `if ctx.renaming_session == Some(i)` block, but the
streaming-dot code after the `else` block (lines 472/478) uses them — out of scope.
This file does not compile as-is.

**Fix:** hoist both definitions above the `if` block. All dependencies (`display_name`,
`name_left`, `max_name_w`, `item_rect`, `item_h`, `scale`) are computed earlier in the
loop body, so this is a pure move.

### 2. Panic on empty session list in persisted state
`src/main.rs:78`

`app_entry()` unconditionally indexes `app_state.sessions[0]` after startup. The
default session is only created when `load_state()` *fails* (lines 51–52). If
`state.json` exists but its `sessions` array is empty, the app panics at startup —
exactly the release profile with `panic = "abort"`.

**Fix:** after `load_state()` succeeds, add:
```rust
if app_state.sessions.is_empty() {
    app_state.create_local_session_default();
}
```
before the `sessions[0]` access. (Related: finding 23 makes session creation robust
against name collisions.)

### 3. Double-click rename is dead — window class missing `CS_DBLCLKS`
`src/platform/windows/window.rs:729` (`register_class`), handler at `window.rs:487`

`WM_LBUTTONDBLCLK` is handled (rename on session double-click), but the window class
is registered with only `CS_HREDRAW | CS_VREDRAW`. Without `CS_DBLCLKS` Windows never
sends `WM_LBUTTONDBLCLK`, so the handler — and the rename gesture — can never fire.
The constant exists, defined but unused, at `src/platform/windows/ffi.rs:87`.

**Fix:** `style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS` at `window.rs:729`.

### 4. `clear_hover()` forgets `hover_link` / `hover_code_copy`
`src/interaction.rs:33–47`, called from `WM_MOUSELEAVE` (`window.rs:563–565`)

`clear_hover` resets every hover field except `hover_link` and `hover_code_copy`.
When the mouse leaves the window while over a link or a code-copy button, the hover
state sticks: the link stays highlighted and `WM_SETCURSOR` keeps returning the
hand cursor until the next mouse move re-diffs hover.

**Fix:** add the two missing resets to `clear_hover`:
```rust
self.hover_link = None;
self.hover_code_copy = None;
```

### 5. Link hover index mismatch: global vs per-message
`src/platform/windows/render/messages.rs:359` vs `:406`

`draw_message` starts `local_link_idx` at `0` for every message, but `hover_link`
stores a *global* index (set in `update_link_hover`, `interaction.rs:461`, compared
against `link_rects.borrow().len()`). With more than one message containing links,
hover highlight and click hit-testing land on the wrong link in every message after
the first.

**Fix:** mirror the code-block pattern — initialize
`let mut local_link_idx = link_rects.borrow().len();` at the top of the per-message
link loop (as `code_block_global_idx` already does at `messages.rs:366`).

### 6. Sidebar wheel scroll uses the wrong layout function
`src/platform/windows/window.rs:589`

The sidebar renders the flat **date-grouped** list (`date_grouped_sidebar_layout`,
`interaction.rs:175`; the renderer builds date groups in `render/sidebar.rs`), but
`WM_MOUSEWHEEL` over the sidebar scrolls using the old **project-based**
`sidebar_layout()` (`interaction.rs:115`). Wheel deltas map to the wrong rows.

**Fix:** call `date_grouped_sidebar_layout()` in the wheel handler. (Once the
project-management feature lands, this becomes whichever single layout function the
sidebar actually renders — see the feature section.)

---

## P1 — DRY / code smells

### 7. Font-selection if/else duplicated five times
`src/platform/windows/render/messages.rs:102, 132, 308, 597, 626`

The same "bold / italic / code / regular" font-pick chain is copy-pasted five times.
A helper already exists: `select_font_for_style` (`src/platform/windows/fonts.rs:120`),
but is used only once (line 375).

**Fix:** replace all five copies with `select_font_for_style(...)`.

### 8. Wrap + height measurement triplicated
`src/platform/windows/render/messages.rs:100–120, 130–155, 593–650`

The `DT_CALCRECT` wrap-and-measure loop (with `u_format` flags, `line_height`,
`text_width`) appears three times with small variations.

**Fix:** extract one helper
`measure_text_height(hdc, runs, text_width, line_height, fonts) -> i32`
and call it from all three sites. `compute_message_heights` (line 573) is the real
layout path — the helper belongs next to it.

### 9. `chat_left` / `chat_right` recomputed at seven call sites
`src/interaction.rs:413, 679, 705, 765, 792, 874, 982`

Every hit-test recomputes `if show_settings { ... } else { ... }` for the chat area
edges. This is the definition of DRY violation — change the rule once, miss a site.

**Fix:** add `AppState::chat_left(&self) -> i32` and `AppState::chat_right(&self) -> i32`
(encoding the `show_settings` / sidebar-width / input-bar rule) and use them at all
seven sites.

### 10. `PaintContext` hand-copies ~30 fields from `AppState` every paint
`src/platform/windows/window.rs:175–214`, struct in `src/platform/windows/paint.rs`

Each frame, `window.rs` copies ~30 fields into a `PaintContext`. Any new field must be
remembered in two places; a missed field renders stale (this is exactly how the
project hover fields became write-only — see finding 16).

**Fix:** pass `&AppState` (plus the few genuinely paint-local values: `hdc`,
`scale`, viewport rects) into `paint()`. Renderers already only read. This also makes
finding 16's "wire or remove" question moot — the fields are reachable by construction.

### 11. Persistence errors silently discarded
`src/app.rs:669–696`

All save functions end in `let _ = ...`, so a full disk, a locked file, or a
permission error means the user believes their chat is saved when it is not.

**Fix:** thread the `io::Result` back to the caller and surface failures through the
existing toast mechanism (e.g. "Failed to save session"). Never panic — report.

### 12. O(n²) URL detection in the markdown parser
`src/markdown.rs:227–242`

`parse_inline` scans for URLs by, at each character position, allocating two `String`s
(prefix/suffix probes) to test `http://` / `https://`. For an n-char line this is
O(n²) allocations in the hot parse path.

**Fix:** replace the String allocations with slice-prefix checks
(`line[i..].starts_with("https://")` — zero-alloc) or a small `is_url_scheme_at`
helper.

### 13. `AppState` god struct
`src/app.rs` — 50+ pub fields, 16 of them hover flags

Hover state, focus state, rename state, scroll state, and settings are all flat fields
on one struct. Every function that touches one field can accidentally clobber another
(see finding 4 — `clear_hover` missed two fields).

**Fix:** group into sub-structs: `HoverState`, `FocusState` (focused element +
rename target), and move scroll offsets into `ChatState` where they belong. Mechanical
refactor, no behavior change. Do this *after* the P0 fixes so the diffs stay small.

### 14. Dead parameter in `finish_code_block`
`src/platform/windows/render/messages.rs:495` (`let _ = end_y;` at :568)

`finish_code_block` takes `end_y` and immediately discards it. Two call sites pass a
value nobody reads.

**Fix:** remove the parameter, the `let _ = end_y;`, and update both call sites.

### 15. Dead layout functions in `chat.rs`
`src/chat.rs:437–467` — `compute_message_height`, `recompute_layout`

Both use a char-count heuristic (`width / 7`) that the real render path abandoned in
favour of GDI measurement (`compute_message_heights`, `messages.rs:573`). Nothing calls
them.

**Fix:** delete both functions (YAGNI — if a future non-Windows backend needs
estimates, it can measure with its own toolkit).

---

## P2 — Design / cleanup

### 16. Project management is dead code — and it should be a feature
See the dedicated section below: **"Feature: Project management"**.

### 17. `Mutex::lock().unwrap()` × 8 in the ACP threads
`src/acp.rs:242, 275, 296, 312, 454, 468, 575, 604`

The writer/reader/monitor threads unwrap mutex locks. If one thread panics while
holding the lock, the mutex is poisoned and every other thread then panics on unwrap —
a cascade that takes the whole agent connection down. These run on background
threads where a poisoned lock is recoverable, not fatal.

**Fix:** replace with `if let Ok(agent) = self.agent.lock() { ... }` (skip the cycle
on poisoning). No behavior change in the healthy path.

### 18. `has_error` is write-only
`src/chat.rs:73`

Set when a message errors, never read — the UI apparently renders errors some other
way, or not at all.

**Fix:** either render it (e.g. error styling on the message row) or remove the field.
Prefer rendering — an invisible error flag is how users lose trust in a chat app.

### 19. Dead helpers in `chat.rs`
`src/chat.rs:337` (`line_end_pos`), `:350` (`input_line_count`)

Neither is called anywhere.

**Fix:** delete both.

### 20. Roundabout cursor byte math in the input box
`src/platform/windows/render/input_box.rs:315`

The cursor x-position is computed as `line_start_pos(cursor_pos) + cursor_col_bytes`,
which algebraically reduces to `cursor_pos` itself (cf. the same math at line 423).
The detour obscures intent and invites off-by-one drift between the two copies.

**Fix:** use `chat.cursor_pos` directly at line 315 and keep one canonical
byte-column computation.

### 21. String-formatted paths in persistence
`src/persistence.rs:215, 219, 274, 324`

Paths are built with `format!("{}/{}", dir, name)`. On Windows the config dir uses `\`
(see lines 192–194), so this produces mixed separators — works by accident, breaks the
moment a path is compared or canonicalized.

**Fix:** use `std::path::Path::new(&dir).join(name)` everywhere (std-only, no new
dependency).

### 22. Duplicate `TOAST_DURATION_MS`
`src/app.rs:32` and `src/platform/windows/render/toast.rs:10`

The same constant is defined twice; they can drift.

**Fix:** keep one definition (in `app.rs` next to the other app-level constants) and
reference it from the toast renderer.

### 23. Session counter can collide with existing names
`src/app.rs:706` (`load_state`: `session_counter = sessions.len()`)

If sessions were deleted (archive/delete feature), `sessions.len()` can equal the
counter of an existing session, so `create_local_session_default` can generate a name
that already exists.

**Fix:** derive the counter from the maximum existing counter (`sessions.iter().map(|s|
s.counter).max().unwrap_or(0) + 1` — or track it explicitly in `state.json`).

---

## Feature: Project management

### How the dead code happened (YAGNI, answered honestly)

This is not a YAGNI violation — it is an **unfinished UI migration**.

The app was originally designed around *projects*: `src/session.rs` has the `Project`
struct (`session_indices`, `expanded`, `name`, `path`), `src/app.rs` has the full data
layer (`ensure_project` :166, `rebuild_project_indices` :175, `project_for_session`
:190, `toggle_project` :198, `remove_project` :205), persistence round-trips projects
(`persistence.rs:99–118, 246, 334–337`), the Ctrl+N folder picker feeds
`new_chat_in_dir` → `ensure_project` (`window.rs:284–296`), and the interaction layer
still has the project vocabulary (`SidebarEntry::ProjectHeader` /
`ProjectRow`, `sidebar_layout()` at `interaction.rs:115`, `PressedElement::NewProject`
/ `ProjectHeader` / `ProjectMenu` / `RemoveProject`, `FocusedElement::NewProject` /
`ProjectHeader` in `src/ui.rs`).

Then the sidebar was redesigned from project-grouped to a flat **date-grouped** list
(`date_grouped_sidebar_layout`, `interaction.rs:175`; `render/sidebar.rs` builds date
groups). The render and interaction path moved to date grouping — but nobody removed
or finished the project half:

- `render/sidebar.rs` contains **zero** project references — the presentation layer
  was never built for projects in the new design.
- `toggle_project` / `remove_project` have **zero** callers.
- `update_hover` (`interaction.rs:515`) never sets `hover_new_project` /
  `hover_project` / `hover_project_menu`, yet `WM_SETCURSOR` checks them
  (`window.rs:631–632`) — checks that can never be true.
- `PaintContext` carries `projects`, `hover_new_project`, `hover_project`,
  `hover_project_menu` (`paint.rs:15, 28–30`) — never read by any renderer.
- The `PressedElement` / `FocusedElement` project variants are never matched.

So: the **data layer is alive** (projects are created via Ctrl+N, persisted, and loaded
back), but the **presentation/interaction layer was never written**. The feature was
half-built, the sidebar pivoted, and the orphaned half was left behind. Classic
migration residue — the fix is to finish the migration, not to delete the data model.

### Implementation plan

**Goal:** sessions grouped under collapsible project headers in the sidebar, with a
"New Project" button, per-project rename/remove, and full keyboard support.

1. **Pick the sidebar model.** Replace date grouping with project grouping as the
   primary structure (project header rows, session rows nested underneath via
   `project.session_indices`). Rationale: the entire data model, persistence, and
   folder picker are already project-shaped; date grouping can be re-added later as
   a secondary sort inside a project if wanted. Keep `date_grouped_sidebar_layout`
   until the new layout lands, then delete it (one layout function, used everywhere —
   this also structurally fixes finding 6).

2. **Render (`src/platform/windows/render/sidebar.rs`).**
   - Top row: "＋ New Project" button (hover state `hover_new_project`).
   - Per project: header row — expand/collapse arrow, project name, session count,
     hover state `hover_project`, and a menu button (`hover_project_menu`) with
     rename / remove actions.
   - Under each expanded project: session rows (existing row rendering, reused),
     iterated in `project.session_indices` order.
   - Rename mode: reuse the existing inline-rename rendering (and fix finding 1 while
     touching this code).

3. **Layout + interaction (`src/interaction.rs`).**
   - Write one project-aware `sidebar_layout()` returning rects for: New Project
     button, each project header, each project menu button, each session row.
   - `update_hover`: set `hover_new_project` / `hover_project` /
     `hover_project_menu` from the layout rects (removes the never-true
     `WM_SETCURSOR` checks at `window.rs:631–632`).
   - `handle_click`: New Project → `Dialogs::pick_folder` → `ensure_project`;
     project header → `toggle_project`; menu → rename (existing rename flow) or
     remove → `remove_project` behind a `confirm` dialog (same pattern as session
     delete at `window.rs:312–336`).
   - Use the same layout function in `WM_MOUSEWHEEL` (fixes finding 6 for good).

4. **Keyboard (`src/platform/windows/window.rs` + `src/interaction.rs`).**
   `cycle_focus` (`interaction.rs:52`) already cycles `NewChat`, `NewProject`,
   `ProjectHeader(i)`, `SidebarItem(i)`, `SettingsBtn`, `Input`, `SendButton`,
   `SettingsClose`, `SettingsToggle(0–3)`, `Reconnect` — but only `SendButton` has
   Enter/Space handling (window.rs:353). Add a single `activate_focused_element()`
   dispatched from the Enter/Space handler covering all variants:
   `NewChat` → `new_chat`, `NewProject` → folder picker + `ensure_project`,
   `SidebarItem(i)` → `switch_session`, `ProjectHeader(i)` → `toggle_project`,
   `SettingsBtn` → open settings, `SettingsClose` → close, `SettingsToggle(i)` →
   toggle setting i, `Reconnect` → `connect_acp`, `Input`/`SendButton` → existing
   behavior. This is a general accessibility fix, not just for projects.

5. **PaintContext (`src/platform/windows/paint.rs`).** Once the renderer consumes the
   project fields they become live; if finding 10 (pass `&AppState`) lands first,
   delete the duplicated fields instead.

6. **Persistence.** Already round-trips projects and `expanded` state — no changes
   needed. Verify manually after the feature lands.

---

## Recommended execution order

1. **P0 bugs (findings 1–6)** — the app currently does not compile (finding 1) and can
   panic at startup (finding 2). Nothing else matters until these are green.
2. **Project-management feature** — it subsumes findings 6 and 16, gives the app its
   missing organizational layer, and the data model is already there.
3. **P1 DRY/smells (7–15)** — mechanical, low-risk, keeps the codebase honest.
4. **P2 cleanup (17–23)** — small deletions and robustness fixes.

## Verification (per AGENTS.md — no unit tests)

- `cargo check`
- `cargo check --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- Manual UI pass on Windows: startup with empty/missing/corrupt `state.json`,
  double-click rename, link hover across multiple messages, sidebar wheel scroll,
  mouse-leave hover reset, Ctrl+N → project appears in sidebar, project
  expand/collapse, project rename/remove, keyboard Tab + Enter through all focusable
  elements, save-failure toast.
