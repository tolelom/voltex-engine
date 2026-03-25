/// Minimal JSON parser for glTF. No external dependencies.

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),  // preserve order
}

impl JsonValue {
    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self { JsonValue::Object(v) => Some(v), _ => None }
    }
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self { JsonValue::Array(v) => Some(v), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { JsonValue::String(s) => Some(s), _ => None }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self { JsonValue::Number(n) => Some(*n), _ => None }
    }
    pub fn as_u32(&self) -> Option<u32> {
        self.as_f64().map(|n| n as u32)
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self { JsonValue::Bool(b) => Some(*b), _ => None }
    }
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.as_object()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn index(&self, i: usize) -> Option<&JsonValue> {
        self.as_array()?.get(i)
    }
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(input);
    let val = parser.parse_value()?;
    Ok(val)
}

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input: input.as_bytes(), pos: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Result<u8, String> {
        if self.pos >= self.input.len() {
            return Err("Unexpected end of JSON".into());
        }
        let b = self.input[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn expect(&mut self, ch: u8) -> Result<(), String> {
        let b = self.advance()?;
        if b != ch {
            return Err(format!("Expected '{}', got '{}'", ch as char, b as char));
        }
        Ok(())
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') => self.parse_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal("null", JsonValue::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(ch) => Err(format!("Unexpected character: '{}'", ch as char)),
            None => Err("Unexpected end of JSON".into()),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            let b = self.advance()?;
            match b {
                b'"' => return Ok(s),
                b'\\' => {
                    let esc = self.advance()?;
                    match esc {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{08}'),
                        b'f' => s.push('\u{0C}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                hex.push(self.advance()? as char);
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| format!("Invalid unicode escape: {}", hex))?;
                            if let Some(ch) = char::from_u32(code) {
                                s.push(ch);
                            }
                        }
                        _ => return Err(format!("Invalid escape: \\{}", esc as char)),
                    }
                }
                _ => s.push(b as char),
            }
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') { self.pos += 1; }
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.input.len() && self.input[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < self.input.len() && (self.input[self.pos] == b'e' || self.input[self.pos] == b'E') {
            self.pos += 1;
            if self.pos < self.input.len() && (self.input[self.pos] == b'+' || self.input[self.pos] == b'-') {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| "Invalid UTF-8 in number".to_string())?;
        let n: f64 = s.parse().map_err(|_| format!("Invalid number: {}", s))?;
        Ok(JsonValue::Number(n))
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        self.skip_whitespace();
        let mut pairs = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            let val = self.parse_value()?;
            pairs.push((key, val));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => { self.pos += 1; }
                Some(b'}') => { self.pos += 1; return Ok(JsonValue::Object(pairs)); }
                _ => return Err("Expected ',' or '}' in object".into()),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let val = self.parse_value()?;
            items.push(val);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => { self.pos += 1; }
                Some(b']') => { self.pos += 1; return Ok(JsonValue::Array(items)); }
                _ => return Err("Expected ',' or ']' in array".into()),
            }
        }
    }

    fn parse_literal(&mut self, expected: &str, value: JsonValue) -> Result<JsonValue, String> {
        for &b in expected.as_bytes() {
            let actual = self.advance()?;
            if actual != b {
                return Err(format!("Expected '{}', got '{}'", b as char, actual as char));
            }
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() {
        assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn test_parse_number() {
        match parse_json("42").unwrap() {
            JsonValue::Number(n) => assert!((n - 42.0).abs() < 1e-10),
            other => panic!("Expected Number, got {:?}", other),
        }
        match parse_json("-3.14").unwrap() {
            JsonValue::Number(n) => assert!((n - (-3.14)).abs() < 1e-10),
            other => panic!("Expected Number, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(parse_json("\"hello\"").unwrap(), JsonValue::String("hello".into()));
    }

    #[test]
    fn test_parse_string_escapes() {
        assert_eq!(
            parse_json(r#""hello\nworld""#).unwrap(),
            JsonValue::String("hello\nworld".into())
        );
    }

    #[test]
    fn test_parse_array() {
        let val = parse_json("[1, 2, 3]").unwrap();
        match val {
            JsonValue::Array(arr) => assert_eq!(arr.len(), 3),
            other => panic!("Expected Array, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_object() {
        let val = parse_json(r#"{"name": "test", "value": 42}"#).unwrap();
        match val {
            JsonValue::Object(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map[0].0, "name");
            }
            other => panic!("Expected Object, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_nested() {
        let json = r#"{"meshes": [{"name": "Cube", "primitives": [{"attributes": {"POSITION": 0}}]}]}"#;
        let val = parse_json(json).unwrap();
        assert!(matches!(val, JsonValue::Object(_)));
    }

    #[test]
    fn test_parse_empty_array() {
        assert_eq!(parse_json("[]").unwrap(), JsonValue::Array(vec![]));
    }

    #[test]
    fn test_parse_empty_object() {
        assert_eq!(parse_json("{}").unwrap(), JsonValue::Object(vec![]));
    }
}
