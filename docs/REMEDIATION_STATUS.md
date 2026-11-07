# Code-review remediation status

This tracks implementation against `docs/CODE_REVIEW.md` (reviewed snapshot
`541d9c3`). It is **not a declaration that all findings or acceptance checks are
complete**. No external dependencies, application-shortcut remappings, platform
backends, release-profile changes, or manifest changes were introduced. The user
explicitly approved native Unicode editors, adaptive sidebar/settings panels, and
accessibility modules. That approved batch adds `src/layout.rs`,
`src/platform/windows/native.rs`, and `src/platform/windows/accessibility.rs`.

## Source changes

| Finding | Implemented | Remaining |
| --- | --- | --- |
| CR-01 | Owned native effects; release application borrows before modal calls; re-resolve stable targets; owner HWND; modal command guard; separate Rc font ownership. | Windows modal/streaming acceptance. |
| CR-02 | Immutable generation snapshots, flushed same-directory temporaries, atomic index replacement, previous-index backup, typed bounded loads, schema/version checks, read-only recovery, persistent save errors. Missing index with recovery files is not first launch. | Full-disk/interruption/recovery exercises; filesystem/power-loss durability characterization. Old generations are retained, not garbage-collected. |
| CR-03 | Ask by default; turn-owned pending permissions, tool details, offered scopes only, default-cancel review, explicit selected/cancelled v1 responses. Removed global approval switch. | Verify with the supported installed agent and allow/reject/cancel flows. |
| CR-04 | Typed pending ACP request map; local-session/turn keys; no unsolicited chat creation or eager Direct remote sessions. | Out-of-order multi-chat agent acceptance. |
| CR-05 | One queued first send through spawn/init/session creation; submission errors; persisted queued/streaming/outcome metadata; failed draft restored; same-text retry reuses user entry, including after reload. | Exercise startup/failure/retry with an agent. Partial failed assistant text remains visible; richer retry/context policy remains to be designed. |
| CR-06 | Init/request/turn timeouts, bounded reconnect attempts, process-exit/transport failure settlement, remote IDs cleared, lazy Code reconnect. Bounded best-effort redacted stderr tail in failures. | Live crash/hang/reconnect acceptance; process-tree policy. |
| CR-07 | Captured turn backend; Direct cancellation flag independent of queue; atomic queued retirement without waiting behind another chat; worker closes in-flight request; Code cancellation; late/stale keys rejected. | Stop/rapid resend/mode switch acceptance; remote tool side effects cannot be undone. |
| CR-08 | Typed single Direct terminal outcome, premature EOF incomplete, SSE limits, metadata before entity bytes, bounded errors/queues, deadlines. | Live/mock provider failure matrix and usage details. |
| CR-09 | Turn-keyed streaming, retired ACP turns, checked channel/write/flush errors, bounded drain work. Removed the obsolete token-count work path rather than allowing optional errors to terminate generation. | Coalesced worker notifications instead of polling, further delta coalescing, adversarial ordering/saturation acceptance; typed revision-bound telemetry if counting is restored. |
| CR-10 | Clipboard owner/RAII, bounded UTF-16, checked allocation/transfer, failed cut preserves draft/selection. | Clipboard contention and malformed/global-memory manual checks. |
| CR-11 | Shared removal operation, active-work refusal, stable surviving selection, replacement chat, publish removal before history cleanup, clear stale UI state, explicit workspace-files notice, visible all-archived state. | Destructive-operation matrix on copied data. |
| CR-12 | Validated session-ID/generation grammar and duplicates before file access; checked storage collision allocation including orphan generations. | Broader PathBuf/OsString ownership work; workspace normalization (separate from safe storage IDs). |
| CR-13 | Legacy v1 remains Code; unknown modes rejected; explicit default-No mode-change policy confirmation; running backend unchanged; producing backend/model/outcome saved; no reasoning replay. Tool activity now also retains typed call IDs/status/kind and the complete original update payload independently of display/context text. | Supported optional tool summaries and live endpoint/schema verification; legacy system-text records cannot reconstruct fields already discarded by older releases. No automatic cross-mode context transfer is promised. |
| CR-14 | Draft debounce and active checkpoints; terminal and partial output saves; interrupted restoration; no destructive history trim; stable runtime message identity. A bounded, revision-ordered persistence worker coalesces pending snapshots; ordered deletion tombstones survive coalescing and clean up only after publication. | Explicit context budget and failure/restart acceptance. Snapshot capture still copies complete storage text on the UI thread; dirty-only capture remains future optimization. |
| CR-15 | Bounded strict JSON grammar, depth/value/string/input limits, duplicate-key rejection, exact integral IDs, checked integer accessors, bounded protocol/files/SSE. Markdown inline recursion also capped. | Boundary/failure acceptance; typed schema validation can be tightened further. |
| CR-16 | Native Unicode RichEdit composer/rename with a distinct rename focus owner, native selection/caret/IME, UNICODE_NOCHAR support, IME-submit guard and consumed activation characters. Parent/child focus and foreground checks prevent synchronization from activating a background editor. | Native composition/candidate/Narrator acceptance, high contrast/text scaling, full retained-canvas accessibility and project/message focus coverage. |
| CR-17 | Shared adaptive panel/header-launcher geometry; sidebar drawer, settings docking/page, fixed settings header with native body scrollbar, shared mode geometry, cursor delegation to native children. Sidebar paints shared interaction entries; screen/client wheel conversion and partial-wheel accumulation retained. | Half-open boundaries everywhere, system wheel policy, complete canvas accessibility bounds and native DPI/resize acceptance. |
| CR-18 | Transcript actions clipped and cleared on navigation/empty/scroll/geometry changes; complete link destinations; HTTP(S) allowlist and ShellExecute error reporting; source-based code copy. | Session/layout-generation-bound input regions generated before paint, rather than retaining paint-produced rectangles. |
| CR-19 | Inline spans share measured lines; explicit breaks; italic measurement; literal ampersands; bounded drawing; actual code-header row; exact fenced source including CRLF/tabs for copy; shared cached layout for height/drawing. | Grapheme-aware line breaking and dedicated horizontal code scrolling; manual rendering/copy acceptance. |
| CR-20 | Windows composer/rename now delegate mouse caret/drag/word selection, preferred-x movement, clipboard, undo/redo and internal scrolling to plain-text RichEdit. Composer growth uses native formatted-size/line queries and an eight-line/window-height cap; selection offsets are converted between UTF-16 and UTF-8. | Native multilingual/IME/clipboard/undo acceptance. Undo is document-local and resets on rebind/replacement. Basic fallback editing is not equivalent to the native editor. |
| CR-21 | No unconditional 60-Hz full repaint; idle/minimized animation suppressed; reusable checked backbuffer via BeginPaint DC; bounded revision/width/font-generation layout cache; bounded event drain; serialization/write/flush/cleanup moved to one owned worker with one coalesced pending snapshot. | Caret-only dirty rectangles, cumulative height/visible-range indexing, frame-budgeted growing-message layout, incremental snapshot capture, profiling on reference hardware. |
| CR-22 | Auto-scroll consulted without forcing follow-tail; decorative theme/approval switches replaced with honest fixed-policy rows. Approved adaptive/scrollable settings and history drawer; keyboard focus reveals offscreen settings/history rows. | Jump-to-latest, stable reading/reflow anchors, system appearance preferences and runtime layout acceptance. |
| CR-23 | Correct COLORREF-to-ARGB channel conversion. | Compare GDI+ and GDI fallback on Windows. |
| CR-24 | Decoded-entity transport contract, request descriptor, headers/status before body, deadlines, checked wire lengths, CR/LF checks, local key preflight, no default requested reasoning or replay. Removed unused raw HTTP framing/status helpers and the buggy legacy chunk decoder; bounded SSE handles split CRLF, standalone CR and initial BOM. | Verify real API/model schema, structured usage/incomplete details and capability choices. |
| CR-25 | Fallible owned Direct worker with queue disconnection and bounded shutdown/join; owned persistence worker with bounded shutdown/join and no racing synchronous fallback; BeginPaint/EndPaint guard; checked backbuffer ownership/selection restoration; clipboard/OLE ownership; font stock fallback excludes stock deletion; GDI+ failure cached and failed DLL loads freed; explicit export type/size checks; graphics released before unrelated GDI; GetMessage error path; ACP Drop fallback. | Owned/joined ACP worker handles and bounded asynchronous shutdown; Job Object decision; modern long-path folder dialog; comprehensive GDI acquisition wrappers; placement/DPI persistence audit. Successful GDI+ module/token intentionally live until process exit. |
| CR-26 | Persisted wall-clock creation/activity and activity ordering; manual-title protection; monotonic ID component plus loaded/storage collision checks; runtime message/turn identity; backend-injected process flags; obsolete pending-session flags removed. | Normalized workspace identities/paths, fewer public invariant-bearing fields and index identities, borrowed history serialization. |
| CR-27 | Declared Rust 1.85 MSRV; removed incompatible const use/unused variadic declaration; corrected native library groups; focused Clippy/MSRV CI and release gates; honest Direct/Code/storage/setup documentation. | Native debug/release link and manual acceptance in a full Windows development environment. |

## Verification evidence

Compilation checks have passed with Rust 1.85 for Windows, Linux, and macOS
(`cargo check`, not runnable Linux/macOS desktop backends). Stable formatting,
all-target compilation and Clippy have also passed during implementation. Existing
SSE tests were retained, obsolete raw-framing tests were removed with their unused
implementation, and split-BOM/CRLF and standalone-CR cases were added. These tests
compile under all-target checks but have not executed without a native linker.

This environment has no MSVC linker/Windows SDK: `cargo test` and
`cargo build --release` attempts failed at linking (Git's unrelated `link.exe`
was found). These are **not** a passing test run or a successful release build.
No application UI, real application-data migration, installed ACP agent, live
API call, or performance profile was exercised. No real application data was
modified; back up the entire storage directory before manual acceptance.

CI/release workflow edits are local and have not run on GitHub. Work remains on
`main`; commits are local only. Pre-existing deleted plan/design files and
untracked review documents are intentionally excluded from remediation commits.

## UX-review implementation (partial)

The UX batches implement the following source changes against
`docs/UX_REVIEW.md`. No UX finding is declared fully accepted:

| Finding | Source changes | Still open |
| --- | --- | --- |
| UX-01/02 | Consistent vibe-chat naming and Chat/Code labels; removed startup transcript notice; local key readiness without exposing/persisting credentials; owned setup-help dialog; useful missing-agent errors; turn-backend/phase status; title/status clipped around shared mode geometry; removed unexplained/flashing status dots. | Separate discoverable Code setup action, project/path detail, per-turn producing-backend presentation, live first-launch/agent acceptance. |
| UX-04 | Next draft remains editable during a turn; busy Enter preserves it; Shift+Enter inserts newline; idle Escape never clears it; busy composer no longer appears disabled. Send/Stop now activates only on release inside the original target, bound to the chat and exact turn/draft. Double-click cannot immediately stop a newly sent turn. | Native editing/IME and persistence/navigation/restart acceptance; native capture checks. |
| UX-09/12 | Destructive confirmations default to No; mouse and keyboard session deletion share one owned confirmation effect with stable targets. Pointer commands use the same hit-test on down/release, stable chat/project/message identities, native capture and drag-out/loss/cancel handling. Darker orange user bubbles, readable labels/timestamps/archived text, stronger input/scroll boundaries and Stop contrast. | Native pointer/modal acceptance, reversible archive/project actions, row-menu/accessibility coverage, real GDI/GDI+ visual/accessibility audit. |
| UX-05 | OS plain-text Unicode RichEdit composer and rename, native clipboard/undo/selection/IME, distinct focus owners, native visual-height queries and internal scrolling; deliberate cancel-on-leave rename and composition cancellation on document rebind. | Real multilingual/emoji/IME/clipboard/undo walkthrough, document-local undo limitations, fallback editing limitations. |
| UX-08 | Shared DPI-scaled adaptive geometry; History drawer below 800 logical client pixels, settings docks only with a 400-pixel chat budget or becomes a full page. Header title/mode/status occupy separate rows/regions. Settings body has wheel/scrollbar input and keyboard reveal under a fixed title/close area. | Monitor/DPI/minimum-size acceptance, placement persistence audit, transcript reading anchors. Initial/minimum window sizes are unchanged. |
| UX-11 | Native primary buttons, mode radios, settings checkboxes/scrollbar and status; MSAA editor names/descriptions, native checked/enabled/focus state, discrete status events, foreground-safe focus synchronization, no custom caret blinking in native mode. Existing application shortcuts retained; standard native editing/radio navigation applies inside controls. | Full accessible history/transcript/message/permission tree and patterns, Narrator/high-contrast/text-scaling/reduced-motion verification. No accessibility compliance claim. |
| UX-10/13/15/16 | Previews ignore system/tool notices; mode-specific settings/setup, honest Follow new replies and Code safety labels; finished reasoning disclosure no longer says Thinking; setup help explains environment precedence and plaintext local retention. | Structured tool/permission cards, fuller privacy/settings discoverability, history search/Archived view, reasoning anchors and accessible controls. |

Added std-only regression cases for busy Enter, busy newline, Unicode editing,
Escape draft/selection preservation, one-time submission, whitespace-only Enter,
and meaningful previews. Formatting, warning-free Clippy, and Rust 1.85
all-target compilation passed for Windows, Linux, and macOS after these changes.
Tests have **compiled only**, not run: the MSVC linker/Windows SDK limitation above
still applies. No native UI or live-provider acceptance was performed.

A second batch moves button, sidebar, mode, transcript link/code-copy/reasoning,
and scrollbar-click commands from pointer down to validated release. Input focus
still follows pointer down. Session rename double-click is also release-bound;
project double-click no longer toggles twice. A press is cancelled by observed
drag-out, capture loss, cancellation, keyboard input, wheel input, resize/DPI
change, navigation, or modal entry. Native capture/release is performed without
an application borrow surviving synchronous re-entry. Transcript action rectangles
are still paint-produced; generation-bound regions and true scrollbar dragging
remain open. Toast hit interception also remains open.

Eleven additional std-only pointer tests cover release-once semantics, drag-out
and cancellation, owned delete intent, row-index reuse, Send/Stop transitions,
turn identity, double-click behavior, draft/session changes, and half-open action
rectangles. These tests compile but have not run. Formatting, warning-free Clippy
and Rust 1.85 all-target checks passed again for Windows, Linux and macOS.

### Approved native/adaptive/accessibility batch

The user approved native composer/rename controls, adaptive panels and new
accessibility modules, while retaining existing application shortcuts. The new
backend module uses owned snapshots and fresh HWND state lookups across native
re-entry. RichEdit comes only from System32, uses plain-text mode and bounded
Unicode reads, and keeps user edits/undo separate from programmatic replacements.
Native primary commands still use stable release-bound core intents; double-click
retargeting to a newly revealed native control is suppressed. The message loop now
reads AppState only from the root HWND, never a native child's private userdata.
Editor replacements revalidate stable document identity and current core text.
Programmatic replacement/composition cancellation is transactional; notification
revisions reject stale reads after nested native/accessibility edits.

Children are destroyed before their font/brush ownership is released after outer
message dispatch. The single successful RichEdit DLL load intentionally remains
until process exit to
protect suspended/native accessibility objects. Failed initialization warns and
falls back without replacing the stored draft; resource failures never unload a
class while a child failed to retire.

Additional std-only cases cover adaptive budgets at 100/125/150/200% DPI,
mode hit geometry, hidden chat/history focus in a settings page, settings scroll
clamping/reveal, drawer draft preservation, cancelled toggle presses, disabled
Send tab exclusion, history focus reveal, rename focus/click-away behavior,
UTF-16/UTF-8 selection boundaries, legacy CRLF conversion, stale editor
identity/text rejection, and native FFI structure layout. These are
compile-checked only. Rust 1.85 all-target checks for Windows/Linux/macOS,
formatting and warning-free Clippy passed; runtime/IME/Narrator/visual acceptance
remains blocked by the missing MSVC linker/Windows SDK.

Further application shortcuts still require approval. Asynchronous permission
modals still steal focus; they need an explicit discoverable Review path before
automatic prompting can be removed. The remainder of UX-01–18 is not represented
as complete by this batch.

## Next acceptance pass

With the MSVC build tools/SDK installed, run the commands in README and the manual
matrix at the end of CODE_REVIEW. Use a copied storage directory for interrupted
saves, malformed/future versions, backup recovery, last-project deletion and
legacy migration. Do not treat source fixes as release acceptance. Finish the
remaining editor/layout/accessibility/resource work above without treating the
approved source implementation as completed native acceptance.

### Windows native acceptance matrix — not executed

Run debug and release builds with MSVC C++ build tools/Windows SDK, on a copied
storage directory and a disposable Code workspace. Record the Windows build,
DPI/text scale, IME, screen reader, provider/agent version and actual result.
Compilation is not a substitute for any row below.

| Area | Exercise / expected result |
| --- | --- |
| Unicode editing | Enter accented text, combining marks, emoji/ZWJ sequences, CJK and mixed RTL/LTR. Click to place the caret; drag, double-click words, Shift-select, Home/End and preferred-x Up/Down. No corruption, doubled input, lost selection or offscreen caret. |
| Native clipboard/undo | Paste multiline text (including CRLF/tabs), cut/copy and Ctrl+Z/Y; test clipboard contention. Scrolling and ordinary synchronization must not reset undo. Rebinding/replacing the document intentionally resets its undo history. |
| IME/document ownership | Compose with a real IME, move candidate selection and commit with Enter without submitting. Switch chats, rename, change DPI and deactivate/reactivate during composition; no old result may enter another document. Exercise accessibility Value/Text edits during synchronization. |
| Submit/next draft | Enter submits once when idle; Shift+Enter inserts one newline; held Enter and modifier-generated CR/LF must not add stray input. Type a new draft during a response; Enter preserves it, Stop affects only that response, Escape does not erase the draft. |
| Rename/focus | Existing name is selected; Enter commits, Escape/Tab/clicking another app control cancels. Alt+Tab, asynchronous status and panel changes must not steal focus from another app or display two active caret owners. No old name glyphs underneath the native field. |
| Adaptive geometry | At 100/125/150/200% DPI, resize across the 800px history and settings-docking thresholds and move monitors. Test long titles/status, minimum size, drawer open/close and settings page/dock. No overlap, stale target, clipped required action or hidden tab stop. |
| Settings scrolling | Wheel, scrollbar arrows/thumb/page/Home/End and keyboard focus reach the last action at minimum height. Title/close stays fixed; paint, native bounds and hit targets agree. Scrolling cancels an in-flight toggle press. |
| Activation safety | Drag out and back, release outside, lose capture, resize or open a modal while pressed. Double-click Send/Stop and controls newly exposed at the same screen location. No second/retargeted command; confirmations remain owned and default to No. |
| Accessibility | With Narrator and an accessibility inspector, verify editor names/descriptions, native Text/Value/selection availability, button names, radio/checkbox checked state, disabled Send, focus bounds/order and discrete status changes. Check high contrast/text scaling. Record painted history/transcript/message tree gaps rather than claiming compliance. |
| Failure/shutdown | With controlled initialization/load/read failure, verify warning and basic fallback retain the last synchronized draft, not a hidden unreadable value. Exercise close during editing, IME, native callbacks and provider activity; no use-after-free, lost persisted draft or orphan worker. |
| Existing bindings/providers | Recheck Ctrl+N folder picking, Ctrl+Shift+A/D, F5 Code startup, mode confirmation and both backends. Run the wider copied-data/provider matrix above; no protocol-flow or shortcut change is accepted implicitly. |
