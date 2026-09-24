//! The mechanical check: the committed types against the normative IDL.
//!
//! # What this file is for
//!
//! `notes/plan/schemas/continuumd-native-protocol.idl` is the wire authority — "where
//! the prose and this file disagree, this file decides". `crates/continuumd/src/protocol`
//! is a transcription of it. A transcription that is merely *believed* to match its
//! source is a claim; this file makes it a checked fact, and `rule
//! conformance.registry_agreement` says how strictly:
//!
//! > A generator or validator MUST fail closed on any disagreement rather than preferring
//! > either source.
//!
//! So every comparison below is exact and every mismatch is a failure. There is no
//! "close enough" and no direction of preference.
//!
//! # Two parsers on purpose
//!
//! The parser in this file is written from the IDL header's EBNF grammar and shares no
//! code with anything the types were authored from. docs/03 §8 asks for exactly this
//! ("Independent paths should differ in: … parser …"), because a codec bug shared by
//! producer and checker is invisible to both. A third leg is in
//! `tests/registry_agreement.rs`, which checks the same types against RFC 0027's
//! independently maintained authority table.
//!
//! # What a failure looks like
//!
//! Rename `components` to `component` in `WorkspaceCreateRequest` and
//! `request_and_response_structs_match_the_idl` reports
//! `workspace.create request field 0: name "component" != IDL "components"`. The rename
//! reaches the [`FieldSpec`] because [`protocol_struct!`] emits the struct field and the
//! spec from one token — see `crates/continuumd/src/protocol/spec.rs`. `the_check_is_not_vacuous`
//! demonstrates that failure and five of its siblings by mutating the IDL text.
//!
//! [`FieldSpec`]: continuumd::protocol::spec::FieldSpec
//! [`protocol_struct!`]: continuumd::protocol_struct

use std::collections::BTreeMap;

use continuumd::protocol::registry::{
    ALIASES, ENCODINGS, ENUMS, HANDLES, IDL_VERSION, MAJORS_SERVED, NAMED_STRUCTS, OPERATION_COUNT,
    OPERATIONS, PROTOCOL_VERSION, UNIONS,
};
use continuumd::protocol::spec::ProtocolEnum;

const IDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../notes/plan/schemas/continuumd-native-protocol.idl"
);

// =====================================================================================
// The parser
// =====================================================================================

mod idl {
    //! A parser for the IDL, written from the grammar in its own header.
    //!
    //! The grammar is small and the file is normative, so the parser is deliberately
    //! unforgiving: anything it does not recognize is a panic naming the offending token,
    //! never a skipped declaration. A parser that silently ignored a construct would let
    //! a whole operation vanish from the comparison.
    //!
    //! Doc comments and `rule` bodies are lexical noise here: they carry normative prose
    //! but no structure this check compares, and the `"""` blocks contain braces and
    //! quotes that would otherwise have to be escaped through the whole grammar.
    //!
    //! One liberty is taken with string literals: `\\` denotes a single backslash, so the
    //! IDL's `@pattern("^[a-z]+\\.[a-z_]+$")` denotes the regular expression
    //! `^[a-z]+\.[a-z_]+$`. The IDL's lexical rules define no escapes at all, and that
    //! reading is the only one under which its `@pattern` values are the regular
    //! expressions they are plainly meant to be.

    use std::collections::BTreeMap;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Token {
        Ident(String),
        Str(String),
        Int(u64),
        Punct(char),
    }

    /// Split the document into tokens, dropping comments and `"""` blocks.
    ///
    /// # Panics
    ///
    /// On an unterminated string or an unexpected character.
    pub fn tokenize(source: &str) -> Vec<Token> {
        let bytes: Vec<char> = source.chars().collect();
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            let character = bytes[index];
            if character.is_whitespace() {
                index += 1;
            } else if character == '/' && bytes.get(index + 1) == Some(&'/') {
                while index < bytes.len() && bytes[index] != '\n' {
                    index += 1;
                }
            } else if character == '"'
                && bytes.get(index + 1) == Some(&'"')
                && bytes.get(index + 2) == Some(&'"')
            {
                index += 3;
                while index + 2 < bytes.len()
                    && !(bytes[index] == '"' && bytes[index + 1] == '"' && bytes[index + 2] == '"')
                {
                    index += 1;
                }
                assert!(index + 2 < bytes.len(), "unterminated triple-quoted block");
                index += 3;
            } else if character == '"' {
                index += 1;
                let mut text = String::new();
                loop {
                    assert!(index < bytes.len(), "unterminated string literal");
                    match bytes[index] {
                        '"' => {
                            index += 1;
                            break;
                        }
                        '\\' if bytes.get(index + 1) == Some(&'\\') => {
                            text.push('\\');
                            index += 2;
                        }
                        other => {
                            text.push(other);
                            index += 1;
                        }
                    }
                }
                tokens.push(Token::Str(text));
            } else if character.is_ascii_digit() {
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                let text: String = bytes[start..index].iter().collect();
                tokens.push(Token::Int(text.parse().expect("digits parse")));
            } else if character.is_ascii_alphabetic() || character == '_' {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == '_')
                {
                    index += 1;
                }
                tokens.push(Token::Ident(bytes[start..index].iter().collect()));
            } else {
                assert!(
                    "{}[]<>,;:=.()@".contains(character),
                    "unexpected character {character:?} in the IDL"
                );
                tokens.push(Token::Punct(character));
                index += 1;
            }
        }
        tokens
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Field {
        pub name: String,
        pub ty: String,
        pub presence: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Member {
        pub ident: String,
        pub wire: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Enum {
        pub name: String,
        pub open: bool,
        pub members: Vec<Member>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Struct {
        pub name: String,
        pub fields: Vec<Field>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Union {
        pub name: String,
        pub variants: Vec<(String, String)>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Body {
        Named(String),
        Anonymous(Vec<Field>),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Operation {
        pub name: String,
        pub namespace: String,
        pub verb: String,
        pub annotations: Vec<String>,
        pub authority: String,
        pub request: Body,
        pub response: Body,
        pub events: Option<String>,
        pub verdict: Option<String>,
        pub errors: Vec<String>,
    }

    #[derive(Debug, Clone, Default)]
    pub struct Document {
        pub settings: BTreeMap<String, Setting>,
        pub scalars: Vec<String>,
        pub handles: Vec<(String, String)>,
        pub aliases: Vec<(String, String, Option<String>)>,
        pub enums: Vec<Enum>,
        pub structs: Vec<Struct>,
        pub unions: Vec<Union>,
        pub operations: Vec<Operation>,
        pub rules: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Setting {
        Text(String),
        Ints(Vec<u64>),
        Texts(Vec<String>),
    }

    struct Parser {
        tokens: Vec<Token>,
        at: usize,
    }

    impl Parser {
        fn peek(&self) -> Option<&Token> {
            self.tokens.get(self.at)
        }

        fn next(&mut self) -> Token {
            let token = self
                .tokens
                .get(self.at)
                .unwrap_or_else(|| panic!("unexpected end of document"))
                .clone();
            self.at += 1;
            token
        }

        fn ident(&mut self) -> String {
            match self.next() {
                Token::Ident(text) => text,
                other => panic!("expected an identifier, found {other:?}"),
            }
        }

        fn text(&mut self) -> String {
            match self.next() {
                Token::Str(text) => text,
                other => panic!("expected a string literal, found {other:?}"),
            }
        }

        fn punct(&mut self, expected: char) {
            match self.next() {
                Token::Punct(found) if found == expected => {}
                other => panic!("expected {expected:?}, found {other:?}"),
            }
        }

        fn eat(&mut self, expected: char) -> bool {
            if self.peek() == Some(&Token::Punct(expected)) {
                self.at += 1;
                true
            } else {
                false
            }
        }

        /// Consume `@name` and `@name(args)` annotations, returning their names.
        fn annotations(&mut self) -> Vec<String> {
            let mut names = Vec::new();
            while self.eat('@') {
                names.push(self.ident());
                if self.eat('(') {
                    while !self.eat(')') {
                        self.at += 1;
                    }
                }
            }
            names
        }

        fn ty(&mut self) -> String {
            let head = self.ident();
            match head.as_str() {
                "list" => {
                    self.punct('<');
                    let inner = self.ident();
                    self.punct('>');
                    format!("list<{inner}>")
                }
                "map" => {
                    self.punct('<');
                    let key = self.ident();
                    self.punct(',');
                    let value = self.ident();
                    self.punct('>');
                    format!("map<{key},{value}>")
                }
                _ => head,
            }
        }

        fn fields(&mut self) -> Vec<Field> {
            let mut fields = Vec::new();
            self.punct('{');
            while !self.eat('}') {
                let name = self.ident();
                self.punct(':');
                let ty = self.ty();
                let presence = self.ident();
                assert!(
                    matches!(presence.as_str(), "required" | "optional" | "nullable"),
                    "field {name} has presence {presence:?}"
                );
                self.annotations();
                self.punct(';');
                fields.push(Field { name, ty, presence });
            }
            fields
        }

        fn body(&mut self) -> Body {
            if self.peek() == Some(&Token::Punct('{')) {
                let fields = self.fields();
                self.punct(';');
                Body::Anonymous(fields)
            } else {
                let name = self.ident();
                self.punct(';');
                Body::Named(name)
            }
        }
    }

    /// Parse the document.
    ///
    /// # Panics
    ///
    /// On any construct the grammar does not describe.
    #[allow(clippy::too_many_lines)]
    pub fn parse(source: &str) -> Document {
        let mut parser = Parser {
            tokens: tokenize(source),
            at: 0,
        };
        let mut document = Document::default();
        while parser.peek().is_some() {
            let annotations = parser.annotations();
            let keyword = parser.ident();
            match keyword.as_str() {
                "protocol" => {
                    parser.ident();
                    parser.punct('.');
                    parser.ident();
                    parser.punct('{');
                    while !parser.eat('}') {
                        let name = parser.ident();
                        parser.punct('=');
                        let value = if parser.eat('[') {
                            let mut ints = Vec::new();
                            let mut texts = Vec::new();
                            while !parser.eat(']') {
                                match parser.next() {
                                    Token::Int(value) => ints.push(value),
                                    Token::Str(value) => texts.push(value),
                                    Token::Punct(',') => {}
                                    other => panic!("unexpected {other:?} in a setting list"),
                                }
                            }
                            if texts.is_empty() {
                                Setting::Ints(ints)
                            } else {
                                Setting::Texts(texts)
                            }
                        } else {
                            match parser.next() {
                                Token::Str(text) => Setting::Text(text),
                                Token::Int(value) => Setting::Text(value.to_string()),
                                other => panic!("unexpected setting value {other:?}"),
                            }
                        };
                        parser.punct(';');
                        document.settings.insert(name, value);
                    }
                }
                "scalar" => {
                    document.scalars.push(parser.ident());
                    parser.punct(';');
                }
                "handle" => {
                    let name = parser.ident();
                    parser.punct('=');
                    let prefix = parser.text();
                    parser.punct(';');
                    document.handles.push((name, prefix));
                }
                "alias" => {
                    let name = parser.ident();
                    parser.punct('=');
                    let base = parser.ident();
                    let mut pattern = None;
                    while parser.eat('@') {
                        let annotation = parser.ident();
                        if parser.eat('(') {
                            let argument = parser.text();
                            parser.punct(')');
                            if annotation == "pattern" {
                                pattern = Some(argument);
                            }
                        }
                    }
                    parser.punct(';');
                    document.aliases.push((name, base, pattern));
                }
                "enum" => {
                    let name = parser.ident();
                    parser.punct('{');
                    let mut members = Vec::new();
                    while !parser.eat('}') {
                        let ident = parser.ident();
                        let wire = if parser.eat('=') {
                            parser.text()
                        } else {
                            ident.clone()
                        };
                        parser.annotations();
                        parser.punct(',');
                        members.push(Member { ident, wire });
                    }
                    document.enums.push(Enum {
                        name,
                        open: annotations.iter().any(|item| item == "open"),
                        members,
                    });
                }
                "struct" => {
                    let name = parser.ident();
                    let fields = parser.fields();
                    document.structs.push(Struct { name, fields });
                }
                "union" => {
                    let name = parser.ident();
                    parser.punct('{');
                    let mut variants = Vec::new();
                    while !parser.eat('}') {
                        let ident = parser.ident();
                        parser.punct('(');
                        let ty = parser.ident();
                        parser.punct(')');
                        parser.punct(',');
                        variants.push((ident, ty));
                    }
                    document.unions.push(Union { name, variants });
                }
                "operation" => {
                    let namespace = parser.ident();
                    parser.punct('.');
                    let verb = parser.ident();
                    parser.punct('{');
                    let mut authority = None;
                    let mut request = None;
                    let mut response = None;
                    let mut events = None;
                    let mut verdict = None;
                    let mut errors = None;
                    while !parser.eat('}') {
                        let clause = parser.ident();
                        match clause.as_str() {
                            "authority" => {
                                authority = Some(parser.ident());
                                parser.punct(';');
                            }
                            "verdict" => {
                                verdict = Some(parser.ident());
                                parser.punct(';');
                            }
                            "request" => request = Some(parser.body()),
                            "response" => response = Some(parser.body()),
                            "events" => match parser.body() {
                                Body::Named(name) => events = Some(name),
                                Body::Anonymous(_) => {
                                    panic!("an anonymous events body is not transcribed")
                                }
                            },
                            "errors" => {
                                parser.punct('[');
                                let mut codes = Vec::new();
                                while !parser.eat(']') {
                                    if parser.eat(',') {
                                        continue;
                                    }
                                    codes.push(parser.ident());
                                }
                                parser.punct(';');
                                errors = Some(codes);
                            }
                            other => panic!("unknown clause {other:?}"),
                        }
                    }
                    document.operations.push(Operation {
                        name: format!("{namespace}.{verb}"),
                        namespace,
                        verb,
                        // `@since` dates a declaration; it is not one of the seven
                        // behavioural annotations the IDL's §1 catalogue lists and RFC
                        // 0027's registry column reproduces. The first operation to carry
                        // one is `evidence.link` at 3.3 (bn-3sypm).
                        annotations: annotations
                            .into_iter()
                            .filter(|annotation| annotation != "since")
                            .collect(),
                        authority: authority.expect("an operation declares its authority"),
                        request: request.expect("an operation declares a request"),
                        response: response.expect("an operation declares a response"),
                        events,
                        verdict,
                        errors: errors.expect("an operation declares an errors clause"),
                    });
                }
                "rule" => {
                    let mut name = parser.ident();
                    while parser.eat('.') {
                        name.push('.');
                        name.push_str(&parser.ident());
                    }
                    parser.punct('{');
                    parser.punct('}');
                    document.rules.push(name);
                }
                other => panic!("unknown declaration {other:?}"),
            }
        }
        document
    }

    /// The IDL's generated-name rule for an anonymous body: the operation name in
    /// PascalCase, plus the clause's suffix.
    #[must_use]
    pub fn generated_name(operation: &str, suffix: &str) -> String {
        let mut out = String::new();
        for part in operation.split(['.', '_']) {
            let mut characters = part.chars();
            if let Some(first) = characters.next() {
                out.extend(first.to_uppercase());
                out.push_str(characters.as_str());
            }
        }
        out.push_str(suffix);
        out
    }
}

fn document() -> idl::Document {
    idl::parse(&std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable"))
}

// =====================================================================================
// The comparisons
//
// Each returns the disagreements it found. An empty vector is agreement; the tests below
// assert emptiness, and `the_check_is_not_vacuous` asserts non-emptiness against mutated
// copies of the IDL so that "no disagreements" cannot mean "nothing was compared".
// =====================================================================================

fn field_mismatches(
    subject: &str,
    declared: &[continuumd::protocol::spec::FieldSpec],
    idl: &[idl::Field],
) -> Vec<String> {
    let mut found = Vec::new();
    if declared.len() != idl.len() {
        found.push(format!(
            "{subject}: {} fields declared, IDL declares {}",
            declared.len(),
            idl.len()
        ));
    }
    for (index, (mine, theirs)) in declared.iter().zip(idl).enumerate() {
        if mine.name != theirs.name {
            found.push(format!(
                "{subject} field {index}: name {:?} != IDL {:?}",
                mine.name, theirs.name
            ));
        }
        if mine.ty != theirs.ty {
            found.push(format!(
                "{subject} field {index} ({}): type {:?} != IDL {:?}",
                theirs.name, mine.ty, theirs.ty
            ));
        }
        if mine.presence.as_idl() != theirs.presence {
            found.push(format!(
                "{subject} field {index} ({}): presence {} != IDL {}",
                theirs.name, mine.presence, theirs.presence
            ));
        }
    }
    found
}

fn body_mismatches(
    subject: &str,
    operation: &str,
    suffix: &str,
    declared: continuumd::protocol::spec::StructSpec,
    body: &idl::Body,
) -> Vec<String> {
    match body {
        idl::Body::Named(name) => {
            if declared.name == name {
                Vec::new()
            } else {
                vec![format!(
                    "{subject}: declares struct {:?}, IDL names {name:?}",
                    declared.name
                )]
            }
        }
        idl::Body::Anonymous(fields) => {
            let expected = idl::generated_name(operation, suffix);
            let mut found = Vec::new();
            if declared.name != expected {
                found.push(format!(
                    "{subject}: struct named {:?}, the IDL's generated-name rule gives {expected:?}",
                    declared.name
                ));
            }
            found.extend(field_mismatches(subject, declared.fields, fields));
            found
        }
    }
}

fn operation_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = Vec::new();

    if OPERATIONS.len() != document.operations.len() {
        found.push(format!(
            "registry declares {} operations, IDL declares {}",
            OPERATIONS.len(),
            document.operations.len()
        ));
    }
    if OPERATIONS.len() != OPERATION_COUNT {
        found.push(format!(
            "OPERATION_COUNT is {OPERATION_COUNT}, the table has {}",
            OPERATIONS.len()
        ));
    }

    let mine: Vec<&str> = OPERATIONS.iter().map(|spec| spec.name).collect();
    let theirs: Vec<&str> = document
        .operations
        .iter()
        .map(|operation| operation.name.as_str())
        .collect();
    if mine != theirs {
        for name in &theirs {
            if !mine.contains(name) {
                found.push(format!("IDL declares {name:?}; the registry does not"));
            }
        }
        for name in &mine {
            if !theirs.contains(name) {
                found.push(format!("the registry declares {name:?}; the IDL does not"));
            }
        }
        if found.is_empty() {
            found
                .push("the registry and the IDL declare the operations in different orders".into());
        }
    }

    let by_name: BTreeMap<&str, &idl::Operation> = document
        .operations
        .iter()
        .map(|operation| (operation.name.as_str(), operation))
        .collect();

    for spec in OPERATIONS {
        let Some(operation) = by_name.get(spec.name) else {
            continue;
        };
        let name = spec.name;

        if spec.authority.ident() != operation.authority {
            found.push(format!(
                "{name}: authority {:?} != IDL {:?}",
                spec.authority.ident(),
                operation.authority
            ));
        }

        let mine: Vec<&str> = spec
            .annotations
            .iter()
            .map(|annotation| annotation.as_idl())
            .collect();
        let theirs: Vec<&str> = operation.annotations.iter().map(String::as_str).collect();
        if mine != theirs {
            found.push(format!("{name}: annotations {mine:?} != IDL {theirs:?}"));
        }

        found.extend(body_mismatches(
            &format!("{name} request"),
            name,
            "Request",
            spec.request,
            &operation.request,
        ));
        found.extend(body_mismatches(
            &format!("{name} response"),
            name,
            "Response",
            spec.response,
            &operation.response,
        ));

        let mine: Vec<&str> = spec.errors.iter().map(|code| code.ident()).collect();
        let theirs: Vec<&str> = operation.errors.iter().map(String::as_str).collect();
        if mine != theirs {
            found.push(format!("{name}: errors {mine:?} != IDL {theirs:?}"));
        }

        if spec.verdict != operation.verdict.as_deref() {
            found.push(format!(
                "{name}: verdict {:?} != IDL {:?}",
                spec.verdict, operation.verdict
            ));
        }
        if spec.events != operation.events.as_deref() {
            found.push(format!(
                "{name}: events {:?} != IDL {:?}",
                spec.events, operation.events
            ));
        }
    }

    found
}

fn struct_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = Vec::new();
    let mine: Vec<&str> = NAMED_STRUCTS.iter().map(|spec| spec.name).collect();
    let theirs: Vec<&str> = document
        .structs
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    if mine != theirs {
        found.push(format!("named structs {mine:?} != IDL {theirs:?}"));
    }
    for spec in NAMED_STRUCTS {
        if let Some(item) = document.structs.iter().find(|item| item.name == spec.name) {
            found.extend(field_mismatches(spec.name, spec.fields, &item.fields));
        }
    }
    found
}

fn enum_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = Vec::new();
    let mine: Vec<&str> = ENUMS.iter().map(|spec| spec.name).collect();
    let theirs: Vec<&str> = document
        .enums
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    if mine != theirs {
        found.push(format!("enums {mine:?} != IDL {theirs:?}"));
    }
    for spec in ENUMS {
        let Some(item) = document.enums.iter().find(|item| item.name == spec.name) else {
            continue;
        };
        if spec.open != item.open {
            found.push(format!(
                "{}: @open is {}, IDL says {}",
                spec.name, spec.open, item.open
            ));
        }
        let mine: Vec<(&str, &str)> = spec
            .members
            .iter()
            .map(|member| (member.ident, member.wire))
            .collect();
        let theirs: Vec<(&str, &str)> = item
            .members
            .iter()
            .map(|member| (member.ident.as_str(), member.wire.as_str()))
            .collect();
        if mine != theirs {
            found.push(format!("{}: members {mine:?} != IDL {theirs:?}", spec.name));
        }
    }
    found
}

fn union_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = Vec::new();
    let mine: Vec<&str> = UNIONS.iter().map(|spec| spec.name).collect();
    let theirs: Vec<&str> = document
        .unions
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    if mine != theirs {
        found.push(format!("unions {mine:?} != IDL {theirs:?}"));
    }
    for spec in UNIONS {
        let Some(item) = document.unions.iter().find(|item| item.name == spec.name) else {
            continue;
        };
        let mine: Vec<(&str, &str)> = spec
            .variants
            .iter()
            .map(|variant| (variant.ident, variant.ty))
            .collect();
        let theirs: Vec<(&str, &str)> = item
            .variants
            .iter()
            .map(|(ident, ty)| (ident.as_str(), ty.as_str()))
            .collect();
        if mine != theirs {
            found.push(format!(
                "{}: variants {mine:?} != IDL {theirs:?}",
                spec.name
            ));
        }
    }
    found
}

fn handle_and_alias_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = Vec::new();
    let mine: Vec<(&str, &str)> = HANDLES
        .iter()
        .map(|spec| (spec.name, spec.prefix))
        .collect();
    let theirs: Vec<(&str, &str)> = document
        .handles
        .iter()
        .map(|(name, prefix)| (name.as_str(), prefix.as_str()))
        .collect();
    if mine != theirs {
        found.push(format!("handles {mine:?} != IDL {theirs:?}"));
    }

    let mine: Vec<(&str, &str, Option<&str>)> = ALIASES
        .iter()
        .map(|spec| (spec.name, spec.base, spec.pattern))
        .collect();
    let theirs: Vec<(&str, &str, Option<&str>)> = document
        .aliases
        .iter()
        .map(|(name, base, pattern)| (name.as_str(), base.as_str(), pattern.as_deref()))
        .collect();
    if mine != theirs {
        found.push(format!("aliases {mine:?} != IDL {theirs:?}"));
    }
    found
}

fn identity_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = Vec::new();
    let setting = |name: &str| {
        document
            .settings
            .get(name)
            .unwrap_or_else(|| panic!("the IDL declares protocol.{name}"))
            .clone()
    };

    if setting("version") != idl::Setting::Text(PROTOCOL_VERSION.to_owned()) {
        found.push(format!(
            "PROTOCOL_VERSION is {PROTOCOL_VERSION:?}, IDL says {:?}",
            setting("version")
        ));
    }
    if setting("idl_version") != idl::Setting::Text(IDL_VERSION.to_owned()) {
        found.push(format!(
            "IDL_VERSION is {IDL_VERSION:?}, IDL says {:?}",
            setting("idl_version")
        ));
    }
    if setting("majors_served")
        != idl::Setting::Ints(
            MAJORS_SERVED
                .iter()
                .map(|major| u64::from(*major))
                .collect(),
        )
    {
        found.push(format!(
            "MAJORS_SERVED is {MAJORS_SERVED:?}, IDL says {:?}",
            setting("majors_served")
        ));
    }
    let encodings: Vec<String> = ENCODINGS
        .iter()
        .map(|encoding| encoding.as_wire().to_owned())
        .collect();
    if setting("encodings") != idl::Setting::Texts(encodings.clone()) {
        found.push(format!(
            "ENCODINGS is {encodings:?}, IDL says {:?}",
            setting("encodings")
        ));
    }
    found
}

fn all_mismatches(document: &idl::Document) -> Vec<String> {
    let mut found = identity_mismatches(document);
    found.extend(operation_mismatches(document));
    found.extend(struct_mismatches(document));
    found.extend(enum_mismatches(document));
    found.extend(union_mismatches(document));
    found.extend(handle_and_alias_mismatches(document));
    found
}

fn assert_agrees(found: &[String]) {
    assert!(
        found.is_empty(),
        "the types and the IDL disagree:\n  {}",
        found.join("\n  ")
    );
}

// =====================================================================================
// The tests
// =====================================================================================

#[test]
fn the_idl_parses_to_the_shape_its_header_declares() {
    let document = document();
    // The IDL's §10 header states the registry's size in prose; if the parser silently
    // dropped a declaration, every comparison below would pass vacuously for it.
    assert_eq!(document.operations.len(), 83, "operations");
    assert_eq!(document.scalars.len(), 9, "scalars");
    assert_eq!(document.handles.len(), 19, "handles");
    // Protocol 3.1 (IDL 1.2) adds one alias (`AuditCorrelationId`), one enum
    // (`DataGrant`), two structs (`ServerReject`, `CapabilityProfile`), and four rules
    // (`audit.correlation`, `handshake.rejection`, `capability.profile_narrowing`,
    // `conformance.adapter_mapping`) — the RFC 0027 F1/F4/F5/F6/F7 payments. The
    // operation count is deliberately unmoved: no flag was paid with a new verb, so the
    // 72-row registry and RFC 0027's authority table are untouched.
    //
    // Protocol 3.2 (IDL 1.3) adds one struct (`FileComponent`) and four rules
    // (`encoding.canonical_form`, `encoding.union_tagging`,
    // `encoding.opaque_payloads`, `snapshot.file_components`) — the bn-i4aem
    // reconciliation and the bn-3bhkp codec decisions. The operation count is again
    // unmoved: no defect was paid with a new verb, and one that needs one
    // (`bn-i4aem` item 4, an evidence-edge append) is deferred to a registry edit in
    // plan §10.2, RFC 0027's table, and this file together.
    //
    // Protocol 3.3 (IDL 1.4) is that deferred registry edit, taken (bn-3sypm): the
    // operation count moves for the first time in this protocol's life, 72 -> 73, and
    // one rule arrives with it (`evidence.edge_identity`). `evidence.link` is RFC
    // 0038's F14 decision on the wire — the only operation that appends an
    // evidence-graph edge, and therefore the first artifact INV-004's `CHECKED_BY`
    // requirement has ever had. Its request and response bodies are anonymous, so the
    // named-struct count is unmoved. **These two numbers are the visibility the count
    // assertions exist for**: an operation cannot enter the protocol without moving
    // them here, in RFC 0027's distribution table, and in RFC 0026's verdict table.
    //
    // IDL 1.5 (bn-1h158) adds one rule, `handshake.bootstrap_encoding`, and nothing
    // else: no operation, alias, enum, struct, or union moves, because the rule
    // documents behavior every conforming daemon already has rather than declaring a
    // wire shape. `version` stays "3.3" with it — see `rule
    // versioning.compatible_change`'s five triggers, none of which this revision
    // pulls.
    //
    // Protocol 3.4 (IDL 1.6, bn-3jrtz) adds one struct — `CertificateRejection`, the
    // first declared `Error.data` shape (RFC 0026 F19) — and nothing else that these
    // counts see: F20, the same bundle's other rider, flips which of
    // `verification.start`'s two declared answer shapes a terminal identity lands on
    // and therefore moves no declaration at all. The operation count stays 73; the
    // struct count is the one number this bump moves, 45 -> 46.
    //
    // IDL 1.7 (bn-3080b) adds one rule, `subscription.delivery`, and nothing else:
    // 39 -> 40, with no operation, alias, enum, struct, or union moving. It is the
    // 1.5 shape — a rule and no declaration, so `version` stays "3.4" — with one
    // difference the rule's own body records: 1.5 wrote down what every conforming
    // daemon already did, while nothing had ever delivered an `events` frame, so 1.7
    // decides the three things a transport needs (what an event frame is, that one
    // delta is one frame however many of a connection's scopes select it, and which
    // deltas a scope selects) rather than reporting them.
    //
    // Protocol 3.6 (IDL 1.11, bn-3of5h) is the third registry edit, and the first that
    // moves **one** of these five numbers rather than four:
    // `workspace.create_by_reference`, 74 -> 75, in the `workspace` namespace it already
    // had, so 19 stays 19. Structs stay 47 and unions 2 — the request and response are
    // anonymous bodies over `Commitment`, `SnapshotEpochs`, `IntentHandle`,
    // `WorkspaceHandle`, and `Diagnostic`, every one already declared, which is what
    // "prefer existing declared types" costs when it is actually done. One rule arrives
    // (`snapshot.by_reference`, 43 -> 44), and it is the operation's whole semantics:
    // what the commitment is over, that resolving it means serving `workspace.create`,
    // which two members do not come from it, and that an unresolvable one is a denial.
    //
    // Protocol 3.5 (IDL 1.9, bn-1as8e) is the second registry edit this protocol has
    // ever taken: `whiteboard.compile`, 73 -> 74, in a **19th** namespace, 18 -> 19 —
    // plan §11.5's whiteboard compiler on the wire and the paragraph RFC 0038 deferred
    // when the compiler landed as a crate-level surface. One struct arrives with it
    // (`WhiteboardTaskProposal`, 46 -> 47) and one rule (`whiteboard.compilation`,
    // 41 -> 42). **These four numbers are the visibility the count assertions exist
    // for**: an operation cannot enter the protocol without moving them here, in RFC
    // 0027's distribution table, and in RFC 0026's verdict table, and a namespace cannot
    // enter without moving the fifth. The declared surface was searched before the verb
    // was minted — three of the last four revisions left `version` alone by serving
    // already-declared vocabulary — and nothing here served a note, which is why this
    // revision bumps and 1.5/1.7/1.8 did not.
    //
    // IDL 1.8 (bn-35l6g) adds one rule, `evidence.traversal`, and nothing else:
    // 40 -> 41, with no operation, alias, enum, struct, or union moving, so `version`
    // stays "3.4". It is the 1.7 shape again — a rule over surface the file already
    // declares. `EvidenceQuery.roots` has read "roots to traverse from" since 3.0 and
    // `max_depth` has been declared beside it just as long, but nothing said which way
    // a traversal walks a directed edge or what the bound counts, so a daemon could
    // serve `roots` as bare handle membership and drop `max_depth` on the floor. The
    // rule says an edge is walked in either direction, that depth counts edges from a
    // depth-0 root, and that an edge takes the greater of its endpoints' depths; the
    // count assertions see none of it, which is the point of stating the versioning
    // argument here rather than inferring it from a diff.
    //
    // IDL 1.13 (bn-3ncfp) adds one rule, `artifact_class.spelling`, and nothing else:
    // 44 -> 45. It fixes the spelling of plan §4.4's class vocabulary in fields that stay
    // `String`, because retyping them to an enum is `rule versioning.breaking_change`.
    // No operation, alias, enum, struct, or union moves, so `version` stays "3.6".
    //
    // IDL 1.14 (bn-ah1k8) adds five rules and nothing else: 45 -> 50. They declare the
    // domains of six `String` leaves the G1-02 sweep pinned (`benchmark.task_identity`,
    // `benchmark.graders`, `evidence.claim_identity`, `query.reuse_reasons`,
    // `query.invalidation_edges`); the fields keep their type, so `version` stays "3.6".
    //
    // Protocol 3.7 (IDL 1.15, bn-28kv4) adds one `optional` field,
    // `CapabilityDescriptor.instances`, and one rule, `capability.instance_scope`:
    // 50 -> 51. A field is not one of these five counts, so only the rule count moves.
    //
    // Protocol 3.8 (IDL 1.16, bn-3glnv) is the signing wire: eight operations, 75 -> 83,
    // six of them in a **20th** namespace, `signing`, and two in `intent`; one alias,
    // `SignerHandle`, 9 -> 10; three enums, `SignedArtifactKind`, `RevocationReason`, and
    // `SignatureOutcome`, 34 -> 37; and three rules, `signing.identities`,
    // `signing.verification`, and `intent.bundles`, 51 -> 54. Its one `optional` field,
    // `EvidenceGetResponse.signature`, moves none of these counts. No named struct is added:
    // every new body is anonymous.
    //
    // Protocol 3.9 (IDL 1.17, bn-18w74) adds one error code, `OutcomeUnknown`, to the
    // `@open` `ErrorCode` — a member, which these counts do not see — and one rule,
    // `signing.custody`, 54 -> 55.
    assert_eq!(document.aliases.len(), 10, "aliases");
    assert_eq!(document.enums.len(), 37, "enums");
    assert_eq!(document.structs.len(), 47, "structs");
    assert_eq!(document.unions.len(), 2, "unions");
    assert_eq!(document.rules.len(), 55, "rules");
    let namespaces: std::collections::BTreeSet<&str> = document
        .operations
        .iter()
        .map(|operation| operation.namespace.as_str())
        .collect();
    assert_eq!(namespaces.len(), 20, "namespaces");
}

#[test]
fn protocol_identity_matches_the_idl() {
    assert_agrees(&identity_mismatches(&document()));
}

#[test]
fn the_operation_set_is_exactly_the_idl_registry() {
    let document = document();
    let mine: Vec<&str> = OPERATIONS.iter().map(|spec| spec.name).collect();
    let theirs: Vec<&str> = document
        .operations
        .iter()
        .map(|operation| operation.name.as_str())
        .collect();
    assert_eq!(mine, theirs);
    assert_eq!(OPERATION_COUNT, 83);
}

#[test]
fn authorities_and_annotations_match_the_idl() {
    let document = document();
    let found: Vec<String> = operation_mismatches(&document)
        .into_iter()
        .filter(|item| item.contains("authority") || item.contains("annotations"))
        .collect();
    assert_agrees(&found);
}

#[test]
fn request_and_response_structs_match_the_idl() {
    let document = document();
    let found: Vec<String> = operation_mismatches(&document)
        .into_iter()
        .filter(|item| item.contains("request") || item.contains("response"))
        .collect();
    assert_agrees(&found);
}

#[test]
fn error_verdict_and_events_clauses_match_the_idl() {
    let document = document();
    let found: Vec<String> = operation_mismatches(&document)
        .into_iter()
        .filter(|item| {
            item.contains("errors") || item.contains("verdict") || item.contains("events")
        })
        .collect();
    assert_agrees(&found);
}

#[test]
fn named_structs_match_the_idl() {
    assert_agrees(&struct_mismatches(&document()));
}

#[test]
fn enums_match_the_idl_member_for_member() {
    assert_agrees(&enum_mismatches(&document()));
}

#[test]
fn unions_handles_and_aliases_match_the_idl() {
    let document = document();
    let mut found = union_mismatches(&document);
    found.extend(handle_and_alias_mismatches(&document));
    assert_agrees(&found);
}

#[test]
fn the_whole_registry_agrees_with_the_idl() {
    assert_agrees(&all_mismatches(&document()));
}

#[test]
fn anonymous_bodies_carry_the_idl_generated_name() {
    // The rule is stated in the IDL header: "an anonymous `request`/`response`/`events`
    // body is a struct whose generated name is the operation name in PascalCase with the
    // suffix `Request`, `Response`, or `Event`".
    assert_eq!(
        idl::generated_name("workspace.create", "Request"),
        "WorkspaceCreateRequest"
    );
    assert_eq!(
        idl::generated_name("intent.propose_revision", "Response"),
        "IntentProposeRevisionResponse"
    );
    let document = document();
    for operation in &document.operations {
        if let idl::Body::Anonymous(_) = operation.request {
            let spec = continuumd::protocol::registry::operation(&operation.name)
                .expect("every IDL operation is registered");
            assert_eq!(
                spec.request.name,
                idl::generated_name(&operation.name, "Request")
            );
        }
    }
}

#[test]
fn the_error_taxonomy_is_the_complete_idl_set() {
    use continuumd::protocol::vocabulary::ErrorCode;

    let document = document();
    let declared = document
        .enums
        .iter()
        .find(|item| item.name == "ErrorCode")
        .expect("the IDL declares ErrorCode");
    let mine: Vec<&str> = ErrorCode::ALL.iter().map(|code| code.ident()).collect();
    let theirs: Vec<&str> = declared
        .members
        .iter()
        .map(|member| member.ident.as_str())
        .collect();
    assert_eq!(mine, theirs, "the §10.3 taxonomy");
    // 20 through 3.8; 3.9 (bn-18w74) adds `OutcomeUnknown`.
    assert_eq!(mine.len(), 21);

    // The seven codes the PR 5 slice names explicitly, asserted by name so that losing
    // one is a failure here and not only inside a list comparison.
    for code in [
        ErrorCode::AcceptanceChainInvalid,
        ErrorCode::StatusConflict,
        ErrorCode::QuotaExhausted,
        ErrorCode::EpochUnsupported,
        ErrorCode::PublicationAborted,
        ErrorCode::StaleSnapshot,
    ] {
        assert!(mine.contains(&code.ident()), "{code:?} is in the taxonomy");
    }
    // `Redacted` is a struct, not an error code: the §10.3 taxonomy reports redaction as
    // a typed stub carried beside the result, never as a failure (plan §4.5).
    assert!(!mine.contains(&"Redacted"));
    assert!(
        continuumd::protocol::registry::NAMED_STRUCTS
            .iter()
            .any(|spec| spec.name == "Redacted")
    );
}

/// `rule errors.common`'s body, as raw text.
///
/// The parser above drops `"""` blocks — they carry normative prose, not structure — so a
/// rule's *content* is invisible to every other comparison in this file. That is a hole
/// wherever a rule states something the types transcribe, and `errors::COMMON` is exactly
/// such a transcription: bn-i4aem moved a code into that union and nothing would have
/// caught the mirror staying behind.
fn rule_body(name: &str) -> String {
    let source = std::fs::read_to_string(IDL_PATH).expect("the IDL is readable");
    let start = source
        .find(&format!("rule {name} {{"))
        .unwrap_or_else(|| panic!("the IDL declares `rule {name}`"));
    let open = source[start..].find("\"\"\"").expect("a rule body opens") + start + 3;
    let close = source[open..].find("\"\"\"").expect("a rule body closes") + open;
    source[open..close].to_owned()
}

#[test]
fn the_common_error_union_agrees_with_the_rule_that_states_it() {
    use continuumd::daemon::errors::{COMMON, MUTATION, SNAPSHOT};
    use continuumd::protocol::vocabulary::ErrorCode;

    // The rule states three clauses, separated by `;`. Each names its codes in backticks,
    // and the codes are the only backticked tokens in it that are `ErrorCode` members —
    // `@mutation` and `snapshot` are not.
    //
    // Only the union sentence is read: the rest of the rule is the *record* of why the
    // 3.2 payment was made, and it names codes in prose. The sentence ends where the
    // clause-meaning sentence begins.
    let body = rule_body("errors.common");
    let union = &body[..body
        .find("An operation's")
        .expect("the rule states what a clause means")];
    let clauses: Vec<&str> = union.split(';').collect();
    assert_eq!(clauses.len(), 3, "the rule states three clauses");

    let codes_in = |text: &str| -> Vec<&'static str> {
        let mut found = Vec::new();
        for member in ErrorCode::ALL {
            let quoted = format!("`{}`", member.ident());
            if text.contains(&quoted) {
                found.push(member.ident());
            }
        }
        found.sort_unstable();
        found
    };
    let mine = |codes: &[ErrorCode]| -> Vec<&'static str> {
        let mut names: Vec<&'static str> = codes.iter().map(|code| code.ident()).collect();
        names.sort_unstable();
        names
    };

    assert_eq!(
        codes_in(clauses[0]),
        mine(COMMON),
        "the always-admissible codes"
    );
    assert_eq!(
        codes_in(clauses[1]),
        mine(MUTATION),
        "the `@mutation` codes"
    );
    assert_eq!(
        codes_in(clauses[2]),
        mine(SNAPSHOT),
        "the non-null-`snapshot` code"
    );
    // The 3.2 payment, named so that losing it is a failure here and not only inside a
    // list comparison: `rule errors.unsupported_surface` requires this code of any
    // operation whose lane has not shipped, and 25 of the 72 could not return it before.
    assert!(COMMON.contains(&ErrorCode::UnsupportedSemanticFeature));
}

#[test]
fn every_enum_agrees_with_itself() {
    // `ALL` and `MEMBERS` are emitted from one repetition, so they are index-aligned by
    // construction. This asserts the construction, because `ProtocolEnum::ident` reads
    // one through the other.
    for spec in ENUMS {
        assert!(!spec.members.is_empty(), "{} has members", spec.name);
    }
    use continuumd::protocol::vocabulary::{AuthorityLevel, ErrorCode, EvidenceKind};
    for (index, member) in AuthorityLevel::MEMBERS.iter().enumerate() {
        assert_eq!(AuthorityLevel::ALL[index].as_wire(), member.wire);
        assert_eq!(AuthorityLevel::ALL[index].ident(), member.ident);
    }
    assert_eq!(AuthorityLevel::ReviseIntent.ident(), "revise_intent");
    assert_eq!(AuthorityLevel::ReviseIntent.as_wire(), "revise-intent");
    assert_eq!(
        EvidenceKind::BoundedSchedules.as_wire(),
        "bounded-schedules"
    );
    assert_eq!(ErrorCode::StaleSnapshot.as_wire(), "StaleSnapshot");
}

// --- non-vacuity ---------------------------------------------------------------------

/// Apply one textual mutation, asserting that it actually changed the document.
fn mutate(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "the mutation anchor {from:?} must occur exactly once"
    );
    source.replace(from, to)
}

#[test]
fn the_check_is_not_vacuous() {
    // Every gate in this project runs its self-test first so it cannot pass vacuously
    // (`just boundaries`, `just governance`). This is that self-test: each mutation is a
    // drift the check exists to catch, applied to the IDL text rather than to the crate,
    // and each MUST be reported.
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    assert_agrees(&all_mismatches(&idl::parse(&source)));

    let cases: [(&str, &str, &str); 7] = [
        (
            "a renamed request field",
            "components: SnapshotComponents required;",
            "component: SnapshotComponents required;",
        ),
        (
            // Anchored through `overlay` because protocol 3.6 gave `seal: Bool optional`
            // a second declaration site: `workspace.create_by_reference` carries the same
            // member, and a mutation anchor that matches twice is not an anchor.
            "a changed presence marker",
            "    overlay: list<FileOverlay> optional;\n    /// Seal the snapshot on creation.\n    seal: Bool optional;",
            "    overlay: list<FileOverlay> optional;\n    /// Seal the snapshot on creation.\n    seal: Bool nullable;",
        ),
        (
            "a changed field type",
            "  root_digest: Commitment required;",
            "  root_digest: String required;",
        ),
        (
            "a changed authority level",
            "operation intent.lock {\n  authority revise_intent;",
            "operation intent.lock {\n  authority promote;",
        ),
        (
            "a dropped error code",
            "errors [IntentMutationDenied, AcceptanceChainInvalid, PolicyGateFailed];",
            "errors [IntentMutationDenied, PolicyGateFailed];",
        ),
        (
            "a renamed enum wire token",
            "  revise_intent = \"revise-intent\",",
            "  revise_intent = \"revise_intent\",",
        ),
        (
            "a removed annotation",
            "@readonly @streaming\noperation task.subscribe {",
            "@readonly\noperation task.subscribe {",
        ),
    ];

    for (label, from, to) in cases {
        let mutated = mutate(&source, from, to);
        let found = all_mismatches(&idl::parse(&mutated));
        assert!(
            !found.is_empty(),
            "{label}: the mutated IDL was accepted; the check is vacuous for this class"
        );
    }
}

#[test]
fn a_removed_operation_is_reported() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    let start = source
        .find("operation workspace.seal {")
        .expect("workspace.seal is declared");
    let end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset + 3)
        .expect("the declaration is terminated");
    let mut mutated = String::with_capacity(source.len());
    mutated.push_str(&source[..start]);
    mutated.push_str(&source[end..]);

    let document = idl::parse(&mutated);
    assert_eq!(document.operations.len(), 82);
    let found = all_mismatches(&document);
    assert!(
        found
            .iter()
            .any(|item| item.contains("workspace.seal") || item.contains("82")),
        "removing an operation must be reported, got {found:?}"
    );
}
