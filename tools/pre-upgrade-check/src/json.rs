//! Bounded JSON reader; keeping offline builds independent of downloaded crates.
use crate::runtime::Result;
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Value {
    Null,
    Bool,
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}
impl Value {
    pub fn field(&self, key: &str) -> Result<&Value> {
        match self {
            Self::Object(m) => m
                .get(key)
                .ok_or_else(|| format!("JSON field missing: {key}")),
            _ => Err("Expected JSON object".into()),
        }
    }
    pub fn string(&self) -> Result<&str> {
        match self {
            Self::String(s) => Ok(s),
            _ => Err("Expected JSON string".into()),
        }
    }
    pub fn array(&self) -> Result<&[Value]> {
        match self {
            Self::Array(a) => Ok(a),
            _ => Err("Expected JSON array".into()),
        }
    }
    pub fn optional_string(&self) -> Result<Option<&str>> {
        match self {
            Self::Null => Ok(None),
            _ => self.string().map(Some),
        }
    }
    pub fn integer(&self) -> Result<u64> {
        match self {
            Self::Number(n) => n
                .parse()
                .map_err(|_| "Expected JSON unsigned integer".into()),
            _ => Err("Expected JSON number".into()),
        }
    }
}
struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self.s.get(self.pos).is_some_and(|b| b" \t\r\n".contains(b)) {
            self.pos += 1;
        }
    }
    fn eat(&mut self, b: u8) -> bool {
        if self.s.get(self.pos) == Some(&b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn need(&mut self, b: u8) -> Result<()> {
        if self.eat(b) {
            Ok(())
        } else {
            Err(format!("Invalid/truncated JSON near byte {}", self.pos))
        }
    }
    fn hex(&mut self) -> Result<u32> {
        let chunk = self
            .s
            .get(self.pos..self.pos + 4)
            .ok_or("Truncated unicode escape")?;
        let text = std::str::from_utf8(chunk).map_err(|_| "Invalid unicode escape")?;
        let n = u32::from_str_radix(text, 16).map_err(|_| "Invalid unicode escape")?;
        self.pos += 4;
        Ok(n)
    }
    fn string(&mut self) -> Result<String> {
        self.need(b'"')?;
        let mut out = Vec::new();
        loop {
            let b = *self.s.get(self.pos).ok_or("Unclosed JSON string")?;
            self.pos += 1;
            match b {
                b'"' => return String::from_utf8(out).map_err(|_| "Invalid UTF-8 in JSON".into()),
                0..=31 => return Err("Unescaped control character in JSON".into()),
                b'\\' => {
                    let b = *self.s.get(self.pos).ok_or("Truncated JSON escape")?;
                    self.pos += 1;
                    match b {
                        b'"' | b'\\' | b'/' => out.push(b),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'u' => {
                            let mut n = self.hex()?;
                            if (0xd800..=0xdbff).contains(&n) {
                                self.need(b'\\')?;
                                self.need(b'u')?;
                                let low = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err("Invalid unicode surrogate pair".into());
                                }
                                n = 0x10000 + ((n - 0xd800) << 10) + low - 0xdc00;
                            }
                            let ch = char::from_u32(n).ok_or("Invalid unicode code point")?;
                            let mut buf = [0; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err("Invalid JSON escape".into()),
                    }
                }
                _ => out.push(b),
            }
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        self.space();
        self.nodes += 1;
        if depth > 64 || self.nodes > 250_000 {
            return Err("JSON complexity limit exceeded".into());
        }
        match self.s.get(self.pos).copied() {
            Some(b'"') => self.string().map(Value::String),
            Some(b'[') => {
                self.pos += 1;
                self.space();
                let mut out = Vec::new();
                if self.eat(b']') {
                    return Ok(Value::Array(out));
                }
                loop {
                    out.push(self.value(depth + 1)?);
                    self.space();
                    if self.eat(b']') {
                        return Ok(Value::Array(out));
                    }
                    self.need(b',')?;
                }
            }
            Some(b'{') => {
                self.pos += 1;
                self.space();
                let mut out = BTreeMap::new();
                if self.eat(b'}') {
                    return Ok(Value::Object(out));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    self.space();
                    self.need(b':')?;
                    if out.insert(key, self.value(depth + 1)?).is_some() {
                        return Err("Duplicate JSON object key".into());
                    }
                    self.space();
                    if self.eat(b'}') {
                        return Ok(Value::Object(out));
                    }
                    self.need(b',')?;
                }
            }
            Some(b'n') if self.s[self.pos..].starts_with(b"null") => {
                self.pos += 4;
                Ok(Value::Null)
            }
            Some(b't') if self.s[self.pos..].starts_with(b"true") => {
                self.pos += 4;
                Ok(Value::Bool)
            }
            Some(b'f') if self.s[self.pos..].starts_with(b"false") => {
                self.pos += 5;
                Ok(Value::Bool)
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.pos;
                self.eat(b'-');
                if !self.eat(b'0') {
                    if !self
                        .s
                        .get(self.pos)
                        .is_some_and(|b| (b'1'..=b'9').contains(b))
                    {
                        return Err("Invalid JSON number".into());
                    }
                    while self.s.get(self.pos).is_some_and(u8::is_ascii_digit) {
                        self.pos += 1;
                    }
                }
                if self.eat(b'.') {
                    self.digits()?;
                }
                if self.eat(b'e') || self.eat(b'E') {
                    if !self.eat(b'+') {
                        self.eat(b'-');
                    }
                    self.digits()?;
                }
                Ok(Value::Number(
                    std::str::from_utf8(&self.s[start..self.pos])
                        .unwrap()
                        .into(),
                ))
            }
            _ => Err("Response is not valid JSON (possibly a login/challenge page)".into()),
        }
    }
    fn digits(&mut self) -> Result<()> {
        let start = self.pos;
        while self.s.get(self.pos).is_some_and(u8::is_ascii_digit) {
            self.pos += 1;
        }
        if self.pos == start {
            Err("Invalid JSON number".into())
        } else {
            Ok(())
        }
    }
}
pub fn parse(s: &str) -> Result<Value> {
    if s.len() > 8 * 1024 * 1024 {
        return Err("JSON exceeds 8 MiB limit".into());
    }
    let mut p = Parser {
        s: s.as_bytes(),
        pos: 0,
        nodes: 0,
    };
    let v = p.value(0)?;
    p.space();
    if p.pos != p.s.len() {
        return Err("Trailing data after JSON document".into());
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_strings_unicode_numbers_and_rejection() {
        let v = parse(r#"{"s":"\uD83E\uDD80\n\"","n":12,"a":[true,null,-1.25e+2]}"#).unwrap();
        assert_eq!(v.field("s").unwrap().string().unwrap(), "🦀\n\"");
        assert_eq!(v.field("n").unwrap().integer().unwrap(), 12);
        for s in [
            "[1,]",
            "{\"a\":1,\"a\":2}",
            "[01]",
            "[1.]",
            "[1e]",
            "[",
            "{} garbage",
            r#""\uD800""#,
            "<html/>",
        ] {
            assert!(parse(s).is_err(), "{s}");
        }
    }
}
