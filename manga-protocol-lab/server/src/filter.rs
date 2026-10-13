//! Parameterized PostgreSQL filter grammar. Unknown remains SQL NULL.
#[derive(Clone, Debug)]
pub enum Value {
    Integer(i64),
    Text(String),
    Bytes(Vec<u8>),
}
impl Value {
    pub fn parameter(&self) -> &(dyn tokio_postgres::types::ToSql + Sync) {
        match self {
            Self::Integer(v) => v,
            Self::Text(v) => v,
            Self::Bytes(v) => v,
        }
    }
}
#[derive(Clone, Debug)]
enum Token {
    Word(String),
    Text(String),
    Num(i64),
    Op(String),
    Open,
    Close,
}

// RFC 4647 section 2.1: a basic range is not necessarily a complete BCP 47 tag.
pub(crate) fn basic_language_range(text: &str) -> bool {
    if text == "*" {
        return true;
    }
    let mut parts = text.split('-');
    let first = parts.next().unwrap_or("");
    (1..=8).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && parts.all(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

pub struct Filter {
    tokens: Vec<Token>,
    at: usize,
    pub args: Vec<Value>,
}
impl Filter {
    pub fn new(s: &str) -> Result<Self, String> {
        if s.len() > crate::protocol_limits::FILTER_BYTES {
            return Err("filter length budget".into());
        }
        let b = s.as_bytes();
        let mut i = 0;
        let mut out = Vec::new();
        while i < b.len() {
            if b[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }
            let start = i;
            match b[i] {
                b'(' => {
                    out.push(Token::Open);
                    i += 1
                }
                b')' => {
                    out.push(Token::Close);
                    i += 1
                }
                b'"' => {
                    i += 1;
                    while i < b.len() {
                        if b[i] == b'\\' {
                            i += 2;
                            continue;
                        }
                        if b[i] == b'"' {
                            break;
                        }
                        i += 1
                    }
                    if i >= b.len() {
                        return Err("unclosed string".into());
                    }
                    i += 1;
                    out.push(Token::Text(
                        serde_json::from_str(&s[start..i]).map_err(|_| "string literal")?,
                    ));
                }
                b'=' | b'!' | b'>' | b'<' => {
                    i += 1;
                    if i < b.len() && b[i] == b'=' {
                        i += 1
                    }
                    let op = &s[start..i];
                    if !["=", "!=", ">", "<", ">=", "<="].contains(&op) {
                        return Err("operator".into());
                    }
                    out.push(Token::Op(op.into()));
                }
                b'-' | b'0'..=b'9' => {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1
                    }
                    out.push(Token::Num(s[start..i].parse().map_err(|_| "integer")?));
                }
                _ if b[i].is_ascii_alphabetic() || b[i] == b'_' => {
                    i += 1;
                    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                        i += 1
                    }
                    out.push(Token::Word(s[start..i].into()));
                }
                _ => return Err("filter syntax".into()),
            }
            if out.len() > crate::protocol_limits::FILTER_TOKENS {
                return Err("filter token budget".into());
            }
        }
        Ok(Self {
            tokens: out,
            at: 0,
            args: Vec::new(),
        })
    }
    fn take(&mut self) -> Result<Token, String> {
        let v = self.tokens.get(self.at).cloned().ok_or("unexpected end")?;
        self.at += 1;
        Ok(v)
    }
    fn word(&self, w: &str) -> bool {
        matches!(self.tokens.get(self.at),Some(Token::Word(s)) if s==w)
    }
    fn arg(&mut self, v: Value) -> String {
        self.args.push(v);
        format!("${}", self.args.len())
    }
    fn field(s: &str) -> Result<&str, String> {
        if crate::protocol_limits::FILTER_FIELDS.contains(&s) {
            Ok(s)
        } else {
            Err("unsupported field".into())
        }
    }
    fn atom(&mut self, depth: usize) -> Result<String, String> {
        if depth > crate::protocol_limits::FILTER_DEPTH {
            return Err("filter nesting budget".into());
        }
        if self.word("NOT") {
            self.at += 1;
            return Ok(format!("(NOT {})", self.atom(depth + 1)?));
        }
        match self.take()? {
            Token::Open => {
                let v = self.or(depth + 1)?;
                if !matches!(self.take()?, Token::Close) {
                    return Err("expected closing parenthesis".into());
                }
                Ok(v)
            }
            Token::Word(w) if w == "exists" || w == "missing" => {
                if !matches!(self.take()?, Token::Open) {
                    return Err("expected (".into());
                }
                let field = match self.take()? {
                    Token::Word(f) => f,
                    _ => return Err("field".into()),
                };
                Self::field(&field)?;
                if !matches!(self.take()?, Token::Close) {
                    return Err("expected )".into());
                }
                Ok(format!(
                    "({field} IS {}NULL)",
                    if w == "exists" { "NOT " } else { "" }
                ))
            }
            Token::Word(field) => {
                Self::field(&field)?;
                let op = match self.take()? {
                    Token::Op(s) => s,
                    _ => return Err("comparison operator".into()),
                };
                let numeric = ["pages", "size", "created_at"].contains(&field.as_str());
                let value = self.take()?;
                let sql = if numeric {
                    let n = match value {
                        Token::Num(n) => n,
                        _ => return Err("integer comparison required".into()),
                    };
                    format!("{field}{op}{}", self.arg(Value::Integer(n)))
                } else {
                    let text = match value {
                        Token::Text(s) => s,
                        _ => return Err("text comparison required".into()),
                    };
                    if op != "=" && op != "!=" {
                        return Err("text ordering excluded".into());
                    }
                    if field == "language" {
                        if !basic_language_range(&text) {
                            return Err("language range".into());
                        }
                        let cmp = if text == "*" {
                            "CASE WHEN language IS NULL THEN NULL ELSE TRUE END".into()
                        } else {
                            let a = self.arg(Value::Text(text.to_ascii_lowercase()));
                            let b =
                                self.arg(Value::Text(format!("{}-%", text.to_ascii_lowercase())));
                            format!(
                                "(lower(language COLLATE \"C\")={a} OR lower(language COLLATE \"C\") LIKE {b})"
                            )
                        };
                        if op == "!=" {
                            format!("NOT ({cmp})")
                        } else {
                            cmp
                        }
                    } else if field == "series" || field == "publisher" {
                        if crate::auth::valid_hash(&text) {
                            format!(
                                "{field}{op}{}",
                                self.arg(Value::Bytes(
                                    hex::decode(text).map_err(|_| "identifier")?
                                ))
                            )
                        } else {
                            format!(
                                "CASE WHEN {field} IS NULL THEN NULL ELSE {} END",
                                if op == "=" { "FALSE" } else { "TRUE" }
                            )
                        }
                    } else {
                        format!("{field} COLLATE \"C\"{op}{}", self.arg(Value::Text(text)))
                    }
                };
                Ok(format!("({sql})"))
            }
            _ => Err("filter atom".into()),
        }
    }
    fn and(&mut self, d: usize) -> Result<String, String> {
        let mut s = self.atom(d)?;
        while self.word("AND") {
            self.at += 1;
            s = format!("({s} AND {})", self.atom(d)?)
        }
        Ok(s)
    }
    fn or(&mut self, d: usize) -> Result<String, String> {
        let mut s = self.and(d)?;
        while self.word("OR") {
            self.at += 1;
            s = format!("({s} OR {})", self.and(d)?)
        }
        Ok(s)
    }
    pub fn compile(mut self) -> Result<(String, Vec<Value>), String> {
        if self.tokens.is_empty() {
            return Ok(("TRUE".into(), vec![]));
        }
        let s = self.or(0)?;
        if self.at != self.tokens.len() {
            return Err("trailing filter tokens".into());
        }
        Ok((s, self.args))
    }
}
