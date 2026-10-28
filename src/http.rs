// Minimal HTTP/SSE utilities for the Responses API client.
// Pure core code: no platform imports, no unsafe.
// Platform transports (WinHTTP) feed raw body bytes in; this module
// provides request-building and SSE event reassembly.
#![allow(dead_code)]

/// Build an HTTP/1.1 request head (request line + headers, no body).
/// The transport sends this followed by the body bytes.
pub fn build_request_head(
    method: &str,
    host: &str,
    path: &str,
    headers: &[(String, String)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} {} HTTP/1.1\r\n", method, path));
    out.push_str(&format!("Host: {}\r\n", host));
    out.push_str("Connection: close\r\n");
    for (k, v) in headers {
        out.push_str(&format!("{}: {}\r\n", k, v));
    }
    out.push_str("\r\n");
    out
}

/// Incremental HTTP chunked-transfer decoder.
/// Feed body bytes via `extend`; pull decoded bytes out of `out_buf`.
pub struct ChunkDecoder {
    /// None until we know the response is chunked (decoder then required).
    enabled: bool,
    /// Bytes received but not yet processed.
    pending: Vec<u8>,
    /// Decoded bytes not yet consumed by the caller.
    out_buf: Vec<u8>,
    /// Chunk size remaining in the current chunk; 0 = need a size line.
    remaining: usize,
    /// State machine: reading size line vs reading chunk data.
    in_chunk_data: bool,
    finished: bool,
}

impl ChunkDecoder {
    pub fn new() -> Self {
        Self {
            enabled: false,
            pending: Vec::new(),
            out_buf: Vec::new(),
            remaining: 0,
            in_chunk_data: false,
            finished: false,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Feed raw body bytes. Decoded bytes accumulate in the out buffer.
    pub fn extend(&mut self, data: &[u8]) {
        if !self.enabled || self.finished {
            return;
        }
        self.pending.extend_from_slice(data);
        self.pump();
    }

    /// Take all decoded bytes accumulated so far.
    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out_buf)
    }

    fn pump(&mut self) {
        loop {
            if self.finished {
                return;
            }
            if !self.in_chunk_data {
                // Find CRLF-terminated size line.
                let line_end = match find_crlf(&self.pending) {
                    Some(i) => i,
                    None => return,
                };
                let line = String::from_utf8_lossy(&self.pending[..line_end]).to_string();
                // Strip chunk extensions (after ';').
                let size_str = line.split(';').next().unwrap_or("").trim();
                let size = match usize::from_str_radix(size_str, 16) {
                    Ok(s) => s,
                    Err(_) => {
                        // Malformed framing: stop decoding, deliver what we have.
                        self.finished = true;
                        return;
                    }
                };
                // Consume size line + CRLF.
                self.pending.drain(..line_end + 2);
                if size == 0 {
                    self.finished = true;
                    return;
                }
                self.remaining = size;
                self.in_chunk_data = true;
            } else {
                if self.pending.is_empty() {
                    return;
                }
                let take = self.remaining.min(self.pending.len());
                let chunk: Vec<u8> = self.pending.drain(..take).collect();
                self.out_buf.extend_from_slice(&chunk);
                self.remaining -= take;
                if self.remaining == 0 {
                    self.in_chunk_data = false;
                    // Expect CRLF after chunk data; consume if present.
                    if self.pending.len() >= 2
                        && self.pending[0] == b'\r'
                        && self.pending[1] == b'\n'
                    {
                        self.pending.drain(..2);
                    }
                }
            }
        }
    }
}

fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}

/// Incremental Server-Sent Events parser.
/// Feed decoded body bytes via `extend`; complete events come out as
/// `(event_name, data_string)` pairs. Handles events split across
/// transport reads and multi-line `data:` fields per the SSE spec.
pub struct SseParser {
    /// Partial line bytes (no terminator yet).
    line_buf: Vec<u8>,
    /// Accumulated data lines for the event being parsed.
    data_lines: Vec<String>,
    /// Event name, if provided by an `event:` line.
    event_name: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            line_buf: Vec::new(),
            data_lines: Vec::new(),
            event_name: None,
        }
    }

    /// Feed decoded bytes; returns any completed events.
    pub fn extend(&mut self, data: &[u8]) -> Vec<(String, String)> {
        let mut events = Vec::new();
        for &b in data {
            if b == b'\n' {
                // Strip optional CR (SSE lines end with CRLF or LF).
                let mut line = std::mem::take(&mut self.line_buf);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if let Some(ev) = self.handle_line(&line) {
                    events.push(ev);
                }
            } else {
                self.line_buf.push(b);
            }
        }
        events
    }

    fn handle_line(&mut self, line: &[u8]) -> Option<(String, String)> {
        let line = String::from_utf8_lossy(line).to_string();
        if line.is_empty() {
            // Blank line dispatches the event.
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
            // Comment / keep-alive. Ignore.
            return None;
        }
        let (field, value) = match line.find(':') {
            Some(i) => {
                let v = &line[i + 1..];
                let v = v.strip_prefix(' ').unwrap_or(v);
                (&line[..i], v)
            }
            None => (line.as_str(), ""),
        };
        match field {
            "data" => {
                self.data_lines.push(value.to_string());
            }
            "event" => {
                self.event_name = Some(value.to_string());
            }
            _ => {}
        }
        None
    }
}

/// Parse an HTTP status line like "HTTP/1.1 200 OK". Returns the code.
pub fn parse_status_line(line: &str) -> Option<u16> {
    let mut parts = line.splitn(3, ' ');
    let version = parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }
    let code = parts.next()?;
    code.parse::<u16>().ok()
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
        let ev = p.extend(b"event: foo\ndata: line1\ndata: line2\n\n");
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].0, "foo");
        assert_eq!(ev[0].1, "line1\nline2");
    }

    #[test]
    fn chunked_round_trip() {
        let mut d = ChunkDecoder::new();
        d.enable();
        d.extend(b"4\r\nWiki\r\n");
        d.extend(b"5\r\npedia\r\n0\r\n\r\n");
        assert_eq!(d.take(), b"Wikipedia".to_vec());
        assert!(d.is_finished());
    }

    #[test]
    fn chunked_split_size_line() {
        let mut d = ChunkDecoder::new();
        d.enable();
        d.extend(b"4\r");
        assert!(d.take().is_empty());
        d.extend(b"\nWiki\r\n0\r\n\r\n");
        assert_eq!(d.take(), b"Wiki".to_vec());
    }

    #[test]
    fn status_line() {
        assert_eq!(parse_status_line("HTTP/1.1 401 Unauthorized"), Some(401));
        assert_eq!(parse_status_line("garbage"), None);
    }

    #[test]
    fn request_head() {
        let head = build_request_head(
            "POST",
            "api.mistral.ai",
            "/v1/responses",
            &[("Content-Type".to_string(), "application/json".to_string())],
        );
        assert!(head.starts_with("POST /v1/responses HTTP/1.1\r\n"));
        assert!(head.contains("Host: api.mistral.ai\r\n"));
        assert!(head.ends_with("Content-Type: application/json\r\n\r\n"));
    }
}
