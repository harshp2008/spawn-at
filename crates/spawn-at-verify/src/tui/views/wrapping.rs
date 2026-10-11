//! Text tokenization, word wrapping, and dimension string normalisation.

use unicode_width::UnicodeWidthStr;

/// Normalizes dimension strings by replacing 'x' with '×' multiplication signs.
pub fn normalise_dimensions(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == 'x' {
            let prev_is_num =
                i > 0 && (chars[i - 1].is_ascii_digit() || (i >= 4 && &s[i - 4..i] == "null"));
            let next_is_num =
                i + 1 < len && (chars[i + 1].is_ascii_digit() || s[i + 1..].starts_with("null"));
            if prev_is_num && next_is_num {
                out.push('×');
                i += 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Splits input text into whitespace-delimited word tokens.
fn tokenize_text(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if (w.starts_with('+') || w.chars().all(|c| c.is_ascii_digit() || c == '.'))
            && i + 1 < words.len()
            && words[i + 1] == "ms"
        {
            tokens.push(format!("{} ms", w));
            i += 2;
            continue;
        }
        if i + 1 < words.len() && words[i + 1].starts_with('(') && words[i + 1].ends_with(')') {
            tokens.push(format!("{} {}", w, words[i + 1]));
            i += 2;
            continue;
        }
        tokens.push(w.to_string());
        i += 1;
    }
    tokens
}

/// Hard-breaks an oversized token to fit within maximum allowed line width.
fn hard_break_token(token: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![token.to_string()];
    }
    let mut chunks = Vec::new();
    let mut rem = token;
    while rem.width() > max_width {
        let mut best_break = None;
        let mut curr_width = 0;
        for (byte_idx, ch) in rem.char_indices() {
            let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
            if curr_width + ch_w > max_width {
                break;
            }
            curr_width += ch_w;
            if ch == '.' || ch == '-' || ch == '/' || ch == ':' {
                best_break = Some(byte_idx + ch.len_utf8());
            }
        }
        let split_pos = match best_break {
            Some(pos) if pos > 0 => pos,
            _ => {
                let mut fit_len = 0;
                let mut fit_w = 0;
                for (byte_idx, ch) in rem.char_indices() {
                    let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                    if fit_w + ch_w > max_width {
                        break;
                    }
                    fit_w += ch_w;
                    fit_len = byte_idx + ch.len_utf8();
                }
                if fit_len == 0 {
                    rem.chars().next().map(|c| c.len_utf8()).unwrap_or(1)
                } else {
                    fit_len
                }
            }
        };
        chunks.push(rem[..split_pos].to_string());
        rem = &rem[split_pos..];
    }
    if !rem.is_empty() {
        chunks.push(rem.to_string());
    }
    chunks
}

/// Wraps tokens into multiple lines respecting left column alignment.
pub fn wrap_value_tokens(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let raw_tokens = tokenize_text(text);
    if raw_tokens.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for token in raw_tokens {
        let sub_tokens = if token.width() > max_width {
            hard_break_token(&token, max_width)
        } else {
            vec![token]
        };

        for sub in sub_tokens {
            if current_line.is_empty() {
                current_line.push_str(&sub);
            } else if current_line.width() + 1 + sub.width() <= max_width {
                current_line.push(' ');
                current_line.push_str(&sub);
            } else {
                lines.push(current_line);
                current_line = sub;
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
