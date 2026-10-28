// Minimal JSON parser and serializer. ~300 lines. Only what JSON-RPC needs.
#![allow(dead_code)]

use std::fmt::{self, Write as _};

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64().map(|n| n as i64)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    fn write_to(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    let _ = write!(out, "{}", *n as i64);
                } else {
                    let _ = write!(out, "{}", n);
                }
            }
            Json::String(s) => write_json_string(s, out),
            Json::Array(arr) => {
                out.push('[');
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write_to(out);
                }
                out.push(']');
            }
            Json::Object(pairs) => {
                out.push('{');
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_string(k, out);
                    out.push(':');
                    v.write_to(out);
                }
                out.push('}');
            }
        }
    }

    /// Create a JSON object from key-value pairs
    pub fn object(pairs: Vec<(&str, Json)>) -> Json {
        Json::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write_to(&mut s);
        f.write_str(&s)
    }
}

fn write_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ── Parser ───────────────────────────────────────────────────

pub fn parse(input: &str) -> Result<Json, String> {
    let bytes: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let val = parse_value(&bytes, &mut pos)?;
    skip_ws(&bytes, &mut pos);
    if pos < bytes.len() {
        return Err(format!("Trailing characters at position {}", pos));
    }
    Ok(val)
}

fn skip_ws(bytes: &[char], pos: &mut usize) {
    while *pos < bytes.len() {
        match bytes[*pos] {
            ' ' | '\t' | '\n' | '\r' => *pos += 1,
            _ => break,
        }
    }
}

fn parse_value(bytes: &[char], pos: &mut usize) -> Result<Json, String> {
    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return Err("Unexpected end of input".to_string());
    }
    match bytes[*pos] {
        '{' => parse_object(bytes, pos),
        '[' => parse_array(bytes, pos),
        '"' => Ok(Json::String(parse_string(bytes, pos)?)),
        't' => parse_literal(bytes, pos, "true", Json::Bool(true)),
        'f' => parse_literal(bytes, pos, "false", Json::Bool(false)),
        'n' => parse_literal(bytes, pos, "null", Json::Null),
        '-' | '0'..='9' => parse_number(bytes, pos),
        c => Err(format!("Unexpected character '{}' at position {}", c, pos)),
    }
}

fn parse_literal(bytes: &[char], pos: &mut usize, lit: &str, val: Json) -> Result<Json, String> {
    let lit_chars: Vec<char> = lit.chars().collect();
    for (i, c) in lit_chars.iter().enumerate() {
        if *pos + i >= bytes.len() || bytes[*pos + i] != *c {
            return Err(format!("Expected '{}' at position {}", lit, pos));
        }
    }
    *pos += lit_chars.len();
    Ok(val)
}

fn parse_number(bytes: &[char], pos: &mut usize) -> Result<Json, String> {
    let start = *pos;
    if *pos < bytes.len() && bytes[*pos] == '-' {
        *pos += 1;
    }
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if *pos < bytes.len() && bytes[*pos] == '.' {
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
    }
    if *pos < bytes.len() && (bytes[*pos] == 'e' || bytes[*pos] == 'E') {
        *pos += 1;
        if *pos < bytes.len() && (bytes[*pos] == '+' || bytes[*pos] == '-') {
            *pos += 1;
        }
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
    }
    let s: String = bytes[start..*pos].iter().collect();
    s.parse::<f64>()
        .map(Json::Number)
        .map_err(|e| format!("Invalid number '{}': {}", s, e))
}

fn parse_string(bytes: &[char], pos: &mut usize) -> Result<String, String> {
    // Assumes bytes[*pos] == '"'
    *pos += 1;
    let mut result = String::new();
    while *pos < bytes.len() {
        match bytes[*pos] {
            '"' => {
                *pos += 1;
                return Ok(result);
            }
            '\\' => {
                *pos += 1;
                if *pos >= bytes.len() {
                    return Err("Unexpected end in escape sequence".to_string());
                }
                match bytes[*pos] {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    '/' => result.push('/'),
                    'b' => result.push('\u{08}'),
                    'f' => result.push('\u{0C}'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'u' => {
                        *pos += 1;
                        if *pos + 4 > bytes.len() {
                            return Err("Invalid unicode escape".to_string());
                        }
                        let hex: String = bytes[*pos..*pos + 4].iter().collect();
                        let code = u32::from_str_radix(&hex, 16)
                            .map_err(|e| format!("Invalid hex in unicode escape: {}", e))?;
                        // Handle surrogate pairs
                        if (0xD800..=0xDBFF).contains(&code)
                            && *pos + 10 <= bytes.len()
                            && bytes[*pos + 4] == '\\'
                            && bytes[*pos + 5] == 'u'
                        {
                            let hex2: String = bytes[*pos + 6..*pos + 10].iter().collect();
                            if let Ok(code2) = u32::from_str_radix(&hex2, 16) {
                                if (0xDC00..=0xDFFF).contains(&code2) {
                                    let combined =
                                        0x10000 + ((code - 0xD800) << 10) + (code2 - 0xDC00);
                                    if let Some(c) = char::from_u32(combined) {
                                        result.push(c);
                                        *pos += 10;
                                        continue;
                                    }
                                }
                            }
                        }
                        if let Some(c) = char::from_u32(code) {
                            result.push(c);
                        }
                        *pos += 3;
                    }
                    c => return Err(format!("Invalid escape character '{}'", c)),
                }
                *pos += 1;
            }
            c => {
                result.push(c);
                *pos += 1;
            }
        }
    }
    Err("Unterminated string".to_string())
}

fn parse_array(bytes: &[char], pos: &mut usize) -> Result<Json, String> {
    *pos += 1; // skip '['
    let mut arr = Vec::new();
    skip_ws(bytes, pos);
    if *pos < bytes.len() && bytes[*pos] == ']' {
        *pos += 1;
        return Ok(Json::Array(arr));
    }
    loop {
        arr.push(parse_value(bytes, pos)?);
        skip_ws(bytes, pos);
        if *pos >= bytes.len() {
            return Err("Unterminated array".to_string());
        }
        match bytes[*pos] {
            ',' => *pos += 1,
            ']' => {
                *pos += 1;
                return Ok(Json::Array(arr));
            }
            c => {
                return Err(format!(
                    "Expected ',' or ']' in array, got '{}' at {}",
                    c, pos
                ))
            }
        }
    }
}

fn parse_object(bytes: &[char], pos: &mut usize) -> Result<Json, String> {
    *pos += 1; // skip '{'
    let mut pairs = Vec::new();
    skip_ws(bytes, pos);
    if *pos < bytes.len() && bytes[*pos] == '}' {
        *pos += 1;
        return Ok(Json::Object(pairs));
    }
    loop {
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != '"' {
            return Err(format!("Expected string key at position {}", pos));
        }
        let key = parse_string(bytes, pos)?;
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != ':' {
            return Err(format!("Expected ':' at position {}", pos));
        }
        *pos += 1;
        let val = parse_value(bytes, pos)?;
        pairs.push((key, val));
        skip_ws(bytes, pos);
        if *pos >= bytes.len() {
            return Err("Unterminated object".to_string());
        }
        match bytes[*pos] {
            ',' => *pos += 1,
            '}' => {
                *pos += 1;
                return Ok(Json::Object(pairs));
            }
            c => {
                return Err(format!(
                    "Expected ',' or '}}' in object, got '{}' at {}",
                    c, pos
                ))
            }
        }
    }
}
