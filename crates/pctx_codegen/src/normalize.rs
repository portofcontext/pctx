//! Normalize hand-authored JSON Schemas so the type generator can resolve their
//! recursion.
//!
//! [`schema_type`](crate::schema_type) resolves a `$ref` by its trailing path
//! segment, looked up in the schema's `definitions` map — so `#/$defs/Foo` and
//! `#/definitions/Foo` work. Hand-written schemas (e.g. a recursive query filter
//! whose `and`/`or` groups reference the filter itself) instead express recursion
//! with an in-document JSON **pointer**, e.g. `#/properties/filter/anyOf/0`. Its
//! trailing segment is `0`, which is not a definition, so `follow` fails with
//! `#/$defs/0 does not exist` and the whole tool falls back to `any`.
//!
//! [`normalize_in_document_refs`] rewrites those into the named form codegen
//! already handles: each distinct pointer target is hoisted into `definitions`
//! under a generated name, and every ref to it — including refs *inside* the
//! hoisted target, so a self-referential filter becomes a proper named recursive
//! type — is repointed at `#/definitions/<name>`. Schemas that already use
//! `#/$defs`/`#/definitions` refs pass through untouched.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

/// Prefix for generated definition names, kept distinctive so a hoisted pointer
/// target can't collide with a real `definitions` entry.
const HOISTED_PREFIX: &str = "InlineRef";

/// Rewrite in-document pointer `$ref`s into named `definitions` refs. Idempotent
/// and a no-op for schemas without such refs.
pub fn normalize_in_document_refs(mut schema: Value) -> Value {
    let mut pointers = BTreeSet::new();
    collect_pointer_refs(&schema, &mut pointers);
    if pointers.is_empty() {
        return schema;
    }

    // Resolve pointers against an unmodified snapshot (adding definitions must not
    // shift the targets), and only rewrite the refs we actually hoisted — an
    // unresolvable pointer is left as-is rather than repointed at a missing def.
    let snapshot = schema.clone();
    let Some(root) = schema.as_object_mut() else {
        return schema;
    };
    let defs = root
        .entry("definitions")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(defs) = defs.as_object_mut() else {
        return schema;
    };
    let mut hoisted = BTreeSet::new();
    for ptr in &pointers {
        // A JSON pointer is the ref without its leading '#'.
        if let Some(target) = snapshot.pointer(&ptr[1..]) {
            defs.entry(def_name(ptr)).or_insert_with(|| target.clone());
            hoisted.insert(ptr.clone());
        }
    }
    rewrite_pointer_refs(&mut schema, &hoisted);
    schema
}

/// Collect distinct `$ref`s that point into the document body (`#/...`) rather
/// than at a `$defs`/`definitions` entry.
fn collect_pointer_refs(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref")
                && is_in_document_pointer(r)
            {
                out.insert(r.clone());
            }
            for child in map.values() {
                collect_pointer_refs(child, out);
            }
        }
        Value::Array(arr) => arr.iter().for_each(|c| collect_pointer_refs(c, out)),
        _ => {}
    }
}

/// Repoint every `$ref` in `pointers` at its hoisted `#/definitions/<name>`.
fn rewrite_pointer_refs(v: &mut Value, pointers: &BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref")
                && pointers.contains(r)
            {
                let name = def_name(r);
                map.insert("$ref".to_string(), json!(format!("#/definitions/{name}")));
            }
            for child in map.values_mut() {
                rewrite_pointer_refs(child, pointers);
            }
        }
        Value::Array(arr) => arr
            .iter_mut()
            .for_each(|c| rewrite_pointer_refs(c, pointers)),
        _ => {}
    }
}

/// A ref that points into the document body (e.g. `#/properties/filter/anyOf/0`)
/// and not at an already-named definition.
fn is_in_document_pointer(r: &str) -> bool {
    r.starts_with("#/") && !r.starts_with("#/$defs/") && !r.starts_with("#/definitions/")
}

/// A stable, collision-resistant definition name for a pointer, e.g.
/// `#/properties/filter/anyOf/0` → `InlineRef_properties_filter_anyOf_0`.
fn def_name(pointer: &str) -> String {
    let path = pointer
        .trim_start_matches("#/")
        .split('/')
        .collect::<Vec<_>>()
        .join("_");
    format!("{HOISTED_PREFIX}_{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hoists_recursive_filter_self_ref_into_definitions() {
        // The shape that broke codegen: a filter whose `or`/`and` branches ref
        // `#/properties/filter/anyOf/0` (a pointer into the body, self-recursive).
        let schema = json!({
            "type": "object",
            "properties": {
                "filter": {
                    "anyOf": [
                        {"anyOf": [
                            {"type": "object", "properties": {"attribute": {"type": "string"}}},
                            {"type": "object", "properties": {
                                "or": {"type": "array", "items": {"$ref": "#/properties/filter/anyOf/0"}}
                            }}
                        ]},
                        {"type": "null"}
                    ]
                }
            }
        });
        let out = normalize_in_document_refs(schema);

        let name = "InlineRef_properties_filter_anyOf_0";
        // The target is hoisted into definitions...
        assert!(out["definitions"][name].is_object(), "hoisted def present");
        // ...the original ref is repointed at the named def...
        assert_eq!(
            out["properties"]["filter"]["anyOf"][0]["anyOf"][1]["properties"]["or"]["items"]["$ref"],
            format!("#/definitions/{name}")
        );
        // ...and the self-ref INSIDE the hoisted def is repointed too, so it's a
        // proper named recursive type rather than a dangling body pointer.
        assert_eq!(
            out["definitions"][name]["anyOf"][1]["properties"]["or"]["items"]["$ref"],
            format!("#/definitions/{name}")
        );
    }

    #[test]
    fn leaves_named_refs_and_plain_schemas_untouched() {
        let named = json!({
            "type": "object",
            "properties": {"child": {"$ref": "#/$defs/Node"}},
            "$defs": {"Node": {"type": "object"}}
        });
        assert_eq!(normalize_in_document_refs(named.clone()), named);

        let plain = json!({"type": "object", "properties": {"n": {"type": "number"}}});
        assert_eq!(normalize_in_document_refs(plain.clone()), plain);
    }
}
