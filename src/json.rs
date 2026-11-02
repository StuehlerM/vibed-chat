// Bounded strict JSON parser/serializer. Integral identifiers are lossless.
#![allow(dead_code)]

use std::fmt::{self, Write as _};

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Integer(i128),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}
impl Json {
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.as_object()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&[Self]> {
        if let Self::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }
    pub const fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => i64::try_from(*n).ok(),
            Self::Number(n)
                if n.is_finite()
                    && n.fract() == 0.0
                    && *n >= -9_223_372_036_854_775_808.0
                    && *n < 9_223_372_036_854_775_808.0 =>
            {
                Some(*n as i64)
            }
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Integer(n) => u64::try_from(*n).ok(),
            Self::Number(n)
                if n.is_finite()
                    && n.fract() == 0.0
                    && *n >= 0.0
                    && *n < 18_446_744_073_709_551_616.0 =>
            {
                Some(*n as u64)
            }
            _ => None,
        }
    }

    pub const fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_object(&self) -> Option<&[(String, Self)]> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    pub fn object(pairs: Vec<(&str, Self)>) -> Self {
        Self::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    fn write_to(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Integer(n) => {
                let _ = write!(out, "{n}");
            }
            Self::Number(n) => {
                // Public callers can construct non-finite numbers. Never emit invalid JSON.
                if n.is_finite() {
                    let _ = write!(out, "{n}");
                } else {
                    out.push_str("null");
                }
            }
            Self::String(s) => write_string(s, out),
            Self::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    v.write_to(out);
                }
                out.push(']');
            }
            Self::Object(o) => {
                out.push('{');
                for (i, (k, v)) in o.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    write_string(k, out);
                    out.push(':');
                    v.write_to(out);
                }
                out.push('}');
            }
        }
    }
}
impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();
        self.write_to(&mut output);
        f.write_str(&output)
    }
}
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\u{20}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_ITEMS: usize = 100_000;
const MAX_STRING_BYTES: usize = 4 * 1024 * 1024;

pub fn parse(input: &str) -> Result<Json, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err("JSON exceeds 16 MiB limit".to_string());
    }
    let mut parser = Parser {
        input,
        position: 0,
        items: 0,
    };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.position != input.len() {
        return Err(format!("Trailing JSON at byte {}", parser.position));
    }
    Ok(value)
}
struct Parser<'a> {
    input: &'a str,
    position: usize,
    items: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.position).copied()
    }
    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }
    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err("JSON nesting limit exceeded".to_string());
        }
        self.items += 1;
        if self.items > MAX_ITEMS {
            return Err("JSON value limit exceeded".to_string());
        }
        self.whitespace();
        match self.peek() {
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => {
                self.position += 1;
                self.whitespace();
                let mut values = Vec::new();
                if self.consume(b']') {
                    return Ok(Json::Array(values));
                }
                loop {
                    values.push(self.value(depth + 1)?);
                    self.whitespace();
                    if self.consume(b']') {
                        break;
                    }
                    if !self.consume(b',') {
                        return Err("Expected comma or closing bracket".to_string());
                    }
                }
                Ok(Json::Array(values))
            }
            Some(b'{') => {
                self.position += 1;
                self.whitespace();
                let mut pairs = Vec::new();
                let mut keys = std::collections::HashSet::new();
                if self.consume(b'}') {
                    return Ok(Json::Object(pairs));
                }
                loop {
                    self.whitespace();
                    let key = self.string()?;
                    if !keys.insert(key.clone()) {
                        return Err("Duplicate JSON object key".to_string());
                    }
                    self.whitespace();
                    if !self.consume(b':') {
                        return Err("Expected colon".to_string());
                    }
                    pairs.push((key, self.value(depth + 1)?));
                    self.whitespace();
                    if self.consume(b'}') {
                        break;
                    }
                    if !self.consume(b',') {
                        return Err("Expected comma or closing brace".to_string());
                    }
                }
                Ok(Json::Object(pairs))
            }
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("Expected JSON value at byte {}", self.position)),
        }
    }
    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if !self.input[self.position..].starts_with(word) {
            return Err("Invalid JSON literal".to_string());
        }
        self.position += word.len();
        Ok(value)
    }
    fn digits(&mut self) -> Result<(), String> {
        let start = self.position;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.position += 1;
        }
        if start == self.position {
            Err("Expected number digits".to_string())
        } else {
            Ok(())
        }
    }
    fn number(&mut self) -> Result<Json, String> {
        let start = self.position;
        self.consume(b'-');
        if self.consume(b'0') {
            if self.peek().is_some_and(|b| b.is_ascii_digit()) {
                return Err("Leading zero in JSON number".to_string());
            }
        } else {
            self.digits()?;
        }
        let mut integral = true;
        if self.consume(b'.') {
            integral = false;
            self.digits()?;
        }
        if self.consume(b'e') || self.consume(b'E') {
            integral = false;
            if !self.consume(b'+') {
                self.consume(b'-');
            }
            self.digits()?;
        }
        let number = &self.input[start..self.position];
        if number.len() > 128 {
            return Err("JSON number limit exceeded".to_string());
        }
        if integral {
            return number
                .parse::<i128>()
                .map(Json::Integer)
                .map_err(|_| "Integral JSON value out of range".to_string());
        }
        let value = number
            .parse::<f64>()
            .map_err(|_| "Invalid JSON number".to_string())?;
        if !value.is_finite() {
            return Err("Non-finite JSON number".to_string());
        }
        Ok(Json::Number(value))
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let mut value = 0;
        for _ in 0..4 {
            let byte = self.peek().ok_or("Incomplete Unicode escape")?;
            let digit = char::from(byte)
                .to_digit(16)
                .ok_or("Invalid Unicode escape")?;
            value = (value << 4) | digit;
            self.position += 1;
        }
        Ok(value)
    }
    fn string(&mut self) -> Result<String, String> {
        if !self.consume(b'"') {
            return Err("Expected JSON string".to_string());
        }
        let mut value = String::new();
        loop {
            if value.len() > MAX_STRING_BYTES {
                return Err("JSON string limit exceeded".to_string());
            }
            match self.peek().ok_or("Unterminated JSON string")? {
                b'"' => {
                    self.position += 1;
                    return Ok(value);
                }
                b'\\' => {
                    self.position += 1;
                    let escaped = self.peek().ok_or("Incomplete escape")?;
                    self.position += 1;
                    match escaped {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'/' => value.push('/'),
                        b'b' => value.push('\u{08}'),
                        b'f' => value.push('\u{0c}'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xd800..=0xdbff).contains(&code) {
                                if !self.consume(b'\\') || !self.consume(b'u') {
                                    return Err("Unpaired high surrogate".to_string());
                                }
                                let low = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err("Invalid low surrogate".to_string());
                                }
                                code = 0x10000 + ((code - 0xd800) << 10) + low - 0xdc00;
                            }
                            value.push(char::from_u32(code).ok_or("Invalid Unicode scalar")?);
                        }
                        _ => return Err("Invalid JSON escape".to_string()),
                    }
                }
                0..=0x1f => return Err("Unescaped control in JSON string".to_string()),
                _ => {
                    let c = self.input[self.position..]
                        .chars()
                        .next()
                        .ok_or("Incomplete Unicode")?;
                    value.push(c);
                    self.position += c.len_utf8();
                }
            }
        }
    }
}
