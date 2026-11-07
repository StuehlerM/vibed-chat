// Native Unicode RichEdit fields and primary controls. All HWNDs stay in the
// Windows backend; owned snapshots cross synchronous Win32 re-entry boundaries.
#![allow(dead_code)]

use super::{accessibility, ffi::*, window};
use crate::app::FontsHandle;
use crate::layout::Rect;
use crate::platform::windows::fonts::Fonts;
use crate::session::SessionMode;
use crate::theme::*;
use crate::ui::{FocusedElement, RenameTarget};
use std::cell::{Cell, RefCell};
use std::ptr;

const COMPOSER: usize = 1001;
const RENAME: usize = 1002;
const BUTTON_BASE: usize = 1100;
const SCROLL: usize = 1200;
const BUTTON_COUNT: usize = 11;
const WM_USER: u32 = 0x400;
const EM_EXGETSEL: u32 = WM_USER + 52;
const EM_EXSETSEL: u32 = WM_USER + 55;
const EM_GETTEXTEX: u32 = WM_USER + 94;
const EM_GETTEXTLENGTHEX: u32 = WM_USER + 95;
const EM_SETBKGNDCOLOR: u32 = WM_USER + 67;
const EM_SETEVENTMASK: u32 = WM_USER + 69;
const EM_GETLINECOUNT: u32 = 0xba;
const EM_EMPTYUNDOBUFFER: u32 = 0xcd;
const BM_CLICK: u32 = 0xf5;
const BM_SETCHECK: u32 = 0xf1;
const EN_CHANGE: usize = 0x300;
const MAX_TEXT: usize = 1_048_576;

#[derive(Clone)]
struct Field {
    key: String,
    text: String,
    cursor: usize,
    anchor: Option<usize>,
    rect: Rect,
    visible: bool,
}
struct Snapshot {
    composer: Option<Field>,
    rename: Option<Field>,
    buttons: Vec<Button>,
    fonts: Option<FontsHandle>,
    focus: Option<FocusedElement>,
    modal: bool,
    scroll: Option<(Rect, i32, i32, i32)>,
    status: String,
    status_rect: Option<Rect>,
}
#[derive(Clone)]
struct Button {
    index: usize,
    name: String,
    rect: Rect,
    focus: FocusedElement,
    checked: Option<bool>,
    enabled: bool,
}
struct Native {
    parent: HWND,
    library: HMODULE,
    composer: HWND,
    rename: HWND,
    buttons: [HWND; BUTTON_COUNT],
    scrollbar: HWND,
    status: HWND,
    font: Option<FontsHandle>,
    fields: [Option<(String, String)>; 2],
    last_status: String,
    brush: HBRUSH,
}
struct PointerRelease {
    hwnd: HWND,
    point: POINT,
    time: std::time::Instant,
}
thread_local! {
    static RECENT_RELEASE: RefCell<Option<PointerRelease>> = const { RefCell::new(None) };
    static NATIVE: RefCell<Option<Native>> = const { RefCell::new(None) };
    static RETIRED: RefCell<Vec<Native>> = const { RefCell::new(Vec::new()) };
    static SYNCING: Cell<bool> = const { Cell::new(false) };
    static COMPOSING: Cell<bool> = const { Cell::new(false) };
    static CONSUMED_RETURN: Cell<bool> = const { Cell::new(false) };
    static CONSUMED_CHAR: Cell<Option<u32>> = const { Cell::new(None) };
    static REQUESTED_HEIGHT: Cell<i32> = const { Cell::new(0) };
    static EDIT_REVISION: Cell<[u64; 2]> = const { Cell::new([0; 2]) };
    static PROGRAMMATIC_SET: Cell<[bool; 2]> = const { Cell::new([false; 2]) };
    static RELEASING_BUTTON: Cell<HWND> = const { Cell::new(ptr::null_mut()) };
}
struct FieldUpdateGuard {
    index: usize,
    armed: bool,
}
impl FieldUpdateGuard {
    fn new(index: usize, armed: bool) -> Self {
        if armed {
            PROGRAMMATIC_SET.with(|setting| {
                let mut values = setting.get();
                values[index] = true;
                setting.set(values);
            });
        }
        Self { index, armed }
    }
}
impl Drop for FieldUpdateGuard {
    fn drop(&mut self) {
        if self.armed {
            PROGRAMMATIC_SET.with(|setting| {
                let mut values = setting.get();
                values[self.index] = false;
                setting.set(values);
            });
        }
    }
}

struct SyncGuard;
impl Drop for SyncGuard {
    fn drop(&mut self) {
        SYNCING.with(|b| b.set(false));
    }
}

fn handles() -> Option<(HWND, HWND, [HWND; BUTTON_COUNT], HWND, HWND)> {
    NATIVE.with(|n| {
        n.borrow()
            .as_ref()
            .map(|n| (n.composer, n.rename, n.buttons, n.scrollbar, n.status))
    })
}

fn field_rect(state: &crate::app::AppState) -> Rect {
    let s = state.dpi as f32 / 96.0;
    let px = |n: i32| (n as f32 * s) as i32;
    let top = state.input_box_top();
    let bottom = state.client_height - px(HINT_ROW_HEIGHT + INPUT_MARGIN);
    Rect {
        left: state.chat_left() + px(INPUT_MARGIN + SPACE_4),
        top: top + px(INPUT_LINE_PADDING),
        right: state.chat_right()
            - px(INPUT_MARGIN + SEND_BTN_MARGIN + SEND_BTN_SIZE + SEND_BTN_MARGIN + SPACE_2),
        bottom: bottom - px(INPUT_LINE_PADDING),
    }
}

fn snapshot(parent: HWND) -> Option<Snapshot> {
    let pointer = window::get_app_state(parent);
    if pointer.is_null() {
        return None;
    }
    // SAFETY: owning UI thread; this borrow ends before any native call.
    let state = unsafe { &mut *pointer };
    if state.window_minimized || state.client_width <= 0 || state.client_height <= 0 {
        return None;
    }
    let panels = state.layout();
    let s = state.dpi as f32 / 96.0;
    let px = |n: i32| (n as f32 * s) as i32;
    let composer = state
        .sessions
        .get(state.active_session)
        .filter(|_| state.native_editor_active)
        .map(|session| Field {
            key: session.local_id.clone(),
            text: session.chat.input_buffer.clone(),
            cursor: session.chat.cursor_pos,
            anchor: session.chat.selection_start,
            rect: field_rect(state),
            visible: panels.content_visible,
        });
    let rename_target = state.rename_target.filter(|_| state.native_editor_active);
    let offset = state.sidebar_scroll_offset;
    let height = state.client_height;
    let rename_rect = state.with_sidebar_layout(|top, _, entries| {
        entries.iter().find_map(|entry| {
            use crate::interaction::SidebarEntry;
            let (is_target, y, bottom) = match *entry {
                SidebarEntry::SessionItem {
                    session_idx,
                    top,
                    bottom,
                    ..
                } => (
                    rename_target == Some(RenameTarget::Session(session_idx)),
                    top,
                    bottom,
                ),
                SidebarEntry::ProjectHeader {
                    project_idx,
                    top,
                    bottom,
                } => (
                    rename_target == Some(RenameTarget::Project(project_idx)),
                    top,
                    bottom,
                ),
                _ => return None,
            };
            if !is_target
                || panels.sidebar.width() == 0
                || y - offset < top
                || bottom - offset > height - px(SIDEBAR_FOOTER_HEIGHT)
            {
                return None;
            }
            Some(Rect {
                left: px(28),
                right: panels.sidebar.right
                    - px(if matches!(rename_target, Some(RenameTarget::Project(_))) {
                        48
                    } else {
                        16
                    }),
                top: y - offset + px(2),
                bottom: y - offset + px(28),
            })
        })
    });
    let rename = rename_target.zip(rename_rect).and_then(|(target, rect)| {
        let key = match target {
            RenameTarget::Session(i) => format!("chat:{}", state.sessions.get(i)?.local_id),
            RenameTarget::Project(i) => format!("project:{}", state.projects.get(i)?.path),
        };
        Some(Field {
            key,
            text: state.rename_buffer.clone(),
            cursor: state.rename_cursor,
            anchor: None,
            rect,
            visible: !panels.settings_sheet,
        })
    });
    if rename_target.is_some() && rename.as_ref().is_none_or(|f| !f.visible) {
        // Moving a rename out of its visible row follows the click-away policy:
        // cancel, never leave an invisible editor intercepting application keys.
        state.cancel_rename();
    }
    let mut buttons = Vec::new();
    if panels.sidebar_collapsed && state.sidebar_drawer_open && !panels.settings_sheet {
        buttons.push(Button {
            index: 0,
            name: "Close history".into(),
            rect: Rect {
                left: panels.sidebar.right + px(16),
                right: (panels.sidebar.right + px(128)).min(state.client_width - px(8)),
                top: px(8),
                bottom: px(36),
            },
            focus: FocusedElement::History,
            checked: None,
            enabled: true,
        });
    }
    if panels.content_visible {
        if panels.sidebar_collapsed {
            buttons.push(Button {
                index: 0,
                name: "History".into(),
                rect: panels.history_button(state.dpi),
                focus: FocusedElement::History,
                checked: None,
                enabled: true,
            });
        }
        buttons.push(Button {
            index: 1,
            name: "Settings".into(),
            rect: panels.settings_button(state.dpi),
            focus: FocusedElement::SettingsBtn,
            checked: None,
            enabled: true,
        });
        if let Some(session) = state.sessions.get(state.active_session) {
            let radio = compute_mode_radio_layout(
                state.chat_left(),
                state.chat_right(),
                panels.header_height,
                s,
            );
            for (index, name, mode, left) in [
                (2, "Chat", SessionMode::Direct, radio.direct_left),
                (3, "Code", SessionMode::Code, radio.code_left),
            ] {
                buttons.push(Button {
                    index,
                    name: name.into(),
                    rect: Rect {
                        left,
                        top: radio.top,
                        right: left + px(MODE_RADIO_OPTION_W),
                        bottom: radio.top + radio.h,
                    },
                    focus: FocusedElement::ModeRadio(mode),
                    checked: Some(mode == session.mode),
                    enabled: true,
                });
            }
            if let Some((left, top, right, bottom)) = state.send_btn_rect() {
                buttons.push(Button {
                    index: 4,
                    name: if session.active_turn.is_some() {
                        "Stop response"
                    } else {
                        "Send message"
                    }
                    .into(),
                    rect: Rect {
                        left,
                        top,
                        right,
                        bottom,
                    },
                    focus: FocusedElement::SendButton,
                    checked: None,
                    enabled: session
                        .active_turn
                        .as_ref()
                        .map_or(!session.chat.input_buffer.trim().is_empty(), |t| {
                            !t.cancelling
                        }),
                });
            }
        }
    }
    if panels.sidebar.width() > 0 && !panels.settings_sheet {
        buttons.push(Button {
            index: 5,
            name: "New chat".into(),
            rect: Rect {
                left: px(8),
                top: px(8),
                right: panels.sidebar.right - px(8),
                bottom: px(48),
            },
            focus: FocusedElement::NewChat,
            checked: None,
            enabled: true,
        });
        buttons.push(Button {
            index: 6,
            name: "Add project".into(),
            rect: Rect {
                left: px(8),
                top: px(52),
                right: panels.sidebar.right - px(8),
                bottom: px(80),
            },
            focus: FocusedElement::NewProject,
            checked: None,
            enabled: true,
        });
        // Use the existing footer action when the drawer covers the header.
        if !panels.content_visible {
            buttons.push(Button {
                index: 1,
                name: "Settings".into(),
                rect: Rect {
                    left: px(8),
                    top: height - px(44),
                    right: panels.sidebar.right - px(8),
                    bottom: height - px(4),
                },
                focus: FocusedElement::SettingsBtn,
                checked: None,
                enabled: true,
            });
        }
    }
    if state.show_settings {
        buttons.push(Button {
            index: 7,
            name: "Close settings".into(),
            rect: Rect {
                left: state.client_width - px(48),
                top: px(24),
                right: state.client_width - px(24),
                bottom: px(48),
            },
            focus: FocusedElement::SettingsClose,
            checked: None,
            enabled: true,
        });
        let layout = compute_settings_layout(s);
        let top = layout.reconnect_y - state.settings_scroll;
        if top >= px(76) && top + layout.reconnect_btn_h <= height {
            let chat = state
                .sessions
                .get(state.active_session)
                .is_none_or(|s| s.mode == SessionMode::Direct);
            buttons.push(Button {
                index: 10,
                name: if chat {
                    "Chat setup"
                } else {
                    "Restart agent (F5)"
                }
                .into(),
                rect: Rect {
                    left: panels.settings.left + px(24),
                    top,
                    right: state.client_width - px(24),
                    bottom: top + layout.reconnect_btn_h,
                },
                focus: FocusedElement::Reconnect,
                checked: None,
                enabled: true,
            });
        }
        for (index, i, name, checked) in [
            (8, 1, "Follow new replies", state.settings.auto_scroll),
            (9, 3, "Show archived", state.settings.show_archived),
        ] {
            let top = layout.toggle_tops[i] - state.settings_scroll;
            if top >= px(76) && top + layout.toggle_row_h <= height {
                buttons.push(Button {
                    index,
                    name: name.into(),
                    rect: Rect {
                        left: panels.settings.left + px(24),
                        top,
                        right: state.client_width - px(24),
                        bottom: top + layout.toggle_row_h,
                    },
                    focus: FocusedElement::SettingsToggle(i),
                    checked: Some(checked),
                    enabled: true,
                });
            }
        }
    }
    let scroll = state.show_settings.then_some((
        Rect {
            left: state.client_width - px(16),
            top: px(76),
            right: state.client_width,
            bottom: height,
        },
        state.settings_scroll,
        state.settings_max_scroll(),
        height - px(76),
    ));
    Some(Snapshot {
        composer,
        rename,
        buttons,
        fonts: state.fonts.clone(),
        focus: state.focused_element,
        modal: state.modal_active,
        scroll,
        status: state.active_status().into(),
        status_rect: panels.content_visible.then_some(Rect {
            left: state.chat_left() + px(164),
            top: px(48),
            right: state.chat_right() - px(16),
            bottom: px(72),
        }),
    })
}

pub fn create(parent: HWND) -> bool {
    // SAFETY: creation runs on the UI thread after parent/AppState association.
    // No borrowed AppState/native registry entry survives any window call.
    unsafe {
        // Load only the OS copy, never a DLL planted in a project/current directory.
        let library = LoadLibraryExW(to_wide("Msftedit.dll").as_ptr(), ptr::null_mut(), 0x800); // LOAD_LIBRARY_SEARCH_SYSTEM32
        if library.is_null() {
            return false;
        }
        let can_unload = Cell::new(true);
        let make = |class: &str, id: usize, style: u32| {
            let hwnd = CreateWindowExW(
                if id == RENAME { 0x200 } else { 0 }, // WS_EX_CLIENTEDGE for inline rename
                to_wide(class).as_ptr(),
                to_wide("").as_ptr(),
                // Advertise native keyboard focusability to accessibility clients;
                // the root still owns Tab order (no IsDialogMessage navigation).
                WS_CHILD | WS_TABSTOP | style,
                0,
                0,
                1,
                1,
                parent,
                id as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null_mut(),
            );
            if !hwnd.is_null() && SetWindowSubclass(hwnd, Some(child_proc), id, 0) == 0 {
                let released = DestroyWindow(hwnd) != 0;
                can_unload.set(can_unload.get() & released);
                ptr::null_mut()
            } else {
                hwnd
            }
        };
        let composer = make(
            "RICHEDIT50W",
            COMPOSER,
            0x0004 | 0x0040 | 0x1000 | 0x2000 | 0x0020_0000,
        ); // multiline/autovscroll/wantreturn/vscroll
        let rename = make("RICHEDIT50W", RENAME, 0x0080); // single-line auto horizontal scroll
        if composer.is_null() || rename.is_null() {
            let mut released = can_unload.get();
            if !composer.is_null() {
                released &= DestroyWindow(composer) != 0;
            }
            if !rename.is_null() {
                released &= DestroyWindow(rename) != 0;
            }
            if released {
                FreeLibrary(library);
            }
            return false;
        }
        for hwnd in [composer, rename] {
            if SendMessageW(hwnd, WM_USER + 89, 1, 0) != 0 {
                // EM_SETTEXTMODE: TM_PLAINTEXT
                let released = (DestroyWindow(composer) != 0) & (DestroyWindow(rename) != 0);
                if released && can_unload.get() {
                    FreeLibrary(library);
                }
                return false;
            }
            SendMessageW(hwnd, WM_USER + 53, 0, MAX_TEXT as LPARAM); // EM_EXLIMITTEXT
            SendMessageW(hwnd, EM_SETBKGNDCOLOR, 0, INPUT_BG_COLOR as LPARAM);
            SendMessageW(hwnd, EM_SETEVENTMASK, 0, 1 | 0x0004_0000); // ENM_CHANGE | ENM_REQUESTRESIZE
        }
        let mut buttons = [ptr::null_mut(); BUTTON_COUNT];
        for (i, button) in buttons.iter_mut().enumerate() {
            let style = if i == 2 || i == 3 {
                4
            } else if i == 8 || i == 9 {
                2
            } else {
                0x0b // BS_OWNERDRAW retains native Button semantics.
            };
            *button = make("BUTTON", BUTTON_BASE + i, style);
        }
        let scrollbar = make("SCROLLBAR", SCROLL, 1);
        let status = CreateWindowExW(
            0,
            to_wide("STATIC").as_ptr(),
            to_wide("").as_ptr(),
            WS_CHILD,
            0,
            0,
            1,
            1,
            parent,
            ptr::null_mut(),
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );
        if buttons.iter().any(|h| h.is_null()) || scrollbar.is_null() || status.is_null() {
            let mut released = can_unload.get();
            for hwnd in [composer, rename, scrollbar, status]
                .into_iter()
                .chain(buttons)
            {
                if !hwnd.is_null() {
                    released &= DestroyWindow(hwnd) != 0;
                }
            }
            // Never unload a class DLL if a child failed to retire.
            if released {
                FreeLibrary(library);
            }
            return false;
        }
        NATIVE.with(|n| {
            *n.borrow_mut() = Some(Native {
                parent,
                library,
                composer,
                rename,
                buttons,
                scrollbar,
                status,
                font: None,
                fields: [None, None],
                last_status: String::new(),
                brush: CreateSolidBrush(SESSION_HEADER_BG),
            })
        });
        let named = accessibility::name(composer, "Message", "Enter sends when idle; during a response Enter keeps your draft. Shift+Enter inserts a newline. Escape never clears the draft.")
            & accessibility::name(rename, "Rename", "Enter commits; Escape or clicking away cancels. The current name is selected.");
        let pointer = window::get_app_state(parent);
        if !pointer.is_null() {
            (*pointer).native_editor_active = true;
            if !named {
                (*pointer).show_toast(
                    "Native editor accessibility labels could not be installed",
                    crate::ui::ToastKind::Error,
                );
            }
        }
        true
    }
}

fn position(hwnd: HWND, rect: Rect, visible: bool) {
    if hwnd.is_null() {
        return;
    }
    // SAFETY: all handles come from our live child registry; no Rust borrow is
    // held while ShowWindow/SetWindowPos dispatch control notifications.
    unsafe {
        let mut old = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let parent = GetParent(hwnd);
        GetWindowRect(hwnd, &mut old);
        let mut p = POINT {
            x: old.left,
            y: old.top,
        };
        ScreenToClient(parent, &mut p);
        if (p.x, p.y, old.width(), old.height())
            != (rect.left, rect.top, rect.width(), rect.height())
        {
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.width().max(1),
                rect.height().max(1),
                0x14,
            );
        }
        if (IsWindowVisible(hwnd) != 0) != visible {
            ShowWindow(hwnd, if visible { SW_SHOW } else { 0 });
        }
    }
}

fn field_matches(state: &crate::app::AppState, index: usize, field: &Field) -> bool {
    if !state.native_editor_active {
        return false;
    }
    if index == 0 {
        state
            .sessions
            .get(state.active_session)
            .is_some_and(|s| s.local_id == field.key && s.chat.input_buffer == field.text)
    } else {
        let key = match state.rename_target {
            Some(RenameTarget::Session(i)) => state
                .sessions
                .get(i)
                .map(|s| format!("chat:{}", s.local_id)),
            Some(RenameTarget::Project(i)) => {
                state.projects.get(i).map(|p| format!("project:{}", p.path))
            }
            None => None,
        };
        key.as_deref() == Some(&field.key) && state.rename_buffer == field.text
    }
}

fn field_current(hwnd: HWND, index: usize, field: &Field) -> bool {
    // SAFETY: UI-thread lookup/query only; the borrow ends before any native
    // operation. Revalidate after native calls that could change the document.
    unsafe {
        let pointer = window::get_app_state(GetParent(hwnd));
        !pointer.is_null() && field_matches(&*pointer, index, field)
    }
}

fn sync_field(hwnd: HWND, index: usize, field: Option<&Field>) -> bool {
    if field.is_some_and(|field| !field_current(hwnd, index, field)) {
        return false;
    }
    let changed = NATIVE.with(|n| {
        let mut n = n.borrow_mut();
        let Some(n) = n.as_mut() else {
            return (false, false);
        };
        let desired = field.map(|f| (f.key.clone(), f.text.clone()));
        let changed_key = n.fields[index].as_ref().map(|f| &f.0) != desired.as_ref().map(|f| &f.0);
        let changed_text = n.fields[index] != desired;
        n.fields[index] = desired;
        (changed_key, changed_text)
    });
    let Some(field) = field else {
        position(hwnd, Rect::default(), false);
        return false;
    };
    let transaction = FieldUpdateGuard::new(index, changed.1);
    position(hwnd, field.rect, field.visible);
    if changed.1 {
        if !field_current(hwnd, index, field) {
            return false;
        }
        {
            // SAFETY: cancel unfinished composition before replacing a document,
            // so an old IME result cannot land in a newly selected chat/draft.
            unsafe {
                let context = ImmGetContext(hwnd);
                if !context.is_null() {
                    ImmNotifyIME(context, 0x15, 4, 0);
                    ImmReleaseContext(hwnd, context);
                }
            }
            COMPOSING.with(|c| c.set(false));
        }
        let wide = to_wide(&native_text(&field.text));
        // SAFETY: strings and selection ranges are owned locally through the
        // synchronous control messages. Matching EN_CHANGE echoes are ignored.
        unsafe {
            if !field_current(hwnd, index, field) {
                return false;
            }
            let installed = wide.len() <= MAX_TEXT + 1 && SetWindowTextW(hwnd, wide.as_ptr()) != 0;
            drop(transaction);
            if !installed {
                let parent = GetParent(hwnd);
                let pointer = window::get_app_state(parent);
                if !pointer.is_null() {
                    (*pointer).native_editor_active = false;
                    (*pointer).show_toast("Native editor could not load this draft; using basic input without changing its text", crate::ui::ToastKind::Error);
                }
                ShowWindow(hwnd, 0);
                return false;
            }
            // Publish only a complete native replacement. Read back once after
            // the setter returns so nested accessibility edits are not dropped,
            // while partial/failed programmatic writes never replace core text.
            command(
                GetParent(hwnd),
                ((EN_CHANGE << 16) | if index == 0 { COMPOSER } else { RENAME }) as WPARAM,
                hwnd as LPARAM,
            );
            let unchanged = NATIVE.with(|n| {
                n.borrow()
                    .as_ref()
                    .and_then(|n| n.fields[index].as_ref())
                    .is_some_and(|f| f.1 == field.text)
            });
            if !unchanged {
                return changed.0 && field.visible;
            }
            SendMessageW(hwnd, EM_EMPTYUNDOBUFFER, 0, 0);
            let caret = utf16_offset(&field.text, field.cursor);
            let range = CHARRANGE {
                cpMin: if index == 1 && changed.0 {
                    0
                } else {
                    field
                        .anchor
                        .map_or(caret, |byte| utf16_offset(&field.text, byte))
                },
                cpMax: if index == 1 && changed.0 { -1 } else { caret },
            };
            SendMessageW(hwnd, EM_EXSETSEL, 0, &range as *const _ as LPARAM);
        }
    }
    changed.0 && field.visible
}

pub fn sync(parent: HWND) {
    if SYNCING.with(|b| b.replace(true)) {
        return;
    }
    let _guard = SyncGuard;
    let Some((composer, rename, buttons, scrollbar, status)) = handles() else {
        return;
    };
    let Some(mut model) = snapshot(parent) else {
        return;
    };
    let old_font = NATIVE.with(|n| n.borrow().as_ref().and_then(|n| n.font.clone()));
    let font_changed = old_font
        .as_ref()
        .and_then(|f| f.downcast_ref::<Fonts>())
        .map(|f| f.normal)
        != model
            .fonts
            .as_ref()
            .and_then(|f| f.downcast_ref::<Fonts>())
            .map(|f| f.normal);
    if font_changed {
        if let Some(fonts) = model.fonts.as_ref().and_then(|f| f.downcast_ref::<Fonts>()) {
            // Owner-drawn controls may paint synchronously during WM_SETFONT;
            // publish the new owner first, retaining old_font until all switch.
            NATIVE.with(|n| {
                if let Some(n) = n.borrow_mut().as_mut() {
                    n.font = model.fonts.clone();
                }
            });
            // SAFETY: both old/new font owners outlive the synchronous WM_SETFONT
            // calls. Drop the old owner only after every control switched fonts.
            unsafe {
                for hwnd in [composer, rename, status].into_iter().chain(buttons) {
                    if !hwnd.is_null() {
                        SendMessageW(hwnd, WM_SETFONT, fonts.normal as WPARAM, 1);
                    }
                }
            }
            for hwnd in [composer, rename] {
                // SAFETY: CHARFORMATW is initialized and copied synchronously.
                unsafe {
                    let format = CHARFORMATW {
                        cbSize: std::mem::size_of::<CHARFORMATW>() as u32,
                        dwMask: 0x4000_0000,
                        dwEffects: 0,
                        yHeight: 0,
                        yOffset: 0,
                        crTextColor: INPUT_TEXT_COLOR,
                        bCharSet: 0,
                        bPitchAndFamily: 0,
                        szFaceName: [0; 32],
                    };
                    SendMessageW(hwnd, WM_USER + 68, 4, &format as *const _ as LPARAM);
                }
            }
            NATIVE.with(|n| {
                if let Some(n) = n.borrow_mut().as_mut() {
                    n.font = model.fonts.clone();
                }
            });
        }
    }
    drop(old_font);
    sync_field(composer, 0, model.composer.as_ref());
    let pointer = window::get_app_state(parent);
    // SAFETY: UI-thread flag query only; no reference crosses position/snapshot.
    if !pointer.is_null() && !unsafe { (*pointer).native_editor_active } {
        position(composer, Rect::default(), false);
        if let Some(fallback) = snapshot(parent) {
            model = fallback;
        }
    }
    // RichEdit counts the same soft-wrapped visual lines it displays. Limit
    // growth by both eight lines and available window height, then scroll inside.
    if model.composer.as_ref().is_some_and(|f| f.visible) {
        // SAFETY: native queries have no AppState borrow; measurements are owned.
        REQUESTED_HEIGHT.with(|h| h.set(0));
        // SAFETY: the synchronous EN_REQUESTRESIZE handler records only an owned
        // height. No application or native-registry borrow crosses this query.
        unsafe {
            SendMessageW(composer, WM_USER + 65, 0, 0);
        }
        let requested_height = REQUESTED_HEIGHT.with(Cell::get);
        let (lines, font_height) = unsafe {
            let lines = SendMessageW(composer, EM_GETLINECOUNT, 0, 0).max(1) as i32;
            let dc = GetDC(composer);
            let mut tm: TEXTMETRICW = std::mem::zeroed();
            if !dc.is_null() {
                let font = model
                    .fonts
                    .as_ref()
                    .and_then(|f| f.downcast_ref::<Fonts>())
                    .map(|f| f.normal);
                let previous = font.map(|f| SelectObject(dc, f));
                GetTextMetricsW(dc, &mut tm);
                if let Some(previous) = previous {
                    SelectObject(dc, previous);
                }
                ReleaseDC(composer, dc);
            }
            (lines, tm.tmHeight.max(14))
        };
        let pointer = window::get_app_state(parent);
        if !pointer.is_null() {
            // SAFETY: local UI borrow ends before the second snapshot/native sync.
            let state = unsafe { &mut *pointer };
            let scale = state.dpi as f32 / 96.0;
            let cap = ((state.client_height as f32 * 0.4) as i32)
                .max((INPUT_MIN_HEIGHT as f32 * scale) as i32);
            let native_height = if requested_height > 0 {
                requested_height
            } else {
                lines * font_height
            };
            let dev =
                (native_height.min(INPUT_MAX_LINES * font_height) + (16.0 * scale) as i32).min(cap);
            let height = ((dev as f32 / scale).ceil() as i32).max(INPUT_MIN_HEIGHT);
            let changed = if let Some(chat) = state.active_chat() {
                let changed = chat.input_height != height;
                chat.input_height = height;
                changed
            } else {
                false
            };
            if changed {
                state.link_rects.clear();
                state.code_block_rects.clear();
            }
        }
        if let Some(new_model) = snapshot(parent) {
            model = new_model;
        }
        if let Some(field) = &model.composer {
            position(composer, field.rect, field.visible);
        }
    }
    let mut new_rename = sync_field(rename, 1, model.rename.as_ref());
    let pointer = window::get_app_state(parent);
    // SAFETY: same UI-thread flag query; a failed rename load retires both native
    // fields immediately, rather than focusing a hidden/unreadable control.
    if !pointer.is_null() && !unsafe { (*pointer).native_editor_active } {
        position(composer, Rect::default(), false);
        position(rename, Rect::default(), false);
        if let Some(fallback) = snapshot(parent) {
            model = fallback;
        }
        new_rename = false;
    }
    if new_rename {
        let project = model
            .rename
            .as_ref()
            .is_some_and(|f| f.key.starts_with("project:"));
        accessibility::name(rename, if project { "Rename project" } else { "Rename chat" },
            "Current name selected. Enter commits; Escape, Tab or clicking another app control cancels.");
    }
    // All controls are moved/hidden from the same final layout used by paint.
    for (index, hwnd) in buttons.into_iter().enumerate() {
        let button = model.buttons.iter().find(|b| b.index == index);
        position(
            hwnd,
            button.map_or(Rect::default(), |b| b.rect),
            button.is_some(),
        );
        if let Some(button) = button {
            // SAFETY: local strings, no registry/application borrows during calls.
            unsafe {
                set_text_if_changed(hwnd, &button.name);
                EnableWindow(hwnd, i32::from(button.enabled && !model.modal));
                if let Some(checked) = button.checked {
                    SendMessageW(hwnd, BM_SETCHECK, usize::from(checked), 0);
                }
            }
        }
    }
    position(
        scrollbar,
        model.scroll.map_or(Rect::default(), |s| s.0),
        model.scroll.is_some(),
    );
    if let Some((_, position, max, page)) = model.scroll {
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: 0x17,
            nMin: 0,
            nMax: max + page - 1,
            nPage: page.max(1) as u32,
            nPos: position,
            nTrackPos: 0,
        };
        // SAFETY: SCROLLINFO lives through the synchronous call.
        unsafe {
            SetScrollInfo(scrollbar, 2, &info, 1);
        }
    }
    let status_changed = NATIVE.with(|n| {
        let mut n = n.borrow_mut();
        let Some(n) = n.as_mut() else {
            return false;
        };
        if n.last_status == model.status {
            return false;
        }
        n.last_status = model.status.clone();
        true
    });
    position(
        status,
        model.status_rect.unwrap_or_default(),
        model.status_rect.is_some(),
    );
    if status_changed {
        // A non-focusable native status label exposes discrete backend states;
        // no token/reasoning text is copied into live accessibility announcements.
        unsafe {
            set_text_if_changed(status, &model.status);
        }
        accessibility::state_changed(status);
    }
    // Never steal focus from another app or from an active modal owner.
    unsafe {
        let current = GetFocus();
        if !model.modal
            && GetForegroundWindow() == parent
            && (current == parent || IsChild(parent, current) != 0)
        {
            let target = if model.focus == Some(FocusedElement::SettingsScrollbar)
                && model.scroll.is_some()
            {
                scrollbar
            } else if model.rename.as_ref().is_some_and(|f| f.visible) {
                rename
            } else if model.focus == Some(FocusedElement::Input)
                && model.composer.as_ref().is_some_and(|f| f.visible)
            {
                composer
            } else {
                model
                    .buttons
                    .iter()
                    .find(|b| Some(b.focus) == model.focus)
                    .map_or(parent, |b| buttons[b.index])
            };
            if current != target {
                SetFocus(target);
            }
        }
    }
}

unsafe fn set_text_if_changed(hwnd: HWND, text: &str) {
    if hwnd.is_null() {
        return;
    }
    let mut current = vec![0u16; text.encode_utf16().count() + 2];
    let count = GetWindowTextW(hwnd, current.as_mut_ptr(), current.len() as i32).max(0) as usize;
    if String::from_utf16_lossy(&current[..count]) != text {
        SetWindowTextW(hwnd, to_wide(text).as_ptr());
    }
}

fn read_text(hwnd: HWND) -> Option<(String, CHARRANGE)> {
    // SAFETY: RichEdit reads/writes only the sized local buffers and structures.
    unsafe {
        let length = GETTEXTLENGTHEX {
            flags: 2 | 8,
            codepage: 1200,
        }; // precise number of UTF-16 characters
        let count = SendMessageW(hwnd, EM_GETTEXTLENGTHEX, &length as *const _ as WPARAM, 0);
        if count < 0 || count as usize > MAX_TEXT {
            return None;
        }
        let mut buffer = vec![0u16; count as usize + 1];
        let request = GETTEXTEX {
            cb: (buffer.len() * 2) as u32,
            flags: 0,
            codepage: 1200,
            lpDefaultChar: ptr::null(),
            lpUsedDefChar: ptr::null_mut(),
        };
        let read = SendMessageW(
            hwnd,
            EM_GETTEXTEX,
            &request as *const _ as WPARAM,
            buffer.as_mut_ptr() as LPARAM,
        );
        if read < 0 || read as usize >= buffer.len() {
            return None;
        }
        let text = String::from_utf16_lossy(&buffer[..read as usize]).replace('\r', "\n");
        let mut range = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(hwnd, EM_EXGETSEL, 0, &mut range as *mut _ as LPARAM);
        Some((text, range))
    }
}

pub fn notify(wparam: WPARAM, lparam: LPARAM) -> bool {
    if wparam != COMPOSER || lparam == 0 {
        return false;
    }
    let Some((composer, _, _, _, _)) = handles() else {
        return false;
    };
    // SAFETY: WM_NOTIFY owns NMHDR for this dispatch. Only read the larger
    // REQRESIZE after validating the sender and EN_REQUESTRESIZE code.
    unsafe {
        let header = &*(lparam as *const NMHDR);
        if header.hwndFrom != composer || header.code != 0x0701 {
            return false;
        }
        let request = &*(lparam as *const REQRESIZE);
        REQUESTED_HEIGHT.with(|h| h.set(request.rc.height().max(0)));
    }
    true
}

pub fn command(parent: HWND, wparam: WPARAM, lparam: LPARAM) -> bool {
    let id = wparam & 0xffff;
    let code = wparam >> 16;
    let Some((composer, rename, buttons, _, _)) = handles() else {
        return false;
    };
    if id == COMPOSER || id == RENAME {
        if code == 0x501 {
            let pointer = window::get_app_state(parent);
            if !pointer.is_null() {
                // SAFETY: UI thread; no native call while state is borrowed.
                unsafe {
                    (*pointer).show_toast(
                        "Text limit reached; the draft is still editable",
                        crate::ui::ToastKind::Error,
                    );
                }
            }
            return true;
        }
        if code != EN_CHANGE {
            return true;
        }
        let hwnd = if id == COMPOSER { composer } else { rename };
        if hwnd as LPARAM != lparam {
            return true;
        }
        let index = usize::from(id == RENAME);
        let revision = EDIT_REVISION.with(|revision| {
            let mut values = revision.get();
            values[index] = values[index].wrapping_add(1);
            revision.set(values);
            values[index]
        });
        if PROGRAMMATIC_SET.with(|setting| setting.get()[index]) {
            return true;
        }
        let pointer = window::get_app_state(parent);
        // SAFETY: ignore retired native fields after fallback; their hidden
        // documents must not overwrite the visible basic editor's current text.
        if pointer.is_null() || !unsafe { (*pointer).native_editor_active } {
            return true;
        }
        let value = read_text(hwnd);
        // Synchronous queries can pump a newer accessibility edit or document
        // rebind. An older notification must not overwrite that newer snapshot.
        if EDIT_REVISION.with(|value| value.get()[index]) != revision {
            return true;
        }
        let Some((text, range)) = value else {
            let pointer = window::get_app_state(parent);
            if !pointer.is_null() {
                // SAFETY: native queries finished. Show the last synchronized
                // core text rather than allow submission of a different draft
                // from the one still visible in an unreadable native control.
                unsafe {
                    (*pointer).native_editor_active = false;
                    (*pointer).show_toast("Native edit could not be read; basic input keeps the last synchronized draft", crate::ui::ToastKind::Error);
                }
            }
            sync(parent);
            unsafe {
                InvalidateRect(parent, ptr::null(), 0);
            }
            return true;
        };
        // Ignore our exact programmatic replacement, not all changes during
        // sync: nested accessibility edits must still reach their owned draft.
        let unchanged = NATIVE.with(|n| {
            n.borrow()
                .as_ref()
                .and_then(|n| n.fields[index].as_ref())
                .is_some_and(|f| f.1 == text)
        });
        if unchanged {
            return true;
        }
        let binding = NATIVE.with(|n| {
            n.borrow()
                .as_ref()
                .and_then(|n| n.fields[index].as_ref().map(|f| f.0.clone()))
        });
        let pointer = window::get_app_state(parent);
        if pointer.is_null() {
            return true;
        }
        // SAFETY: native queries are complete before borrowing the application.
        let state = unsafe { &mut *pointer };
        if index == 0 {
            if let Some(session) = state.sessions.get_mut(state.active_session) {
                if binding.as_deref() != Some(&session.local_id) {
                    return true;
                }
                session.chat.input_buffer = text.clone();
                session.chat.cursor_pos = byte_offset(&text, range.cpMax);
                session.chat.selection_start =
                    (range.cpMin != range.cpMax).then(|| byte_offset(&text, range.cpMin));
                state.draft_save_due = Some(std::time::Instant::now());
            }
        } else if state.rename_target.is_some() {
            let target_key = match state.rename_target {
                Some(RenameTarget::Session(i)) => state
                    .sessions
                    .get(i)
                    .map(|s| format!("chat:{}", s.local_id)),
                Some(RenameTarget::Project(i)) => {
                    state.projects.get(i).map(|p| format!("project:{}", p.path))
                }
                None => None,
            };
            if binding != target_key {
                return true;
            }
            state.rename_buffer = text.clone();
            state.rename_cursor = byte_offset(&text, range.cpMax);
        }
        state.repaint_needed = true;
        NATIVE.with(|n| {
            if let Some(n) = n.borrow_mut().as_mut() {
                n.fields[index] = binding.map(|key| (key, text));
            }
        });
        // Parent paint will remeasure visual lines; don't reset selection/undo.
        unsafe {
            InvalidateRect(parent, ptr::null(), 0);
        }
        return true;
    }
    if (BUTTON_BASE..BUTTON_BASE + BUTTON_COUNT).contains(&id) {
        if code == 0 && buttons[id - BUTTON_BASE] as LPARAM == lparam {
            finish_button(parent, id - BUTTON_BASE);
        }
        return true;
    }
    false
}

fn button_center(parent: HWND, index: usize) -> Option<(i32, i32)> {
    snapshot(parent)?
        .buttons
        .into_iter()
        .find(|b| b.index == index && b.enabled)
        .map(|b| {
            (
                (b.rect.left + b.rect.right) / 2,
                (b.rect.top + b.rect.bottom) / 2,
            )
        })
}
fn begin_button(parent: HWND, index: usize) {
    let point = button_center(parent, index);
    let pointer = window::get_app_state(parent);
    if let Some((x, y)) = point {
        if !pointer.is_null() {
            // SAFETY: UI-thread-only, no native calls inside the hit-test.
            unsafe {
                if !(*pointer).modal_active {
                    (*pointer).begin_pointer_press(x, y, false);
                }
            }
        }
    }
}
fn finish_button(parent: HWND, index: usize) {
    let point = button_center(parent, index);
    let pointer = window::get_app_state(parent);
    if let Some((x, y)) = point {
        if !pointer.is_null() {
            // SAFETY: no native calls until the owned core action has completed.
            unsafe {
                if !(*pointer).modal_active {
                    (*pointer).finish_pointer_press(x, y);
                }
            }
        }
    }
    window::run_requested_effect(parent);
    unsafe {
        InvalidateRect(parent, ptr::null(), 0);
    }
}

// SAFETY: installed only on our UI-thread-owned children. Native callbacks use
// owned/copied state, and no AppState/registry borrow crosses DefSubclassProc.
unsafe extern "system" fn child_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    let parent = GetParent(hwnd);
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(child_proc), id);
        return DefSubclassProc(hwnd, msg, wparam, lparam);
    }
    let editor = id == COMPOSER || id == RENAME;
    let button = (BUTTON_BASE..BUTTON_BASE + BUTTON_COUNT).contains(&id);
    let pointer = window::get_app_state(parent);
    if !pointer.is_null()
        && (*pointer).modal_active
        && matches!(
            msg,
            WM_KEYDOWN | WM_CHAR | WM_UNICHAR | WM_LBUTTONDOWN | WM_LBUTTONUP | BM_CLICK
        )
    {
        return 0;
    }
    if msg == WM_IME_STARTCOMPOSITION || msg == WM_IME_ENDCOMPOSITION {
        COMPOSING.with(|c| c.set(msg == WM_IME_STARTCOMPOSITION));
    }
    if msg == WM_SETFOCUS && !pointer.is_null() {
        (*pointer).window_focused = true;
        if id == SCROLL {
            (*pointer).focused_element = Some(FocusedElement::SettingsScrollbar);
        }
        if editor {
            if id == COMPOSER && !SYNCING.with(Cell::get) {
                (*pointer).cancel_rename();
            }
            (*pointer).focused_element = Some(if id == RENAME {
                FocusedElement::Rename
            } else {
                FocusedElement::Input
            });
        } else if button {
            if let Some(b) = snapshot(parent)
                .and_then(|s| s.buttons.into_iter().find(|b| b.index == id - BUTTON_BASE))
            {
                (*pointer).focused_element = Some(b.focus);
            }
        }
    }
    if editor && matches!(msg, WM_KILLFOCUS | WM_KEYUP | WM_LBUTTONUP) {
        checkpoint_selection(parent);
    }
    if msg == WM_KILLFOCUS && !pointer.is_null() {
        (*pointer).window_focused =
            wparam as HWND == parent || IsChild(parent, wparam as HWND) != 0;
        if editor {
            COMPOSING.with(|c| c.set(false));
        }
        if id == RENAME && (wparam as HWND == parent || IsChild(parent, wparam as HWND) != 0) {
            (*pointer).cancel_rename();
            (*pointer).repaint_needed = true;
        }
    }
    if matches!(msg, WM_SETFOCUS | WM_KILLFOCUS) && !pointer.is_null() {
        (*pointer).repaint_needed = true;
        InvalidateRect(parent, ptr::null(), 0);
    }
    if msg == WM_UNICHAR && editor {
        if wparam == 0xffff {
            return 1;
        }
        if let Some(c) = u32::try_from(wparam).ok().and_then(char::from_u32) {
            let mut buffer = [0u16; 2];
            for unit in c.encode_utf16(&mut buffer) {
                SendMessageW(hwnd, WM_CHAR, *unit as WPARAM, 0);
            }
        }
        return 1;
    }
    if msg == WM_KEYDOWN {
        let key = wparam as u8;
        let ctrl = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
        let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
        let composing = COMPOSING.with(Cell::get);
        if button
            && matches!(id, 1102 | 1103)
            && matches!(key, VK_LEFT | VK_RIGHT)
            && !ctrl
            && !shift
        {
            let next = if id == 1102 { 3 } else { 2 };
            if let Some((_, _, buttons, _, _)) = handles() {
                SetFocus(buttons[next]);
                begin_button(parent, next);
                finish_button(parent, next);
                sync(parent);
            }
            return 0;
        }
        if key == VK_RETURN && composing {
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }
        if key == VK_RETURN && editor && !shift && lparam & (1 << 30) != 0 {
            CONSUMED_RETURN.with(|c| c.set(true));
            return 0;
        }
        if key == VK_RETURN && editor && id == COMPOSER && shift && !composing {
            CONSUMED_RETURN.with(|c| c.set(true));
            SendMessageW(hwnd, 0xc2, 1, to_wide("\r").as_ptr() as LPARAM); // EM_REPLACESEL, undoable newline
            return 0;
        }
        if key == VK_RETURN && button {
            CONSUMED_RETURN.with(|c| c.set(true));
            if lparam & (1 << 30) == 0 {
                begin_button(parent, id - BUTTON_BASE);
                finish_button(parent, id - BUTTON_BASE);
            }
            sync(parent);
            return 0;
        }
        if key == VK_TAB
            || key == VK_ESCAPE
            || key == VK_F5
            || ctrl && (key == VK_N || shift && matches!(key, b'A' | b'D'))
            || editor && key == VK_RETURN && (id == RENAME || !shift)
        {
            if key == VK_RETURN {
                CONSUMED_RETURN.with(|c| c.set(true));
            }
            if ctrl && matches!(key, VK_N | b'A' | b'D') {
                CONSUMED_CHAR.with(|c| c.set(Some(u32::from(key & 0x1f))));
            }
            SendMessageW(parent, WM_KEYDOWN, wparam, lparam);
            sync(parent);
            return 0;
        }
    }
    if msg == WM_CHAR && CONSUMED_CHAR.with(|c| c.take()) == Some(wparam as u32) {
        return 0;
    }
    if msg == WM_CHAR
        && (wparam == 9
            || wparam == 27
            || (wparam == 13 || wparam == 10) && CONSUMED_RETURN.with(|c| c.replace(false)))
    {
        return 0;
    }
    if button && msg == BM_CLICK {
        begin_button(parent, id - BUTTON_BASE);
        finish_button(parent, id - BUTTON_BASE);
        sync(parent);
        return 0;
    }
    if button && msg == WM_LBUTTONDOWN {
        // A drawer/page can reveal a different HWND under the same point after
        // release. Windows won't call that a double-click; don't retarget it.
        let mut point = POINT { x: 0, y: 0 };
        let dpi = GetDpiForWindow(hwnd);
        let interval = GetDoubleClickTime();
        let dx = GetSystemMetricsForDpi(36, dpi).max(1);
        let dy = GetSystemMetricsForDpi(37, dpi).max(1);
        if GetCursorPos(&mut point) != 0
            && RECENT_RELEASE.with(|last| {
                last.borrow().as_ref().is_some_and(|last| {
                    last.hwnd != hwnd
                        && last.time.elapsed().as_millis() <= u128::from(interval)
                        && last.point.x.abs_diff(point.x) <= dx as u32 / 2
                        && last.point.y.abs_diff(point.y) <= dy as u32 / 2
                })
            })
        {
            return 0;
        }
    }
    if button && msg == WM_LBUTTONDBLCLK {
        return 0;
    }
    if button && msg == WM_LBUTTONUP && !pointer.is_null() && (*pointer).pointer_press.is_some() {
        let mut point = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut point) != 0 {
            RECENT_RELEASE.with(|last| {
                *last.borrow_mut() = Some(PointerRelease {
                    hwnd,
                    point,
                    time: std::time::Instant::now(),
                })
            });
        }
    }
    if button && msg == WM_MOUSEMOVE && !pointer.is_null() {
        let mut p = POINT {
            x: (lparam & 0xffff) as i16 as i32,
            y: ((lparam >> 16) & 0xffff) as i16 as i32,
        };
        ClientToScreen(hwnd, &mut p);
        ScreenToClient(parent, &mut p);
        (*pointer).update_pointer_press(p.x, p.y);
    }
    let releasing = button && (msg == WM_LBUTTONUP || msg == WM_KEYUP && wparam as u8 == VK_SPACE);
    if releasing {
        RELEASING_BUTTON.with(|b| b.set(hwnd));
    }
    let result = DefSubclassProc(hwnd, msg, wparam, lparam);
    if releasing {
        RELEASING_BUTTON.with(|b| b.set(ptr::null_mut()));
        let pointer = window::get_app_state(parent);
        if !pointer.is_null() {
            (*pointer).cancel_pointer_press();
        }
    }
    let pointer = window::get_app_state(parent);
    if button
        && (msg == WM_LBUTTONDOWN
            || msg == WM_KEYDOWN && wparam as u8 == VK_SPACE && lparam & (1 << 30) == 0)
    {
        begin_button(parent, id - BUTTON_BASE);
    }
    if button
        && matches!(msg, WM_CAPTURECHANGED | WM_CANCELMODE | WM_KILLFOCUS)
        && !pointer.is_null()
        && !RELEASING_BUTTON.with(|b| b.get() == hwnd)
    {
        (*pointer).cancel_pointer_press();
    }
    result
}

pub fn scroll(parent: HWND, wparam: WPARAM, lparam: LPARAM) -> bool {
    let Some((_, _, _, scrollbar, _)) = handles() else {
        return false;
    };
    if scrollbar as LPARAM != lparam {
        return false;
    }
    let mut info = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: 0x10,
        nMin: 0,
        nMax: 0,
        nPage: 0,
        nPos: 0,
        nTrackPos: 0,
    };
    // SAFETY: owned local query structure; borrow AppState only afterwards.
    unsafe {
        if GetScrollInfo(scrollbar, 2, &mut info) == 0 {
            return true;
        }
    }
    let pointer = window::get_app_state(parent);
    if !pointer.is_null() {
        let state = unsafe { &mut *pointer };
        let scale = state.dpi as f32 / 96.0;
        let delta = match (wparam & 0xffff) as i32 {
            0 => -(32.0 * scale) as i32,
            1 => (32.0 * scale) as i32,
            2 => -state.client_height / 2,
            3 => state.client_height / 2,
            4 | 5 => info.nTrackPos - state.settings_scroll,
            6 => -state.settings_scroll,
            7 => state.settings_max_scroll(),
            _ => 0,
        };
        state.scroll_settings(delta);
    }
    sync(parent);
    unsafe {
        InvalidateRect(parent, ptr::null(), 0);
    }
    true
}

pub fn checkpoint_selection(parent: HWND) {
    let Some((composer, _, _, _, _)) = handles() else {
        return;
    };
    let binding = NATIVE.with(|n| {
        n.borrow()
            .as_ref()
            .and_then(|n| n.fields[0].as_ref().map(|f| f.0.clone()))
    });
    let mut range = CHARRANGE { cpMin: 0, cpMax: 0 };
    // SAFETY: query native selection first, then borrow the matching core draft.
    unsafe {
        SendMessageW(composer, EM_EXGETSEL, 0, &mut range as *mut _ as LPARAM);
    }
    let pointer = window::get_app_state(parent);
    if pointer.is_null() {
        return;
    }
    // SAFETY: UI-thread state, with no native calls while it is borrowed.
    let state = unsafe { &mut *pointer };
    if let Some(session) = state.sessions.get_mut(state.active_session) {
        if binding.as_deref() == Some(&session.local_id) {
            session.chat.cursor_pos = byte_offset(&session.chat.input_buffer, range.cpMax);
            session.chat.selection_start = (range.cpMin != range.cpMax)
                .then(|| byte_offset(&session.chat.input_buffer, range.cpMin));
        }
    }
}

pub fn draw_button(lparam: LPARAM) -> bool {
    if lparam == 0 {
        return false;
    }
    // SAFETY: WM_DRAWITEM owns this structure/DC for the dispatch. Validate the
    // native child/id before reading its name; retain the font across drawing.
    unsafe {
        let item = &*(lparam as *const DRAWITEMSTRUCT);
        let index = item.CtlID as usize;
        if item.CtlType != 4 || !(BUTTON_BASE..BUTTON_BASE + BUTTON_COUNT).contains(&index) {
            return false;
        }
        let Some((_, _, buttons, _, _)) = handles() else {
            return false;
        };
        if buttons[index - BUTTON_BASE] != item.hwndItem || item.hDC.is_null() {
            return false;
        }
        let font = NATIVE.with(|n| n.borrow().as_ref().and_then(|n| n.font.clone()));
        let mut name = [0u16; 96];
        let length =
            GetWindowTextW(item.hwndItem, name.as_mut_ptr(), name.len() as i32).max(0) as usize;
        let stop =
            name[..length].starts_with(&[b'S' as u16, b't' as u16, b'o' as u16, b'p' as u16]);
        let primary = index == BUTTON_BASE + 4 || index == BUTTON_BASE + 5;
        let disabled = item.itemState & 4 != 0;
        let pressed = item.itemState & 1 != 0;
        let background = if disabled {
            SEND_BTN_DISABLED
        } else if stop {
            if pressed {
                STOP_BTN_PRESSED
            } else {
                STOP_BTN_BG
            }
        } else if primary {
            if pressed {
                NEW_CHAT_BTN_PRESSED
            } else {
                NEW_CHAT_BTN_BG
            }
        } else if pressed {
            SURFACE_4
        } else {
            SURFACE_2
        };
        let brush = CreateSolidBrush(background);
        if !brush.is_null() {
            FillRect(item.hDC, &item.rcItem, brush);
            DeleteObject(brush);
        }
        let old_font = font
            .as_ref()
            .and_then(|f| f.downcast_ref::<Fonts>())
            .map(|f| SelectObject(item.hDC, f.normal));
        SetBkMode(item.hDC, TRANSPARENT);
        SetTextColor(
            item.hDC,
            if disabled {
                TEXT_SECONDARY
            } else if stop {
                STOP_BTN_ICON
            } else if primary {
                TEXT_ON_ACCENT
            } else {
                TEXT_PRIMARY
            },
        );
        let mut rect = item.rcItem;
        let short = to_wide(if index == BUTTON_BASE + 7 {
            "X"
        } else if stop {
            "Stop"
        } else {
            "Send"
        });
        let label = if index == BUTTON_BASE + 7 || index == BUTTON_BASE + 4 {
            &short[..short.len() - 1]
        } else {
            &name[..length]
        };
        DrawTextW(
            item.hDC,
            label.as_ptr(),
            label.len() as i32,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        if item.itemState & 0x10 != 0 && item.itemState & 0x200 == 0 {
            let inset = (2.0 * GetDpiForWindow(item.hwndItem) as f32 / 96.0).max(1.0) as i32;
            rect.left += inset;
            rect.top += inset;
            rect.right -= inset;
            rect.bottom -= inset;
            let border = CreateSolidBrush(FOCUS_RING);
            if !border.is_null() {
                FrameRect(item.hDC, &rect, border);
                DeleteObject(border);
            }
        }
        if let Some(previous) = old_font {
            SelectObject(item.hDC, previous);
        }
    }
    true
}

pub fn control_color(dc: HDC) -> Option<HBRUSH> {
    let brush = NATIVE.with(|n| n.borrow().as_ref().map(|n| n.brush))?;
    // SAFETY: the parent supplied the live control paint DC; the retained brush
    // remains alive until every child has been destroyed.
    unsafe {
        SetTextColor(dc, TEXT_PRIMARY);
        SetBkColor(dc, SESSION_HEADER_BG);
    }
    (!brush.is_null()).then_some(brush)
}

pub fn destroy() {
    let native = NATIVE.with(|n| n.borrow_mut().take());
    if let Some(native) = native {
        // SAFETY: explicitly destroy all children before retiring their native
        // resources. No registry borrow survives a destruction callback.
        unsafe {
            let mut released = true;
            for hwnd in [
                native.composer,
                native.rename,
                native.scrollbar,
                native.status,
            ]
            .into_iter()
            .chain(native.buttons)
            {
                if !hwnd.is_null() && IsWindow(hwnd) != 0 {
                    accessibility::clear(hwnd);
                    released &= DestroyWindow(hwnd) != 0;
                }
            }
            if !released {
                // A still-live child may reference its class/font/brush. Retain
                // these resources rather than unloading code from under it.
                std::mem::forget(native);
                return;
            }
            // Destruction can occur inside a suspended native control callback.
            // Keep its borrowed brush/fonts alive until outer dispatch unwinds.
            RETIRED.with(|retired| retired.borrow_mut().push(native));
        }
    }
}

/// Called by the entry point after the message loop and final DestroyWindow,
/// never from WndProc: all suspended control callbacks have now returned.
pub fn finish_resources() {
    let retired = RETIRED.with(|retired| std::mem::take(&mut *retired.borrow_mut()));
    for native in retired {
        // SAFETY: all children are gone and their paint callbacks have unwound.
        unsafe {
            if !native.brush.is_null() {
                DeleteObject(native.brush);
            }
        }
        // The single successful Msftedit load intentionally remains until process
        // exit; external accessibility providers may still hold native objects.
        // Dropping the retained font owner here no longer invalidates a control.
    }
}

fn native_text(text: &str) -> String {
    // Older drafts may contain CRLF. RichEdit uses one CR per paragraph;
    // replacing LF alone would turn each old CRLF into two paragraphs.
    text.replace("\r\n", "\n").replace('\n', "\r")
}

fn utf16_offset(text: &str, byte: usize) -> i32 {
    text.char_indices()
        .take_while(|(i, _)| *i < byte)
        .map(|(_, c)| c.len_utf16() as i32)
        .sum()
}
fn byte_offset(text: &str, unit: i32) -> usize {
    let mut units = 0;
    for (byte, c) in text.char_indices() {
        if units + c.len_utf16() as i32 > unit.max(0) {
            return byte;
        }
        units += c.len_utf16() as i32;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_editor_snapshots_cannot_replace_another_chat_or_newer_text() {
        let mut state = crate::app::AppState::new(500);
        state.native_editor_active = true;
        state
            .sessions
            .push(crate::session::Session::new("", "First", "."));
        state
            .sessions
            .push(crate::session::Session::new("", "Second", "."));
        state.sessions[0].chat.insert_str("Draft");
        state.sessions[1].chat.insert_str("Draft");
        let field = Field {
            key: state.sessions[0].local_id.clone(),
            text: "Draft".into(),
            cursor: 5,
            anchor: None,
            rect: Rect::default(),
            visible: true,
        };
        assert!(field_matches(&state, 0, &field));
        state.active_session = 1;
        assert!(!field_matches(&state, 0, &field));
        state.active_session = 0;
        state.sessions[0].chat.insert_char('!');
        assert!(!field_matches(&state, 0, &field));
        state.start_rename(0);
        let rename = Field {
            key: format!("chat:{}", state.sessions[0].local_id),
            text: state.rename_buffer.clone(),
            ..field
        };
        assert!(field_matches(&state, 1, &rename));
        state.cancel_rename();
        assert!(!field_matches(&state, 1, &rename));
    }

    #[test]
    fn legacy_crlf_drafts_do_not_gain_extra_paragraphs() {
        let native = native_text("first\r\nsecond\nthird\rfourth😀");
        assert_eq!(native, "first\rsecond\rthird\rfourth😀");
        assert_eq!(native.replace('\r', "\n"), "first\nsecond\nthird\nfourth😀");
    }

    #[test]
    fn editor_offsets_are_scalar_safe_across_utf16_and_utf8() {
        let text = "a\n😀e\u{301}漢";
        for (byte, _) in text
            .char_indices()
            .chain(std::iter::once((text.len(), ' ')))
        {
            assert_eq!(byte_offset(text, utf16_offset(text, byte)), byte);
        }
        assert_eq!(byte_offset(text, 3), 2); // inside surrogate pair snaps before emoji
    }
}
