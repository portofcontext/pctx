use pctx_codegen::{RootSchema, Tool, ToolSet};
use serde::Deserialize;

const BASIC_TOOL: &str = include_str!("./fixtures/tools/basic.yml");
const NESTED_TYPES_TOOL: &str = include_str!("./fixtures/tools/nested_types.yml");
const NO_OUTPUT_TOOL: &str = include_str!("./fixtures/tools/no_output.yml");

#[derive(Debug, Deserialize)]
struct ToolFixture {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
}

impl ToolFixture {
    fn input_root_schema(&self) -> RootSchema {
        serde_json::from_value(self.input_schema.clone()).expect("invalid input_schema")
    }

    fn output_root_schema(&self) -> Option<RootSchema> {
        self.output_schema
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()).expect("invalid output_schema"))
    }

    fn to_mcp_tool(&self) -> Tool {
        Tool::new_mcp(
            &self.name,
            self.description.clone(),
            self.input_root_schema(),
            self.output_root_schema(),
        )
        .expect("Tool::new_mcp failed")
    }

    fn to_callback_tool(&self) -> Tool {
        Tool::new_callback(
            &self.name,
            self.description.clone(),
            self.input_root_schema(),
            self.output_root_schema(),
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
        #[test]
        fn $test_name() {
            let fixture = load_fixture($fixture);
            let tool = fixture.$variant();

            insta::assert_snapshot!(
                format!("{}__fn_signature.ts", stringify!($test_name)),
                tool.fn_signature(true)
            );

            insta::assert_snapshot!(
                format!("{}__fn_impl.ts", stringify!($test_name)),
                tool.fn_impl("test_server")
            );
        }
    };
}

tool_test!(test_basic, variant: to_mcp_tool, BASIC_TOOL);
tool_test!(test_no_output, variant: to_callback_tool, NO_OUTPUT_TOOL);
tool_test!(test_nested_types, variant: to_mcp_tool, NESTED_TYPES_TOOL);

// --- ToolSet tests ---

#[test]
fn test_toolset_namespace() {
    let basic = load_fixture(BASIC_TOOL);
    let notif = load_fixture(NESTED_TYPES_TOOL);

    let toolset = ToolSet::new(
        "my_tools",
        "A collection of utility tools",
        vec![basic.to_mcp_tool(), notif.to_callback_tool()],
    );

    insta::assert_snapshot!(
        "toolset__namespace_interface.ts",
        toolset.namespace_interface(true)
    );
}
