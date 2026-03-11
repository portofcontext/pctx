use pctx_config::ToolDisclosure;

pub const EXECUTE_BASH: &str = include_str!("../../../tool_descriptions/execute_bash/v1.txt");
pub const EXECUTE_TYPESCRIPT_CATALOG: &str =
    include_str!("../../../tool_descriptions/execute_typescript_catalog/v1.txt");
pub const EXECUTE_TYPESCRIPT_FILESYSTEM: &str =
    include_str!("../../../tool_descriptions/execute_typescript_filesystem/v1.txt");
pub const EXECUTE_TYPESCRIPT_SIDECAR: &str =
    include_str!("../../../tool_descriptions/execute_typescript_sidecar/v1.txt");
pub const GET_FUNCTION_DETAILS: &str =
    include_str!("../../../tool_descriptions/get_function_details/v1.txt");
pub const LIST_FUNCTIONS: &str = include_str!("../../../tool_descriptions/list_functions/v1.txt");

pub fn disclosure_execute_description(disclosure: ToolDisclosure) -> String {
    match disclosure {
        ToolDisclosure::Catalog => EXECUTE_TYPESCRIPT_CATALOG.into(),
        ToolDisclosure::Filesystem => EXECUTE_TYPESCRIPT_FILESYSTEM.into(),
        ToolDisclosure::Sidecar => EXECUTE_TYPESCRIPT_SIDECAR.into(),
    }
}
