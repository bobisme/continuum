//! Version-aware request decoding: a request field the negotiated version does not define
//! is ignored, not decoded (bn-7xz8v, review cr-88az2y).
//!
//! > Servers MUST ignore unknown optional request fields and MUST NOT emit fields the
//! > negotiated version does not define.
//! >
//! > — `rule versioning.compatible_change`
//!
//! A field dated after the negotiated version is unknown to that version. The strict codec
//! would still decode it, so a 3.1 request whose `file_components` held a value of the
//! wrong type was refused, where 3.1 says the field does not exist. This module removes
//! such a field from the parsed document before the typed decode, so its value is never
//! interpreted. Every field the version defines keeps the strict codec.
//!
//! # What is skipped, and what is not
//!
//! - **Framing is never skipped.** The bytes are parsed as one canonical document first,
//!   by the same bounded parser as every request (`json::MAX_DEPTH`, the frame bound the
//!   transport applies before). A document that does not parse is refused at every
//!   version, because a skip that cannot find a field's end is a broken frame.
//! - **The skipped set is derived, not listed.** It is `protocol::since::FIELDS` above
//!   the negotiated version, and the path to each owner is found through the registry's
//!   own struct specs. A newly dated request field is skipped here with no edit.
//! - **Cost.** Nothing is done unless a dated field above the version is present. The
//!   parsed tree is stripped in place: a skipped value is dropped, never copied, so the
//!   memory held is the parse's and no more. The walk descends only through types that
//!   can reach a dated owner; its depth is bounded by the parser's `MAX_DEPTH` whatever
//!   the type graph, and `the_reaching_subgraph_has_no_cycle` pins that the graph itself
//!   is acyclic today, so the depth is also fixed by the types.
//! - **Unions are edges.** A union's wire form is a one-key object whose value is the
//!   variant's type, and the walk follows it (`registry::UNIONS`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::{CodecError, Document, ProtocolValue};
use crate::protocol::registry::{NAMED_STRUCTS, OPERATIONS, UNIONS};
use crate::protocol::scalar::ProtocolVersion;
use crate::protocol::since;
use crate::protocol::spec::{FieldSpec, ProtocolStruct};

/// Every struct the registry describes by name: the named structs and every operation's
/// request and response body.
pub fn structs() -> &'static BTreeMap<&'static str, &'static [FieldSpec]> {
    static STRUCTS: OnceLock<BTreeMap<&'static str, &'static [FieldSpec]>> = OnceLock::new();
    STRUCTS.get_or_init(|| {
        NAMED_STRUCTS
            .iter()
            .map(|spec| (spec.name, spec.fields))
            .chain(
                OPERATIONS
                    .iter()
                    .flat_map(|spec| [spec.request, spec.response])
                    .map(|spec| (spec.name, spec.fields)),
            )
            .collect()
    })
}

/// The element type of a container type spelling, or the spelling itself.
pub fn element(ty: &str) -> &str {
    if let Some(inner) = ty
        .strip_prefix("list<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        return element(inner);
    }
    if let Some(inner) = ty
        .strip_prefix("map<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        return element(
            inner
                .split_once(',')
                .map_or(inner, |(_, value)| value.trim()),
        );
    }
    ty
}

/// Every union the registry declares: its name and the types its variants carry.
pub fn unions() -> &'static BTreeMap<&'static str, Vec<&'static str>> {
    static UNIONS_BY_NAME: OnceLock<BTreeMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    UNIONS_BY_NAME.get_or_init(|| {
        UNIONS
            .iter()
            .map(|spec| {
                (
                    spec.name,
                    spec.variants.iter().map(|variant| variant.ty).collect(),
                )
            })
            .collect()
    })
}

/// The types a value of type `ty` directly contains: a struct's field element types, or a
/// union's variant types.
pub fn edges(ty: &str) -> Vec<&'static str> {
    if let Some(fields) = structs().get(ty) {
        return fields.iter().map(|field| element(field.ty)).collect();
    }
    unions().get(ty).cloned().unwrap_or_default()
}

/// The struct types from which some dated field is reachable, the owners included.
///
/// A fixed point over the static type graph: it terminates after at most one round per
/// struct, whatever the input.
pub fn reaching() -> &'static BTreeSet<&'static str> {
    static REACHING: OnceLock<BTreeSet<&'static str>> = OnceLock::new();
    REACHING.get_or_init(|| {
        let mut set: BTreeSet<&'static str> =
            since::FIELDS.iter().map(|(owner, _, _)| *owner).collect();
        let names: Vec<&'static str> = structs().keys().chain(unions().keys()).copied().collect();
        loop {
            let before = set.len();
            for name in &names {
                if edges(name).iter().any(|edge| set.contains(edge)) {
                    set.insert(name);
                }
            }
            if set.len() == before {
                return set;
            }
        }
    })
}

/// Whether `owner.field` is dated after `version`.
fn undefined_at(owner: &str, field: &str, version: ProtocolVersion) -> bool {
    since::FIELDS.iter().any(|(dated_owner, dated_field, at)| {
        *dated_owner == owner && *dated_field == field && !since::defines(Some(*at), version)
    })
}

/// Remove, in place, every field `version` does not define from `value` read as a `ty`.
/// A value of the wrong kind is left for the strict decode to report, as at every version.
fn strip<D: Document>(value: &mut D, ty: &str, version: ProtocolVersion) {
    if let Some(inner) = ty
        .strip_prefix("list<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        if reaching().contains(element(inner)) {
            if let Some(items) = value.as_items_mut() {
                for item in items {
                    strip(item, inner, version);
                }
            }
        }
        return;
    }
    if let Some(inner) = ty
        .strip_prefix("map<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        let value_ty = inner
            .split_once(',')
            .map_or(inner, |(_, value)| value.trim());
        if reaching().contains(element(value_ty)) {
            if let Some(entries) = value.as_entries_mut() {
                for item in entries.values_mut() {
                    strip(item, value_ty, version);
                }
            }
        }
        return;
    }
    if !reaching().contains(ty) {
        return;
    }
    if let Some(variants) = unions().get(ty) {
        // Externally tagged: one key naming the variant, whose value is its type.
        if let Some(entries) = value.as_entries_mut() {
            if entries.len() == 1 {
                if let Some((tag, item)) = entries.iter_mut().next() {
                    let carried = UNIONS
                        .iter()
                        .find(|spec| spec.name == ty)
                        .and_then(|spec| spec.variants.iter().find(|variant| variant.ident == tag))
                        .map(|variant| variant.ty);
                    if let Some(carried) = carried.filter(|carried| variants.contains(carried)) {
                        strip(item, carried, version);
                    }
                }
            }
        }
        return;
    }
    let Some(fields) = structs().get(ty) else {
        return;
    };
    let Some(entries) = value.as_entries_mut() else {
        return;
    };
    for field in *fields {
        if undefined_at(ty, field.name, version) {
            // Dropped, never copied and never interpreted.
            entries.remove(field.name);
        } else if reaching().contains(element(field.ty)) {
            if let Some(item) = entries.get_mut(field.name) {
                strip(item, field.ty, version);
            }
        }
    }
}

/// Read a request body of type `T` at `version`: parse the bytes as one canonical
/// document, drop every field `version` does not define, and decode the rest strictly.
/// With no version, every field is decoded (the newest reading).
///
/// # Errors
///
/// [`CodecError`] when the bytes are not a canonical document, at every version, or when
/// a field the version defines does not decode.
pub fn read_at<D: Document, T: ProtocolValue + ProtocolStruct>(
    bytes: &[u8],
    version: Option<ProtocolVersion>,
) -> Result<T, CodecError> {
    let mut tree = D::from_canonical_bytes(bytes)?;
    if let Some(version) = version {
        strip(&mut tree, T::STRUCT_NAME, version);
    }
    T::decode(&tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_reads_through_containers() {
        assert_eq!(element("list<FileComponent>"), "FileComponent");
        assert_eq!(element("map<String,Budget>"), "Budget");
        assert_eq!(element("Commitment"), "Commitment");
    }

    #[test]
    fn the_reaching_subgraph_has_no_cycle() {
        // Depth-first over `edges`, restricted to `reaching()`: a type met again on its own
        // path is a cycle, and the walk's depth would then follow the input.
        fn visit<'a>(ty: &'a str, path: &mut Vec<&'a str>) {
            assert!(!path.contains(&ty), "a cycle through {ty}: {path:?}");
            path.push(ty);
            for edge in edges(ty) {
                if reaching().contains(edge) {
                    visit(edge, path);
                }
            }
            path.pop();
        }
        for ty in reaching() {
            visit(ty, &mut Vec::new());
        }
    }

    #[test]
    fn every_dated_owner_reaches_itself() {
        for (owner, _, _) in since::FIELDS {
            assert!(reaching().contains(owner), "{owner}");
        }
    }
}
