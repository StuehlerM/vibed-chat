# Code-review remediation status

This tracks implementation against `docs/CODE_REVIEW.md` (reviewed snapshot
`541d9c3`). It is **not a declaration that all findings or acceptance checks are
complete**. No external dependencies, source modules, new shortcuts, platform
backends, release-profile changes, or manifest changes were introduced.

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
| CR-13 | Legacy v1 remains Code; unknown modes rejected; explicit default-No mode-change policy confirmation; running backend unchanged; producing backend/model/outcome saved; no reasoning replay. Full displayed tool text is retained. | Typed complete tool records and supported optional summaries, rather than tool/system text; live endpoint/schema verification. No automatic cross-mode context transfer is promised. |
| CR-14 | Draft debounce and active checkpoints; terminal and partial output saves; interrupted restoration; no destructive history trim; stable runtime message identity. | Revision-ordered background persistence and explicit context budget. Saves still serialize/write on the UI thread. |
| CR-15 | Bounded strict JSON grammar, depth/value/string/input limits, duplicate-key rejection, exact integral IDs, checked integer accessors, bounded protocol/files/SSE. Markdown inline recursion also capped. | Boundary/failure acceptance; typed schema validation can be tightened further. |
| CR-16 | Shared focus-aware Unicode routing, surrogate pairs, UNICODE_NOCHAR probe, IME-submit guard, activation character consumption, focus/caret reset, no duplicate composer focus flag. | Native-level composition/candidate placement, accessibility, complete keyboard focus coverage of mode/project/message actions. |
| CR-17 | Sidebar paints shared interaction entries; fixed vs scrolled controls corrected; visible action targets; chat-width-aware mode hit testing; screen/client wheel conversion, partial-wheel accumulation, cursor handling. | Unified whole-window layout, half-open boundaries everywhere, system wheel policy, accessibility bounds. |
| CR-18 | Transcript actions clipped and cleared on navigation/empty/scroll/geometry changes; complete link destinations; HTTP(S) allowlist and ShellExecute error reporting; source-based code copy. | Session/layout-generation-bound input regions generated before paint, rather than retaining paint-produced rectangles. |
| CR-19 | Inline spans share measured lines; explicit breaks; italic measurement; literal ampersands; bounded drawing; actual code-header row; exact fenced source including CRLF/tabs for copy; shared cached layout for height/drawing. | Grapheme-aware line breaking and dedicated horizontal code scrolling; manual rendering/copy acceptance. |
| CR-20 | Shift-motion anchor preserved and selection painted underneath transparent text. | Shared composer visual-line model, preferred-x navigation, mouse caret/drag/word selection, undo/redo, internal scroll/clipping. These require the layout/shortcut approvals in AGENTS.md. |
| CR-21 | No unconditional 60-Hz full repaint; idle/minimized animation suppressed; reusable checked backbuffer via BeginPaint DC; bounded revision/width/font-generation layout cache; bounded event drain. | Caret-only dirty rectangles, cumulative height/visible-range indexing, frame-budgeted growing-message layout, worker persistence, profiling on reference hardware. |
| CR-22 | Auto-scroll consulted without forcing follow-tail; decorative theme/approval switches replaced with honest fixed-policy rows. | Jump-to-latest, reflow anchors, adaptive/scrollable settings. Changes to layout/shortcuts require approval. |
| CR-23 | Correct COLORREF-to-ARGB channel conversion. | Compare GDI+ and GDI fallback on Windows. |
| CR-24 | Decoded-entity transport contract, request descriptor, headers/status before body, deadlines, checked wire lengths, CR/LF checks, local key preflight, no default requested reasoning or replay. | Verify real API/model schema, structured usage/incomplete details and capability choices; remove remaining unused legacy HTTP framing helpers. |
| CR-25 | BeginPaint/EndPaint guard; checked backbuffer ownership/selection restoration; clipboard/OLE ownership; font stock fallback excludes stock deletion; GDI+ failure cached and failed DLL loads freed; explicit export type/size checks; graphics released before unrelated GDI; GetMessage error path; ACP Drop fallback. | Owned/joined worker handles and bounded asynchronous shutdown; Job Object decision; modern long-path folder dialog; comprehensive GDI acquisition wrappers; placement/DPI persistence audit. Successful GDI+ module/token intentionally live until process exit. |
| CR-26 | Persisted wall-clock creation/activity and activity ordering; manual-title protection; monotonic ID component plus loaded/storage collision checks; runtime message/turn identity; backend-injected process flags; obsolete pending-session flags removed. | Normalized workspace identities/paths, fewer public invariant-bearing fields and index identities, borrowed history serialization. |
| CR-27 | Declared Rust 1.85 MSRV; removed incompatible const use/unused variadic declaration; corrected native library groups; focused Clippy/MSRV CI and release gates; honest Direct/Code/storage/setup documentation. | Native debug/release link and manual acceptance in a full Windows development environment. |

## Verification evidence

Compilation checks have passed with Rust 1.85 for Windows, Linux, and macOS
(`cargo check`, not runnable Linux/macOS desktop backends). Stable formatting,
all-target compilation and Clippy have also passed during implementation.

This environment has no MSVC linker/Windows SDK: `cargo test` and
`cargo build --release` attempts failed at linking (Git's unrelated `link.exe`
was found). These are **not** a passing test run or a successful release build.
No application UI, real application-data migration, installed ACP agent, live
API call, or performance profile was exercised. No real application data was
modified; back up the entire storage directory before manual acceptance.

CI/release workflow edits are local and have not run on GitHub. Work remains on
`main`; commits are local only. Pre-existing deleted plan/design files and
untracked review documents are intentionally excluded from remediation commits.

## Next acceptance pass

With the MSVC build tools/SDK installed, run the commands in README and the manual
matrix at the end of CODE_REVIEW. Use a copied storage directory for interrupted
saves, malformed/future versions, backup recovery, last-project deletion and
legacy migration. Do not treat source fixes as release acceptance. Finish the
remaining editor/layout/resource work above after the required design approvals.
