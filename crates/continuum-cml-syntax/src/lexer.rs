//! The lexer.
//!
//! Decision: RFC 0003 (the CML surface) and ADR-0025 (the Finite fragment), PR 15a.
//!
//! Hand-written and single-pass. Line breaks are tokens because they end statements in
//! a block; a run of line breaks (with the blank lines and `//` comments between them)
//! is one [`Tok::Newline`]. Identifiers and keywords are ASCII; any other character
//! outside a comment or string literal is an [`ParseErrorKind::UnexpectedCharacter`].

use crate::error::{ParseError, ParseErrorKind, Unsupported};
use crate::span::Span;

/// A reserved word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Kw {
    Module,
    Model,
    Type,
    Enum,
    Const,
    State,
    Init,
    Action,
    Invariant,
    Fairness,
    Behavior,
    Def,
    Require,
    Let,
    Next,
    Unchanged,
    Forall,
    Exists,
    In,
    NotIn,
    Union,
    Intersect,
    SubsetEq,
    If,
    Then,
    Else,
    True,
    False,
    Always,
    Eventually,
    Where,
    Choose,
}

impl Kw {
    fn from_word(word: &str) -> Option<Kw> {
        Some(match word {
            "module" => Kw::Module,
            "model" => Kw::Model,
            "type" => Kw::Type,
            "enum" => Kw::Enum,
            "const" => Kw::Const,
            "state" => Kw::State,
            "init" => Kw::Init,
            "action" => Kw::Action,
            "invariant" => Kw::Invariant,
            "fairness" => Kw::Fairness,
            "behavior" => Kw::Behavior,
            "def" => Kw::Def,
            "require" => Kw::Require,
            "let" => Kw::Let,
            "next" => Kw::Next,
            "unchanged" => Kw::Unchanged,
            "forall" => Kw::Forall,
            "exists" => Kw::Exists,
            "in" => Kw::In,
            "notin" => Kw::NotIn,
            "union" => Kw::Union,
            "intersect" => Kw::Intersect,
            "subseteq" => Kw::SubsetEq,
            "if" => Kw::If,
            "then" => Kw::Then,
            "else" => Kw::Else,
            "true" => Kw::True,
            "false" => Kw::False,
            "always" => Kw::Always,
            "eventually" => Kw::Eventually,
            "where" => Kw::Where,
            "choose" => Kw::Choose,
            _ => return None,
        })
    }
}

/// A token kind.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Tok {
    Ident(String),
    Kw(Kw),
    Int(u64),
    Str(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    ColonEq,
    Semi,
    Dot,
    DotDot,
    Arrow,
    FatArrow,
    IffArrow,
    LeadsTo,
    Assign,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Bang,
    AndAnd,
    OrOr,
    Pipe,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Backslash,
    Prime,
    Newline,
    /// End of input.
    Eof,
    /// The point where lexing failed; always the last token of a partial lex.
    LexError,
}

/// A token with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The kind.
    pub tok: Tok,
    /// The location.
    pub span: Span,
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    line: u32,
    col: u32,
}

/// Lex `src` into tokens, ending with [`Tok::Eof`].
///
/// # Errors
///
/// The first lexical error in source order.
pub fn lex(src: &str) -> Result<Vec<Token>, ParseError> {
    let (toks, err) = lex_partial(src);
    match err {
        Some(e) => Err(e),
        None => Ok(toks),
    }
}

/// Lex `src` as far as the first lexical error.
///
/// The token list always ends with [`Tok::Eof`] or, when lexing failed, with
/// [`Tok::LexError`] at the error's span; the error itself is returned beside it. The
/// parser reports a lexical error only when it reaches that point, so a syntax error
/// earlier in the source is reported first.
pub(crate) fn lex_partial(src: &str) -> (Vec<Token>, Option<ParseError>) {
    let origin = Span {
        start: 0,
        end: 0,
        line: 1,
        col: 1,
    };
    if u32::try_from(src.len()).map_or(true, |n| n > crate::MAX_SOURCE_BYTES) {
        return (
            vec![Token {
                tok: Tok::LexError,
                span: origin,
            }],
            Some(ParseError {
                kind: ParseErrorKind::InputTooLarge,
                span: origin,
            }),
        );
    }
    let mut lx = Lexer {
        src,
        pos: 0,
        line: 1,
        col: 1,
    };
    let mut out: Vec<Token> = Vec::new();
    loop {
        let tok = match lx.next_token() {
            Ok(tok) => tok,
            Err(e) => {
                out.push(Token {
                    tok: Tok::LexError,
                    span: e.span,
                });
                return (out, Some(e));
            }
        };
        let is_eof = tok.tok == Tok::Eof;
        let collapse =
            tok.tok == Tok::Newline && out.last().is_none_or(|t: &Token| t.tok == Tok::Newline);
        if !collapse {
            out.push(tok);
        }
        if is_eof {
            return (out, None);
        }
    }
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.src[self.pos..].chars();
        it.next();
        it.next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn here(&self) -> Span {
        let at = self.offset();
        Span {
            start: at,
            end: at,
            line: self.line,
            col: self.col,
        }
    }

    fn offset(&self) -> u32 {
        // The whole source was checked against MAX_SOURCE_BYTES (< u32::MAX) in `lex`.
        u32::try_from(self.pos).unwrap_or(u32::MAX)
    }

    fn finish(&self, start: Span) -> Span {
        Span {
            end: self.offset(),
            ..start
        }
    }

    fn err(&self, kind: ParseErrorKind, start: Span) -> ParseError {
        ParseError {
            kind,
            span: self.finish(start),
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        // Skip horizontal whitespace and comments.
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\r') => {
                    self.bump();
                }
                Some('/') if self.peek2() == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
        let start = self.here();
        let Some(c) = self.bump() else {
            return Ok(Token {
                tok: Tok::Eof,
                span: start,
            });
        };
        let tok = match c {
            '\n' => Tok::Newline,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '[' => Tok::LBracket,
            ']' => Tok::RBracket,
            '{' => Tok::LBrace,
            '}' => Tok::RBrace,
            ',' => Tok::Comma,
            ';' => Tok::Semi,
            '+' => Tok::Plus,
            '*' => Tok::Star,
            '/' => Tok::Slash,
            '%' => Tok::Percent,
            '\\' => Tok::Backslash,
            '\'' => Tok::Prime,
            ':' => self.pick('=', Tok::ColonEq, Tok::Colon),
            '.' => self.pick('.', Tok::DotDot, Tok::Dot),
            '-' => self.pick('>', Tok::Arrow, Tok::Minus),
            '~' => {
                if self.peek() == Some('>') {
                    self.bump();
                    Tok::LeadsTo
                } else {
                    return Err(self.err(ParseErrorKind::UnexpectedCharacter('~'), start));
                }
            }
            '=' => match self.peek() {
                Some('=') => {
                    self.bump();
                    Tok::EqEq
                }
                Some('>') => {
                    self.bump();
                    Tok::FatArrow
                }
                _ => Tok::Assign,
            },
            '!' => self.pick('=', Tok::NotEq, Tok::Bang),
            '<' => match (self.peek(), self.peek2()) {
                (Some('='), Some('>')) => {
                    self.bump();
                    self.bump();
                    Tok::IffArrow
                }
                (Some('='), _) => {
                    self.bump();
                    Tok::Le
                }
                _ => Tok::Lt,
            },
            '>' => self.pick('=', Tok::Ge, Tok::Gt),
            '&' => {
                if self.peek() == Some('&') {
                    self.bump();
                    Tok::AndAnd
                } else {
                    return Err(self.err(ParseErrorKind::UnexpectedCharacter('&'), start));
                }
            }
            '|' => self.pick('|', Tok::OrOr, Tok::Pipe),
            '?' => {
                if self.peek() == Some('?') {
                    self.bump();
                    // The hole's span covers `??` and the name that follows it.
                    while self.peek().is_some_and(is_ident_continue) {
                        self.bump();
                    }
                    return Err(self.err(
                        ParseErrorKind::Unsupported(Unsupported::SynthesisHole),
                        start,
                    ));
                }
                return Err(self.err(ParseErrorKind::UnexpectedCharacter('?'), start));
            }
            '"' => self.string(start)?,
            c if c.is_ascii_digit() => self.number(start)?,
            c if is_ident_start(c) => {
                while self.peek().is_some_and(is_ident_continue) {
                    self.bump();
                }
                let word = &self.src[start.start as usize..self.pos];
                match Kw::from_word(word) {
                    Some(kw) => Tok::Kw(kw),
                    None => Tok::Ident(word.to_owned()),
                }
            }
            other => return Err(self.err(ParseErrorKind::UnexpectedCharacter(other), start)),
        };
        Ok(Token {
            tok,
            span: self.finish(start),
        })
    }

    fn pick(&mut self, next: char, two: Tok, one: Tok) -> Tok {
        if self.peek() == Some(next) {
            self.bump();
            two
        } else {
            one
        }
    }

    fn number(&mut self, start: Span) -> Result<Tok, ParseError> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        // `1.5` is a float; `0..5` is a range.
        if self.peek() == Some('.') && self.peek2().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
            return Err(self.err(
                ParseErrorKind::Unsupported(Unsupported::FloatLiteral),
                start,
            ));
        }
        let digits = &self.src[start.start as usize..self.pos];
        digits
            .parse::<u64>()
            .map(Tok::Int)
            .map_err(|_| self.err(ParseErrorKind::IntegerTooLarge, start))
    }

    fn string(&mut self, start: Span) -> Result<Tok, ParseError> {
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(self.err(ParseErrorKind::UnterminatedString, start));
                }
                Some('"') => {
                    self.bump();
                    return Ok(Tok::Str(out));
                }
                Some('\\') => {
                    let esc_start = self.here();
                    self.bump();
                    match self.peek() {
                        Some('"') => out.push('"'),
                        Some('\\') => out.push('\\'),
                        Some('n') => out.push('\n'),
                        None | Some('\n') => {
                            return Err(self.err(ParseErrorKind::UnterminatedString, start));
                        }
                        Some(other) => {
                            self.bump();
                            return Err(self.err(ParseErrorKind::InvalidEscape(other), esc_start));
                        }
                    }
                    self.bump();
                }
                Some(c) => {
                    out.push(c);
                    self.bump();
                }
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
