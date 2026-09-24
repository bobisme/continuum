//! The recursive-descent parser.
//!
//! Decision: RFC 0003 (the CML surface) and ADR-0025 (the Finite fragment), PR 15a. RFC
//! 0003 leaves exact syntax open; this grammar is the Finite core surface that the
//! replicated register and the Wave 0 fixtures need, and nothing more.
//!
//! # Grammar (Finite core fragment)
//!
//! ```text
//! file      := 'module' IDENT decl*                       -- to end of input
//!            | 'model' IDENT '{' decl* '}'
//! decl      := 'type' IDENT ('=' type)?
//!            | 'enum' IDENT '{' IDENT (',' IDENT)* ','? '}'
//!            | 'const' IDENT ':' type
//!            | 'state' '{' (IDENT ':' type ('where' expr)?)* '}'  -- ',' ';' or newline between
//!            | 'init' IDENT? block
//!            | 'action' IDENT params? (block | '=' IDENT ('|' IDENT)*)
//!            | 'invariant' IDENT block
//!            | 'fairness' ('weak'|'strong') IDENT (',' IDENT)*
//!            | 'behavior' IDENT '=' expr
//!            | 'def' IDENT params ':' type '=' expr
//! params    := '(' (IDENT ':' type (',' IDENT ':' type)*)? ')'
//! block     := '{' stmt* '}'                              -- ';' or newline between
//! stmt      := 'require' expr | 'let' IDENT '=' expr | 'next' IDENT '=' expr
//!            | 'unchanged' IDENT (',' IDENT)* | 'unchanged' '(' IDENT (',' IDENT)* ')'
//!            | expr
//! type      := tatom ('->' type)?
//! tatom     := IDENT ('[' type (',' type)* ']')? | '(' type (',' type)* ')'
//!            | '{' IDENT ':' type (',' IDENT ':' type)* '}'
//! expr      := binary operators, loosest first:
//!              ~>  (non-assoc)   <=>  (non-assoc)   =>  (right)   ||   &&
//!              == = != < <= > >= in notin subseteq  (non-assoc)
//!              ..  (non-assoc)   union intersect \   + -   * / %
//!            then prefix ! -, then postfix ' .f .m(..) [i] [k := v]
//! primary   := INT | STRING | 'true' | 'false' | IDENT | IDENT '(' args ')' | 'state'
//!            | '(' expr ')' | '(' expr (',' expr)+ ')' | '[' args ']'
//!            | '{' '}' | '{' args '}' | '{' k '->' v (',' k '->' v)* '}'
//!            | '{' e '|' binders ('where' expr)? '}' | '{' k '->' v '|' binders ('where' expr)? '}'
//!            | '{' IDENT ':' expr (',' IDENT ':' expr)* '}'
//!            | ('forall'|'exists') binders ':' expr | 'if' expr 'then' expr 'else' expr
//!            | ('always'|'eventually') '(' expr ')'
//! binders   := IDENT ('in' range_expr)? (',' IDENT ('in' range_expr)?)*
//! ```
//!
//! # Line breaks
//!
//! A line break ends a statement or a declaration. Inside `(…)`, `[…]`, and an
//! expression `{…}` line breaks are insignificant. Elsewhere an expression continues
//! onto the next line when the line ends with a binary operator or `:`, or when the
//! next line starts with a binary operator other than `-` (which could start a new
//! clause).
//!
//! Quantifier, `if`, and `always`/`eventually` bodies extend as far right as possible.
//!
//! # Binder domains
//!
//! A domain binds only the name written directly before it: in `exists p, q in S: …`,
//! `q` ranges over `S` and `p` has no written domain. The tree records exactly that;
//! the elaborator decides whether a binder without a domain is admissible.

use crate::ast::{
    ActionBody, BinOp, Binder, Decl, DeclKind, Expr, ExprKind, FairnessStrength, Header,
    HeaderStyle, Ident, Param, Quantifier, SourceFile, StateField, Stmt, StmtKind, TemporalOp,
    TypeExpr, TypeKind, UnOp,
};
use crate::error::{ParseError, ParseErrorKind, Unsupported};
use crate::lexer::{Kw, Tok, Token, lex_partial};
use crate::span::Span;

/// Parse a whole `.ctm` source file.
///
/// # Errors
///
/// The first lexical, syntactic, out-of-fragment, or resource error in source order.
pub fn parse(src: &str) -> Result<SourceFile, ParseError> {
    let mut p = Parser::new(src)?;
    p.file()
}

/// Parse a single expression (surrounding line breaks are ignored).
///
/// # Errors
///
/// As [`parse`].
pub fn parse_expr(src: &str) -> Result<Expr, ParseError> {
    let mut p = Parser::new(src)?;
    p.skip_newlines();
    let e = p.expr()?;
    p.skip_newlines();
    p.expect_eof()?;
    Ok(e)
}

/// Parse a single type expression (surrounding line breaks are ignored).
///
/// # Errors
///
/// As [`parse`].
pub fn parse_type(src: &str) -> Result<TypeExpr, ParseError> {
    let mut p = Parser::new(src)?;
    p.skip_newlines();
    let t = p.ty()?;
    p.skip_newlines();
    p.expect_eof()?;
    Ok(t)
}

/// The kind of block a statement appears in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockCtx {
    Init,
    Action,
    Invariant,
}

impl BlockCtx {
    fn name(self) -> &'static str {
        match self {
            BlockCtx::Init => "an init block",
            BlockCtx::Action => "an action block",
            BlockCtx::Invariant => "an invariant block",
        }
    }
}

type PResult<T> = Result<T, ParseError>;

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
    /// When positive, line breaks are insignificant (inside brackets).
    nl_skip: u32,
    /// Current nesting depth, bounded by `MAX_NESTING`: the level the next entered node
    /// sits at, counted from the root of the enclosing declaration's expression or type.
    depth: u32,
    /// The deepest level any node parsed since the last reset sits at, *after* the
    /// operator chains around it are accounted for. A left-deep chain (`a + b + c`,
    /// `x.f[0]`) pushes nodes that were already parsed one level down per operator, so
    /// the level they were entered at understates where they end up; `binary` and
    /// `postfix` measure each operand with this mark and add the shift (bn-1nmq).
    high: u32,
    /// The lexical error at the final [`Tok::LexError`] token, if lexing failed.
    lex_error: Option<ParseError>,
}

/// Binary-operator binding levels, loosest first.
const LEVEL_LEADS_TO: u8 = 1;
const LEVEL_IFF: u8 = 2;
const LEVEL_IMPLIES: u8 = 3;
const LEVEL_OR: u8 = 4;
const LEVEL_AND: u8 = 5;
const LEVEL_CMP: u8 = 6;
const LEVEL_RANGE: u8 = 7;
const LEVEL_SET: u8 = 8;
const LEVEL_ADD: u8 = 9;
const LEVEL_MUL: u8 = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Assoc {
    Left,
    Right,
    Non,
}

fn level_of(op: BinOp) -> (u8, Assoc) {
    match op {
        BinOp::LeadsTo => (LEVEL_LEADS_TO, Assoc::Non),
        BinOp::Iff => (LEVEL_IFF, Assoc::Non),
        BinOp::Implies => (LEVEL_IMPLIES, Assoc::Right),
        BinOp::Or => (LEVEL_OR, Assoc::Left),
        BinOp::And => (LEVEL_AND, Assoc::Left),
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::In
        | BinOp::NotIn
        | BinOp::SubsetEq => (LEVEL_CMP, Assoc::Non),
        BinOp::Range => (LEVEL_RANGE, Assoc::Non),
        BinOp::Union | BinOp::Intersect | BinOp::Diff => (LEVEL_SET, Assoc::Left),
        BinOp::Add | BinOp::Sub => (LEVEL_ADD, Assoc::Left),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (LEVEL_MUL, Assoc::Left),
    }
}

fn binop_of(tok: &Tok) -> Option<BinOp> {
    Some(match tok {
        Tok::LeadsTo => BinOp::LeadsTo,
        Tok::IffArrow => BinOp::Iff,
        Tok::FatArrow => BinOp::Implies,
        Tok::OrOr => BinOp::Or,
        Tok::AndAnd => BinOp::And,
        Tok::EqEq | Tok::Assign => BinOp::Eq,
        Tok::NotEq => BinOp::Ne,
        Tok::Lt => BinOp::Lt,
        Tok::Le => BinOp::Le,
        Tok::Gt => BinOp::Gt,
        Tok::Ge => BinOp::Ge,
        Tok::Kw(Kw::In) => BinOp::In,
        Tok::Kw(Kw::NotIn) => BinOp::NotIn,
        Tok::Kw(Kw::SubsetEq) => BinOp::SubsetEq,
        Tok::DotDot => BinOp::Range,
        Tok::Kw(Kw::Union) => BinOp::Union,
        Tok::Kw(Kw::Intersect) => BinOp::Intersect,
        Tok::Backslash => BinOp::Diff,
        Tok::Plus => BinOp::Add,
        Tok::Minus => BinOp::Sub,
        Tok::Star => BinOp::Mul,
        Tok::Slash => BinOp::Div,
        Tok::Percent => BinOp::Mod,
        _ => return None,
    })
}

/// A declaration keyword the Finite core does not accept, spelled as an identifier.
fn unsupported_decl_word(word: &str) -> Option<Unsupported> {
    Some(match word {
        "process" => Unsupported::Process,
        "view" | "refines" => Unsupported::View,
        "forge" => Unsupported::ForgeBlock,
        "progress" => Unsupported::ProgressProperty,
        "transition" => Unsupported::TransitionInvariant,
        "hyperproperty" => Unsupported::Hyperproperty,
        "symmetry" => Unsupported::Symmetry,
        "check" => Unsupported::CheckDirective,
        "import" | "use" => Unsupported::ModuleImport,
        "extern" | "opaque" => Unsupported::OpaqueDomain,
        _ => return None,
    })
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> PResult<Self> {
        let (toks, lex_error) = lex_partial(src);
        Ok(Parser {
            src,
            toks,
            pos: 0,
            nl_skip: 0,
            depth: 0,
            high: 0,
            lex_error,
        })
    }

    // --- token access ---------------------------------------------------------------

    /// The index of the next significant token at or after `from`.
    fn sig_from(&self, mut i: usize) -> usize {
        if self.nl_skip > 0 {
            while self.toks[i].tok == Tok::Newline {
                i += 1;
            }
        }
        i
    }

    fn peek_tok(&self) -> &Tok {
        &self.toks[self.sig_from(self.pos)].tok
    }

    fn peek_span(&self) -> Span {
        self.toks[self.sig_from(self.pos)].span
    }

    /// The significant token after the next one.
    fn peek2_tok(&self) -> &Tok {
        let i = self.sig_from(self.pos);
        if matches!(self.toks[i].tok, Tok::Eof | Tok::LexError) {
            return &self.toks[i].tok;
        }
        &self.toks[self.sig_from(i + 1)].tok
    }

    fn bump(&mut self) -> Token {
        let i = self.sig_from(self.pos);
        let t = self.toks[i].clone();
        if !matches!(t.tok, Tok::Eof | Tok::LexError) {
            self.pos = i + 1;
        } else {
            self.pos = i;
        }
        t
    }

    fn at(&self, tok: &Tok) -> bool {
        self.peek_tok() == tok
    }

    fn eat(&mut self, tok: &Tok) -> Option<Token> {
        if self.at(tok) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn skip_newlines(&mut self) {
        while self.toks[self.pos].tok == Tok::Newline {
            self.pos += 1;
        }
    }

    fn skip_separators(&mut self) {
        while matches!(self.toks[self.pos].tok, Tok::Newline | Tok::Semi) {
            self.pos += 1;
        }
    }

    /// The previous consumed token's end, for closing spans.
    fn prev_span(&self) -> Span {
        let mut i = self.pos;
        while i > 0 {
            i -= 1;
            if self.toks[i].tok != Tok::Newline {
                return self.toks[i].span;
            }
        }
        self.toks[0].span
    }

    fn describe(&self, t: &Token) -> String {
        match t.tok {
            Tok::Eof | Tok::LexError => "end of input".to_owned(),
            Tok::Newline => "end of line".to_owned(),
            _ => format!(
                "`{}`",
                &self.src[t.span.start as usize..t.span.end as usize]
            ),
        }
    }

    fn unexpected<T>(&self, expected: &'static str) -> PResult<T> {
        let t = &self.toks[self.sig_from(self.pos)];
        if t.tok == Tok::LexError {
            if let Some(e) = &self.lex_error {
                return Err(e.clone());
            }
        }
        Err(ParseError {
            kind: ParseErrorKind::UnexpectedToken {
                expected,
                found: self.describe(t),
            },
            span: t.span,
        })
    }

    fn expect(&mut self, tok: &Tok, expected: &'static str) -> PResult<Token> {
        match self.eat(tok) {
            Some(t) => Ok(t),
            None => self.unexpected(expected),
        }
    }

    fn expect_eof(&mut self) -> PResult<()> {
        if self.at(&Tok::Eof) {
            Ok(())
        } else {
            self.unexpected("end of input")
        }
    }

    fn ident(&mut self, expected: &'static str) -> PResult<Ident> {
        if let Tok::Ident(name) = self.peek_tok() {
            let name = name.clone();
            let span = self.bump().span;
            Ok(Ident { name, span })
        } else {
            self.unexpected(expected)
        }
    }

    fn unsupported<T>(what: Unsupported, span: Span) -> PResult<T> {
        Err(ParseError {
            kind: ParseErrorKind::Unsupported(what),
            span,
        })
    }

    fn enter(&mut self) -> PResult<()> {
        self.depth += 1;
        self.high = self.high.max(self.depth);
        if self.depth > crate::MAX_NESTING {
            return Err(ParseError {
                kind: ParseErrorKind::NestingTooDeep,
                span: self.peek_span(),
            });
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Run `f` with line breaks insignificant.
    fn bracketed<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        self.nl_skip += 1;
        let r = f(self);
        self.nl_skip -= 1;
        r
    }

    /// Run `f` with line breaks significant again (a statement block).
    fn unbracketed<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        let saved = self.nl_skip;
        self.nl_skip = 0;
        let r = f(self);
        self.nl_skip = saved;
        r
    }

    // --- file and declarations ------------------------------------------------------

    fn file(&mut self) -> PResult<SourceFile> {
        self.skip_separators();
        let start = self.peek_span();
        let (style, kw_span) = match self.peek_tok() {
            Tok::Kw(Kw::Module) => (HeaderStyle::Module, self.bump().span),
            Tok::Kw(Kw::Model) => (HeaderStyle::Model, self.bump().span),
            _ => return self.unexpected("`module` or `model`"),
        };
        let name = self.ident("model name")?;
        if self.at(&Tok::Lt) {
            return Self::unsupported(Unsupported::ParameterizedModel, self.peek_span());
        }
        let header = Header {
            style,
            span: kw_span.to(name.span),
            name,
        };
        let mut decls = Vec::new();
        match style {
            HeaderStyle::Module => {
                self.end_of_decl(false)?;
                loop {
                    self.skip_separators();
                    if self.at(&Tok::Eof) {
                        break;
                    }
                    decls.push(self.decl()?);
                    self.end_of_decl(false)?;
                }
            }
            HeaderStyle::Model => {
                self.skip_newlines();
                self.expect(&Tok::LBrace, "`{`")?;
                loop {
                    self.skip_separators();
                    if self.at(&Tok::RBrace) {
                        break;
                    }
                    decls.push(self.decl()?);
                    self.end_of_decl(true)?;
                }
                self.bump();
                self.skip_separators();
                self.expect_eof()?;
            }
        }
        let end = self.prev_span();
        Ok(SourceFile {
            header,
            decls,
            span: start.to(end),
        })
    }

    /// A declaration ends at a line break, `;`, end of input, or (inside `model {…}`)
    /// the closing brace.
    fn end_of_decl(&mut self, braced: bool) -> PResult<()> {
        match self.peek_tok() {
            Tok::Newline | Tok::Semi | Tok::Eof => Ok(()),
            Tok::RBrace if braced => Ok(()),
            _ => self.unexpected("end of declaration"),
        }
    }

    fn decl(&mut self) -> PResult<Decl> {
        let start = self.peek_span();
        let kind = match self.peek_tok().clone() {
            Tok::Kw(Kw::Type) => {
                self.bump();
                let name = self.ident("type name")?;
                if self.eat(&Tok::Assign).is_some() {
                    let ty = self.ty()?;
                    DeclKind::TypeAlias { name, ty }
                } else {
                    DeclKind::Sort { name }
                }
            }
            Tok::Kw(Kw::Enum) => {
                self.bump();
                let name = self.ident("enum name")?;
                self.expect(&Tok::LBrace, "`{`")?;
                let variants = self.bracketed(|p| {
                    let mut vs = vec![p.ident("enum variant")?];
                    while p.eat(&Tok::Comma).is_some() {
                        if p.at(&Tok::RBrace) {
                            break;
                        }
                        vs.push(p.ident("enum variant")?);
                    }
                    p.expect(&Tok::RBrace, "`,` or `}`")?;
                    Ok(vs)
                })?;
                DeclKind::Enum { name, variants }
            }
            Tok::Kw(Kw::Const) => {
                self.bump();
                let name = self.ident("constant name")?;
                self.expect(&Tok::Colon, "`:`")?;
                let ty = self.ty()?;
                DeclKind::Const { name, ty }
            }
            Tok::Kw(Kw::State) => {
                self.bump();
                DeclKind::State {
                    fields: self.state_fields()?,
                }
            }
            Tok::Kw(Kw::Init) => {
                self.bump();
                let name = match self.peek_tok() {
                    Tok::Ident(_) => Some(self.ident("init name")?),
                    _ => None,
                };
                let body = self.block(BlockCtx::Init)?;
                DeclKind::Init { name, body }
            }
            Tok::Kw(Kw::Action) => {
                self.bump();
                let name = self.ident("action name")?;
                let params = if self.at(&Tok::LParen) {
                    self.params()?
                } else {
                    Vec::new()
                };
                let body = if self.eat(&Tok::Assign).is_some() {
                    let mut alts = vec![self.ident("action name")?];
                    while self.eat(&Tok::Pipe).is_some() {
                        alts.push(self.ident("action name")?);
                    }
                    ActionBody::Choice(alts)
                } else {
                    ActionBody::Block(self.block(BlockCtx::Action)?)
                };
                DeclKind::Action { name, params, body }
            }
            Tok::Kw(Kw::Invariant) => {
                self.bump();
                let name = self.ident("invariant name")?;
                let body = self.block(BlockCtx::Invariant)?;
                DeclKind::Invariant { name, body }
            }
            Tok::Kw(Kw::Fairness) => {
                self.bump();
                let strength = match self.peek_tok() {
                    Tok::Ident(w) if w == "weak" => FairnessStrength::Weak,
                    Tok::Ident(w) if w == "strong" => FairnessStrength::Strong,
                    _ => return self.unexpected("`weak` or `strong`"),
                };
                self.bump();
                let mut actions = vec![self.ident("action name")?];
                while self.eat(&Tok::Comma).is_some() {
                    actions.push(self.ident("action name")?);
                }
                DeclKind::Fairness { strength, actions }
            }
            Tok::Kw(Kw::Behavior) => {
                self.bump();
                let name = self.ident("behavior name")?;
                self.expect(&Tok::Assign, "`=`")?;
                self.skip_newlines();
                let expr = self.expr()?;
                DeclKind::Behavior { name, expr }
            }
            Tok::Kw(Kw::Def) => {
                self.bump();
                let name = self.ident("function name")?;
                if !self.at(&Tok::LParen) {
                    return self.unexpected("`(`");
                }
                let params = self.params()?;
                self.expect(&Tok::Colon, "`:`")?;
                let ret = self.ty()?;
                self.expect(&Tok::Assign, "`=`")?;
                self.skip_newlines();
                let body = self.expr()?;
                DeclKind::Def {
                    name,
                    params,
                    ret,
                    body,
                }
            }
            Tok::Kw(Kw::Eventually) => {
                return Self::unsupported(Unsupported::EventuallyProperty, start);
            }
            Tok::Ident(word) => {
                if let Some(u) = unsupported_decl_word(&word) {
                    return Self::unsupported(u, start);
                }
                return self.unexpected("declaration");
            }
            _ => return self.unexpected("declaration"),
        };
        Ok(Decl {
            kind,
            span: start.to(self.prev_span()),
        })
    }

    fn state_fields(&mut self) -> PResult<Vec<StateField>> {
        self.expect(&Tok::LBrace, "`{`")?;
        let mut fields = Vec::new();
        loop {
            while matches!(self.peek_tok(), Tok::Newline | Tok::Semi | Tok::Comma) {
                self.bump();
            }
            if self.eat(&Tok::RBrace).is_some() {
                return Ok(fields);
            }
            let name = self.ident("state variable name or `}`")?;
            self.expect(&Tok::Colon, "`:`")?;
            let ty = self.ty()?;
            let refinement = if self.eat(&Tok::Kw(Kw::Where)).is_some() {
                Some(self.expr()?)
            } else {
                None
            };
            fields.push(StateField {
                span: name.span.to(self.prev_span()),
                name,
                ty,
                refinement,
            });
            match self.peek_tok() {
                Tok::Newline | Tok::Semi | Tok::Comma | Tok::RBrace => {}
                _ => return self.unexpected("`,`, end of line, or `}`"),
            }
        }
    }

    fn params(&mut self) -> PResult<Vec<Param>> {
        self.expect(&Tok::LParen, "`(`")?;
        self.bracketed(|p| {
            let mut params = Vec::new();
            if p.eat(&Tok::RParen).is_some() {
                return Ok(params);
            }
            loop {
                let name = p.ident("parameter name")?;
                p.expect(&Tok::Colon, "`:`")?;
                let ty = p.ty()?;
                params.push(Param {
                    span: name.span.to(ty.span),
                    name,
                    ty,
                });
                if p.eat(&Tok::Comma).is_none() {
                    break;
                }
            }
            p.expect(&Tok::RParen, "`,` or `)`")?;
            Ok(params)
        })
    }

    // --- blocks and statements ------------------------------------------------------

    fn block(&mut self, ctx: BlockCtx) -> PResult<Vec<Stmt>> {
        self.expect(&Tok::LBrace, "`{`")?;
        self.enter()?;
        let r = self.unbracketed(|p| {
            let mut stmts = Vec::new();
            loop {
                p.skip_separators();
                if p.eat(&Tok::RBrace).is_some() {
                    return Ok(stmts);
                }
                stmts.push(p.stmt(ctx)?);
                match p.peek_tok() {
                    Tok::Newline | Tok::Semi | Tok::RBrace => {}
                    _ => return p.unexpected("end of statement"),
                }
            }
        });
        self.leave();
        r
    }

    fn not_allowed<T>(statement: &'static str, ctx: BlockCtx, span: Span) -> PResult<T> {
        Err(ParseError {
            kind: ParseErrorKind::StatementNotAllowed {
                statement,
                context: ctx.name(),
            },
            span,
        })
    }

    fn stmt(&mut self, ctx: BlockCtx) -> PResult<Stmt> {
        let start = self.peek_span();
        let kind = match self.peek_tok().clone() {
            Tok::Kw(Kw::Require) => {
                if ctx != BlockCtx::Action {
                    return Self::not_allowed("require", ctx, start);
                }
                self.bump();
                StmtKind::Require(self.expr()?)
            }
            Tok::Kw(Kw::Let) => {
                self.bump();
                let name = self.ident("local name")?;
                self.expect(&Tok::Assign, "`=`")?;
                StmtKind::Let(name, self.expr()?)
            }
            Tok::Kw(Kw::Next) => {
                if self.peek2_tok() == &Tok::LParen {
                    return Self::unsupported(Unsupported::TemporalNext, start);
                }
                if ctx != BlockCtx::Action {
                    return Self::not_allowed("next", ctx, start);
                }
                self.bump();
                let name = self.ident("state variable name")?;
                self.expect(&Tok::Assign, "`=`")?;
                StmtKind::Next(name, self.expr()?)
            }
            Tok::Kw(Kw::Unchanged) => {
                if ctx != BlockCtx::Action {
                    return Self::not_allowed("unchanged", ctx, start);
                }
                self.bump();
                let names = if self.at(&Tok::LParen) {
                    self.bump();
                    self.bracketed(|p| {
                        let names = p.ident_list()?;
                        p.expect(&Tok::RParen, "`,` or `)`")?;
                        Ok(names)
                    })?
                } else {
                    self.ident_list()?
                };
                StmtKind::Unchanged(names)
            }
            Tok::Ident(w)
                if (w == "await" || w == "goto") && matches!(self.peek2_tok(), Tok::Ident(_)) =>
            {
                let what = if w == "await" {
                    Unsupported::Await
                } else {
                    Unsupported::Goto
                };
                return Self::unsupported(what, start);
            }
            _ => StmtKind::Expr(self.expr()?),
        };
        Ok(Stmt {
            kind,
            span: start.to(self.prev_span()),
        })
    }

    fn ident_list(&mut self) -> PResult<Vec<Ident>> {
        let mut names = vec![self.ident("state variable name")?];
        while self.eat(&Tok::Comma).is_some() {
            names.push(self.ident("state variable name")?);
        }
        Ok(names)
    }

    // --- types ----------------------------------------------------------------------

    fn ty(&mut self) -> PResult<TypeExpr> {
        self.enter()?;
        let r = self.ty_inner();
        self.leave();
        r
    }

    /// A type, and its arrow if it has one.
    ///
    /// `A -> B` re-attaches the already parsed `A` one level down, under the new
    /// `Function` node, exactly as an operator chain does in an expression (see
    /// [`Parser::binary`]). So the left side's deepest level is measured with
    /// [`Parser::high`] and charged one more when the arrow arrives, and a function type
    /// whose left side would then pass the bound is refused at the arrow (bn-1nmq,
    /// cr-3rqxh8: before, `Set[Set[…] -> Nat] -> Nat` nested 63 times parsed to a type
    /// tree 127 deep, and every recursive consumer of types inherited the depth).
    fn ty_inner(&mut self) -> PResult<TypeExpr> {
        let at = self.depth;
        let outer = std::mem::replace(&mut self.high, at);
        let lhs = self.ty_atom()?;
        let mut top = self.high;
        let arrow = self.peek_span();
        if self.eat(&Tok::Arrow).is_some() {
            // The left side moves one level down, below the `Function` node at `at`.
            top += 1;
            if top > crate::MAX_NESTING {
                return Err(ParseError {
                    kind: ParseErrorKind::NestingTooDeep,
                    span: arrow,
                });
            }
            // The right side is entered as a child of the `Function` node.
            self.high = at;
            let rhs = self.ty()?;
            top = top.max(self.high);
            self.high = outer.max(top);
            return Ok(TypeExpr {
                span: lhs.span.to(rhs.span),
                kind: TypeKind::Function(Box::new(lhs), Box::new(rhs)),
            });
        }
        self.high = outer.max(top);
        Ok(lhs)
    }

    fn ty_atom(&mut self) -> PResult<TypeExpr> {
        let start = self.peek_span();
        match self.peek_tok() {
            Tok::Ident(_) => {
                let name = self.ident("type")?;
                if self.at(&Tok::Lt) {
                    return Self::unsupported(Unsupported::ParameterizedSort, self.peek_span());
                }
                if self.eat(&Tok::LBracket).is_some() {
                    let args = self.bracketed(|p| {
                        let mut args = vec![p.ty()?];
                        while p.eat(&Tok::Comma).is_some() {
                            args.push(p.ty()?);
                        }
                        p.expect(&Tok::RBracket, "`,` or `]`")?;
                        Ok(args)
                    })?;
                    return Ok(TypeExpr {
                        kind: TypeKind::Applied(name, args),
                        span: start.to(self.prev_span()),
                    });
                }
                Ok(TypeExpr {
                    span: name.span,
                    kind: TypeKind::Named(name),
                })
            }
            Tok::LParen => {
                self.bump();
                let mut items = self.bracketed(|p| {
                    let mut items = vec![p.ty()?];
                    while p.eat(&Tok::Comma).is_some() {
                        items.push(p.ty()?);
                    }
                    p.expect(&Tok::RParen, "`,` or `)`")?;
                    Ok(items)
                })?;
                if items.len() == 1 {
                    return Ok(items.remove(0));
                }
                Ok(TypeExpr {
                    kind: TypeKind::Tuple(items),
                    span: start.to(self.prev_span()),
                })
            }
            Tok::LBrace => {
                self.bump();
                let fields = self.bracketed(|p| {
                    let mut fields = Vec::new();
                    loop {
                        let name = p.ident("record field name")?;
                        p.expect(&Tok::Colon, "`:`")?;
                        fields.push((name, p.ty()?));
                        if p.eat(&Tok::Comma).is_none() {
                            break;
                        }
                    }
                    p.expect(&Tok::RBrace, "`,` or `}`")?;
                    Ok(fields)
                })?;
                Ok(TypeExpr {
                    kind: TypeKind::Record(fields),
                    span: start.to(self.prev_span()),
                })
            }
            _ => self.unexpected("type"),
        }
    }

    // --- expressions ----------------------------------------------------------------

    fn expr(&mut self) -> PResult<Expr> {
        self.binary(LEVEL_LEADS_TO)
    }

    /// The binary operator binding at `min_level` or tighter that continues the current
    /// expression, if any. Consumes a line break when the next line starts with a
    /// continuation operator.
    fn peek_binop(&mut self, min_level: u8) -> Option<BinOp> {
        let mut i = self.sig_from(self.pos);
        if self.toks[i].tok == Tok::Newline {
            let next = &self.toks[i + 1].tok;
            match binop_of(next) {
                Some(op) if op != BinOp::Sub => i += 1,
                _ => return None,
            }
        }
        let op = binop_of(&self.toks[i].tok)?;
        if level_of(op).0 < min_level {
            return None;
        }
        self.pos = i;
        Some(op)
    }

    /// Precedence climbing over the operators binding at `min_level` or tighter.
    ///
    /// # The nesting bound on a chain
    ///
    /// The bound limits the depth of the returned tree, not only the parser's recursion,
    /// so every later recursive pass over the tree (dump, print, elaboration, `Drop`)
    /// inherits it. A left-associative chain builds its tree left-deep: each operator
    /// puts a new root above everything parsed so far, so the left operand and every
    /// earlier right operand move one level down. The level an operand was entered at
    /// therefore understates where it ends up, and a bound on that level alone admitted a
    /// staircase such as `((x + x + …) + x + …) + x + …`, whose tree is about
    /// `MAX_NESTING² / 2` levels deep, from a few kilobytes of source (bn-1nmq). So the
    /// chain tracks `top`, the deepest level of the tree built so far, measured with
    /// [`Parser::high`]: each operator adds one, each right operand is entered as a
    /// child of the new root and its own deepest level joins `top`, and `top` past the
    /// bound is [`ParseErrorKind::NestingTooDeep`] at the operator.
    fn binary(&mut self, min_level: u8) -> PResult<Expr> {
        let base = self.depth;
        let outer = std::mem::replace(&mut self.high, base);
        let mut lhs = self.unary()?;
        let mut top = self.high;
        self.binary_loop(min_level, base, &mut lhs, &mut top)?;
        self.depth = base;
        self.high = outer.max(top);
        Ok(lhs)
    }

    fn binary_loop(
        &mut self,
        min_level: u8,
        base: u32,
        lhs: &mut Expr,
        top: &mut u32,
    ) -> PResult<()> {
        let mut last_non: Option<u8> = None;
        while let Some(op) = self.peek_binop(min_level) {
            let (level, assoc) = level_of(op);
            let op_span = self.peek_span();
            if assoc == Assoc::Non && last_non == Some(level) {
                return Err(ParseError {
                    kind: ParseErrorKind::ChainedOperator(op.symbol()),
                    span: op_span,
                });
            }
            // The new root takes the chain's level; everything built so far moves down.
            *top += 1;
            if *top > crate::MAX_NESTING {
                return Err(ParseError {
                    kind: ParseErrorKind::NestingTooDeep,
                    span: op_span,
                });
            }
            self.bump();
            // An operator at the end of a line continues onto the next one.
            self.skip_newlines();
            // The right operand is a child of the new root, one level below it.
            self.depth = base + 1;
            self.high = base + 1;
            let rhs = if assoc == Assoc::Right {
                self.binary(level)?
            } else {
                self.binary(level + 1)?
            };
            self.depth = base;
            *top = (*top).max(self.high);
            let left = std::mem::replace(lhs, placeholder());
            *lhs = Expr {
                span: left.span.to(rhs.span),
                kind: ExprKind::Binary(op, Box::new(left), Box::new(rhs)),
            };
            last_non = (assoc == Assoc::Non).then_some(level);
        }
        Ok(())
    }

    fn unary(&mut self) -> PResult<Expr> {
        self.enter()?;
        let r = self.unary_inner();
        self.leave();
        r
    }

    fn unary_inner(&mut self) -> PResult<Expr> {
        let start = self.peek_span();
        let op = match self.peek_tok() {
            Tok::Bang => Some(UnOp::Not),
            Tok::Minus => Some(UnOp::Neg),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let operand = self.unary()?;
            return Ok(Expr {
                span: start.to(operand.span),
                kind: ExprKind::Unary(op, Box::new(operand)),
            });
        }
        // The primary is the bottom of any postfix chain after it: measure its depth.
        let at = self.depth;
        let outer = std::mem::replace(&mut self.high, at);
        let base = self.primary()?;
        let mut top = self.high;
        let e = self.postfix(base, at, &mut top)?;
        self.depth = at;
        self.high = outer.max(top);
        Ok(e)
    }

    /// Postfix operators: `'`, `.f`, `.m(..)`, `[k]`, `[k := v]`.
    ///
    /// A postfix chain is left-deep like an infix one (`x` with three primes is three
    /// `Prime` nodes over `x`), so it is bounded the same way (see [`Parser::binary`]):
    /// `top` is the deepest level of the tree so far, each operator adds one, and each
    /// argument, key, or value is entered as a child of the node the operator builds,
    /// which sits at `at`. Before bn-1nmq a postfix chain was not counted at all, and `x`
    /// followed by thousands of primes parsed.
    fn postfix(&mut self, mut e: Expr, at: u32, top: &mut u32) -> PResult<Expr> {
        while matches!(self.peek_tok(), Tok::Prime | Tok::Dot | Tok::LBracket) {
            *top += 1;
            if *top > crate::MAX_NESTING {
                return Err(ParseError {
                    kind: ParseErrorKind::NestingTooDeep,
                    span: self.peek_span(),
                });
            }
            self.depth = at;
            self.high = at;
            e = self.postfix_step(e)?;
            self.depth = at;
            *top = (*top).max(self.high);
        }
        Ok(e)
    }

    /// Apply the one postfix operator at the current token to `e`.
    fn postfix_step(&mut self, mut e: Expr) -> PResult<Expr> {
        match self.peek_tok() {
            Tok::Prime => {
                let end = self.bump().span;
                e = Expr {
                    span: e.span.to(end),
                    kind: ExprKind::Prime(Box::new(e)),
                };
            }
            Tok::Dot => {
                self.bump();
                let name = self.ident("field or method name")?;
                if self.at(&Tok::LParen) {
                    let args = self.call_args()?;
                    e = Expr {
                        span: e.span.to(self.prev_span()),
                        kind: ExprKind::Method(Box::new(e), name, args),
                    };
                } else {
                    e = Expr {
                        span: e.span.to(name.span),
                        kind: ExprKind::Field(Box::new(e), name),
                    };
                }
            }
            Tok::LBracket => {
                self.bump();
                let (key, value) = self.bracketed(|p| {
                    let key = p.expr()?;
                    let value = if p.eat(&Tok::ColonEq).is_some() {
                        Some(p.expr()?)
                    } else {
                        None
                    };
                    p.expect(&Tok::RBracket, "`]` or `:=`")?;
                    Ok((key, value))
                })?;
                let span = e.span.to(self.prev_span());
                e = Expr {
                    span,
                    kind: match value {
                        Some(v) => ExprKind::Update(Box::new(e), Box::new(key), Box::new(v)),
                        None => ExprKind::Index(Box::new(e), Box::new(key)),
                    },
                };
            }
            _ => {}
        }
        Ok(e)
    }

    fn call_args(&mut self) -> PResult<Vec<Expr>> {
        self.expect(&Tok::LParen, "`(`")?;
        self.bracketed(|p| {
            let mut args = Vec::new();
            if p.eat(&Tok::RParen).is_some() {
                return Ok(args);
            }
            args.push(p.expr()?);
            while p.eat(&Tok::Comma).is_some() {
                args.push(p.expr()?);
            }
            p.expect(&Tok::RParen, "`,` or `)`")?;
            Ok(args)
        })
    }

    /// A primary expression.
    ///
    /// Each form is parsed by its own small function. In an unoptimized build a
    /// function's stack frame holds the locals of every match arm, and this function is
    /// on the stack once per nesting level, so keeping it a thin dispatcher is what keeps
    /// [`crate::MAX_NESTING`] levels within a default thread stack.
    fn primary(&mut self) -> PResult<Expr> {
        let start = self.peek_span();
        let kind = match self.peek_tok() {
            Tok::Int(_) | Tok::Str(_) | Tok::Kw(Kw::True | Kw::False | Kw::State) => self.literal(),
            Tok::Ident(_) => self.name_or_call()?,
            Tok::LParen => return self.paren(start),
            Tok::LBracket => self.seq_lit()?,
            Tok::LBrace => {
                self.bump();
                self.bracketed(Self::braces)?
            }
            Tok::Kw(Kw::Forall | Kw::Exists) => self.quantifier()?,
            Tok::Kw(Kw::If) => self.if_then_else()?,
            Tok::Kw(Kw::Always | Kw::Eventually) => self.temporal()?,
            Tok::Kw(Kw::Next) => return Self::unsupported(Unsupported::TemporalNext, start),
            Tok::Kw(Kw::Choose) => return Self::unsupported(Unsupported::Choose, start),
            _ => return self.unexpected("expression"),
        };
        Ok(Expr {
            kind,
            span: start.to(self.prev_span()),
        })
    }

    fn literal(&mut self) -> ExprKind {
        match self.bump().tok {
            Tok::Int(n) => ExprKind::Int(n),
            Tok::Str(s) => ExprKind::Str(s),
            Tok::Kw(Kw::True) => ExprKind::Bool(true),
            Tok::Kw(Kw::False) => ExprKind::Bool(false),
            _ => ExprKind::WholeState,
        }
    }

    fn name_or_call(&mut self) -> PResult<ExprKind> {
        let name = self.ident("name")?;
        if self.at(&Tok::LParen) {
            Ok(ExprKind::Call(name, self.call_args()?))
        } else {
            Ok(ExprKind::Name(name))
        }
    }

    fn paren(&mut self, start: Span) -> PResult<Expr> {
        self.bump();
        // A group is not a tree node: its expression takes the group's own level. The
        // expression is still entered one level down, so the parser's recursion stays
        // bounded by the parentheses, but the depth it reports up is one less (bn-1nmq),
        // so a chain *around* a group is not charged for the group. A chain *inside*
        // groups still counts them: it is checked against the raw level, which counts
        // every enclosing parenthesis, so the bound is conservative there (for example
        // forty groups around `x` with thirty primes is refused, a tree 31 deep).
        let at = self.depth;
        let outer = std::mem::replace(&mut self.high, at);
        let mut items = self.bracketed(|p| {
            let mut items = vec![p.expr()?];
            while p.eat(&Tok::Comma).is_some() {
                items.push(p.expr()?);
            }
            p.expect(&Tok::RParen, "`,` or `)`")?;
            Ok(items)
        })?;
        let span = start.to(self.prev_span());
        if items.len() == 1 {
            // Parentheses group; they are not a tree node.
            self.high = outer.max(self.high.saturating_sub(1).max(at));
            let mut inner = items.remove(0);
            inner.span = span;
            return Ok(inner);
        }
        self.high = outer.max(self.high);
        Ok(Expr {
            kind: ExprKind::Tuple(items),
            span,
        })
    }

    fn seq_lit(&mut self) -> PResult<ExprKind> {
        self.bump();
        let items = self.bracketed(|p| {
            let mut items = Vec::new();
            if p.eat(&Tok::RBracket).is_some() {
                return Ok(items);
            }
            items.push(p.expr()?);
            while p.eat(&Tok::Comma).is_some() {
                items.push(p.expr()?);
            }
            p.expect(&Tok::RBracket, "`,` or `]`")?;
            Ok(items)
        })?;
        Ok(ExprKind::SeqLit(items))
    }

    fn quantifier(&mut self) -> PResult<ExprKind> {
        let quant = if self.bump().tok == Tok::Kw(Kw::Forall) {
            Quantifier::Forall
        } else {
            Quantifier::Exists
        };
        let binders = self.binders()?;
        self.expect(&Tok::Colon, "`,` or `:`")?;
        self.skip_newlines();
        let body = self.expr()?;
        Ok(ExprKind::Quant(quant, binders, Box::new(body)))
    }

    fn if_then_else(&mut self) -> PResult<ExprKind> {
        self.bump();
        let cond = self.expr()?;
        self.skip_newlines();
        self.expect(&Tok::Kw(Kw::Then), "`then`")?;
        self.skip_newlines();
        let then = self.expr()?;
        self.skip_newlines();
        self.expect(&Tok::Kw(Kw::Else), "`else`")?;
        self.skip_newlines();
        let els = self.expr()?;
        Ok(ExprKind::If(Box::new(cond), Box::new(then), Box::new(els)))
    }

    fn temporal(&mut self) -> PResult<ExprKind> {
        let op = if self.bump().tok == Tok::Kw(Kw::Always) {
            TemporalOp::Always
        } else {
            TemporalOp::Eventually
        };
        self.expect(&Tok::LParen, "`(`")?;
        let inner = self.bracketed(|p| {
            let e = p.expr()?;
            p.expect(&Tok::RParen, "`)`")?;
            Ok(e)
        })?;
        Ok(ExprKind::Temporal(op, Box::new(inner)))
    }

    /// The inside of an expression `{…}`; the `{` is consumed.
    fn braces(&mut self) -> PResult<ExprKind> {
        if self.eat(&Tok::RBrace).is_some() {
            return Ok(ExprKind::EmptyBraces);
        }
        if matches!(self.peek_tok(), Tok::Ident(_)) && self.peek2_tok() == &Tok::Colon {
            return self.record();
        }
        let first = Box::new(self.expr()?);
        if self.eat(&Tok::Arrow).is_some() {
            return self.map_tail(first);
        }
        if self.eat(&Tok::Pipe).is_some() {
            let (binders, filter) = self.comprehension_tail()?;
            return Ok(ExprKind::SetComp(first, binders, filter));
        }
        self.set_tail(*first)
    }

    fn record(&mut self) -> PResult<ExprKind> {
        let mut fields = Vec::new();
        loop {
            let name = self.ident("record field name")?;
            self.expect(&Tok::Colon, "`:`")?;
            fields.push((name, self.expr()?));
            if self.eat(&Tok::Comma).is_none() {
                break;
            }
        }
        self.expect(&Tok::RBrace, "`,` or `}`")?;
        Ok(ExprKind::Record(fields))
    }

    fn map_tail(&mut self, first: Box<Expr>) -> PResult<ExprKind> {
        let value = Box::new(self.expr()?);
        if self.eat(&Tok::Pipe).is_some() {
            let (binders, filter) = self.comprehension_tail()?;
            return Ok(ExprKind::MapComp(first, value, binders, filter));
        }
        let mut entries = vec![(*first, *value)];
        while self.eat(&Tok::Comma).is_some() {
            let k = self.expr()?;
            self.expect(&Tok::Arrow, "`->`")?;
            entries.push((k, self.expr()?));
        }
        self.expect(&Tok::RBrace, "`,` or `}`")?;
        Ok(ExprKind::MapLit(entries))
    }

    fn set_tail(&mut self, first: Expr) -> PResult<ExprKind> {
        let mut items = vec![first];
        while self.eat(&Tok::Comma).is_some() {
            items.push(self.expr()?);
        }
        self.expect(&Tok::RBrace, "`,`, `|`, `->`, or `}`")?;
        Ok(ExprKind::SetLit(items))
    }

    #[allow(clippy::type_complexity)]
    fn comprehension_tail(&mut self) -> PResult<(Vec<Binder>, Option<Box<Expr>>)> {
        let binders = self.binders()?;
        let filter = if self.eat(&Tok::Kw(Kw::Where)).is_some() {
            Some(Box::new(self.expr()?))
        } else {
            None
        };
        self.expect(&Tok::RBrace, "`,`, `where`, or `}`")?;
        Ok((binders, filter))
    }

    fn binders(&mut self) -> PResult<Vec<Binder>> {
        let mut out = Vec::new();
        loop {
            let name = self.ident("bound variable name")?;
            let domain = if self.eat(&Tok::Kw(Kw::In)).is_some() {
                Some(self.binary(LEVEL_RANGE)?)
            } else {
                None
            };
            let end = domain.as_ref().map_or(name.span, |d| d.span);
            out.push(Binder {
                span: name.span.to(end),
                name,
                domain,
            });
            if self.eat(&Tok::Comma).is_none() {
                return Ok(out);
            }
        }
    }
}

/// A throwaway value for `std::mem::replace`; it is overwritten before anyone reads it.
fn placeholder() -> Expr {
    Expr {
        kind: ExprKind::EmptyBraces,
        span: Span {
            start: 0,
            end: 0,
            line: 1,
            col: 1,
        },
    }
}
