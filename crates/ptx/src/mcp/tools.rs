use anyhow::Result;
use codegen::{case::Case, generate_docstring};
use indexmap::{IndexMap, IndexSet};
use log::debug;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
        Tool,
    },
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::deno_pool::DenoExecutor;

type McpResult<T> = Result<T, McpError>;

#[derive(Clone)]
pub(crate) struct PtxTools {
    executor: DenoExecutor,
    upstream: Vec<UpstreamMcp>,
    tool_router: ToolRouter<PtxTools>,
}
#[tool_router]
impl PtxTools {
    pub(crate) fn with_executor(executor: DenoExecutor) -> Self {
        Self {
            executor,
            upstream: vec![],
            tool_router: Self::tool_router(),
        }
    }

    pub(crate) fn with_upstream_mcps(mut self, upstream: Vec<UpstreamMcp>) -> Self {
        self.upstream = upstream;
        self
    }

    #[tool(
        title = "List Functions",
        description = "List functions organized in namespaces based on available services"
    )]
    async fn list_functions(&self) -> McpResult<CallToolResult> {
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
                    docstring = generate_docstring(&m.description),
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
        title = "Get Function Details",
        description = "Get details about specific functions as listed in `list_functions`, organized in namespaces"
    )]
    async fn get_function_details(
        &self,
        Parameters(GetFunctionDetailsInput { functions }): Parameters<GetFunctionDetailsInput>,
    ) -> McpResult<CallToolResult> {
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
                        docstring = generate_docstring(&mcp.description),
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

    #[tool(
        title = "Execute Code",
        description = "Runs TypeScript code that can use the namespaced functions listed in `list_functions`
        You are a skilled programmer writing code to interact with the available namespaced functions.

        Always define an async function called `run` that accepts no arguments:

        async function run() {
            // YOUR CODE GOES HERE

            // log results and return output
        }

        The only available methods are returned by the `list_functions` tool, and the inputs and outputs of the methods can be obtained by the `get_function_details` tool, no other inputs or outputs exist.
        When calling the functions you MUST include the namespace. e.g. if e.g. If there is a function `getData` within the `DataApi` namespace, to call the function you must write `DataApi.getData`.
        You will be returned anything that your function returns, plus the results of any console.log statements.
        If any code triggers an error, the tool will return an error response, so you do not need to add error handling unless you want to output something more helpful than the raw error.
        It is not necessary to add comments to code, unless by adding those comments you believe that you can generate better code.
        This code will run in a container, and you will not be able to use the filesystem, `fetch` or otherwise interact with the network calls other than through the namespaced functions you are given.
        Any variables you define won't live between successive uses of this tool, so make sure to return or log any data you might need later.
        Try to avoid logging or returning large objects, try to only return and log the specific fields you may need.
        If you are making calls to multiple methods, add logs between the method calls so in case of a failure, you are aware of how far the execution got.
        "
    )]
    async fn execute(
        &self,
        Parameters(ExecuteInput { code }): Parameters<ExecuteInput>,
    ) -> McpResult<CallToolResult> {
        let registrations = self
            .upstream
            .iter()
            .map(|m| {
                format!(
                    "registerMCP({{ name: {name}, url: {url} }});",
                    name = json!(&m.name),
                    url = json!(&m.url)
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n");
        let namespaces = self
            .upstream
            .iter()
            .map(|m| {
                let fns: Vec<String> = m.tools.iter().map(|(_, t)| t.fn_impl(&m.name)).collect();

                format!(
                    "{docstring}
namespace {namespace} {{
  {fns}
}}",
                    docstring = generate_docstring(&m.description),
                    namespace = &m.namespace,
                    fns = fns.join("\n\n")
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n");

        let to_execute = format!(
            "import {{ registerMCP, callMCPTool }} from \"mcp-client\"\n{registrations}\n{namespaces}\n{code}\n\nexport default await run();"
        );

        let result = self
            .executor
            .execute(to_execute)
            .await
            .map_err(|e| McpError::internal_error(e, None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{result:#?}"
        ))]))
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct ExecuteInput {
    /// Typescript code to execute.
    /// Example:
    /// async function ``run()`` {
    ///   // YOUR CODE GOES HERE e.g. const result = await ``client.method();``
    ///   // ALWAYS RETURN THE RESULT e.g. return result;
    /// }
    ///
    pub code: String,
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
    pub(crate) description: Option<String>,
    pub(crate) fn_name: String,
    pub(crate) input_type: String,
    pub(crate) output_type: String,
    pub(crate) types: String,
}

impl UpstreamTool {
    pub(crate) fn from_tool(tool: Tool) -> Result<Self> {
        let fn_name = Case::Camel.sanitize(&tool.name);
        debug!(
            "Generating Typescript interface for tool: '{}' -> function {fn_name}",
            &tool.name
        );

        let input_types =
            codegen::typegen::generate_types(json!(tool.input_schema), &format!("{fn_name}Input"))?;
        debug!(
            "Generated {} types for input schema",
            input_types.types_generated
        );

        let mut types = input_types.types;

        let output_type = if let Some(output_schema) = tool.output_schema {
            let output_types = codegen::typegen::generate_types(
                json!(output_schema),
                &format!("{fn_name}Output"),
            )?;
            debug!(
                "Generated {} types for output schema",
                output_types.types_generated
            );
            types = format!("{types}\n\n{}", output_types.types);
            output_types.type_signature
        } else {
            debug!("No output type listed, falling back on `string`");
            "string".to_string()
        };

        Ok(Self {
            tool_name: tool.name.to_string(),
            title: tool.title,
            description: tool.description.map(String::from),
            fn_name: codegen::case::Case::Camel.sanitize(&tool.name),
            input_type: input_types.type_signature,
            output_type,
            types,
        })
    }

    pub(crate) fn fn_signature(&self, include_types: bool) -> String {
        let docstring_content = format!(
            "{title}{desc}",
            title = &self
                .title
                .as_ref()
                .map(|t| format!("{t}\n\n"))
                .unwrap_or_default(),
            desc = &self.description.clone().unwrap_or_default()
        );

        let types = if include_types && !self.types.is_empty() {
            format!("{}\n\n", &self.types)
        } else {
            String::new()
        };

        format!(
            "{types}{docstring}\nasync function {fn_name}(input: {input}): Promise<{output}>",
            docstring = generate_docstring(&docstring_content),
            fn_name = &self.fn_name,
            input = &self.input_type,
            output = &self.output_type
        )
    }

    pub(crate) fn fn_impl(&self, mcp_name: &str) -> String {
        format!(
            "{fn_sig} {{
  return await callMCPTool<{output}>({{
    name: {name},
    tool: {tool},
    arguments: input,
  }});
}}",
            fn_sig = self.fn_signature(true),
            name = json!(mcp_name),
            tool = json!(&self.tool_name),
            output = &self.output_type,
        )
    }
}
