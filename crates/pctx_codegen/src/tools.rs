use schemars::schema::RootSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::debug;

use crate::{
    CodegenResult,
    case::Case,
    ts_generate_docstring,
    typegen::{TypegenResult, generate_types},
};

pub const DEFAULT_NAMESPACE: &str = "Tools";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSet {
    pub name: Option<String>,
    pub description: String,
    pub tools: Vec<Tool>,
}

impl ToolSet {
    pub fn new(name: Option<String>, description: &str, tools: Vec<Tool>) -> Self {
        Self {
            name,
            description: description.into(),
            tools,
        }
    }

    pub fn tool_ids(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|t| t.id(self.name.as_deref()))
            .collect()
    }

    /// Returns the pascal case of the registered namespace
    /// falling back on `Tools` if not present
    pub fn pascal_namespace(&self) -> String {
        self.name
            .as_ref()
            .map(|n| Case::Pascal.sanitize(n))
            .unwrap_or(DEFAULT_NAMESPACE.to_string())
    }

    // ------------- Typescript-Specific Code Generation -------------

    /// Returns the generated typescript declaration (`.d.ts`) code for the ToolSet
    /// as a typescript `namespace`
    pub fn ts_namespace_declaration(&self, include_types: bool) -> String {
        let fns: Vec<String> = self
            .tools
            .iter()
            .map(|t| t.ts_fn_signature(include_types))
            .collect();

        self.ts_wrap_with_namespace(&fns.join("\n\n"))
    }

    /// Returns the full generated typescript code for this ToolSet as
    /// a typescript `namespace`
    pub fn ts_namespace_impl(&self) -> String {
        let fns: Vec<String> = self
            .tools
            .iter()
            .map(|t| t.ts_fn_impl(self.name.as_deref()))
            .collect();
        self.ts_wrap_with_namespace(&fns.join("\n\n"))
    }

    pub fn ts_wrap_with_namespace(&self, content: &str) -> String {
        format!(
            "{docstring}
namespace {namespace} {{
  {content}
}}",
            docstring = ts_generate_docstring(&self.description),
            namespace = self.pascal_namespace(),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub fn_name: String,
    pub description: Option<String>,

    pub input_schema: Option<RootSchema>,
    pub output_schema: Option<RootSchema>,

    input_type: Option<TypegenResult>,
    output_type: Option<TypegenResult>,
}

impl Tool {
    pub fn new(
        name: &str,
        description: Option<String>,
        input: Option<RootSchema>,
        output: Option<RootSchema>,
    ) -> CodegenResult<Self> {
        let fn_name = Case::Camel.sanitize(name);
        debug!("Generating Typescript interface for tool: '{name}' -> function {fn_name}",);

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
        })
    }

    pub fn id(&self, toolset_name: Option<&str>) -> String {
        format!(
            "{}{}",
            toolset_name.map(|n| format!("{n}__")).unwrap_or_default(),
            &self.name
        )
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

    /// Returns all the input and output types as a string for the Tool
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

    /// Returns the typescript function signature for the Tool with a docstring
    ///
    /// e.g.
    /// ```typescript
    /// /**
    ///  * function docstring
    /// */
    /// export async function myFunction(input: InputType): Promise<OutputType>
    /// ```
    ///
    /// The function signature has no trailing `;` or `{` so can be used for either
    /// .ts or .d.ts generation
    pub fn ts_fn_signature(&self, include_types: bool) -> String {
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
            docstring = ts_generate_docstring(&docstring_content),
            fn_name = &self.fn_name,
            output = &self.output_signature(),
        )
    }

    /// Returns the typescript function implementation including
    /// the function signature, input/output types, and internal tool
    /// functionality.
    pub fn ts_fn_impl(&self, toolset_name: Option<&str>) -> String {
        let arguments = self
            .input_schema
            .as_ref()
            .map(|_| "arguments: input,".to_string())
            .unwrap_or_default();

        format!(
            "{fn_sig} {{
  return await invokeInternal({{ name: {name}, {arguments} }});
}}",
            fn_sig = self.ts_fn_signature(true),
            name = json!(self.id(toolset_name))
        )
    }

    /// Creates a typescript type map entry for the given tool,
    /// meant to be wrapped by `type InvokeMap { ...entries } `
    pub fn ts_invoke_map_entry(&self, toolset_name: Option<&str>) -> String {
        let args = match &self.input_type {
            Some(i) if i.all_optional => format!("{} | undefined", &i.type_signature),
            Some(i) => format!("{}", &i.type_signature),
            None => format!("any | undefined"),
        };

        format!(
            "{name}: {{ args: {args}, returns: {returns} }};",
            name = json!(self.id(toolset_name)),
            returns = self.output_signature()
        )
    }
}
