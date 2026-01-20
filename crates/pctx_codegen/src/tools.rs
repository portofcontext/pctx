use schemars::schema::RootSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::debug;
use uuid::Uuid;

use crate::{CodegenResult, case::Case, generate_docstring, typegen::generate_types_new};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSet {
    pub name: String,
    pub namespace: String,
    pub description: String,
    pub tools: Vec<Tool>,
}

impl ToolSet {
    pub fn new(name: &str, description: &str, tools: Vec<Tool>) -> Self {
        Self {
            name: name.into(),
            namespace: Case::Pascal.sanitize(name),
            description: description.into(),
            tools,
        }
    }

    pub fn namespace_interface(&self, include_types: bool) -> String {
        let fns: Vec<String> = self
            .tools
            .iter()
            .map(|t| t.fn_signature(include_types))
            .collect();

        self.wrap_with_namespace(&fns.join("\n\n"))
    }

    pub fn namespace(&self) -> String {
        let fns: Vec<String> = self.tools.iter().map(|t| t.fn_impl(&self.name)).collect();
        self.wrap_with_namespace(&fns.join("\n\n"))
    }

    pub fn wrap_with_namespace(&self, content: &str) -> String {
        format!(
            "{docstring}
namespace {namespace} {{
  {content}
}}",
            docstring = generate_docstring(&self.description),
            namespace = &self.namespace,
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    /// Unique identifier for this tool, auto-generated on creation
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: RootSchema,
    pub output_schema: Option<RootSchema>,

    pub fn_name: String,
    pub input_signature: String,
    pub output_signature: String,
    pub types: String,

    pub variant: ToolVariant,
}

impl Tool {
    pub fn new_mcp(
        name: &str,
        description: Option<String>,
        input: RootSchema,
        output: Option<RootSchema>,
    ) -> CodegenResult<Self> {
        Self::_new(name, description, input, output, ToolVariant::Mcp)
    }

    pub fn new_callback(
        name: &str,
        description: Option<String>,
        input: RootSchema,
        output: Option<RootSchema>,
    ) -> CodegenResult<Self> {
        Self::_new(name, description, input, output, ToolVariant::Callback)
    }

    fn _new(
        name: &str,
        description: Option<String>,
        input: RootSchema,
        output: Option<RootSchema>,
        variant: ToolVariant,
    ) -> CodegenResult<Self> {
        let fn_name = Case::Camel.sanitize(name);
        debug!(
            variant =? variant,
            "Generating Typescript interface for tool: '{name}' -> function {fn_name}",
        );

        let input_types = generate_types_new(input.clone(), &format!("{fn_name}Input"))?;
        let mut type_defs = input_types.types;
        let output_signature = if let Some(o) = output.clone() {
            let output_types = generate_types_new(o, &format!("{fn_name}Output"))?;
            type_defs = format!("{type_defs}\n\n{}", output_types.types);
            output_types.type_signature
        } else {
            debug!("No output type listed, falling back on `any`");
            "any".to_string()
        };

        Ok(Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description,
            input_schema: input,
            output_schema: output,
            fn_name,
            input_signature: input_types.type_signature,
            output_signature,
            types: type_defs,
            variant,
        })
    }

    pub fn fn_signature(&self, include_types: bool) -> String {
        let docstring_content = self.description.clone().unwrap_or_default();

        let types = if include_types && !self.types.is_empty() {
            format!("{}\n\n", &self.types)
        } else {
            String::new()
        };

        format!(
            "{types}{docstring}\nexport async function {fn_name}(input: {input}): Promise<{output}>",
            docstring = generate_docstring(&docstring_content),
            fn_name = &self.fn_name,
            input = &self.input_signature,
            output = &self.output_signature,
        )
    }

    pub fn fn_impl(&self, toolset_name: &str) -> String {
        match self.variant {
            ToolVariant::Mcp => {
                format!(
                    "{fn_sig} {{
  return await callMCPTool<{output}>({{
    serverName: {name},
    toolName: {tool},
    arguments: input,
  }});
}}",
                    fn_sig = self.fn_signature(true),
                    name = json!(toolset_name),
                    tool = json!(&self.name),
                    output = &self.output_signature,
                )
            }
            ToolVariant::Callback => {
                format!(
                    "{fn_sig} {{
  return await invokeCallback<{output}>({{
     id: {id},
     arguments: input,
  }});
}}",
                    fn_sig = self.fn_signature(true),
                    id = json!(format!("{toolset_name}.{}", &self.name)),
                    output = &self.output_signature,
                )
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolVariant {
    Mcp,
    Callback,
}
