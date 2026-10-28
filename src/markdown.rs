// Minimal markdown parser: **bold**, *italic*, `code`, ```code blocks```
// Outputs Vec<StyledRun> for render.rs to draw.
#![allow(dead_code)]

use crate::theme::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub color: u32,
    pub is_code_block: bool,
    pub is_link: bool,
    pub code_lang: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            code: false,
            color: MD_TEXT,
            is_code_block: false,
            is_link: false,
            code_lang: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StyledRun {
    pub text: String,
    pub style: Style,
}

/// Find the end of a URL in a character slice starting at `start`.
/// URLs end at whitespace or certain punctuation.
fn find_url_end(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == '<' || c == '>' || c == '"' || c == '\'' {
            break;
        }
        // Stop at trailing punctuation that's unlikely part of a URL
        if (c == '.' || c == ',' || c == ';' || c == '!' || c == '?' || c == ')')
            && i + 1 < chars.len()
            && (chars[i + 1].is_whitespace() || chars[i + 1] == '\n')
        {
            break;
        }
        i += 1;
    }
    i
}

/// Parse markdown text into styled runs.
/// Also returns any code block language labels found: Vec<(start_line_idx_in_runs, lang)>.
pub fn parse(text: &str) -> Vec<StyledRun> {
    let mut runs = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut in_code_block = false;
    let mut current_code_lang: Option<String> = None;

    while i < lines.len() {
        let line = lines[i];

        // Check for code block delimiters
        if line.trim_start().starts_with("```") {
            let lang = line.trim_start()[3..].trim().to_string();
            in_code_block = !in_code_block;
            current_code_lang = if in_code_block && !lang.is_empty() {
                Some(lang)
            } else {
                None
            };
            i += 1;
            continue;
        }

        if in_code_block {
            // Code block line — tag the first line with the language so the renderer can draw a label
            let mut style = Style {
                code: true,
                is_code_block: true,
                color: MD_CODE_BLOCK,
                ..Default::default()
            };
            style.code_lang = current_code_lang.clone();
            runs.push(StyledRun {
                text: line.to_string(),
                style,
            });
            i += 1;
            continue;
        }

        // Parse inline markdown on this line
        let inline_runs = parse_inline(line);
        runs.extend(inline_runs);

        // Add newline as a separate run if not the last line
        if i < lines.len() - 1 {
            runs.push(StyledRun {
                text: "\n".to_string(),
                style: Style::default(),
            });
        }

        i += 1;
    }

    runs
}

/// If a URL scheme (http:// or https://) starts at `i`, return its length.
fn url_scheme_len(chars: &[char], i: usize) -> Option<usize> {
    if chars.get(i) != Some(&'h') {
        return None;
    }
    let rest = &chars[i..];
    for (scheme, len) in [("https://", 8usize), ("http://", 7)] {
        let matches = rest.len() >= len && scheme.chars().zip(rest.iter()).all(|(a, b)| a == *b);
        if matches {
            return Some(len);
        }
    }
    None
}

fn parse_inline(text: &str) -> Vec<StyledRun> {
    let mut runs = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut current_text = String::new();
    let current_style = Style::default();

    macro_rules! flush {
        () => {
            if !current_text.is_empty() {
                runs.push(StyledRun {
                    text: std::mem::take(&mut current_text),
                    style: current_style.clone(),
                });
            }
        };
    }

    while i < chars.len() {
        // **bold**
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            // Find closing **
            if let Some(end) = find_delimiter(&chars, i + 2, "**") {
                flush!();
                let inner: String = chars[i + 2..end].iter().collect();
                // Recursively parse inside bold
                let inner_runs = parse_inline(&inner);
                for mut r in inner_runs {
                    r.style.bold = true;
                    if r.style.color == current_style.color {
                        r.style.color = MD_BOLD;
                    }
                    runs.push(r);
                }
                i = end + 2;
                continue;
            }
        }

        // *italic* or _italic_
        if (chars[i] == '*' || chars[i] == '_') && (i == 0 || chars[i - 1] != chars[i]) {
            let delim = chars[i];
            if i + 1 < chars.len() && chars[i + 1] != delim {
                if let Some(end) = find_delimiter(&chars, i + 1, &delim.to_string()) {
                    if end > i + 1 {
                        flush!();
                        let inner: String = chars[i + 1..end].iter().collect();
                        let inner_runs = parse_inline(&inner);
                        for mut r in inner_runs {
                            r.style.italic = true;
                            runs.push(r);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
        }

        // `code`
        if chars[i] == '`' {
            if let Some(end) = find_delimiter(&chars, i + 1, "`") {
                flush!();
                let inner: String = chars[i + 1..end].iter().collect();
                let mut code_style = current_style.clone();
                code_style.code = true;
                code_style.color = MD_INLINE_CODE; // bright orange for inline code
                runs.push(StyledRun {
                    text: inner,
                    style: code_style,
                });
                i = end + 1;
                continue;
            }
        }

        // Headers (# ## ###)
        if i == 0 && chars[i] == '#' {
            let mut level = 0;
            while i + level < chars.len() && chars[i + level] == '#' {
                level += 1;
            }
            if level <= 6 && i + level < chars.len() && chars[i + level] == ' ' {
                flush!();
                let mut header_style = current_style.clone();
                header_style.bold = true;
                header_style.color = MD_HEADING;
                let header_text: String = chars[i + level + 1..].iter().collect();
                runs.push(StyledRun {
                    text: header_text,
                    style: header_style,
                });
                return runs;
            }
        }

        // Bullet point (- or * at start of line)
        if i == 0
            && (chars[i] == '-' || chars[i] == '*')
            && i + 1 < chars.len()
            && chars[i + 1] == ' '
        {
            current_text.push('\u{2022}'); // bullet character
            i += 2;
            continue;
        }

        // URL detection (http:// or https://) - zero-alloc prefix checks
        let url_prefix_len = url_scheme_len(&chars, i);
        if let Some(prefix_len) = url_prefix_len {
            let url_end = find_url_end(&chars, i + prefix_len);
            if url_end > i + prefix_len {
                flush!();
                let url: String = chars[i..url_end].iter().collect();
                let mut link_style = current_style.clone();
                link_style.is_link = true;
                link_style.color = ACCENT_HOVER;
                runs.push(StyledRun {
                    text: url,
                    style: link_style,
                });
                i = url_end;
                continue;
            }
        }

        current_text.push(chars[i]);
        i += 1;
    }

    flush!();
    runs
}

fn find_delimiter(chars: &[char], start: usize, delim: &str) -> Option<usize> {
    let d: Vec<char> = delim.chars().collect();
    if d.is_empty() {
        return None;
    }
    let mut i = start;
    while i + d.len() <= chars.len() {
        // Compare char-by-char against the delimiter slice — avoids building a
        // candidate `Vec<char>` window on every probe.
        if d.iter()
            .enumerate()
            .all(|(offset, &dc)| chars[i + offset] == dc)
        {
            return Some(i);
        }
        i += 1;
    }
    None
}
