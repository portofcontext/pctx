use indexmap::{IndexMap, IndexSet};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone)]
pub(crate) struct PtxTools {
    upstream: Vec<UpstreamMcp>,
    tool_router: ToolRouter<PtxTools>,
}
#[tool_router]
impl PtxTools {
    pub(crate) fn new() -> Self {
        Self {
            upstream: vec![],
            tool_router: Self::tool_router(),
        }
    }

    pub(crate) fn register_mcp(mut self, mcp: UpstreamMcp) -> Self {
        self.upstream.push(mcp);
        self
    }

    #[tool(description = "List functions organized in namespaces based on available services")]
    async fn list_functions(&self) -> Result<CallToolResult, McpError> {
        let namespaces: Vec<String> = self
            .upstream
            .iter()
            .map(|m| {
                let fns: Vec<String> = m.tools.iter().map(|(_, t)| t.fn_signature(false)).collect();

                format!(
                    "{docstring}
namespace {namespace} {{
  {fns}
}}",
                    docstring = to_docstring(&m.description),
                    namespace = &m.namespace,
                    fns = fns.join("\n\n")
                )
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            namespaces.join("\n\n"),
        )]))
    }

    #[tool(
        description = "Get details about specific functions as listed in `list_functions`, organized in namespaces"
    )]
    async fn get_function_details(
        &self,
        Parameters(GetFunctionDetailsInput { functions }): Parameters<GetFunctionDetailsInput>,
    ) -> Result<CallToolResult, McpError> {
        // organize tool input by namespace and handle any deduping
        let mut by_namespace: IndexMap<String, IndexSet<String>> = IndexMap::new();
        for func in functions {
            let parts: Vec<&str> = func.split('.').collect();
            if parts.len() != 2 {
                // incorrect format
                continue;
            }
            by_namespace
                .entry(parts[0].to_string())
                .or_default()
                .insert(parts[1].to_string());
        }

        let mut namespace_details = vec![];

        for (namespace, functions) in by_namespace {
            if let Some(mcp) = self.upstream.iter().find(|m| m.namespace == namespace) {
                let mut fn_details = vec![];
                for fn_name in functions {
                    if let Some(tool) = mcp.tools.get(&fn_name) {
                        fn_details.push(tool.fn_signature(true));
                    }
                }

                if !fn_details.is_empty() {
                    namespace_details.push(format!(
                        "{docstring}
namespace {namespace} {{
  {fns}
}}",
                        docstring = to_docstring(&mcp.description),
                        namespace = &mcp.namespace,
                        fns = fn_details.join("\n\n")
                    ));
                }
            }
        }

        let content = if namespace_details.is_empty() {
            "No namespaces/functions match the request".to_string()
        } else {
            namespace_details.join("\n\n")
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }
}

// #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
// pub(crate) struct GetFunctionDetailsInput {
//     /// List of functions, organized by their namespace to get more details on
//     pub namespaced_functions: Vec<NamespacedFunctions>,
// }

// #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
// pub(crate) struct NamespacedFunctions {
//     /// The namespace the function is defined in, as returned by `list_functions`
//     pub namespace: String,
//     /// List of function names within the name space to get more details on
//     pub functions: Vec<String>,
// }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct GetFunctionDetailsInput {
    /// List of functions to get details of. Functions should be in the form "<namespace>.<function name>".
    /// e.g. If there is a function `getData` within the `DataApi` namespace the value provided in this field is "DataApi.getData"
    pub functions: Vec<String>,
}

#[tool_handler]
impl ServerHandler for PtxTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(format!(
                "This server provides tools to explore SDK functions and execute SDK scripts for the following services: {}",
                self.upstream
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            )),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct UpstreamMcp {
    pub(crate) name: String,
    pub(crate) namespace: String,
    pub(crate) description: String,
    pub(crate) url: String,
    pub(crate) tools: IndexMap<String, UpstreamTool>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct UpstreamTool {
    pub(crate) tool_name: String,
    pub(crate) title: Option<String>,
    pub(crate) description: String,
    pub(crate) fn_name: String,
    pub(crate) input_type: Option<String>,
    pub(crate) output_type: String,
    pub(crate) types: String,
}

impl UpstreamTool {
    pub(crate) fn fn_signature(&self, include_types: bool) -> String {
        let docstring_content = format!(
            "{title}{desc}",
            title = &self
                .title
                .as_ref()
                .map(|t| format!("{t}\n\n"))
                .unwrap_or_default(),
            desc = &self.description
        );
        let args = self
            .input_type
            .as_ref()
            .map(|t| format!("input: {t}"))
            .unwrap_or_default();

        let types = if include_types && !self.types.is_empty() {
            format!("{}\n\n", &self.types)
        } else {
            String::new()
        };

        format!(
            "{types}{docstring}\nasync function {fn_name}({args}): Promise<{output}>",
            docstring = to_docstring(&docstring_content),
            fn_name = &self.fn_name,
            output = &self.output_type
        )
    }

    pub(crate) fn fn_impl(&self, mcp_name: &str) -> String {
        format!(
            "{fn_sig} {{
  return await callMCPTool({{
    name: {name},
    tool: {tool},
    arguments: {args},
  }});
}}",
            fn_sig = self.fn_signature(true),
            name = json!(mcp_name),
            tool = json!(&self.tool_name),
            args = self.input_type.as_ref().map_or("undefined", |_| "input")
        )
    }
}

fn to_docstring(content: &str) -> String {
    let mut lines = vec!["/**".to_string()];
    lines.extend(content.split('\n').map(|c| format!(" * {c}")));
    lines.push("*/".into());

    lines.join("\n")
}
