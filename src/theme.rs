// Theme: colors, spacing tokens, typography scale, layout constants, and Settings.
// All pixel values are in logical pixels (scaled by DPI at render time).
// This module is fully platform-independent - colors are stored as 0x00BBGGRR (COLORREF).
#![allow(dead_code)]

/// Pack RGB into a COLORREF-style u32 (0x00BBGGRR), matching Win32 convention.
/// Platform backends convert this to their native color format as needed.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | (r as u32)
}

// ── Spacing token scale (8pt base grid, logical px) ──────────
pub const SPACE_0: i32 = 0;
pub const SPACE_1: i32 = 4; // 0.5x - hairline gaps
pub const SPACE_2: i32 = 8; // 1x - tight (item gaps, inner padding)
pub const SPACE_3: i32 = 12; // 1.5x - compact
pub const SPACE_4: i32 = 16; // 2x - default (gutters, section gaps)
pub const SPACE_5: i32 = 24; // 3x - loose (panel padding, section spacing)
pub const SPACE_6: i32 = 32; // 4x - extra loose (title areas)
pub const SPACE_8: i32 = 48; // 6x - footer/header heights

// ── Corner radius scale ──────────────────────────────────────
pub const RADIUS_SM: i32 = 4; // Small elements (tags, dots)
pub const RADIUS_MD: i32 = 8; // Bubbles, input box, session items
pub const RADIUS_LG: i32 = 12; // Panels, modals

// ── Layout constants (logical pixels, scaled by DPI at render time) ──
pub const SIDEBAR_WIDTH: i32 = 280;
pub const SIDEBAR_HEADER_HEIGHT: i32 = 56;
pub const SIDEBAR_ITEM_HEIGHT: i32 = 56;
pub const SIDEBAR_ITEM_MARGIN: i32 = SPACE_1;
pub const SIDEBAR_FOOTER_HEIGHT: i32 = 48;
pub const SIDEBAR_PADDING: i32 = SPACE_2;
pub const SIDEBAR_GROUP_HEIGHT: i32 = 28;
pub const SIDEBAR_ACCENT_BAR_W: i32 = 3;

pub const SETTINGS_PANEL_WIDTH: i32 = 296;

pub const INPUT_HEIGHT: i32 = 44;
pub const INPUT_MARGIN: i32 = SPACE_4;
pub const INPUT_MIN_HEIGHT: i32 = 48;
pub const SEND_BTN_SIZE: i32 = 36;
pub const SEND_BTN_MARGIN: i32 = SPACE_2;
pub const HINT_ROW_HEIGHT: i32 = 24;
pub const SESSION_HEADER_HEIGHT: i32 = 48;

pub const MESSAGE_MAX_WIDTH_RATIO: f32 = 0.75;

pub const INPUT_MAX_LINES: i32 = 8;
pub const INPUT_LINE_PADDING: i32 = SPACE_2;
pub const THINKING_HEADER_HEIGHT: i32 = 28;

pub const MESSAGE_SPACING: i32 = 8;
pub const MESSAGE_PADDING: i32 = 16;
pub const MESSAGE_INNER_PADDING: i32 = 8;

// ── Typographic scale (logical px @ 96 DPI) ──────────────────
// Clean sans-serif (Segoe UI),
// 1.55 body line-height for editorial readability.
pub const TEXT_CAPTION_SIZE: f32 = 11.0;
pub const TEXT_BODY_SIZE: f32 = 14.0;
pub const TEXT_UI_SIZE: f32 = 13.0;
pub const TEXT_HEADING_SIZE: f32 = 16.0;
pub const TEXT_CODE_SIZE: f32 = 13.0;
pub const TEXT_TIMESTAMP_SIZE: f32 = 10.0;
pub const TEXT_SESSION_NAME_SIZE: f32 = 14.0;
pub const TEXT_SESSION_PREVIEW_SIZE: f32 = 11.0;
pub const TEXT_HINT_SIZE: f32 = 11.0;
pub const TEXT_GROUP_HEADER_SIZE: f32 = 11.0;

pub const LINE_HEIGHT_RATIO: f32 = 1.55;

// ── Color system (warm charcoal + orange) ───
//
// Warm charcoal surfaces with a vivid orange accent.
// The signature gradient runs yellow → amber → orange → red-orange → red:
//   #FFD800 → #FFAF00 → #FF8205 → #FA500F → #E10500
//
// Surface elevation scale (warm charcoal, not pure black):
//   SURFACE_0  app background (deepest warm charcoal)
//   SURFACE_1  sidebar, panels, headers
//   SURFACE_2  raised elements (input, cards, agent bubble)
//   SURFACE_3  hover states
//   SURFACE_4  active / pressed states
//
// Accent scale (orange gradient):
//   ACCENT          #FF8205 - primary brand orange
//   ACCENT_HOVER    lighter orange for hover/focus
//   ACCENT_PRESSED  #FA500F - red-orange for pressed
//   ACCENT_DIM      dimmed orange (streaming dot off, etc.)
//   ACCENT_DEEP     #FA500F - user bubble (vivid red-orange)
//
// Text scale:
//   TEXT_PRIMARY    #FFFAEB - warm cream (off-white)
//   TEXT_SECONDARY  warm gray for labels, metadata
//   TEXT_TERTIARY   muted warm gray for hints, timestamps
//   TEXT_EMPHASIS   brighter cream for bold text
//   TEXT_ON_ACCENT  dark text on orange buttons
//   TEXT_DISABLED   disabled text

// ── Gradient tokens (sunset stripe) ──────────────────
pub const GRADIENT_YELLOW: u32 = rgb(255, 216, 0); // #FFD800
pub const GRADIENT_AMBER: u32 = rgb(255, 175, 0); // #FFAF00
pub const GRADIENT_ORANGE: u32 = rgb(255, 130, 5); // #FF8205
pub const GRADIENT_RED_ORANGE: u32 = rgb(250, 80, 15); // #FA500F
pub const GRADIENT_RED: u32 = rgb(225, 5, 0); // #E10500
pub const CREAM: u32 = rgb(255, 250, 235); // #FFFAEB

// ── Core surface tokens (warm charcoal) ──────────────────────
pub const SURFACE_0: u32 = rgb(26, 24, 22); // #1A1816 - app background
pub const SURFACE_1: u32 = rgb(32, 30, 27); // #201E1B - panels, headers
pub const SURFACE_2: u32 = rgb(40, 37, 34); // #282522 - raised elements
pub const SURFACE_3: u32 = rgb(50, 46, 42); // #322E2A - hover
pub const SURFACE_4: u32 = rgb(60, 55, 50); // #3C3732 - pressed

// ── Core accent tokens (orange) ──────────────────────
pub const ACCENT: u32 = GRADIENT_ORANGE; // #FF8205
pub const ACCENT_HOVER: u32 = rgb(255, 155, 40); // lighter orange
pub const ACCENT_PRESSED: u32 = GRADIENT_RED_ORANGE; // #FA500F
pub const ACCENT_DIM: u32 = rgb(180, 90, 30); // dimmed orange
pub const ACCENT_DEEP: u32 = GRADIENT_RED_ORANGE; // #FA500F - user bubble

// ── Core text tokens (warm cream + grays) ────────────────────
pub const TEXT_PRIMARY: u32 = CREAM; // #FFFAEB
pub const TEXT_SECONDARY: u32 = rgb(180, 172, 160); // warm gray
pub const TEXT_TERTIARY: u32 = rgb(140, 130, 118); // muted warm gray
pub const TEXT_EMPHASIS: u32 = rgb(255, 252, 245); // brighter cream
pub const TEXT_ON_ACCENT: u32 = rgb(26, 22, 19); // dark on orange
pub const TEXT_DISABLED: u32 = rgb(100, 92, 84); // disabled

// ── Core border tokens ───────────────────────────────────────
pub const BORDER_SUBTLE: u32 = rgb(48, 44, 40); // hairline dividers
pub const BORDER_DEFAULT: u32 = rgb(60, 55, 50); // default borders

// ── Semantic colors ──────────────────────────────────────────
pub const SUCCESS: u32 = rgb(100, 200, 100);
pub const ERROR_COLOR: u32 = rgb(220, 80, 70);
pub const WARNING_COLOR: u32 = rgb(255, 180, 60);
pub const INFO_COLOR: u32 = rgb(140, 160, 180);

// ── App background ───────────────────────────────────────────
pub const BG_COLOR: u32 = SURFACE_0;

// ── Sidebar ──────────────────────────────────────────────────
pub const SIDEBAR_BG: u32 = rgb(22, 20, 18); // #161412 - darker than SURFACE_0
pub const SIDEBAR_TEXT: u32 = TEXT_SECONDARY;
pub const SIDEBAR_DIVIDER: u32 = BORDER_SUBTLE;
pub const SIDEBAR_ITEM_HOVER_BG: u32 = SURFACE_3;
pub const SIDEBAR_ITEM_ACTIVE_BG: u32 = rgb(52, 40, 24); // warm orange-tinted active
pub const SIDEBAR_ITEM_ACTIVE_BAR: u32 = ACCENT;
pub const SIDEBAR_ITEM_ACTIVE_TEXT: u32 = TEXT_PRIMARY;
pub const SIDEBAR_PREVIEW_TEXT: u32 = TEXT_TERTIARY;
pub const SIDEBAR_TIMESTAMP: u32 = rgb(130, 120, 108);
pub const SIDEBAR_GROUP_TEXT: u32 = TEXT_TERTIARY;
pub const SIDEBAR_GROUP_DIVIDER: u32 = BORDER_SUBTLE;

// ── Session header bar ───────────────────────────────────────
pub const SESSION_HEADER_BG: u32 = SURFACE_1;
pub const SESSION_HEADER_BORDER: u32 = BORDER_SUBTLE;
pub const SESSION_NAME_TEXT: u32 = TEXT_PRIMARY;

// ── Primary buttons (New Chat, Reconnect) ────────────────────
pub const NEW_CHAT_BTN_BG: u32 = ACCENT;
pub const NEW_CHAT_BTN_HOVER: u32 = ACCENT_HOVER;
pub const NEW_CHAT_BTN_PRESSED: u32 = ACCENT_PRESSED;
pub const NEW_CHAT_BTN_TEXT: u32 = TEXT_ON_ACCENT;

// ── Send button ──────────────────────────────────────────────
pub const SEND_BTN_BG: u32 = ACCENT;
pub const SEND_BTN_HOVER: u32 = ACCENT_HOVER;
pub const SEND_BTN_PRESSED: u32 = ACCENT_PRESSED;
pub const SEND_BTN_DISABLED: u32 = SURFACE_4;
pub const SEND_BTN_ICON: u32 = TEXT_ON_ACCENT;

// ── Stop button (shown when agent is streaming) ──
pub const STOP_BTN_BG: u32 = ERROR_COLOR;
pub const STOP_BTN_HOVER: u32 = rgb(240, 100, 90);
pub const STOP_BTN_PRESSED: u32 = rgb(180, 60, 50);
pub const STOP_BTN_ICON: u32 = CREAM;

// ── Settings button (sidebar footer) ─────────────────────────
pub const SETTINGS_BTN_BG: u32 = SURFACE_3;
pub const SETTINGS_BTN_HOVER: u32 = SURFACE_4;
pub const SETTINGS_BTN_PRESSED: u32 = rgb(54, 49, 44);
pub const SETTINGS_BTN_TEXT: u32 = TEXT_SECONDARY;

// ── Settings panel ───────────────────────────────────────────
pub const SETTINGS_PANEL_BG: u32 = SURFACE_1;
pub const SETTINGS_BORDER: u32 = BORDER_DEFAULT;
pub const SETTINGS_TEXT: u32 = TEXT_PRIMARY;
pub const SETTINGS_LABEL: u32 = TEXT_SECONDARY;
pub const SETTINGS_ACCENT: u32 = ACCENT;
pub const SETTINGS_CLOSE_BG: u32 = SURFACE_3;
pub const SETTINGS_CLOSE_HOVER: u32 = SURFACE_4;
pub const SETTINGS_CLOSE_PRESSED: u32 = rgb(75, 68, 60);

// ── Toggle switch knob ───────────────────────────────────────
pub const KNOB_ON: u32 = TEXT_PRIMARY;
pub const KNOB_OFF: u32 = TEXT_SECONDARY;

// ── Input box ────────────────────────────────────────────────
pub const INPUT_BG_COLOR: u32 = SURFACE_2;
pub const INPUT_BORDER_COLOR: u32 = BORDER_DEFAULT;
pub const INPUT_FOCUS_BORDER: u32 = ACCENT_HOVER;
pub const INPUT_TEXT_COLOR: u32 = TEXT_PRIMARY;
pub const INPUT_PLACEHOLDER: u32 = TEXT_TERTIARY;
pub const INPUT_HINT_TEXT: u32 = TEXT_TERTIARY;
pub const BORDER_DISABLED: u32 = rgb(52, 48, 43);

// ── Message bubbles ──────────────────────────────────────────
pub const USER_BUBBLE_BG: u32 = ACCENT_DEEP; // #FA500F - vivid red-orange
pub const USER_BUBBLE_TEXT: u32 = CREAM; // cream on orange
pub const AGENT_BUBBLE_BG: u32 = rgb(42, 39, 35); // warm surface 2 variant
pub const LABEL_USER: u32 = rgb(255, 175, 100); // warm peachy label on user bubble
pub const LABEL_AGENT: u32 = TEXT_SECONDARY;
pub const SYSTEM_TEXT_COLOR: u32 = TEXT_SECONDARY;

// ── Thinking section ─────────────────────────────────────────
pub const THINKING_TEXT_COLOR: u32 = TEXT_TERTIARY;
pub const THINKING_BG_COLOR: u32 = rgb(34, 31, 28); // inset surface

// ── State colors ─────────────────────────────────────────────
pub const FOCUS_RING: u32 = ACCENT_HOVER;
pub const CURSOR_COLOR: u32 = ACCENT_HOVER;
pub const DISABLED_TEXT: u32 = TEXT_DISABLED;
pub const DISABLED_BG: u32 = rgb(34, 31, 28);
pub const PRESSED_SURFACE: u32 = SURFACE_4;

// ── Selection highlight (on-brand warm amber) ────────────────
pub const SELECTION_COLOR: u32 = rgb(140, 80, 30);

// ── Scrollbar ────────────────────────────────────────────────
pub const SCROLLBAR_TRACK: u32 = SURFACE_1;
pub const SCROLLBAR_THUMB: u32 = rgb(70, 64, 58);
pub const SCROLLBAR_THUMB_HOVER: u32 = rgb(90, 82, 74);

// ── Empty state ──────────────────────────────────────────────
pub const EMPTY_STATE_TITLE: u32 = TEXT_SECONDARY;
pub const EMPTY_STATE_SUB: u32 = TEXT_TERTIARY;
pub const CARD_BG: u32 = SURFACE_2;
pub const CARD_BORDER: u32 = BORDER_DEFAULT;

// ── Markdown text colors ─────────────────────────────────────
pub const MD_TEXT: u32 = TEXT_PRIMARY;
pub const MD_CODE_BLOCK: u32 = TEXT_SECONDARY;
pub const MD_BOLD: u32 = TEXT_EMPHASIS;
pub const MD_INLINE_CODE: u32 = ACCENT_HOVER;
pub const MD_HEADING: u32 = rgb(255, 180, 100);
pub const MD_LINK: u32 = ACCENT_HOVER;
pub const MD_LINK_HOVER: u32 = rgb(255, 175, 80);

// ── Legacy alias (used in a few places, maps to ACCENT) ──────
pub const ACCENT_COLOR: u32 = ACCENT;

// ── Settings ─────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Settings {
    pub dark_theme: bool,
    pub auto_scroll: bool,
    pub auto_approve: bool,
    pub show_archived: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark_theme: true,
            auto_scroll: true,
            auto_approve: true,
            show_archived: false,
        }
    }
}

// ── Font weight constants ────────────────────────────────────
pub const FW_SEMIBOLD: i32 = 600;

// ── Project UI tokens ────────────────────────────────────────
pub const PROJECT_HEADER_HEIGHT: i32 = 32;
pub const PROJECT_ITEM_INDENT: i32 = 16;
pub const PROJECT_CHEVRON_SIZE: i32 = 12;
pub const PROJECT_MENU_BTN_SIZE: i32 = 20;

pub const PROJECT_HEADER_BG: u32 = SURFACE_1;
pub const PROJECT_HEADER_HOVER_BG: u32 = SURFACE_3;
pub const PROJECT_HEADER_TEXT: u32 = TEXT_PRIMARY;
pub const PROJECT_PATH_TEXT: u32 = TEXT_TERTIARY;
pub const PROJECT_CHEVRON_COLOR: u32 = TEXT_SECONDARY;
pub const PROJECT_COUNT_TEXT: u32 = TEXT_TERTIARY;
pub const PROJECT_MENU_BTN_COLOR: u32 = TEXT_TERTIARY;
pub const PROJECT_MENU_BTN_HOVER: u32 = TEXT_PRIMARY;

pub const NEW_PROJECT_BTN_BG: u32 = SURFACE_2;
pub const NEW_PROJECT_BTN_HOVER: u32 = SURFACE_3;
pub const NEW_PROJECT_BTN_PRESSED: u32 = SURFACE_4;
pub const NEW_PROJECT_BTN_TEXT: u32 = TEXT_SECONDARY;

pub const SESSION_INDENT: i32 = 20;
pub const WORKING_DIR_TEXT: u32 = TEXT_TERTIARY;

// ── Session action buttons (archive / delete) ────────────────
pub const SESSION_ACTION_BTN_SIZE: i32 = 20;
pub const SESSION_ACTION_BTN_GAP: i32 = 4;
pub const SESSION_ARCHIVED_TEXT: u32 = TEXT_DISABLED;
pub const SESSION_ARCHIVED_BG: u32 = rgb(28, 26, 23);
pub const SESSION_ACTION_ARCHIVE: u32 = TEXT_TERTIARY;
pub const SESSION_ACTION_ARCHIVE_HOVER: u32 = ACCENT_HOVER;
pub const SESSION_ACTION_DELETE: u32 = TEXT_TERTIARY;
pub const SESSION_ACTION_DELETE_HOVER: u32 = ERROR_COLOR;
pub const SESSION_ARCHIVED_LABEL: u32 = TEXT_DISABLED;
