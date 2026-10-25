// Minimal markdown parser: **bold**, *italic*, `code`, ```code blocks```
// Outputs Vec<StyledRun> for render.rs to draw.
#![allow(dead_code)]

use crate::theme::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub color: u32,
    pub is_code_block: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            code: false,
            color: MD_TEXT, // warm white for agent text
            is_code_block: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StyledRun {
    pub text: String,
    pub style: Style,
}

/// Parse markdown text into styled runs.
pub fn parse(text: &str) -> Vec<StyledRun> {
    let mut runs = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut in_code_block = false;

    while i < lines.len() {
        let line = lines[i];

        // Check for code block delimiters
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            i += 1;
            continue;
        }

        if in_code_block {
            // Code block line
            let style = Style {
                code: true,
                is_code_block: true,
                color: MD_CODE_BLOCK,
                ..Default::default()
            };
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
                    style: current_style,
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
                let mut code_style = current_style;
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
                let mut header_style = current_style;
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
        if chars[i..i + d.len()] == d[..] {
            return Some(i);
        }
        i += 1;
    }
    None
}
