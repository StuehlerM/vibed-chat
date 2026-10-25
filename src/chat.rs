// Chat state: messages, scroll, input buffer.
#![allow(dead_code)]

use crate::markdown::{parse as parse_markdown, StyledRun};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Role {
    User,
    Agent,
    System,
}

impl Role {
    pub fn label(&self) -> &'static str {
        match self {
            Role::User => "You",
            Role::Agent => "Agent",
            Role::System => "System",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub text: String,
    pub runs: Vec<StyledRun>,
    pub height: i32,
    pub thinking: String,
    pub thinking_runs: Vec<StyledRun>,
    pub thinking_collapsed: bool,
    pub thinking_height: i32,
}

impl Message {
    pub fn new(role: Role, text: &str) -> Self {
        let runs = if role == Role::Agent {
            parse_markdown(text)
        } else {
            vec![StyledRun {
                text: text.to_string(),
                style: Default::default(),
            }]
        };
        Self {
            role,
            text: text.to_string(),
            runs,
            height: 0,
            thinking: String::new(),
            thinking_runs: Vec::new(),
            thinking_collapsed: true,
            thinking_height: 0,
        }
    }
}

#[derive(Debug)]
pub struct ChatState {
    pub messages: Vec<Message>,
    pub scroll_offset: i32,
    pub input_buffer: String,
    pub cursor_pos: usize,
    pub selection_start: Option<usize>,
    pub input_focused: bool,
    pub total_content_height: i32,
    pub is_at_bottom: bool,
    pub max_messages: usize,
    pub streaming_message_idx: Option<usize>,
    pub is_waiting_for_agent: bool,
    pub input_height: i32,
    pub input_scroll_offset: i32,
    pub has_error: bool,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            input_buffer: String::new(),
            cursor_pos: 0,
            selection_start: None,
            input_focused: false,
            total_content_height: 0,
            is_at_bottom: true,
            max_messages: 500,
            streaming_message_idx: None,
            is_waiting_for_agent: false,
            input_height: 44,
            input_scroll_offset: 0,
            has_error: false,
        }
    }

    pub fn add_message(&mut self, role: Role, text: &str) {
        let msg = Message::new(role, text);
        self.messages.push(msg);
        self.trim_history();
        self.is_at_bottom = true;
    }

    pub fn add_system_message(&mut self, text: &str) {
        self.add_message(Role::System, text);
    }

    pub fn start_streaming(&mut self) {
        let msg = Message::new(Role::Agent, "");
        self.messages.push(msg);
        self.streaming_message_idx = Some(self.messages.len() - 1);
        self.is_waiting_for_agent = true;
        self.is_at_bottom = true;
    }

    pub fn append_stream_chunk(&mut self, text: &str) {
        if let Some(idx) = self.streaming_message_idx {
            if let Some(msg) = self.messages.get_mut(idx) {
                msg.text.push_str(text);
                msg.runs = parse_markdown(&msg.text);
            }
        }
    }

    pub fn append_thought_chunk(&mut self, text: &str) {
        if let Some(idx) = self.streaming_message_idx {
            if let Some(msg) = self.messages.get_mut(idx) {
                msg.thinking.push_str(text);
                msg.thinking_runs = parse_markdown(&msg.thinking);
            }
        }
    }

    pub fn finish_streaming(&mut self) {
        self.streaming_message_idx = None;
        self.is_waiting_for_agent = false;
        self.has_error = false;
    }

    pub fn trim_history(&mut self) {
        while self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.delete_selection();
        self.input_buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.input_scroll_offset = 0;
    }

    pub fn insert_str(&mut self, s: &str) {
        self.delete_selection();
        self.input_buffer.insert_str(self.cursor_pos, s);
        self.cursor_pos += s.len();
        self.input_scroll_offset = 0;
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while pos > 0 && !self.input_buffer.is_char_boundary(pos) {
                pos -= 1;
            }
            self.input_buffer.remove(pos);
            self.cursor_pos = pos;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_pos < self.input_buffer.len() {
            self.input_buffer.remove(self.cursor_pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.selection_start = None;
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while pos > 0 && !self.input_buffer.is_char_boundary(pos) {
                pos -= 1;
            }
            self.cursor_pos = pos;
        }
    }

    pub fn move_cursor_right(&mut self) {
        self.selection_start = None;
        if self.cursor_pos < self.input_buffer.len() {
            let mut pos = self.cursor_pos + 1;
            while pos < self.input_buffer.len() && !self.input_buffer.is_char_boundary(pos) {
                pos += 1;
            }
            self.cursor_pos = pos;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.selection_start = None;
        let buf = &self.input_buffer;
        let mut pos = self.cursor_pos;
        while pos > 0 && buf.as_bytes()[pos - 1] != b'\n' {
            let mut p = pos - 1;
            while p > 0 && !buf.is_char_boundary(p) {
                p -= 1;
            }
            pos = p;
        }
        self.cursor_pos = pos;
    }

    pub fn move_cursor_end(&mut self) {
        self.selection_start = None;
        let buf = &self.input_buffer;
        let mut pos = self.cursor_pos;
        while pos < buf.len() && buf.as_bytes()[pos] != b'\n' {
            let mut p = pos + 1;
            while p < buf.len() && !buf.is_char_boundary(p) {
                p += 1;
            }
            pos = p;
        }
        self.cursor_pos = pos;
    }

    pub fn move_cursor_up(&mut self) {
        self.selection_start = None;
        let buf = &self.input_buffer;
        let mut line_start = self.cursor_pos;
        while line_start > 0 && buf.as_bytes()[line_start - 1] != b'\n' {
            let mut p = line_start - 1;
            while p > 0 && !buf.is_char_boundary(p) {
                p -= 1;
            }
            line_start = p;
        }
        if line_start == 0 {
            self.cursor_pos = 0;
            return;
        }
        let col_bytes = self.cursor_pos - line_start;
        let prev_line_end = line_start - 1;
        let mut prev_line_start = prev_line_end;
        while prev_line_start > 0 && buf.as_bytes()[prev_line_start - 1] != b'\n' {
            let mut p = prev_line_start - 1;
            while p > 0 && !buf.is_char_boundary(p) {
                p -= 1;
            }
            prev_line_start = p;
        }
        let prev_line_len = prev_line_end - prev_line_start;
        let target_col = col_bytes.min(prev_line_len);
        self.cursor_pos = prev_line_start + target_col;
        // Snap backward to char boundary if we landed inside a multi-byte char
        while self.cursor_pos > prev_line_start && !buf.is_char_boundary(self.cursor_pos) {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_cursor_down(&mut self) {
        self.selection_start = None;
        let buf = &self.input_buffer;
        let mut line_start = self.cursor_pos;
        while line_start > 0 && buf.as_bytes()[line_start - 1] != b'\n' {
            let mut p = line_start - 1;
            while p > 0 && !buf.is_char_boundary(p) {
                p -= 1;
            }
            line_start = p;
        }
        let mut line_end = self.cursor_pos;
        while line_end < buf.len() && buf.as_bytes()[line_end] != b'\n' {
            let mut p = line_end + 1;
            while p < buf.len() && !buf.is_char_boundary(p) {
                p += 1;
            }
            line_end = p;
        }
        if line_end >= buf.len() {
            self.cursor_pos = buf.len();
            return;
        }
        let next_line_start = line_end + 1;
        let mut next_line_end = next_line_start;
        while next_line_end < buf.len() && buf.as_bytes()[next_line_end] != b'\n' {
            let mut p = next_line_end + 1;
            while p < buf.len() && !buf.is_char_boundary(p) {
                p += 1;
            }
            next_line_end = p;
        }
        let col_bytes = self.cursor_pos - line_start;
        let next_line_len = next_line_end - next_line_start;
        let target_col = col_bytes.min(next_line_len);
        self.cursor_pos = next_line_start + target_col;
        // Snap backward to char boundary if we landed inside a multi-byte char
        while self.cursor_pos > next_line_start && !buf.is_char_boundary(self.cursor_pos) {
            self.cursor_pos -= 1;
        }
    }

    pub fn cursor_line_col(&self) -> (usize, usize) {
        let buf = &self.input_buffer;
        let mut line = 0usize;
        let mut line_start = 0usize;
        for (i, &b) in buf.as_bytes().iter().enumerate() {
            if i >= self.cursor_pos {
                break;
            }
            if b == b'\n' {
                line += 1;
                line_start = i + 1;
            }
        }
        (line, self.cursor_pos - line_start)
    }

    pub fn line_start_pos(&self, byte_pos: usize) -> usize {
        let buf = &self.input_buffer;
        let mut pos = byte_pos.min(buf.len());
        while pos > 0 && buf.as_bytes()[pos - 1] != b'\n' {
            let mut p = pos - 1;
            while p > 0 && !buf.is_char_boundary(p) {
                p -= 1;
            }
            pos = p;
        }
        pos
    }

    pub fn line_end_pos(&self, byte_pos: usize) -> usize {
        let buf = &self.input_buffer;
        let mut pos = byte_pos.min(buf.len());
        while pos < buf.len() && buf.as_bytes()[pos] != b'\n' {
            let mut p = pos + 1;
            while p < buf.len() && !buf.is_char_boundary(p) {
                p += 1;
            }
            pos = p;
        }
        pos
    }

    pub fn input_line_count(&self) -> usize {
        if self.input_buffer.is_empty() {
            return 1;
        }
        self.input_buffer.bytes().filter(|&b| b == b'\n').count() + 1
    }

    pub fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_pos = self.input_buffer.len();
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.selection_start = None;
        self.input_scroll_offset = 0;
    }

    pub fn take_input(&mut self) -> String {
        let text = std::mem::take(&mut self.input_buffer);
        self.cursor_pos = 0;
        self.selection_start = None;
        self.input_scroll_offset = 0;
        text
    }

    pub fn delete_selection(&mut self) -> bool {
        if let Some(start) = self.selection_start {
            let end = self.cursor_pos;
            let (lo, hi) = if start < end { (start, end) } else { (end, start) };
            if lo < hi {
                self.input_buffer.replace_range(lo..hi, "");
                self.cursor_pos = lo;
                self.selection_start = None;
                return true;
            }
            self.selection_start = None;
        }
        false
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_start.map(|s| {
            let e = self.cursor_pos;
            if s < e { (s, e) } else { (e, s) }
        })
    }

    pub fn scroll_up(&mut self, delta: i32, viewport_height: i32) {
        self.scroll_offset -= delta;
        let max_scroll = (self.total_content_height - viewport_height).max(0);
        if self.scroll_offset > max_scroll { self.scroll_offset = max_scroll; }
        if self.scroll_offset < 0 { self.scroll_offset = 0; }
        self.is_at_bottom = self.scroll_offset >= max_scroll;
    }

    pub fn scroll_down(&mut self, delta: i32, viewport_height: i32) {
        self.scroll_offset += delta;
        let max_scroll = (self.total_content_height - viewport_height).max(0);
        if self.scroll_offset > max_scroll { self.scroll_offset = max_scroll; }
        if self.scroll_offset < 0 { self.scroll_offset = 0; }
        self.is_at_bottom = self.scroll_offset >= max_scroll;
    }

    pub fn scroll_to_bottom(&mut self, viewport_height: i32) {
        let max_scroll = (self.total_content_height - viewport_height).max(0);
        self.scroll_offset = max_scroll;
        self.is_at_bottom = true;
    }

    pub fn compute_message_height(msg: &Message, content_width: i32, line_height: i32) -> i32 {
        use crate::theme::{MESSAGE_INNER_PADDING, MESSAGE_PADDING, MESSAGE_SPACING};
        let usable_width = (content_width - 2 * MESSAGE_PADDING - 2 * MESSAGE_INNER_PADDING).max(50);
        let chars_per_line = (usable_width / 7).max(10);
        let mut total_lines = 0i32;
        for run in &msg.runs {
            for line in run.text.split('\n') {
                let char_count = line.chars().count() as i32;
                let lines_needed = if char_count == 0 { 1 } else { (char_count + chars_per_line - 1) / chars_per_line };
                total_lines += lines_needed.max(1);
            }
        }
        if total_lines == 0 { total_lines = 1; }
        total_lines * line_height + 2 * MESSAGE_INNER_PADDING + MESSAGE_SPACING
    }

    pub fn recompute_layout(&mut self, content_width: i32, line_height: i32) {
        let mut total = 0i32;
        for msg in &mut self.messages {
            msg.height = Self::compute_message_height(msg, content_width, line_height);
            total += msg.height;
        }
        self.total_content_height = total;
    }

    pub fn visible_messages(&self, viewport_height: i32, scroll_offset: i32) -> Vec<(usize, i32)> {
        let mut result = Vec::new();
        let mut y = -scroll_offset;
        for (i, msg) in self.messages.iter().enumerate() {
            if y + msg.height >= 0 && y <= viewport_height {
                result.push((i, y));
            }
            y += msg.height;
            if y > viewport_height { break; }
        }
        result
    }
}
