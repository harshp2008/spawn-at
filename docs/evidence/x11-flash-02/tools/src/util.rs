//! Small helpers shared by the three subcommands: the monotonic clock, JSON text (written by
//! hand, because the tool has no serde dependency) and a seeded shuffle.

use std::fmt::Write as _;

/// `CLOCK_MONOTONIC` in nanoseconds. The extension's `mono_ms` fields and the fixture's event
/// files use the same clock.
pub fn mono_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec for the duration of the call.
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// `s` as a JSON string literal, quotes included.
pub fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The text after `"key":` in a flat JSON object, up to the next `,` or `}` at depth 0. Enough for
/// the one-level objects this tool writes and the one `spawn-at spawn --json` prints.
pub fn raw_field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\":", key);
    let start = json.find(&pat)? + pat.len();
    let rest = json[start..].trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let mut esc = false;
        for (i, c) in stripped.char_indices() {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                return Some(&rest[..i + 2]);
            }
        }
        return None;
    }
    if rest.starts_with('[') || rest.starts_with('{') {
        let (open, close) = if rest.starts_with('[') {
            ('[', ']')
        } else {
            ('{', '}')
        };
        let mut depth = 0i32;
        let mut in_str = false;
        let mut esc = false;
        for (i, c) in rest.char_indices() {
            if in_str {
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    in_str = false;
                }
                continue;
            }
            if c == '"' {
                in_str = true;
            } else if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[..=i]);
                }
            }
        }
        return None;
    }
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim())
}

pub fn num_field(json: &str, key: &str) -> Option<f64> {
    raw_field(json, key)?.parse().ok()
}

pub fn str_field(json: &str, key: &str) -> Option<String> {
    let raw = raw_field(json, key)?;
    let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    Some(out)
}

/// xorshift64*: a seeded generator, so a plan can be reproduced from the seed in `meta.json`.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

pub fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len();
    Some(if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    })
}

pub fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.1}", x),
        None => "-".into(),
    }
}
