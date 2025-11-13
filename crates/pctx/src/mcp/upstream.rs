use anyhow::Result;
use codegen::{case::Case, generate_docstring};
use indexmap::IndexMap;
use log::debug;
use pctx_config::server::ServerConfig;
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct UpstreamMcp {
    pub(crate) name: String,
    pub(crate) namespace: String,
    pub(crate) description: String,
    pub(crate) url: Url,
    pub(crate) tools: IndexMap<String, UpstreamTool>,
    pub(crate) registration: serde_json::Value,
}
impl UpstreamMcp {
    pub(crate) async fn from_server(server: &ServerConfig) -> Result<Self> {
        debug!("Fetching tools from '{}'({})...", &server.name, &server.url);

        let mcp_client = server.connect().await?;

        debug!(
            "Successfully connected to '{}', inspecting tools...",
            server.name
        );

        let listed_tools = mcp_client.list_all_tools().await?;
        debug!("Found {} tools", listed_tools.len());

        let mut tools = IndexMap::new();
        for t in listed_tools {
            let tool = UpstreamTool::from_tool(t)?;
            tools.insert(tool.fn_name.clone(), tool);
        }

        let description = mcp_client
            .peer_info()
            .and_then(|p| p.server_info.title.clone())
            .unwrap_or(format!("MCP server at {}", server.url));

        mcp_client.cancel().await?;

        Ok(Self {
            name: server.name.clone(),
            namespace: Case::Pascal.sanitize(&server.name),
            description,
            url: server.url.clone(),
            tools,
            registration: json!(server),
        })
    }
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
    pub(crate) fn from_tool(tool: rmcp::model::Tool) -> Result<Self> {
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
            debug!("No output type listed, falling back on `any`");
            "any".to_string()
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
            "{types}{docstring}\nexport async function {fn_name}(input: {input}): Promise<{output}>",
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
