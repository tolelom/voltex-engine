//! Self-contained mini TOML parser for `voltex.toml` (the subset described
//! in the parent module docs). Errors carry a 1-based line and column.

use crate::error::{CliError, Result};

#[derive(Debug, Clone, Default)]
pub(super) struct TomlSection {
    pub(super) entries: Vec<(String, TomlValue)>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Int/Bool are produced by the parser even though no
                    // current manifest field reads them; future fields may.
pub(super) enum TomlValue {
    String(String),
    Int(i64),
    Bool(bool),
    StringArray(Vec<String>),
    IntArray(Vec<i64>),
}

impl TomlSection {
    pub(super) fn get(&self, key: &str) -> Option<&TomlValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub(super) fn get_string(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            TomlValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    pub(super) fn get_string_array(&self, key: &str) -> Result<Option<Vec<String>>> {
        match self.get(key) {
            None => Ok(None),
            Some(TomlValue::StringArray(v)) => Ok(Some(v.clone())),
            Some(other) => Err(CliError::InvalidValue {
                field: key.to_string(),
                got: format!("{:?}", other),
            }),
        }
    }
    pub(super) fn get_int_array(&self, key: &str) -> Result<Option<Vec<i64>>> {
        match self.get(key) {
            None => Ok(None),
            Some(TomlValue::IntArray(v)) => Ok(Some(v.clone())),
            Some(other) => Err(CliError::InvalidValue {
                field: key.to_string(),
                got: format!("{:?}", other),
            }),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct TomlDoc {
    pub(super) sections: Vec<(String, TomlSection)>,
}

impl TomlDoc {
    pub(super) fn get(&self, name: &str) -> Option<&TomlSection> {
        self.sections
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }
    pub(super) fn sections(&self) -> impl Iterator<Item = (&String, &TomlSection)> {
        self.sections.iter().map(|(k, v)| (k, v))
    }
}

/// Where a parse error happened: the 1-based line and the raw line text.
/// Every substring the parser inspects is a slice of `raw`, so an error can
/// point at the slice and the column falls out of its offset.
pub(super) struct Loc<'a> {
    pub(super) line_no: usize,
    pub(super) raw: &'a str,
}

impl Loc<'_> {
    /// `ManifestParse` at the start of `at` (a slice of `raw`, possibly the
    /// empty slice at its end). Column is 1-based, in characters.
    pub(super) fn err(&self, at: &str, msg: impl Into<String>) -> CliError {
        let offset = (at.as_ptr() as usize).saturating_sub(self.raw.as_ptr() as usize);
        let offset = offset.min(self.raw.len());
        let column = self.raw[..offset].chars().count() + 1;
        CliError::ManifestParse {
            line: self.line_no,
            column,
            msg: msg.into(),
        }
    }
}

pub(super) fn parse_toml(text: &str) -> Result<TomlDoc> {
    let mut doc = TomlDoc::default();
    let mut current = String::new();
    let mut current_entries: Vec<(String, TomlValue)> = Vec::new();

    let flush = |doc: &mut TomlDoc, name: &mut String, entries: &mut Vec<(String, TomlValue)>| {
        if !name.is_empty() || !entries.is_empty() {
            doc.sections.push((
                name.clone(),
                TomlSection {
                    entries: std::mem::take(entries),
                },
            ));
            name.clear();
        }
    };

    for (i, raw_line) in text.lines().enumerate() {
        let loc = Loc { line_no: i + 1, raw: raw_line };
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = parse_header(line, &loc)? {
            flush(&mut doc, &mut current, &mut current_entries);
            current = header;
            continue;
        }
        let (key, value) = parse_kv(line, &loc)?;
        current_entries.push((key, value));
    }
    flush(&mut doc, &mut current, &mut current_entries);
    Ok(doc)
}

pub(super) fn strip_comment(line: &str) -> &str {
    // Naive but correct for our subset: `#` outside of a string starts a comment.
    let bytes = line.as_bytes();
    let mut in_str = false;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'"' {
            in_str = !in_str;
        }
        if b == b'#' && !in_str {
            return &line[..i];
        }
    }
    line
}

pub(super) fn parse_header(line: &str, loc: &Loc) -> Result<Option<String>> {
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Ok(None);
    }
    if bytes.last() != Some(&b']') {
        return Err(loc.err(&line[line.len()..], "section header missing closing `]`"));
    }
    let inner = line[1..line.len() - 1].trim();
    if inner.is_empty() {
        return Err(loc.err(line, "empty section header"));
    }
    Ok(Some(inner.to_string()))
}

pub(super) fn parse_kv(line: &str, loc: &Loc) -> Result<(String, TomlValue)> {
    let Some(eq) = line.find('=') else {
        return Err(loc.err(line, "expected `key = value`"));
    };
    let key = line[..eq].trim().to_string();
    if key.is_empty() {
        return Err(loc.err(&line[eq..], "empty key"));
    }
    let value_part = line[eq + 1..].trim();
    let value = parse_value(value_part, loc)?;
    Ok((key, value))
}

pub(super) fn parse_value(raw: &str, loc: &Loc) -> Result<TomlValue> {
    if raw.starts_with('"') {
        return parse_string(raw, loc).map(TomlValue::String);
    }
    if raw == "true" {
        return Ok(TomlValue::Bool(true));
    }
    if raw == "false" {
        return Ok(TomlValue::Bool(false));
    }
    if raw.starts_with('[') {
        return parse_array(raw, loc);
    }
    // Plain integer?
    if let Ok(n) = raw.parse::<i64>() {
        return Ok(TomlValue::Int(n));
    }
    Err(loc.err(raw, format!("unrecognised value `{}`", raw)))
}

pub(super) fn parse_string(raw: &str, loc: &Loc) -> Result<String> {
    if !raw.ends_with('"') || raw.len() < 2 {
        return Err(loc.err(raw, "unterminated string"));
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {
                    return Err(loc.err(raw, "trailing backslash in string"))
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

pub(super) fn parse_array(raw: &str, loc: &Loc) -> Result<TomlValue> {
    if !raw.ends_with(']') || raw.len() < 2 {
        return Err(loc.err(raw, "unterminated array"));
    }
    let inner = raw[1..raw.len() - 1].trim();
    if inner.is_empty() {
        return Ok(TomlValue::StringArray(Vec::new()));
    }
    let items = split_array_items(inner);

    // Decide string vs int based on the first element.
    let first = items[0].trim();
    if first.starts_with('"') {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(parse_string(item.trim(), loc)?);
        }
        Ok(TomlValue::StringArray(out))
    } else {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let item = item.trim();
            let n: i64 = item
                .parse()
                .map_err(|_| loc.err(item, format!("expected integer in array, got `{}`", item)))?;
            out.push(n);
        }
        Ok(TomlValue::IntArray(out))
    }
}

/// Split on top-level commas, respecting `"..."` string boundaries.
pub(super) fn split_array_items(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_str = false;
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b',' if !in_str => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    let tail = &s[start..];
    if !tail.trim().is_empty() {
        parts.push(tail);
    }
    parts
}
