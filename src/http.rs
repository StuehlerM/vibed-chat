// Bounded SSE reassembly over decoded entity bytes. HTTP framing/status/header
// handling belongs to the transport; never decode WinHTTP transfer framing twice.
#![allow(dead_code)]

/// Incremental Server-Sent Events parser. UTF-8, field lines, and event boundaries
/// may span reads; LF, CRLF, and CR terminators are accepted.
pub struct SseParser {
    line_buf: Vec<u8>,
    data_lines: Vec<String>,
    event_name: Option<String>,
    event_bytes: usize,
    error: Option<String>,
    skip_lf: bool,
    first_line: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            line_buf: Vec::new(),
            data_lines: Vec::new(),
            event_name: None,
            event_bytes: 0,
            error: None,
            skip_lf: false,
            first_line: true,
        }
    }

    /// Returns complete events only. An unfinished event at EOF must not turn a
    /// premature transport close into a successful generation.
    pub fn extend(&mut self, data: &[u8]) -> Vec<(String, String)> {
        let mut events = Vec::new();
        for &byte in data {
            if self.error.is_some() {
                break;
            }
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            self.event_bytes += 1;
            if self.event_bytes > 1024 * 1024 || self.line_buf.len() >= 1024 * 1024 {
                self.error = Some("SSE frame exceeds 1 MiB".to_string());
                self.line_buf.clear();
                self.data_lines.clear();
                break;
            }
            if byte == b'\n' || byte == b'\r' {
                self.skip_lf = byte == b'\r';
                let line = std::mem::take(&mut self.line_buf);
                if let Some(event) = self.handle_line(&line) {
                    events.push(event);
                }
            } else {
                self.line_buf.push(byte);
            }
        }
        events
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn handle_line(&mut self, line: &[u8]) -> Option<(String, String)> {
        let mut line = match std::str::from_utf8(line) {
            Ok(line) => line,
            Err(_) => {
                self.error = Some("Invalid UTF-8 in SSE frame".to_string());
                return None;
            }
        };
        if self.first_line {
            self.first_line = false;
            line = line.strip_prefix('\u{FEFF}').unwrap_or(line);
        }
        if line.is_empty() {
            self.event_bytes = 0;
            if self.data_lines.is_empty() {
                self.event_name = None;
                return None;
            }
            let data = self.data_lines.join("\n");
            let name = self.event_name.take().unwrap_or_default();
            self.data_lines.clear();
            return Some((name, data));
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.find(':') {
            Some(index) => {
                let value = &line[index + 1..];
                (&line[..index], value.strip_prefix(' ').unwrap_or(value))
            }
            None => (line, ""),
        };
        match field {
            "data" => self.data_lines.push(value.to_string()),
            "event" => self.event_name = Some(value.to_string()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_simple_event() {
        let mut p = SseParser::new();
        let events = p.extend(b"data: {\"type\":\"response.output_text.delta\"}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "");
        assert_eq!(events[0].1, "{\"type\":\"response.output_text.delta\"}");
    }

    #[test]
    fn sse_split_across_reads() {
        let mut p = SseParser::new();
        let mut all = Vec::new();
        all.extend(p.extend(b"data: {\"a\":"));
        all.extend(p.extend(b"1}\n"));
        all.extend(p.extend(b"\n"));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1, "{\"a\":1}");
    }

    #[test]
    fn sse_named_event_multiline_data() {
        let mut p = SseParser::new();
        let events = p.extend(b"event: foo\ndata: line1\ndata: line2\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "foo");
        assert_eq!(events[0].1, "line1\nline2");
    }

    #[test]
    fn sse_bom_and_split_crlf() {
        let mut p = SseParser::new();
        assert!(p.extend(b"\xEF\xBB").is_empty());
        assert!(p.extend(b"\xBFdata: hello\r").is_empty());
        let events = p.extend(b"\n\r");
        assert_eq!(events, vec![(String::new(), "hello".to_string())]);
        assert!(p.extend(b"\n").is_empty());
    }

    #[test]
    fn sse_cr_only() {
        let mut p = SseParser::new();
        assert_eq!(p.extend(b"data: one\r\rdata: two\r\r").len(), 2);
    }
}
