//! Regression: a hand-authored schema can express recursion as an in-document
//! JSON pointer (`#/properties/filter/anyOf/0`) rather than a `#/$defs/` named
//! ref — e.g. a query filter whose `and`/`or` groups reference the filter itself.
//! Before the normalize pass, codegen failed with `#/$defs/0 does not exist` and
//! the tool fell back to `any`. It must now generate a real recursive type.
use pctx_codegen::RootSchema;

/// A recursive query filter: `and`/`or` groups reference the top filter via an
/// in-document pointer (`#/properties/filter/anyOf/0`).
fn recursive_filter_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "object": {"type": "string"},
            "filter": {
                "anyOf": [
                    {"anyOf": [
                        {"type": "object",
                         "properties": {"attribute": {"type": "string"}, "op": {"type": "string"}},
                         "required": ["attribute", "op"]},
                        {"type": "object",
                         "properties": {"or": {"type": "array",
                            "items": {"$ref": "#/properties/filter/anyOf/0"}}},
                         "required": ["or"]},
                        {"type": "object",
                         "properties": {"and": {"type": "array",
                            "items": {"$ref": "#/properties/filter/anyOf/0"}}},
                         "required": ["and"]}
                    ]},
                    {"type": "null"}
                ]
            }
        },
        "required": ["object"]
    })
}

#[test]
fn in_document_recursive_ref_generates_real_types() {
    let schema: RootSchema =
        serde_json::from_value(recursive_filter_schema()).expect("schema deserializes");
    let res = pctx_codegen::typegen::generate_types(schema, "ListRecordsInput")
        .expect("recursive in-document filter ref must resolve, not error");

    // `object` keeps its real type; the whole input is not collapsed.
    assert!(
        res.types.contains("object: string"),
        "types:\n{}",
        res.types
    );
    // The filter is a real recursive union (its `or`/`and` branches reference the
    // hoisted filter type), not degraded to `any`.
    assert!(
        res.types.contains("InlineRef"),
        "expected a hoisted recursive filter type; got:\n{}",
        res.types
    );
    assert!(
        !res.types.contains("any"),
        "filter must not fall back to `any`; got:\n{}",
        res.types
    );
}
