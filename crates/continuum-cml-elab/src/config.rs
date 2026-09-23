//! The run configuration: finite instantiations of a model's sorts and values for its
//! constants, read from its wire form (RFC 0003 "Configurations and bounds").
//!
//! Decision: RFC 0003 "Configurations and bounds" and its correction 3, whose shape is
//! `notes/plan/schemas/run-config.schema.json` (class
//! `https://continuum.dev/schema/run-config.json`, schema epoch 1). Schemas decide
//! (INV-003): this reader accepts exactly the documents that schema admits and refuses
//! everything else with a typed [`ConfigError`]; the typing of each binding against
//! the model it configures is the lowering's ([`crate::lower::lower_configured`]).
//!
//! # Reading
//!
//! The bytes are parsed by the workspace's one strict JSON reader
//! (`continuum_intent::canonical_json`): a duplicate key, a floating-point number,
//! trailing bytes, and nesting past its depth bound are refused there. Every object
//! here is closed (`additionalProperties: false`), every value is exactly one tag of
//! the value union, a sort's elements are non-empty, duplicate-free printable ASCII
//! names of at most [`MAX_NAME_BYTES`] bytes, at most [`MAX_SORT_ELEMENTS`] per sort,
//! and a `set` or a `map`'s keys are duplicate-free. The input is at most
//! [`MAX_CONFIG_BYTES`] bytes, so reading is linear in a bounded input.
//!
//! # Identity
//!
//! [`ConfigIdentity`] is content identity (ADR-0013): the canonical ID5 encoding of the
//! document after the one normalization that does not change its meaning — a `set`'s
//! elements and a `map`'s entries sorted by their canonical encoding — behind the tag
//! [`CONFIG_IDENTITY_TAG`]. Key order, whitespace, escapes, and set order therefore do
//! not change it; every binding, and the order of a sort's elements (which is their
//! integer encoding), does. [`RunIdentity`] is one typed handle for a lowered model under
//! a configuration: the model's own identity and the configuration's, length-prefixed
//! behind [`RUN_IDENTITY_TAG`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use continuum_intent::canonical_json::{Json, JsonError};
use continuum_model_core::ModelIdentity;
use continuum_model_core::ident::MAX_IDENT_BYTES;

/// The run configuration's schema class identity.
pub const RUN_CONFIG_SCHEMA_ID: &str = "https://continuum.dev/schema/run-config.json";

/// The schema epoch this reader implements.
pub const RUN_CONFIG_SCHEMA_EPOCH: i64 = 1;

/// The most elements one sort may be instantiated with.
pub const MAX_SORT_ELEMENTS: usize = 1 << 16;

/// The longest element name, in bytes: the model's own name limit.
pub const MAX_NAME_BYTES: usize = MAX_IDENT_BYTES;

/// The largest configuration document read, in bytes (the parser's source bound).
pub const MAX_CONFIG_BYTES: usize = 16 << 20;

/// The tag every [`ConfigIdentity`] starts with.
pub const CONFIG_IDENTITY_TAG: &[u8] = b"continuum-run-config/1\n";

/// The tag every [`RunIdentity`] starts with.
pub const RUN_IDENTITY_TAG: &[u8] = b"continuum-run/1\n";

/// A value bound to a model constant: one tag of the schema's value union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    /// `{"int": n}`.
    Int(i64),
    /// `{"bool": b}`.
    Bool(bool),
    /// `{"str": s}`.
    Str(String),
    /// `{"elem": {"sort": S, "name": a}}`: element `a` of the sort `S`.
    Elem {
        /// The sort.
        sort: String,
        /// The element name.
        name: String,
    },
    /// `{"variant": {"enum": E, "name": v}}`: variant `v` of the enumeration `E`.
    Variant {
        /// The enumeration.
        enumeration: String,
        /// The variant.
        name: String,
    },
    /// `{"tuple": [v, …]}`, two or more components.
    Tuple(Vec<ConfigValue>),
    /// `{"record": {f: v, …}}`.
    Record(BTreeMap<String, ConfigValue>),
    /// `{"set": [v, …]}`, duplicate-free, held in canonical order.
    Set(Vec<ConfigValue>),
    /// `{"seq": [v, …]}`.
    Seq(Vec<ConfigValue>),
    /// `{"map": [[k, v], …]}`, keys duplicate-free, held in canonical key order.
    Map(Vec<(ConfigValue, ConfigValue)>),
    /// `{"none": null}`.
    None,
    /// `{"some": v}`.
    Some(Box<ConfigValue>),
}

impl ConfigValue {
    /// The value's size in budget nodes, for the lowering's budgets: one per value
    /// node, plus the text it owns (a string, an element or variant name and its
    /// sort or enumeration, a record's field names) at one node per
    /// `crate::budget::BYTES_PER_NODE` bytes, so a long string is not one node.
    /// Iterative.
    ///
    /// Computed once per constant while the document is read (bounded there by the
    /// reader's depth and size caps) and stored: see [`RunConfig::constant_size`].
    fn size(&self) -> usize {
        let text = |t: &str| crate::budget::text_cost(t.len());
        let mut size = 0_usize;
        let mut stack = vec![self];
        while let Some(v) = stack.pop() {
            size = size.saturating_add(1);
            match v {
                Self::Str(t) => size = size.saturating_add(text(t)),
                Self::Elem { sort: a, name: b }
                | Self::Variant {
                    enumeration: a,
                    name: b,
                } => size = size.saturating_add(text(a)).saturating_add(text(b)),
                Self::Tuple(xs) | Self::Set(xs) | Self::Seq(xs) => stack.extend(xs),
                Self::Record(fields) => {
                    for (k, x) in fields {
                        size = size.saturating_add(text(k));
                        stack.push(x);
                    }
                }
                Self::Map(entries) => {
                    for (k, x) in entries {
                        stack.push(k);
                        stack.push(x);
                    }
                }
                Self::Some(x) => stack.push(x),
                _ => {}
            }
        }
        size
    }
}

/// A parsed, shape-checked run configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    model: String,
    sorts: BTreeMap<String, Vec<String>>,
    constants: BTreeMap<String, ConfigValue>,
    /// Each constant's value size, computed once while reading.
    sizes: BTreeMap<String, usize>,
    identity: Arc<ConfigIdentity>,
}

impl RunConfig {
    /// Read a configuration from its wire form.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] naming the JSON path of the first defect.
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(ConfigError::new(ConfigErrorKind::TooLarge, "$"));
        }
        let json = Json::parse(bytes).map_err(|e| ConfigError {
            kind: ConfigErrorKind::Json(e),
            path: "$".to_owned(),
        })?;
        let top = object(
            &json,
            "$",
            &["schema_id", "schema_epoch", "model", "sorts", "constants"],
        )?;
        match top.get("schema_id").and_then(Json::as_str) {
            Some(RUN_CONFIG_SCHEMA_ID) => {}
            _ => return Err(ConfigError::new(ConfigErrorKind::Header, "$.schema_id")),
        }
        match top.get("schema_epoch").and_then(Json::as_integer) {
            Some(RUN_CONFIG_SCHEMA_EPOCH) => {}
            _ => return Err(ConfigError::new(ConfigErrorKind::Header, "$.schema_epoch")),
        }
        let model = top
            .get("model")
            .and_then(Json::as_str)
            .filter(|m| is_name(m))
            .ok_or_else(|| shape("$.model", "a printable ASCII name of at most 128 bytes"))?
            .to_owned();

        let mut sorts = BTreeMap::new();
        let mut sorts_json = BTreeMap::new();
        let sort_map = top
            .get("sorts")
            .and_then(Json::as_object)
            .ok_or_else(|| shape("$.sorts", "an object"))?;
        for (sort, spec) in sort_map {
            let path = format!("$.sorts.{sort}");
            if !is_identifier(sort) {
                return Err(shape(&path, "a sort name is a CML identifier"));
            }
            let spec = object(spec, &path, &["elements"])?;
            let elements = spec
                .get("elements")
                .and_then(Json::as_array)
                .ok_or_else(|| shape(&format!("{path}.elements"), "an array"))?;
            if elements.is_empty() {
                return Err(shape(&format!("{path}.elements"), "at least one element"));
            }
            if elements.len() > MAX_SORT_ELEMENTS {
                return Err(ConfigError::new(
                    ConfigErrorKind::SortTooLarge,
                    &format!("{path}.elements"),
                ));
            }
            let mut seen = BTreeSet::new();
            let mut names = Vec::with_capacity(elements.len());
            for (i, e) in elements.iter().enumerate() {
                let at = format!("{path}.elements[{i}]");
                let name = e
                    .as_str()
                    .filter(|n| is_name(n))
                    .ok_or_else(|| shape(&at, "a printable ASCII name of at most 128 bytes"))?;
                if !seen.insert(name) {
                    return Err(ConfigError::new(ConfigErrorKind::Duplicate, &at));
                }
                names.push(name.to_owned());
            }
            sorts_json.insert(
                sort.clone(),
                Json::Object(BTreeMap::from([(
                    "elements".to_owned(),
                    Json::Array(names.iter().cloned().map(Json::String).collect()),
                )])),
            );
            sorts.insert(sort.clone(), names);
        }

        let mut constants = BTreeMap::new();
        let mut sizes = BTreeMap::new();
        let mut constants_json = BTreeMap::new();
        let const_map = top
            .get("constants")
            .and_then(Json::as_object)
            .ok_or_else(|| shape("$.constants", "an object"))?;
        for (name, value) in const_map {
            let path = format!("$.constants.{name}");
            if !is_identifier(name) {
                return Err(shape(&path, "a constant name is a CML identifier"));
            }
            let (v, normal) = read_value(value, &path)?;
            sizes.insert(name.clone(), v.size());
            constants.insert(name.clone(), v);
            constants_json.insert(name.clone(), normal);
        }

        let normal = Json::Object(BTreeMap::from([
            (
                "schema_id".to_owned(),
                Json::String(RUN_CONFIG_SCHEMA_ID.to_owned()),
            ),
            (
                "schema_epoch".to_owned(),
                Json::Integer(RUN_CONFIG_SCHEMA_EPOCH),
            ),
            ("model".to_owned(), Json::String(model.clone())),
            ("sorts".to_owned(), Json::Object(sorts_json)),
            ("constants".to_owned(), Json::Object(constants_json)),
        ]));
        let mut bytes = CONFIG_IDENTITY_TAG.to_vec();
        normal.write_canonical(&mut bytes);
        Ok(Self {
            model,
            sorts,
            constants,
            sizes,
            identity: Arc::new(ConfigIdentity { bytes }),
        })
    }

    /// The model the configuration is written for.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Each sort's elements, in their encoding order (element `i` is the integer `i`).
    #[must_use]
    pub fn sorts(&self) -> &BTreeMap<String, Vec<String>> {
        &self.sorts
    }

    /// The constant bindings.
    #[must_use]
    pub fn constants(&self) -> &BTreeMap<String, ConfigValue> {
        &self.constants
    }

    /// The size of constant `name`'s value in budget nodes (one per value node, text
    /// by bytes), computed once while the document was read: a lookup, not a walk.
    #[must_use]
    pub fn constant_size(&self, name: &str) -> Option<usize> {
        self.sizes.get(name).copied()
    }

    /// The configuration's content identity.
    #[must_use]
    pub fn identity(&self) -> &ConfigIdentity {
        &self.identity
    }

    /// The configuration's content identity, shared: a lowered model and its run
    /// identity hold this one allocation rather than copies of its bytes.
    #[must_use]
    pub fn shared_identity(&self) -> Arc<ConfigIdentity> {
        Arc::clone(&self.identity)
    }
}

/// The content identity of a run configuration: its normalized canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigIdentity {
    bytes: Vec<u8>,
}

impl ConfigIdentity {
    /// The canonical encoding, tag included.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One typed content identity for a lowered model under a run configuration: the
/// model's [`ModelIdentity`] and the [`ConfigIdentity`].
///
/// It holds the two identities rather than a concatenation of their bytes, sharing the
/// configuration's with the [`RunConfig`] it came from, so making one copies neither.
/// Its canonical encoding ([`RunIdentity::encode`]) is each part length-prefixed (u64
/// big-endian) behind [`RUN_IDENTITY_TAG`]; that encoding is injective, so equality of
/// the pair (what `==` compares) is equality of the encoding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunIdentity {
    model: ModelIdentity,
    config: Arc<ConfigIdentity>,
}

impl RunIdentity {
    /// The run identity of `model` under `config`. Takes both by value: nothing is
    /// copied.
    #[must_use]
    pub fn of(model: ModelIdentity, config: Arc<ConfigIdentity>) -> Self {
        Self { model, config }
    }

    /// The model half.
    #[must_use]
    pub fn model(&self) -> &ModelIdentity {
        &self.model
    }

    /// The configuration half.
    #[must_use]
    pub fn config(&self) -> &ConfigIdentity {
        &self.config
    }

    /// The canonical encoding: allocated on request, proportional to the two parts.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = RUN_IDENTITY_TAG.to_vec();
        for part in [self.model.as_bytes(), self.config.as_bytes()] {
            bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
            bytes.extend_from_slice(part);
        }
        bytes
    }
}

/// Why a configuration document was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// What went wrong.
    pub kind: ConfigErrorKind,
    /// Where: a JSON path such as `$.constants.Nodes.set[2]`.
    pub path: String,
}

impl ConfigError {
    fn new(kind: ConfigErrorKind, path: &str) -> Self {
        Self {
            kind,
            path: path.to_owned(),
        }
    }

    /// A stable, machine-readable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match &self.kind {
            ConfigErrorKind::TooLarge => "cml.config.too_large",
            ConfigErrorKind::Json(_) => "cml.config.json",
            ConfigErrorKind::Header => "cml.config.header",
            ConfigErrorKind::Shape(_) => "cml.config.shape",
            ConfigErrorKind::Duplicate => "cml.config.duplicate",
            ConfigErrorKind::SortTooLarge => "cml.config.sort_too_large",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: ", self.path, self.code())?;
        match &self.kind {
            ConfigErrorKind::TooLarge => {
                write!(f, "the document is larger than {MAX_CONFIG_BYTES} bytes")
            }
            ConfigErrorKind::Json(e) => write!(f, "{e}"),
            ConfigErrorKind::Header => write!(
                f,
                "not a {RUN_CONFIG_SCHEMA_ID} document of schema epoch {RUN_CONFIG_SCHEMA_EPOCH}"
            ),
            ConfigErrorKind::Shape(what) => write!(f, "expected {what}"),
            ConfigErrorKind::Duplicate => write!(f, "a duplicate element, set member, or map key"),
            ConfigErrorKind::SortTooLarge => {
                write!(f, "a sort has more than {MAX_SORT_ELEMENTS} elements")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// The kinds of [`ConfigError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigErrorKind {
    /// The document is larger than [`MAX_CONFIG_BYTES`].
    TooLarge,
    /// The bytes are not one strict JSON document.
    Json(JsonError),
    /// The `schema_id` or `schema_epoch` is not this reader's.
    Header,
    /// The document does not have the schema's shape; the text says what was expected.
    Shape(String),
    /// A sort element, a set member, or a map key appears twice.
    Duplicate,
    /// A sort has more than [`MAX_SORT_ELEMENTS`] elements.
    SortTooLarge,
}

fn shape(path: &str, what: &str) -> ConfigError {
    ConfigError::new(ConfigErrorKind::Shape(what.to_owned()), path)
}

/// Printable ASCII (`!`..`~`), one to [`MAX_NAME_BYTES`] bytes.
fn is_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_NAME_BYTES && s.bytes().all(|b| b.is_ascii_graphic())
}

/// A CML identifier: a letter or `_`, then letters, digits, or `_`.
fn is_identifier(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && s.len() <= MAX_NAME_BYTES
}

/// `json` as an object whose keys are exactly `keys` (all required, none other).
fn object<'j>(
    json: &'j Json,
    path: &str,
    keys: &[&str],
) -> Result<&'j BTreeMap<String, Json>, ConfigError> {
    let map = json.as_object().ok_or_else(|| shape(path, "an object"))?;
    for k in map.keys() {
        if !keys.contains(&k.as_str()) {
            return Err(shape(&format!("{path}.{k}"), "no such property"));
        }
    }
    for k in keys {
        if !map.contains_key(*k) {
            return Err(shape(&format!("{path}.{k}"), "a required property"));
        }
    }
    Ok(map)
}

/// One tagged value, and its normalized JSON (sets and maps in canonical order).
/// Recursion is bounded by the JSON reader's depth bound.
fn read_value(json: &Json, path: &str) -> Result<(ConfigValue, Json), ConfigError> {
    let map = json
        .as_object()
        .filter(|m| m.len() == 1)
        .ok_or_else(|| shape(path, "a value: an object with exactly one tag"))?;
    let Some((tag, body)) = map.iter().next() else {
        return Err(shape(path, "a value"));
    };
    let at = format!("{path}.{tag}");
    let wrap = |normal: Json| Json::Object(BTreeMap::from([(tag.clone(), normal)]));
    let (value, normal) = match tag.as_str() {
        "int" => {
            let n = body.as_integer().ok_or_else(|| shape(&at, "an integer"))?;
            (ConfigValue::Int(n), Json::Integer(n))
        }
        "bool" => {
            let b = body.as_bool().ok_or_else(|| shape(&at, "a boolean"))?;
            (ConfigValue::Bool(b), Json::Bool(b))
        }
        "str" => {
            let s = body.as_str().ok_or_else(|| shape(&at, "a string"))?;
            (ConfigValue::Str(s.to_owned()), Json::String(s.to_owned()))
        }
        "elem" | "variant" => {
            let owner = if tag == "elem" { "sort" } else { "enum" };
            let fields = object(body, &at, &[owner, "name"])?;
            let o = fields
                .get(owner)
                .and_then(Json::as_str)
                .filter(|s| is_identifier(s))
                .ok_or_else(|| shape(&format!("{at}.{owner}"), "a CML identifier"))?;
            // The schema: an element's name is a printable ASCII `name`, a variant's is
            // a CML `identifier` (run-config.schema.json `$defs/value`).
            let n = fields
                .get("name")
                .and_then(Json::as_str)
                .filter(|s| {
                    if tag == "elem" {
                        is_name(s)
                    } else {
                        is_identifier(s)
                    }
                })
                .ok_or_else(|| {
                    shape(
                        &format!("{at}.name"),
                        if tag == "elem" {
                            "a printable ASCII name"
                        } else {
                            "a CML identifier"
                        },
                    )
                })?;
            let normal = Json::Object(BTreeMap::from([
                (owner.to_owned(), Json::String(o.to_owned())),
                ("name".to_owned(), Json::String(n.to_owned())),
            ]));
            let value = if tag == "elem" {
                ConfigValue::Elem {
                    sort: o.to_owned(),
                    name: n.to_owned(),
                }
            } else {
                ConfigValue::Variant {
                    enumeration: o.to_owned(),
                    name: n.to_owned(),
                }
            };
            (value, normal)
        }
        "tuple" | "seq" | "set" => {
            let items = body.as_array().ok_or_else(|| shape(&at, "an array"))?;
            if tag == "tuple" && items.len() < 2 {
                return Err(shape(&at, "at least two components"));
            }
            let mut read = Vec::with_capacity(items.len());
            for (i, x) in items.iter().enumerate() {
                read.push(read_value(x, &format!("{at}[{i}]"))?);
            }
            if tag == "set" {
                let mut keyed: Vec<(Vec<u8>, (ConfigValue, Json))> = read
                    .into_iter()
                    .map(|(v, n)| (n.to_canonical_bytes(), (v, n)))
                    .collect();
                keyed.sort_by(|a, b| a.0.cmp(&b.0));
                if keyed
                    .windows(2)
                    .any(|w| w.first().map(|a| &a.0) == w.get(1).map(|b| &b.0))
                {
                    return Err(ConfigError::new(ConfigErrorKind::Duplicate, &at));
                }
                read = keyed.into_iter().map(|(_, x)| x).collect();
            }
            let (values, normals): (Vec<ConfigValue>, Vec<Json>) = read.into_iter().unzip();
            let value = match tag.as_str() {
                "tuple" => ConfigValue::Tuple(values),
                "seq" => ConfigValue::Seq(values),
                _ => ConfigValue::Set(values),
            };
            (value, Json::Array(normals))
        }
        "record" => {
            let fields = body.as_object().ok_or_else(|| shape(&at, "an object"))?;
            let mut values = BTreeMap::new();
            let mut normals = BTreeMap::new();
            for (f, x) in fields {
                if !is_identifier(f) {
                    return Err(shape(
                        &format!("{at}.{f}"),
                        "a field name is a CML identifier",
                    ));
                }
                let (v, n) = read_value(x, &format!("{at}.{f}"))?;
                values.insert(f.clone(), v);
                normals.insert(f.clone(), n);
            }
            (ConfigValue::Record(values), Json::Object(normals))
        }
        "map" => {
            let entries = body.as_array().ok_or_else(|| shape(&at, "an array"))?;
            let mut keyed = Vec::with_capacity(entries.len());
            for (i, e) in entries.iter().enumerate() {
                let here = format!("{at}[{i}]");
                let pair = e
                    .as_array()
                    .filter(|p| p.len() == 2)
                    .ok_or_else(|| shape(&here, "a [key, value] pair"))?;
                let (Some(k), Some(x)) = (pair.first(), pair.get(1)) else {
                    return Err(shape(&here, "a [key, value] pair"));
                };
                let (kv, kn) = read_value(k, &format!("{here}[0]"))?;
                let (xv, xn) = read_value(x, &format!("{here}[1]"))?;
                keyed.push((kn.to_canonical_bytes(), (kv, xv), (kn, xn)));
            }
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            if keyed
                .windows(2)
                .any(|w| w.first().map(|a| &a.0) == w.get(1).map(|b| &b.0))
            {
                return Err(ConfigError::new(ConfigErrorKind::Duplicate, &at));
            }
            let mut values = Vec::with_capacity(keyed.len());
            let mut normals = Vec::with_capacity(keyed.len());
            for (_, v, (kn, xn)) in keyed {
                values.push(v);
                normals.push(Json::Array(vec![kn, xn]));
            }
            (ConfigValue::Map(values), Json::Array(normals))
        }
        "none" => {
            if *body != Json::Null {
                return Err(shape(&at, "null"));
            }
            (ConfigValue::None, Json::Null)
        }
        "some" => {
            let (v, n) = read_value(body, &at)?;
            (ConfigValue::Some(Box::new(v)), n)
        }
        _ => return Err(shape(&at, "one of the value tags")),
    };
    Ok((value, wrap(normal)))
}
