pub mod tools {
    use pctx_config::ToolDisclosure;

    pub const EXECUTE_BASH: &str = include_str!("../descriptions/tools/execute_bash/v1.txt");
    pub const EXECUTE_TYPESCRIPT_CATALOG: &str =
        include_str!("../descriptions/tools/execute_typescript_catalog/v1.txt");
    pub const EXECUTE_TYPESCRIPT_FILESYSTEM: &str =
        include_str!("../descriptions/tools/execute_typescript_filesystem/v1.txt");
    pub const EXECUTE_TYPESCRIPT_SIDECAR: &str =
        include_str!("../descriptions/tools/execute_typescript_sidecar/v1.txt");
    pub const GET_FUNCTION_DETAILS: &str =
        include_str!("../descriptions/tools/get_function_details/v1.txt");
    pub const LIST_FUNCTIONS: &str = include_str!("../descriptions/tools/list_functions/v1.txt");

    pub fn disclosure_execute_description(disclosure: ToolDisclosure) -> String {
        match disclosure {
            ToolDisclosure::Catalog => EXECUTE_TYPESCRIPT_CATALOG.into(),
            ToolDisclosure::Filesystem => EXECUTE_TYPESCRIPT_FILESYSTEM.into(),
            ToolDisclosure::Sidecar => EXECUTE_TYPESCRIPT_SIDECAR.into(),
        }
    }
}

pub mod workflow {
    use pctx_config::ToolDisclosure;

    const COMMON: &str = include_str!("../descriptions/workflows/common.txt");
    const CATALOG: &str = include_str!("../descriptions/workflows/catalog/v1.txt");
    const FILESYSTEM: &str = include_str!("../descriptions/workflows/filesystem/v1.txt");
    const SIDECAR: &str = include_str!("../descriptions/workflows/sidecar/v1.txt");

    pub fn get_workflow_description(disclosure: ToolDisclosure) -> String {
        let disclosure_specific = match disclosure {
            ToolDisclosure::Catalog => CATALOG,
            ToolDisclosure::Filesystem => FILESYSTEM,
            ToolDisclosure::Sidecar => SIDECAR,
        };

        format!("{COMMON}\n{disclosure_specific}")
    }
}
