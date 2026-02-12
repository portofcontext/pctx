use schemars::schema::RootSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::debug;

use crate::{
    CodegenResult,
    case::Case,
    generate_docstring,
    typegen::{TypegenResult, generate_types},
};

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
    pub name: String,
    pub fn_name: String,
    pub description: Option<String>,
    pub variant: ToolVariant,

    pub input_schema: Option<RootSchema>,
    pub output_schema: Option<RootSchema>,

    input_type: Option<TypegenResult>,
    output_type: Option<TypegenResult>,
}

impl Tool {
    pub fn new_mcp(
        name: &str,
        description: Option<String>,
        input: Option<RootSchema>,
        output: Option<RootSchema>,
    ) -> CodegenResult<Self> {
        Self::_new(name, description, input, output, ToolVariant::Mcp)
    }

    pub fn new_callback(
        name: &str,
        description: Option<String>,
        input: Option<RootSchema>,
        output: Option<RootSchema>,
    ) -> CodegenResult<Self> {
        Self::_new(name, description, input, output, ToolVariant::Callback)
    }

    fn _new(
        name: &str,
        description: Option<String>,
        input: Option<RootSchema>,
        output: Option<RootSchema>,
        variant: ToolVariant,
    ) -> CodegenResult<Self> {
        let fn_name = Case::Camel.sanitize(name);
        debug!(
            variant =? variant,
            "Generating Typescript interface for tool: '{name}' -> function {fn_name}",
        );

        let input_type = if let Some(i) = &input {
            Some(generate_types(i.clone(), &format!("{fn_name}Input"))?)
        } else {
            None
        };

        let output_type = if let Some(o) = output.clone() {
            Some(generate_types(o, &format!("{fn_name}Output"))?)
        } else {
            None
        };

        Ok(Self {
            name: name.into(),
            description,
            input_schema: input,
            output_schema: output,
            fn_name,
            input_type,
            output_type,
            variant,
        })
    }

    pub fn input_signature(&self) -> Option<String> {
        // No input schema -> no params for the generated function
        self.input_type.as_ref().map(|i| i.type_signature.clone())
    }

    pub fn output_signature(&self) -> String {
        // No output schema -> usually means not documented so output type fallback is `any`, not `void`
        self.output_type
            .as_ref()
            .map(|o| o.type_signature.clone())
            .unwrap_or("any".into())
    }

    pub fn types(&self) -> String {
        let mut type_defs = String::new();
        if let Some(i) = &self.input_type {
            type_defs = i.types.clone();
        }
        if let Some(o) = &self.output_type {
            type_defs = format!("{type_defs}\n\n{}", &o.types);
        }

        type_defs
    }

    pub fn fn_signature(&self, include_types: bool) -> String {
        let docstring_content = self.description.clone().unwrap_or_default();

        let mut types = String::new();
        if include_types && !self.types().is_empty() {
            types = format!("{}\n\n", self.types());
        }

        let params = match &self.input_type {
            Some(i) if i.all_optional => format!("input: {} = {{}}", &i.type_signature),
            Some(i) => format!("input: {}", &i.type_signature),
            None => String::default(),
        };

        format!(
            "{types}{docstring}\nexport async function {fn_name}({params}): Promise<{output}>",
            docstring = generate_docstring(&docstring_content),
            fn_name = &self.fn_name,
            output = &self.output_signature(),
        )
    }

    pub fn fn_impl(&self, toolset_name: &str) -> String {
        let arguments = self
            .input_schema
            .as_ref()
            .map(|_| format!("arguments: input,"))
            .unwrap_or_default();
        match self.variant {
            ToolVariant::Mcp => {
                format!(
                    "{fn_sig} {{
  return await callMCPTool<{output}>({{
    serverName: {name},
    toolName: {tool},
    {arguments}
  }});
}}",
                    fn_sig = self.fn_signature(true),
                    name = json!(toolset_name),
                    tool = json!(&self.name),
                    output = &self.output_signature(),
                )
            }
            ToolVariant::Callback => {
                format!(
                    "{fn_sig} {{
  return await invokeCallback<{output}>({{
     id: {id},
     {arguments}
  }});
}}",
                    fn_sig = self.fn_signature(true),
                    id = json!(format!("{toolset_name}.{}", &self.name)),
                    output = &self.output_signature(),
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
