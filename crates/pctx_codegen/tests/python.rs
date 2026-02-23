use pctx_codegen::{RootSchema, Tool, ToolSet};
use serde::Deserialize;

const BASIC_TOOL: &str = include_str!("./fixtures/tools/basic.yml");
const NESTED_TYPES_TOOL: &str = include_str!("./fixtures/tools/nested_types.yml");
const NO_OUTPUT_TOOL: &str = include_str!("./fixtures/tools/no_output.yml");
const NO_INPUT_TOOL: &str = include_str!("./fixtures/tools/no_input.yml");
const NO_INPUT_OR_OUTPUT_TOOL: &str = include_str!("./fixtures/tools/no_input_or_output.yml");
const ALL_OPTIONAL_INPUT_TOOL: &str = include_str!("./fixtures/tools/all_optional_input.yml");
const RESERVED_WORDS_TOOL: &str = include_str!("./fixtures/tools/reserved_words.yml");

#[derive(Debug, Deserialize)]
struct ToolFixture {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<RootSchema>,
    pub output_schema: Option<RootSchema>,
}

impl ToolFixture {
    fn to_callback_tool(&self) -> Tool {
        Tool::new_callback(
            &self.name,
            self.description.clone(),
            self.input_schema.clone(),
            self.output_schema.clone(),
        )
        .expect("Tool::new_callback failed")
    }
}

fn load_fixture(yml: &str) -> ToolFixture {
    serde_yaml::from_str(yml).expect("Failed to parse tool fixture YAML")
}

macro_rules! python_stub_test {
    ($test_name:ident, $fixture:expr) => {
        #[test]
        fn $test_name() {
            let fixture = load_fixture($fixture);
            let tool = fixture.to_callback_tool();
            let stub =
                pctx_codegen::python::generate_stubs(&[tool]).expect("generate_stubs failed");
            insta::assert_snapshot!(format!("{}__stub.py", stringify!($test_name)), stub);
        }
    };
}

python_stub_test!(test_basic, BASIC_TOOL);
python_stub_test!(test_nested_types, NESTED_TYPES_TOOL);
python_stub_test!(test_no_output, NO_OUTPUT_TOOL);
python_stub_test!(test_no_input, NO_INPUT_TOOL);
python_stub_test!(test_no_input_or_output, NO_INPUT_OR_OUTPUT_TOOL);
python_stub_test!(test_all_optional_input, ALL_OPTIONAL_INPUT_TOOL);
python_stub_test!(test_reserved_words, RESERVED_WORDS_TOOL);

// ── param rename tests ────────────────────────────────────────────────────────

/// Build a minimal callback tool with the given JSON Schema input properties.
fn tool_with_props(name: &str, props: serde_json::Value) -> Tool {
    let schema: RootSchema = serde_json::from_value(serde_json::json!({
        "type": "object",
        "required": [],
        "properties": props
    }))
    .unwrap();
    Tool::new_callback(name, None, Some(schema), None).unwrap()
}

/// Convenience: run `generate_mappings_for_toolsets` on a single toolset and
/// return the `param_renames` map for the first (only) tool.
fn param_renames_for(tool: Tool) -> std::collections::HashMap<String, String> {
    let ts = ToolSet::new("myns", "desc", vec![tool]);
    let mut mappings = pctx_codegen::python::generate_mappings_for_toolsets(&[ts]);
    assert_eq!(mappings.len(), 1);
    mappings.remove(0).param_renames
}

/// No rename needed — property names are already valid snake_case Python identifiers.
#[test]
fn test_param_renames_no_rename() {
    let renames = param_renames_for(tool_with_props(
        "get_customer",
        serde_json::json!({
            "customer_id": { "type": "string" },
            "phone_number": { "type": "string" }
        }),
    ));
    assert!(renames.is_empty(), "expected no renames, got: {renames:?}");
}

/// camelCase properties must be converted to snake_case in Python stubs.
/// The callback should still receive the original camelCase key.
#[test]
fn test_param_renames_camel_case() {
    let renames = param_renames_for(tool_with_props(
        "get_user",
        serde_json::json!({
            "phoneNumber": { "type": "string" },
            "firstName": { "type": "string" },
            "lastName": { "type": "string" }
        }),
    ));
    assert_eq!(
        renames.get("phone_number").map(String::as_str),
        Some("phoneNumber")
    );
    assert_eq!(
        renames.get("first_name").map(String::as_str),
        Some("firstName")
    );
    assert_eq!(
        renames.get("last_name").map(String::as_str),
        Some("lastName")
    );
    assert_eq!(renames.len(), 3);
}

/// Python keyword `from` → `from_`; callback must receive `from`.
#[test]
fn test_param_renames_keyword_from() {
    let renames = param_renames_for(tool_with_props(
        "get_history",
        serde_json::json!({
            "from": { "type": "string" },
            "to": { "type": "string" }
        }),
    ));
    assert_eq!(renames.get("from_").map(String::as_str), Some("from"));
    // `to` is not a Python keyword; no rename.
    assert!(!renames.contains_key("to"), "unexpected rename for 'to'");
}

/// Python keyword `in` → `in_`.
#[test]
fn test_param_renames_keyword_in() {
    let renames = param_renames_for(tool_with_props(
        "check_membership",
        serde_json::json!({ "in": { "type": "string" } }),
    ));
    assert_eq!(renames.get("in_").map(String::as_str), Some("in"));
}

/// `fromDate` is NOT a Python keyword after snake_casing → `from_date`; no `_` suffix.
#[test]
fn test_param_renames_from_date_not_keyword() {
    let renames = param_renames_for(tool_with_props(
        "get_range",
        serde_json::json!({ "fromDate": { "type": "string" } }),
    ));
    assert_eq!(
        renames.get("from_date").map(String::as_str),
        Some("fromDate"),
        "fromDate should rename to from_date (not a keyword)"
    );
}

/// Mixed: some props rename, some do not.
#[test]
fn test_param_renames_mixed() {
    let renames = param_renames_for(tool_with_props(
        "send_message",
        serde_json::json!({
            "from": { "type": "string" },   // keyword → from_
            "toAddress": { "type": "string" }, // camelCase → to_address
            "subject": { "type": "string" }    // no change
        }),
    ));
    assert_eq!(renames.get("from_").map(String::as_str), Some("from"));
    assert_eq!(
        renames.get("to_address").map(String::as_str),
        Some("toAddress")
    );
    assert!(
        !renames.contains_key("subject"),
        "unexpected rename for 'subject'"
    );
    assert_eq!(renames.len(), 2);
}

/// Multi-toolset: each mapping carries only its own tool's renames.
#[test]
fn test_param_renames_multi_toolset() {
    let tool_a = tool_with_props(
        "search",
        serde_json::json!({ "query": { "type": "string" } }),
    );
    let tool_b = tool_with_props(
        "filter",
        serde_json::json!({ "fromDate": { "type": "string" } }),
    );
    let ts = ToolSet::new("api", "desc", vec![tool_a, tool_b]);
    let mappings = pctx_codegen::python::generate_mappings_for_toolsets(&[ts]);
    assert_eq!(mappings.len(), 2);

    let search_mapping = mappings.iter().find(|m| m.py_name == "search").unwrap();
    assert!(search_mapping.param_renames.is_empty());

    let filter_mapping = mappings.iter().find(|m| m.py_name == "filter").unwrap();
    assert_eq!(
        filter_mapping
            .param_renames
            .get("from_date")
            .map(String::as_str),
        Some("fromDate")
    );
}

/// Stubs generated for reserved-word params use the Python-safe name.
/// This is a snapshot test; update with `cargo insta review` if the output changes.
#[test]
fn test_reserved_words_stub_uses_python_safe_names() {
    let fixture = load_fixture(RESERVED_WORDS_TOOL);
    let tool = fixture.to_callback_tool();
    let stub = pctx_codegen::python::generate_stubs(&[tool]).unwrap();
    // `from` must appear as `from_` in the stub (it's a keyword).
    assert!(
        stub.contains("from_:"),
        "expected 'from_:' in stub, got:\n{stub}"
    );
    // `id` must NOT be renamed — it was previously mangled to `id_`, now fixed.
    assert!(stub.contains("id:"), "expected 'id:' in stub, got:\n{stub}");
    // `format` must NOT be renamed either.
    assert!(
        stub.contains("format:"),
        "expected 'format:' in stub, got:\n{stub}"
    );
}
