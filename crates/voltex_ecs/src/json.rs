/// Mini JSON writer and parser for scene serialization.
/// No external dependencies — self-contained within voltex_ecs.

#[derive(Debug, Clone, PartialEq)]
pub enum JsonVal {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonVal>),
    Object(Vec<(String, JsonVal)>),
}

// ── Writer helpers ──────────────────────────────────────────────────

pub fn json_write_null() -> String {
    "null".to_string()
}

pub fn json_write_f32(v: f32) -> String {
    // Emit integer form when the value has no fractional part
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

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

/// Write a JSON array from pre-formatted element strings.
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

/// Write a JSON object from (key, pre-formatted-value) pairs.
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

// ── Accessors on JsonVal ────────────────────────────────────────────

impl JsonVal {
    pub fn get(&self, key: &str) -> Option<&JsonVal> {
        match self {
            JsonVal::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonVal::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<JsonVal>> {
        match self {
            JsonVal::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, JsonVal)>> {
        match self {
            JsonVal::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonVal::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

// ── Parser (recursive descent) ──────────────────────────────────────

pub fn json_parse(input: &str) -> crate::Result<JsonVal> {
    let bytes = input.as_bytes();
    let (val, pos) = parse_value(bytes, skip_ws(bytes, 0))?;
    let pos = skip_ws(bytes, pos);
    if pos != bytes.len() {
        return Err(format!("Unexpected trailing content at position {}", pos).into());
    }
    Ok(val)
}

fn skip_ws(b: &[u8], mut pos: usize) -> usize {
    while pos < b.len() && matches!(b[pos], b' ' | b'\t' | b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

fn parse_value(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    if pos >= b.len() {
        return Err("Unexpected end of input".into());
    }
    match b[pos] {
        b'"' => parse_string(b, pos),
        b'{' => parse_object(b, pos),
        b'[' => parse_array(b, pos),
        b't' | b'f' => parse_bool(b, pos),
        b'n' => parse_null(b, pos),
        _ => parse_number(b, pos),
    }
}

fn parse_string(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    let (s, end) = read_string(b, pos)?;
    Ok((JsonVal::Str(s), end))
}

fn read_string(b: &[u8], pos: usize) -> crate::Result<(String, usize)> {
    if pos >= b.len() || b[pos] != b'"' {
        return Err(format!("Expected '\"' at position {}", pos).into());
    }
    let mut i = pos + 1;
    let mut s = String::new();
    while i < b.len() {
        match b[i] {
            b'"' => return Ok((s, i + 1)),
            b'\\' => {
                i += 1;
                if i >= b.len() {
                    return Err("Unexpected end in string escape".into());
                }
                match b[i] {
                    b'"' => s.push('"'),
                    b'\\' => s.push('\\'),
                    b'/' => s.push('/'),
                    b'n' => s.push('\n'),
                    b'r' => s.push('\r'),
                    b't' => s.push('\t'),
                    _ => {
                        s.push('\\');
                        s.push(b[i] as char);
                    }
                }
                i += 1;
            }
            ch => {
                s.push(ch as char);
                i += 1;
            }
        }
    }
    Err("Unterminated string".into())
}

fn parse_number(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    let mut i = pos;
    // optional minus
    if i < b.len() && b[i] == b'-' {
        i += 1;
    }
    // digits
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    // fractional
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    // exponent
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i == pos {
        return Err(format!("Expected number at position {}", pos).into());
    }
    let s = std::str::from_utf8(&b[pos..i]).unwrap();
    let n: f64 = s.parse().map_err(|e| format!("Invalid number '{}': {}", s, e))?;
    Ok((JsonVal::Number(n), i))
}

fn parse_bool(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    if b[pos..].starts_with(b"true") {
        Ok((JsonVal::Bool(true), pos + 4))
    } else if b[pos..].starts_with(b"false") {
        Ok((JsonVal::Bool(false), pos + 5))
    } else {
        Err(format!("Expected bool at position {}", pos).into())
    }
}

fn parse_null(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    if b[pos..].starts_with(b"null") {
        Ok((JsonVal::Null, pos + 4))
    } else {
        Err(format!("Expected null at position {}", pos).into())
    }
}

fn parse_array(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    let mut i = pos + 1; // skip '['
    let mut arr = Vec::new();
    i = skip_ws(b, i);
    if i < b.len() && b[i] == b']' {
        return Ok((JsonVal::Array(arr), i + 1));
    }
    loop {
        i = skip_ws(b, i);
        let (val, next) = parse_value(b, i)?;
        arr.push(val);
        i = skip_ws(b, next);
        if i >= b.len() {
            return Err("Unterminated array".into());
        }
        if b[i] == b']' {
            return Ok((JsonVal::Array(arr), i + 1));
        }
        if b[i] != b',' {
            return Err(format!("Expected ',' or ']' at position {}", i).into());
        }
        i += 1; // skip ','
    }
}

fn parse_object(b: &[u8], pos: usize) -> crate::Result<(JsonVal, usize)> {
    let mut i = pos + 1; // skip '{'
    let mut pairs = Vec::new();
    i = skip_ws(b, i);
    if i < b.len() && b[i] == b'}' {
        return Ok((JsonVal::Object(pairs), i + 1));
    }
    loop {
        i = skip_ws(b, i);
        let (key, next) = read_string(b, i)?;
        i = skip_ws(b, next);
        if i >= b.len() || b[i] != b':' {
            return Err(format!("Expected ':' at position {}", i).into());
        }
        i = skip_ws(b, i + 1);
        let (val, next) = parse_value(b, i)?;
        pairs.push((key, val));
        i = skip_ws(b, next);
        if i >= b.len() {
            return Err("Unterminated object".into());
        }
        if b[i] == b'}' {
            return Ok((JsonVal::Object(pairs), i + 1));
        }
        if b[i] != b',' {
            return Err(format!("Expected ',' or '}}' at position {}", i).into());
        }
        i += 1; // skip ','
    }
}

// ── JsonVal -> String serialization ─────────────────────────────────

impl JsonVal {
    /// Serialize this JsonVal to a compact JSON string.
    pub fn to_json_string(&self) -> String {
        match self {
            JsonVal::Null => "null".to_string(),
            JsonVal::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            JsonVal::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            JsonVal::Str(s) => json_write_string(s),
            JsonVal::Array(arr) => {
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
            JsonVal::Object(pairs) => {
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
            JsonVal::Number(n) => assert!((n - 42.5).abs() < 1e-10),
            _ => panic!("expected number"),
        }
    }

    #[test]
    #[allow(clippy::approx_constant)] // "-3.14" is a representative negative decimal, not π.
    fn test_parse_negative_number() {
        match json_parse("-3.14").unwrap() {
            JsonVal::Number(n) => assert!((n - (-3.14)).abs() < 1e-10),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(json_parse("\"hello\"").unwrap(), JsonVal::Str("hello".into()));
    }

    #[test]
    fn test_parse_string_with_escapes() {
        assert_eq!(
            json_parse(r#""a\"b\\c""#).unwrap(),
            JsonVal::Str("a\"b\\c".into())
        );
    }

    #[test]
    fn test_parse_array() {
        match json_parse("[1,2,3]").unwrap() {
            JsonVal::Array(a) => assert_eq!(a.len(), 3),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_parse_empty_array() {
        match json_parse("[]").unwrap() {
            JsonVal::Array(a) => assert_eq!(a.len(), 0),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_parse_object() {
        let val = json_parse(r#"{"x":1,"y":2}"#).unwrap();
        assert!(matches!(val, JsonVal::Object(_)));
        assert_eq!(val.get("x").unwrap().as_f64().unwrap(), 1.0);
        assert_eq!(val.get("y").unwrap().as_f64().unwrap(), 2.0);
    }

    #[test]
    fn test_parse_null() {
        assert_eq!(json_parse("null").unwrap(), JsonVal::Null);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(json_parse("true").unwrap(), JsonVal::Bool(true));
        assert_eq!(json_parse("false").unwrap(), JsonVal::Bool(false));
    }

    #[test]
    fn test_parse_nested() {
        let val = json_parse(r#"{"a":[1,2],"b":{"c":3}}"#).unwrap();
        assert!(matches!(val, JsonVal::Object(_)));
        let arr = val.get("a").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let inner = val.get("b").unwrap();
        assert_eq!(inner.get("c").unwrap().as_f64().unwrap(), 3.0);
    }

    #[test]
    fn test_parse_whitespace() {
        let val = json_parse("  { \"a\" : 1 , \"b\" : [ 2 , 3 ] }  ").unwrap();
        assert!(matches!(val, JsonVal::Object(_)));
    }

    #[test]
    fn test_json_val_to_json_string_roundtrip() {
        let val = JsonVal::Object(vec![
            ("name".into(), JsonVal::Str("test".into())),
            ("count".into(), JsonVal::Number(42.0)),
            ("items".into(), JsonVal::Array(vec![
                JsonVal::Number(1.0),
                JsonVal::Null,
                JsonVal::Bool(true),
            ])),
        ]);
        let s = val.to_json_string();
        let parsed = json_parse(&s).unwrap();
        assert_eq!(parsed, val);
    }
}
