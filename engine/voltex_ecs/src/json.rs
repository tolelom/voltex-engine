//! Mini JSON writer and parser for scene serialization.
//! No external dependencies — self-contained within voltex_ecs.

/// JSON value variants understood by this mini parser/writer.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// JSON number (always read as f64).
    Number(f64),
    /// JSON string.
    Str(String),
    /// JSON array.
    Array(Vec<JsonValue>),
    /// JSON object — key insertion order is preserved.
    Object(Vec<(String, JsonValue)>),
}

/// Old name of [`JsonValue`].
#[deprecated(note = "use `JsonValue`")]
pub type JsonVal = JsonValue;

// ── Writer helpers ──────────────────────────────────────────────────

/// Serialises `null`.
pub fn json_write_null() -> String {
    "null".to_string()
}

/// Serialises an `f32` (integer form when fractional == 0).
pub fn json_write_f32(v: f32) -> String {
    // Emit integer form when the value has no fractional part
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Serialises a JSON string (escapes `"`, `\\`, `\n`, `\r`, `\t`).
pub fn json_write_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Builds a JSON array literal from pre-formatted element strings.
pub fn json_write_array(elements: &[&str]) -> String {
    let mut out = String::from("[");
    for (i, elem) in elements.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(elem);
    }
    out.push(']');
    out
}

/// Builds a JSON object literal from `(key, pre-formatted-value)` pairs.
pub fn json_write_object(pairs: &[(&str, &str)]) -> String {
    let mut out = String::from("{");
    for (i, (key, val)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&json_write_string(key));
        out.push(':');
        out.push_str(val);
    }
    out.push('}');
    out
}

// ── Accessors on JsonValue ────────────────────────────────────────────

impl JsonValue {
    /// Looks up a key on an object value, or returns `None`.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Returns the value as f64, or `None` if it is not a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the value as `&str`, or `None` if it is not a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns the underlying array, or `None` if not an array.
    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Returns the underlying object, or `None` if not an object.
    pub fn as_object(&self) -> Option<&Vec<(String, JsonValue)>> {
        match self {
            JsonValue::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Returns the value as `bool`, or `None` if not a bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

// ── Parser (recursive descent) ──────────────────────────────────────

/// Parses a complete JSON document from `input`. Returns `Err` on any
/// trailing garbage or malformed token.
pub fn json_parse(input: &str) -> crate::Result<JsonValue> {
    let bytes = input.as_bytes();
    let (val, pos) = parse_value(bytes, skip_ws(bytes, 0))?;
    let pos = skip_ws(bytes, pos);
    if pos != bytes.len() {
        return Err(format!("Unexpected trailing content at position {}", pos).into());
    }
    Ok(val)
}

fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

fn parse_value(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    if pos >= bytes.len() {
        return Err("Unexpected end of input".into());
    }
    match bytes[pos] {
        b'"' => parse_string(bytes, pos),
        b'{' => parse_object(bytes, pos),
        b'[' => parse_array(bytes, pos),
        b't' | b'f' => parse_bool(bytes, pos),
        b'n' => parse_null(bytes, pos),
        _ => parse_number(bytes, pos),
    }
}

fn parse_string(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    let (s, end) = read_string(bytes, pos)?;
    Ok((JsonValue::Str(s), end))
}

fn read_string(bytes: &[u8], pos: usize) -> crate::Result<(String, usize)> {
    if pos >= bytes.len() || bytes[pos] != b'"' {
        return Err(format!("Expected '\"' at position {}", pos).into());
    }
    let mut i = pos + 1;
    // Raw bytes, decoded once at the end: pushing each byte as a `char`
    // turned non-ASCII UTF-8 (e.g. a component NAME key) into mojibake.
    let mut s: Vec<u8> = Vec::new();
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let text = String::from_utf8(s).map_err(|_| "Invalid UTF-8 in string")?;
                return Ok((text, i + 1));
            }
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    return Err("Unexpected end in string escape".into());
                }
                match bytes[i] {
                    b'"' => s.push(b'"'),
                    b'\\' => s.push(b'\\'),
                    b'/' => s.push(b'/'),
                    b'n' => s.push(b'\n'),
                    b'r' => s.push(b'\r'),
                    b't' => s.push(b'\t'),
                    other => {
                        s.push(b'\\');
                        s.push(other);
                    }
                }
                i += 1;
            }
            ch => {
                s.push(ch);
                i += 1;
            }
        }
    }
    Err("Unterminated string".into())
}

fn parse_number(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    let mut i = pos;
    // optional minus
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    // digits
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    // fractional
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    // exponent
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i == pos {
        return Err(format!("Expected number at position {}", pos).into());
    }
    let s = std::str::from_utf8(&bytes[pos..i]).unwrap();
    let n: f64 = s
        .parse()
        .map_err(|e| format!("Invalid number '{}': {}", s, e))?;
    Ok((JsonValue::Number(n), i))
}

fn parse_bool(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    if bytes[pos..].starts_with(b"true") {
        Ok((JsonValue::Bool(true), pos + 4))
    } else if bytes[pos..].starts_with(b"false") {
        Ok((JsonValue::Bool(false), pos + 5))
    } else {
        Err(format!("Expected bool at position {}", pos).into())
    }
}

fn parse_null(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    if bytes[pos..].starts_with(b"null") {
        Ok((JsonValue::Null, pos + 4))
    } else {
        Err(format!("Expected null at position {}", pos).into())
    }
}

fn parse_array(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    let mut i = pos + 1; // skip '['
    let mut arr = Vec::new();
    i = skip_ws(bytes, i);
    if i < bytes.len() && bytes[i] == b']' {
        return Ok((JsonValue::Array(arr), i + 1));
    }
    loop {
        i = skip_ws(bytes, i);
        let (val, next) = parse_value(bytes, i)?;
        arr.push(val);
        i = skip_ws(bytes, next);
        if i >= bytes.len() {
            return Err("Unterminated array".into());
        }
        if bytes[i] == b']' {
            return Ok((JsonValue::Array(arr), i + 1));
        }
        if bytes[i] != b',' {
            return Err(format!("Expected ',' or ']' at position {}", i).into());
        }
        i += 1; // skip ','
    }
}

fn parse_object(bytes: &[u8], pos: usize) -> crate::Result<(JsonValue, usize)> {
    let mut i = pos + 1; // skip '{'
    let mut pairs = Vec::new();
    i = skip_ws(bytes, i);
    if i < bytes.len() && bytes[i] == b'}' {
        return Ok((JsonValue::Object(pairs), i + 1));
    }
    loop {
        i = skip_ws(bytes, i);
        let (key, next) = read_string(bytes, i)?;
        i = skip_ws(bytes, next);
        if i >= bytes.len() || bytes[i] != b':' {
            return Err(format!("Expected ':' at position {}", i).into());
        }
        i = skip_ws(bytes, i + 1);
        let (val, next) = parse_value(bytes, i)?;
        pairs.push((key, val));
        i = skip_ws(bytes, next);
        if i >= bytes.len() {
            return Err("Unterminated object".into());
        }
        if bytes[i] == b'}' {
            return Ok((JsonValue::Object(pairs), i + 1));
        }
        if bytes[i] != b',' {
            return Err(format!("Expected ',' or '}}' at position {}", i).into());
        }
        i += 1; // skip ','
    }
}

// ── JsonValue -> String serialization ─────────────────────────────────

impl JsonValue {
    /// Serialize this JsonValue to a compact JSON string.
    pub fn to_json_string(&self) -> String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            JsonValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            JsonValue::Str(s) => json_write_string(s),
            JsonValue::Array(arr) => {
                let mut out = String::from("[");
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&v.to_json_string());
                }
                out.push(']');
                out
            }
            JsonValue::Object(pairs) => {
                let mut out = String::from("{");
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&json_write_string(k));
                    out.push(':');
                    out.push_str(&v.to_json_string());
                }
                out.push('}');
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_null() {
        assert_eq!(json_write_null(), "null");
    }

    #[test]
    #[allow(clippy::approx_constant)] // "3.14" is a representative decimal, not π.
    fn test_write_number() {
        assert_eq!(json_write_f32(3.14), "3.14");
        assert_eq!(json_write_f32(1.0), "1");
    }

    #[test]
    fn test_write_string() {
        assert_eq!(json_write_string("hello"), "\"hello\"");
        assert_eq!(json_write_string("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn test_write_array() {
        assert_eq!(json_write_array(&["1", "2", "3"]), "[1,2,3]");
    }

    #[test]
    fn test_write_object() {
        let pairs = vec![("name", "\"test\""), ("value", "42")];
        let result = json_write_object(&pairs);
        assert_eq!(result, r#"{"name":"test","value":42}"#);
    }

    #[test]
    fn test_parse_number() {
        match json_parse("42.5").unwrap() {
            JsonValue::Number(n) => assert!((n - 42.5).abs() < 1e-10),
            _ => panic!("expected number"),
        }
    }

    #[test]
    #[allow(clippy::approx_constant)] // "-3.14" is a representative negative decimal, not π.
    fn test_parse_negative_number() {
        match json_parse("-3.14").unwrap() {
            JsonValue::Number(n) => assert!((n - (-3.14)).abs() < 1e-10),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(
            json_parse("\"hello\"").unwrap(),
            JsonValue::Str("hello".into())
        );
    }

    #[test]
    fn test_parse_string_with_escapes() {
        assert_eq!(
            json_parse(r#""a\"b\\c""#).unwrap(),
            JsonValue::Str("a\"b\\c".into())
        );
    }

    #[test]
    fn test_parse_array() {
        match json_parse("[1,2,3]").unwrap() {
            JsonValue::Array(a) => assert_eq!(a.len(), 3),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_parse_empty_array() {
        match json_parse("[]").unwrap() {
            JsonValue::Array(a) => assert_eq!(a.len(), 0),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_parse_object() {
        let val = json_parse(r#"{"x":1,"y":2}"#).unwrap();
        assert!(matches!(val, JsonValue::Object(_)));
        assert_eq!(val.get("x").unwrap().as_f64().unwrap(), 1.0);
        assert_eq!(val.get("y").unwrap().as_f64().unwrap(), 2.0);
    }

    #[test]
    fn test_parse_null() {
        assert_eq!(json_parse("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(json_parse("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(json_parse("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn test_parse_nested() {
        let val = json_parse(r#"{"a":[1,2],"b":{"c":3}}"#).unwrap();
        assert!(matches!(val, JsonValue::Object(_)));
        let arr = val.get("a").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let inner = val.get("b").unwrap();
        assert_eq!(inner.get("c").unwrap().as_f64().unwrap(), 3.0);
    }

    #[test]
    fn test_parse_whitespace() {
        let val = json_parse("  { \"a\" : 1 , \"b\" : [ 2 , 3 ] }  ").unwrap();
        assert!(matches!(val, JsonValue::Object(_)));
    }

    #[test]
    fn test_json_val_to_json_string_roundtrip() {
        let val = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("test".into())),
            ("count".into(), JsonValue::Number(42.0)),
            (
                "items".into(),
                JsonValue::Array(vec![
                    JsonValue::Number(1.0),
                    JsonValue::Null,
                    JsonValue::Bool(true),
                ]),
            ),
        ]);
        let s = val.to_json_string();
        let parsed = json_parse(&s).unwrap();
        assert_eq!(parsed, val);
    }
}
