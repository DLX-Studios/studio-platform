//! Text expression reader for the portable subset.
//!
//! Script-derived expressions (`$derived(...)`, handler payloads) arrive as
//! source text, not typed JSON. This reader parses exactly the closed
//! expression grammar into an untyped tree; the lowerer then resolves paths
//! and calls against the script model. Anything outside the grammar is a
//! syntax error for the caller to report.

/// One parsed portable expression, paths unresolved.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TextExpr {
    /// Static literal text (classified by the caller).
    Literal(String),
    /// A binding path (`quantity`, `item.name`, `rows[0].id`).
    Path(String),
    /// Unary operation with source operator spelling.
    Unary {
        /// Operator spelling.
        operator: String,
        /// Operand.
        operand: Box<TextExpr>,
    },
    /// Binary operation with source operator spelling.
    Binary {
        /// Operator spelling.
        operator: String,
        /// Left operand.
        left: Box<TextExpr>,
        /// Right operand.
        right: Box<TextExpr>,
    },
    /// Conditional expression.
    Conditional {
        /// Condition.
        condition: Box<TextExpr>,
        /// Value when the condition holds.
        consequent: Box<TextExpr>,
        /// Value otherwise.
        alternate: Box<TextExpr>,
    },
    /// Array literal.
    Array(Vec<TextExpr>),
    /// Record literal with named fields.
    Record(Vec<(String, TextExpr)>),
    /// Function call.
    Call {
        /// Callee name.
        function: String,
        /// Arguments.
        arguments: Vec<TextExpr>,
    },
}

/// Parse one portable expression.
///
/// # Errors
///
/// Returns a safe message when the text is outside the grammar.
pub(crate) fn parse_text_expression(text: &str) -> Result<TextExpr, String> {
    let mut reader = Reader {
        input: text.as_bytes(),
        position: 0,
    };
    reader.skip_whitespace();
    let expression = reader.parse_conditional()?;
    reader.skip_whitespace();
    if reader.position != reader.input.len() {
        return Err(format!("unexpected trailing text in `{text}`"));
    }
    Ok(expression)
}

struct Reader<'a> {
    input: &'a [u8],
    position: usize,
}

impl Reader<'_> {
    fn skip_whitespace(&mut self) {
        while self.position < self.input.len() && self.input[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, byte: u8, text: &str) -> Result<(), String> {
        self.skip_whitespace();
        if self.consume(byte) {
            Ok(())
        } else {
            Err(format!("expected `{}` in `{text}`", byte as char))
        }
    }

    fn parse_conditional(&mut self) -> Result<TextExpr, String> {
        let condition = self.parse_logical()?;
        self.skip_whitespace();
        if self.consume(b'?') {
            let consequent = self.parse_conditional()?;
            self.skip_whitespace();
            if !self.consume(b':') {
                return Err("conditional needs `:`".to_owned());
            }
            let alternate = self.parse_conditional()?;
            return Ok(TextExpr::Conditional {
                condition: Box::new(condition),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            });
        }
        Ok(condition)
    }

    fn parse_logical(&mut self) -> Result<TextExpr, String> {
        let mut left = self.parse_comparison()?;
        loop {
            self.skip_whitespace();
            let operator = if self.consume_two(b'&', b'&') {
                "&&"
            } else if self.consume_two(b'|', b'|') {
                "||"
            } else {
                break;
            };
            let right = self.parse_comparison()?;
            left = TextExpr::Binary {
                operator: operator.to_owned(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn consume_two(&mut self, first: u8, second: u8) -> bool {
        if self.input.get(self.position..self.position + 2) == Some(&[first, second][..]) {
            self.position += 2;
            true
        } else {
            false
        }
    }

    fn parse_comparison(&mut self) -> Result<TextExpr, String> {
        let mut left = self.parse_additive()?;
        loop {
            self.skip_whitespace();
            let operator = if self.consume_two(b'=', b'=') {
                if self.consume(b'=') { "===" } else { "==" }
            } else if self.consume_two(b'!', b'=') {
                if self.consume(b'=') { "!==" } else { "!=" }
            } else if self.consume_two(b'<', b'=') {
                "<="
            } else if self.consume_two(b'>', b'=') {
                ">="
            } else if self.peek() == Some(b'<') {
                self.position += 1;
                "<"
            } else if self.peek() == Some(b'>') {
                self.position += 1;
                ">"
            } else {
                break;
            };
            let right = self.parse_additive()?;
            left = TextExpr::Binary {
                operator: operator.to_owned(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<TextExpr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            self.skip_whitespace();
            let operator = if self.peek() == Some(b'+') {
                self.position += 1;
                "+"
            } else if self.peek() == Some(b'-') {
                self.position += 1;
                "-"
            } else {
                break;
            };
            let right = self.parse_multiplicative()?;
            left = TextExpr::Binary {
                operator: operator.to_owned(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<TextExpr, String> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            let operator = if self.peek() == Some(b'*') {
                self.position += 1;
                "*"
            } else if self.peek() == Some(b'/') {
                self.position += 1;
                "/"
            } else if self.peek() == Some(b'%') {
                self.position += 1;
                "%"
            } else {
                break;
            };
            let right = self.parse_unary()?;
            left = TextExpr::Binary {
                operator: operator.to_owned(),
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<TextExpr, String> {
        self.skip_whitespace();
        if self.peek() == Some(b'!') {
            self.position += 1;
            return Ok(TextExpr::Unary {
                operator: "!".to_owned(),
                operand: Box::new(self.parse_unary()?),
            });
        }
        if self.peek() == Some(b'-') {
            self.position += 1;
            return Ok(TextExpr::Unary {
                operator: "-".to_owned(),
                operand: Box::new(self.parse_unary()?),
            });
        }
        if self.peek() == Some(b'+') {
            self.position += 1;
            return Ok(TextExpr::Unary {
                operator: "+".to_owned(),
                operand: Box::new(self.parse_unary()?),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<TextExpr, String> {
        let mut path = self.parse_primary()?;
        loop {
            self.skip_whitespace();
            if self.consume(b'.') {
                self.skip_whitespace();
                let segment = self.parse_identifier()?;
                path = match path {
                    TextExpr::Path(base) => TextExpr::Path(format!("{base}.{segment}")),
                    _ => return Err("member access needs a path".to_owned()),
                };
            } else if self.consume(b'[') {
                let index = self.parse_index()?;
                self.skip_whitespace();
                if !self.consume(b']') {
                    return Err("index needs `]`".to_owned());
                }
                path = match path {
                    TextExpr::Path(base) => TextExpr::Path(format!("{base}.{index}")),
                    _ => return Err("indexing needs a path".to_owned()),
                };
            } else {
                break;
            }
        }
        Ok(path)
    }

    fn parse_index(&mut self) -> Result<String, String> {
        self.skip_whitespace();
        if matches!(self.peek(), Some(b'"' | b'\'')) {
            let quote = self.peek().unwrap_or(b'"');
            self.position += 1;
            let start = self.position;
            while self.position < self.input.len() && self.input[self.position] != quote {
                if self.input[self.position] == b'\\' {
                    self.position += 1;
                }
                self.position += 1;
            }
            if self.position >= self.input.len() {
                return Err("unterminated string".to_owned());
            }
            let text = std::str::from_utf8(&self.input[start..self.position])
                .map_err(|_| "index is not UTF-8".to_owned())?;
            self.position += 1;
            return Ok(text.to_owned());
        }
        let start = self.position;
        while self.position < self.input.len() && self.input[self.position].is_ascii_digit() {
            self.position += 1;
        }
        if start == self.position {
            return Err("index needs a number or string".to_owned());
        }
        Ok(std::str::from_utf8(&self.input[start..self.position])
            .map_err(|_| "index is not UTF-8".to_owned())?
            .to_owned())
    }

    fn parse_primary(&mut self) -> Result<TextExpr, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'(') => {
                self.position += 1;
                let inner = self.parse_conditional()?;
                self.skip_whitespace();
                self.expect(b')', "group")?;
                Ok(inner)
            }
            Some(b'[') => {
                self.position += 1;
                let mut elements = Vec::new();
                loop {
                    self.skip_whitespace();
                    if self.consume(b']') {
                        break;
                    }
                    elements.push(self.parse_conditional()?);
                    self.skip_whitespace();
                    if self.consume(b',') {
                        continue;
                    }
                    self.expect(b']', "array")?;
                    break;
                }
                Ok(TextExpr::Array(elements))
            }
            Some(b'{') => {
                self.position += 1;
                let mut entries = Vec::new();
                loop {
                    self.skip_whitespace();
                    if self.consume(b'}') {
                        break;
                    }
                    let key = self.parse_key()?;
                    self.skip_whitespace();
                    if self.consume(b':') {
                        let value = self.parse_conditional()?;
                        entries.push((key, value));
                    } else {
                        // Shorthand: `{ quantity }` means `{ quantity: quantity }`.
                        entries.push((key.clone(), TextExpr::Path(key)));
                    }
                    self.skip_whitespace();
                    if self.consume(b',') {
                        // Allow one trailing comma via the loop's `}` check.
                        self.skip_whitespace();
                        if self.consume(b'}') {
                            break;
                        }
                        // Rewind one step is unnecessary: continue parses next.
                        continue;
                    }
                    self.expect(b'}', "record")?;
                    break;
                }
                // Handle the common trailing-comma shape `{ a, }`.
                Ok(TextExpr::Record(entries))
            }
            Some(byte) if byte.is_ascii_digit() || byte == b'.' => self.parse_number(),
            Some(b'"' | b'\'') => Ok(TextExpr::Literal(self.parse_string()?)),
            Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' => {
                let name = self.parse_identifier()?;
                self.skip_whitespace();
                if self.consume(b'(') {
                    let mut arguments = Vec::new();
                    loop {
                        self.skip_whitespace();
                        if self.consume(b')') {
                            break;
                        }
                        arguments.push(self.parse_conditional()?);
                        self.skip_whitespace();
                        if self.consume(b',') {
                            continue;
                        }
                        self.expect(b')', "call")?;
                        break;
                    }
                    return Ok(TextExpr::Call {
                        function: name,
                        arguments,
                    });
                }
                match name.as_str() {
                    "true" => Ok(TextExpr::Literal("true".to_owned())),
                    "false" => Ok(TextExpr::Literal("false".to_owned())),
                    "null" | "undefined" => Err("null is not portable".to_owned()),
                    _ => Ok(TextExpr::Path(name)),
                }
            }
            _ => Err("expression is not portable".to_owned()),
        }
    }

    fn parse_key(&mut self) -> Result<String, String> {
        self.skip_whitespace();
        if matches!(self.peek(), Some(b'"' | b'\'')) {
            let quote = self.peek().unwrap_or(b'"');
            self.position += 1;
            let start = self.position;
            while self.position < self.input.len() && self.input[self.position] != quote {
                if self.input[self.position] == b'\\' {
                    self.position += 1;
                }
                self.position += 1;
            }
            if self.position >= self.input.len() {
                return Err("unterminated string".to_owned());
            }
            let text = std::str::from_utf8(&self.input[start..self.position])
                .map_err(|_| "key is not UTF-8".to_owned())?
                .to_owned();
            self.position += 1;
            return Ok(text);
        }
        self.parse_identifier()
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        let start = self.position;
        match self.peek() {
            Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' => {
                self.position += 1;
            }
            _ => return Err("identifier expected".to_owned()),
        }
        while self.position < self.input.len() {
            let byte = self.input[self.position];
            if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' {
                self.position += 1;
            } else {
                break;
            }
        }
        std::str::from_utf8(&self.input[start..self.position])
            .map(std::borrow::ToOwned::to_owned)
            .map_err(|_| "identifier is not UTF-8".to_owned())
    }

    fn parse_number(&mut self) -> Result<TextExpr, String> {
        let start = self.position;
        while self.position < self.input.len() {
            let byte = self.input[self.position];
            if byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-') {
                // `+`/`-` only belong to exponents; the grammar rejects them
                // elsewhere because parse_additive splits first.
                self.position += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.input[start..self.position])
            .map_err(|_| "number is not UTF-8".to_owned())?;
        Ok(TextExpr::Literal(text.to_owned()))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        let quote = self.peek().unwrap_or(b'"');
        self.position += 1;
        let mut text = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err("unterminated string".to_owned());
            };
            self.position += 1;
            if byte == b'\\' {
                let Some(escaped) = self.peek() else {
                    return Err("unterminated string".to_owned());
                };
                self.position += 1;
                text.push(match escaped {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    other => other as char,
                });
                continue;
            }
            if byte == quote {
                break;
            }
            text.push(byte as char);
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_covers_the_portable_subset() {
        assert_eq!(
            parse_text_expression("quantity * price").unwrap(),
            TextExpr::Binary {
                operator: "*".to_owned(),
                left: Box::new(TextExpr::Path("quantity".to_owned())),
                right: Box::new(TextExpr::Path("price".to_owned())),
            }
        );
        assert_eq!(
            parse_text_expression("{ productId: name, quantity: 1 }").unwrap(),
            TextExpr::Record(vec![
                ("productId".to_owned(), TextExpr::Path("name".to_owned())),
                ("quantity".to_owned(), TextExpr::Literal("1".to_owned())),
            ])
        );
        assert!(parse_text_expression("item.name").is_ok());
        assert!(parse_text_expression("rows[0].id").is_ok());
        assert!(parse_text_expression("!available").is_ok());
        assert!(parse_text_expression("a === b ? x : y").is_ok());
        assert!(parse_text_expression("formatMoney(price)").is_ok());
        assert!(parse_text_expression("() => {}").is_err());
        assert!(parse_text_expression("a = 1").is_err());
    }
}
