//! Token-aware WSON boundary shared by manifests and lockfiles.
//! Legacy strings retain their backslashes; only explicitly versioned input
//! decodes JSON escapes. The upstream parser is used for scalar literals only.
pub use wson_rs::{WsonMap, WsonValue};

pub fn loads(raw: &str, version_key: &str, escaped_version: i64) -> Result<WsonMap, String> {
    let mut parser = Parser {
        raw,
        pos: 0,
        depth: 0,
        escapes: None,
    };
    let map = parser.document()?;
    let escaped = matches!(map.get(version_key), Some(WsonValue::Int(v)) if *v == escaped_version);
    parser.pos = 0;
    parser.escapes = Some(escaped);
    parser.document()
}

pub fn quote(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

struct Parser<'a> {
    raw: &'a str,
    pos: usize,
    depth: usize,
    escapes: Option<bool>,
}
impl Parser<'_> {
    fn document(&mut self) -> Result<WsonMap, String> {
        let value = self.value()?;
        self.space()?;
        if self.pos != self.raw.len() {
            return Err(self.error("unexpected trailing input"));
        }
        let WsonValue::Object(map) = value else {
            return Err(self.error("WSON root must be an object"));
        };
        Ok(map)
    }
    fn error(&self, message: &str) -> String {
        let prefix = &self.raw[..self.pos];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        format!("{message} (line {line}, column {column})")
    }
    fn peek(&self) -> Option<u8> {
        self.raw.as_bytes().get(self.pos).copied()
    }
    fn space(&mut self) -> Result<(), String> {
        loop {
            while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
                self.pos += 1;
            }
            let rest = &self.raw[self.pos..];
            if rest.starts_with("//") || rest.starts_with('#') {
                self.pos += rest.find('\n').unwrap_or(rest.len());
            } else if rest.starts_with("/*") {
                let end = rest
                    .find("*/")
                    .ok_or_else(|| self.error("unterminated comment"))?;
                self.pos += end + 2;
            } else {
                return Ok(());
            }
        }
    }
    fn take(&mut self, byte: u8) -> Result<(), String> {
        self.space()?;
        if self.peek() != Some(byte) {
            return Err(self.error(&format!("expected `{}`", byte as char)));
        }
        self.pos += 1;
        Ok(())
    }
    fn value(&mut self) -> Result<WsonValue, String> {
        self.space()?;
        if self.depth >= 128 {
            return Err(self.error("WSON nesting exceeds 128 levels"));
        }
        self.depth += 1;
        let result = self.inner();
        self.depth -= 1;
        result
    }
    fn inner(&mut self) -> Result<WsonValue, String> {
        match self.peek() {
            Some(b'"') => {
                self.pos += 1;
                let start = self.pos;
                let mut escaped = false;
                while let Some(b) = self.peek() {
                    if b == b'"' && !escaped {
                        let text = &self.raw[start..self.pos];
                        let value = if self.escapes == Some(true) {
                            serde_json::from_str(&self.raw[start - 1..=self.pos]).map_err(|e| {
                                self.error(&format!("invalid versioned string escape: {e}"))
                            })?
                        } else {
                            if self.escapes == Some(false) && text.contains(['\n', '\r']) {
                                return Err(self.error("ambiguous multiline legacy string; use an explicitly versioned escaped string"));
                            }
                            text.to_owned()
                        };
                        self.pos += 1;
                        return Ok(WsonValue::String(value));
                    }
                    escaped = b == b'\\' && !escaped;
                    self.pos += self.raw[self.pos..].chars().next().unwrap().len_utf8();
                }
                Err(self.error("unterminated or ambiguous string; use a versioned escaped string"))
            }
            Some(b'{') => {
                self.pos += 1;
                let mut map = WsonMap::new();
                loop {
                    self.space()?;
                    if self.peek() == Some(b'}') {
                        self.pos += 1;
                        return Ok(WsonValue::Object(map));
                    }
                    let start = self.pos;
                    while self
                        .peek()
                        .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                    {
                        self.pos += 1;
                    }
                    if start == self.pos {
                        return Err(self.error("expected field name"));
                    }
                    let key = self.raw[start..self.pos].to_string();
                    self.space()?;
                    if !matches!(self.peek(), Some(b'=') | Some(b':')) {
                        return Err(self.error("expected `=` or `:`"));
                    }
                    self.pos += 1;
                    let value = self.value()?;
                    if map.insert(key.clone(), value).is_some() {
                        return Err(self.error(&format!("duplicate field `{key}`")));
                    }
                    self.space()?;
                    if self.peek() != Some(b'}') {
                        self.take(b',')?;
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut values = Vec::new();
                loop {
                    self.space()?;
                    if self.peek() == Some(b']') {
                        self.pos += 1;
                        return Ok(WsonValue::Array(values));
                    }
                    values.push(self.value()?);
                    self.space()?;
                    if self.peek() != Some(b']') {
                        self.take(b',')?;
                    }
                }
            }
            _ => {
                let start = self.pos;
                while self.peek().is_some_and(|b| {
                    !b.is_ascii_whitespace() && !matches!(b, b',' | b']' | b'}' | b'#' | b'/')
                }) {
                    self.pos += self.raw[self.pos..].chars().next().unwrap().len_utf8();
                }
                if start == self.pos {
                    return Err(self.error("expected value"));
                }
                let atom = &self.raw[start..self.pos];
                let mut result = wson_rs::loads(&format!("{{value={atom}}}"))
                    .map_err(|_| self.error(&format!("invalid scalar `{atom}`")))?;
                result
                    .remove("value")
                    .ok_or_else(|| self.error("missing scalar"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn string_contents_do_not_become_syntax() {
        for text in [
            "https://host/a#b",
            "a,b={x}[y]/*hi*/",
            "__VEX_WSON_URL_SEPARATOR__",
            "__VEX_LOCK_URL_SEPARATOR__",
            "안녕",
        ] {
            let input = format!("{{format=2, value={}}}", quote(text));
            assert!(
                matches!(&loads(&input, "format", 2).unwrap()["value"], WsonValue::String(s) if s == text)
            );
        }
    }
    #[test]
    fn escapes_are_explicitly_versioned() {
        let legacy = loads(r#"{value="C:\tmp\new"}"#, "format", 2).unwrap();
        assert!(matches!(&legacy["value"], WsonValue::String(s) if s == r"C:\tmp\new"));
        for value in ["quote\"slash\\", "\t\n\r", "😀", "\0", r"C:\tmp\new"] {
            let input = format!("{{value={}, format=2}}", quote(value));
            assert!(
                matches!(&loads(&input, "format", 2).unwrap()["value"], WsonValue::String(s) if s == value)
            );
        }
        assert!(loads(r#"{format=2, value="\q"}"#, "format", 2).is_err());
        assert!(loads("{value=1,value=2}", "format", 2)
            .unwrap_err()
            .contains("duplicate"));
    }
}
