//! A small strict JSON reader for the two documents these tests read:
//! `cargo metadata` output and `tools/governance/claims-baseline.json`.
//!
//! Written here because the workspace admits no new crates.io dependency and no
//! workspace crate exports a general JSON reader. Numbers follow the RFC 8259 grammar
//! and are kept as their text; raw control bytes, surrogate escapes, duplicate keys,
//! and trailing bytes are refused; nesting is bounded. The duplicate-key check is a
//! scan per object, which is fine at these documents' sizes.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 64;

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

pub fn parse(text: &str) -> Result<Json, String> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let value = value(bytes, &mut at, 0)?;
    skip(bytes, &mut at);
    if at != bytes.len() {
        return Err(format!("trailing bytes at {at}"));
    }
    Ok(value)
}

fn skip(bytes: &[u8], at: &mut usize) {
    while *at < bytes.len() && matches!(bytes[*at], b' ' | b'\t' | b'\n' | b'\r') {
        *at += 1;
    }
}

fn expect(bytes: &[u8], at: &mut usize, literal: &str) -> Result<(), String> {
    if bytes[*at..].starts_with(literal.as_bytes()) {
        *at += literal.len();
        Ok(())
    } else {
        Err(format!("expected {literal} at {at}"))
    }
}

fn value(bytes: &[u8], at: &mut usize, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err("nesting too deep".to_owned());
    }
    skip(bytes, at);
    match bytes.get(*at) {
        None => Err("unexpected end".to_owned()),
        Some(b'n') => expect(bytes, at, "null").map(|()| Json::Null),
        Some(b't') => expect(bytes, at, "true").map(|()| Json::Bool(true)),
        Some(b'f') => expect(bytes, at, "false").map(|()| Json::Bool(false)),
        Some(b'"') => string(bytes, at).map(Json::String),
        Some(b'[') => {
            *at += 1;
            let mut items = Vec::new();
            skip(bytes, at);
            if bytes.get(*at) == Some(&b']') {
                *at += 1;
                return Ok(Json::Array(items));
            }
            loop {
                items.push(value(bytes, at, depth + 1)?);
                skip(bytes, at);
                match bytes.get(*at) {
                    Some(b',') => *at += 1,
                    Some(b']') => {
                        *at += 1;
                        return Ok(Json::Array(items));
                    }
                    _ => return Err(format!("expected , or ] at {at}")),
                }
            }
        }
        Some(b'{') => {
            *at += 1;
            let mut fields = Vec::new();
            skip(bytes, at);
            if bytes.get(*at) == Some(&b'}') {
                *at += 1;
                return Ok(Json::Object(fields));
            }
            loop {
                skip(bytes, at);
                let key = string(bytes, at)?;
                skip(bytes, at);
                expect(bytes, at, ":")?;
                let item = value(bytes, at, depth + 1)?;
                if fields.iter().any(|(k, _)| *k == key) {
                    return Err(format!("duplicate key {key}"));
                }
                fields.push((key, item));
                skip(bytes, at);
                match bytes.get(*at) {
                    Some(b',') => *at += 1,
                    Some(b'}') => {
                        *at += 1;
                        return Ok(Json::Object(fields));
                    }
                    _ => return Err(format!("expected , or }} at {at}")),
                }
            }
        }
        Some(b'-' | b'0'..=b'9') => number(bytes, at).map(Json::Number),
        Some(other) => Err(format!("unexpected byte {other} at {at}")),
    }
}

/// RFC 8259 §6: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`.
fn number(bytes: &[u8], at: &mut usize) -> Result<String, String> {
    let start = *at;
    let digits = |at: &mut usize| {
        let from = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        *at - from
    };
    if bytes.get(*at) == Some(&b'-') {
        *at += 1;
    }
    match bytes.get(*at) {
        Some(b'0') => *at += 1,
        Some(b'1'..=b'9') => {
            digits(at);
        }
        _ => return Err(format!("bad number at {start}")),
    }
    if bytes.get(*at) == Some(&b'.') {
        *at += 1;
        if digits(at) == 0 {
            return Err(format!("bad fraction at {start}"));
        }
    }
    if matches!(bytes.get(*at), Some(b'e' | b'E')) {
        *at += 1;
        if matches!(bytes.get(*at), Some(b'+' | b'-')) {
            *at += 1;
        }
        if digits(at) == 0 {
            return Err(format!("bad exponent at {start}"));
        }
    }
    Ok(String::from_utf8_lossy(&bytes[start..*at]).into_owned())
}

fn string(bytes: &[u8], at: &mut usize) -> Result<String, String> {
    expect(bytes, at, "\"")?;
    let mut out: Vec<u8> = Vec::new();
    loop {
        match bytes.get(*at) {
            None => return Err("unterminated string".to_owned()),
            Some(b'"') => {
                *at += 1;
                return String::from_utf8(out).map_err(|e| e.to_string());
            }
            Some(b'\\') => {
                let escape = *bytes.get(*at + 1).ok_or("unterminated escape")?;
                *at += 2;
                match escape {
                    b'"' => out.push(b'"'),
                    b'\\' => out.push(b'\\'),
                    b'/' => out.push(b'/'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'u' => {
                        let hex = bytes.get(*at..*at + 4).ok_or("short \\u escape")?;
                        if !hex.iter().all(u8::is_ascii_hexdigit) {
                            return Err(format!("bad \\u escape at {at}"));
                        }
                        let code = u32::from_str_radix(
                            std::str::from_utf8(hex).map_err(|e| e.to_string())?,
                            16,
                        )
                        .map_err(|e| e.to_string())?;
                        *at += 4;
                        // Surrogates are not needed by either document; refuse them.
                        let ch = char::from_u32(code).ok_or("surrogate \\u escape")?;
                        let mut buf = [0; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    _ => return Err(format!("bad escape at {at}")),
                }
            }
            Some(byte) if *byte < 0x20 => {
                return Err(format!("raw control byte in string at {at}"));
            }
            Some(byte) => {
                out.push(*byte);
                *at += 1;
            }
        }
    }
}
