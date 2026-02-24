use pctx_codegen::{RootSchema, Tool, ToolSet};
use pctx_type_check_runtime::type_check;
use serde::Deserialize;

const BASIC_TOOL: &str = include_str!("./fixtures/tools/basic.yml");
const NESTED_TYPES_TOOL: &str = include_str!("./fixtures/tools/nested_types.yml");
const NO_OUTPUT_TOOL: &str = include_str!("./fixtures/tools/no_output.yml");
const NO_INPUT_TOOL: &str = include_str!("./fixtures/tools/no_input.yml");
const NO_INPUT_OR_OUTPUT_TOOL: &str = include_str!("./fixtures/tools/no_input_or_output.yml");
const ALL_OPTIONAL_INPUT_TOOL: &str = include_str!("./fixtures/tools/all_optional_input.yml");

#[derive(Debug, Deserialize)]
struct ToolFixture {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<RootSchema>,
    pub output_schema: Option<RootSchema>,
}

impl ToolFixture {
    fn to_mcp_tool(&self) -> Tool {
        Tool::new_mcp(
            &self.name,
            self.description.clone(),
            self.input_schema.clone(),
            self.output_schema.clone(),
        )
        .expect("Tool::new_mcp failed")
    }

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

// --- Tool tests ---

macro_rules! tool_test {
    ($test_name:ident, variant: $variant:ident, $fixture:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let fixture = load_fixture($fixture);
            let tool = fixture.$variant();

            let impl_code =
                pctx_codegen::format::format_ts(&tool.ts_fn_impl(Some("test_server".into())));
            let check_res = type_check(&impl_code).expect("failed typecheck");

            assert!(
                check_res.success,
                "tool fn_impl failed typecheck: {check_res:?}"
            );
            insta::assert_snapshot!(format!("{}__fn_impl.ts", stringify!($test_name)), impl_code);
        }
    };
}

tool_test!(test_basic, variant: to_mcp_tool, BASIC_TOOL);
tool_test!(test_nested_types, variant: to_mcp_tool, NESTED_TYPES_TOOL);
tool_test!(test_no_output, variant: to_callback_tool, NO_OUTPUT_TOOL);
tool_test!(test_no_input, variant: to_callback_tool, NO_INPUT_TOOL);
tool_test!(test_no_input_or_output, variant: to_callback_tool, NO_INPUT_OR_OUTPUT_TOOL);
tool_test!(test_all_optional_input, variant: to_mcp_tool, ALL_OPTIONAL_INPUT_TOOL);

// --- ToolSet tests ---

#[test]
fn test_toolset_namespace() {
    let basic = load_fixture(BASIC_TOOL);
    let notif = load_fixture(NESTED_TYPES_TOOL);

    let toolset = ToolSet::new(
        Some("my_tools".into()),
        "A collection of utility tools",
        vec![basic.to_mcp_tool(), notif.to_callback_tool()],
    );

    insta::assert_snapshot!(
        "toolset__namespace_interface.ts",
        toolset.ts_namespace_declaration(true)
    );
}
