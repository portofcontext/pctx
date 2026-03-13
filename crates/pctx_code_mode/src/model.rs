use std::fmt::Display;

use schemars::{JsonSchema, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use utoipa::ToSchema;

// -------------- List Functions --------------
#[derive(Debug, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListFunctionsOutput {
    /// Available functions
    pub functions: Vec<ListedFunction>,

    pub code: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListedFunction {
    /// Namespace the function belongs in
    pub namespace: String,
    /// Function name
    pub name: String,
    /// Function description
    pub description: Option<String>,
}

// -------------- Get Function Details --------------

#[derive(Debug, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetFunctionDetailsInput {
    /// List of functions to get details of.
    #[schema(value_type = Vec<String>)]
    pub functions: Vec<FunctionId>,
}

#[derive(Debug, Clone, Default)]
pub struct FunctionId {
    pub mod_name: String,
    pub fn_name: String,
}

impl Display for FunctionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.mod_name, self.fn_name)
    }
}

impl JsonSchema for FunctionId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FunctionId".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "string",
            "description": "Function representation in the form should be in the form '<namespace>.<function name>'. e.g. If there is a function `getData` within the `DataApi` namespace the value provided in this field is DataApi.getData"
        })
    }
}

impl Serialize for FunctionId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for FunctionId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.splitn(2, '.').collect();

        if parts.len() != 2 {
            return Err(serde::de::Error::custom(format!(
                "Expected format '<mod_name>.<fn_name>', got '{}'",
                s
            )));
        }

        Ok(FunctionId {
            mod_name: parts[0].to_string(),
            fn_name: parts[1].to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetFunctionDetailsOutput {
    pub functions: Vec<FunctionDetails>,

    pub code: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct FunctionDetails {
    #[serde(flatten)]
    pub listed: ListedFunction,

    /// typescript input type for the function
    pub input_type: String,
    /// typescript output type for the function
    pub output_type: String,
    /// full typescript type definitions for input/output types
    pub types: String,
}

// -------------- Execute --------------

#[allow(clippy::doc_markdown)]
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema, ToSchema)]
#[serde(default)]
pub struct ExecuteBashInput {
    /// Bash command to execute
    pub command: String,
}

#[allow(clippy::doc_markdown)]
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema, ToSchema)]
#[serde(default)]
pub struct ExecuteInput {
    /// Typescript code to execute.
    ///
    /// REQUIRED FORMAT:
    /// async function ``run()`` {
    ///   // YOUR CODE GOES HERE
    ///   // ALWAYS RETURN THE RESULT e.g. return result;
    /// }
    ///
    /// IMPORTANT: Your code should ONLY contain the function definition.
    /// The sandbox automatically calls run() and exports the result.
    ///
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, ToSchema)]
pub struct ExecuteOutput {
    /// Success of executed code
    pub success: bool,
    /// Standard output of executed code
    pub stdout: String,
    /// Standard error of executed code
    pub stderr: String,
    /// Value returned by executed function
    #[schema(value_type = Object)]
    pub output: Option<serde_json::Value>,
}
impl ExecuteOutput {
    pub fn markdown(&self) -> String {
        format!(
            "Code Executed Successfully: {success}

# Return Value
```json
{return_val}
```

# STDOUT
{stdout}

# STDERR
{stderr}
",
            success = self.success,
            return_val = serde_json::to_string_pretty(&self.output)
                .unwrap_or(json!(&self.output).to_string()),
            stdout = &self.stdout,
            stderr = &self.stderr,
        )
    }
}
impl Display for ExecuteOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", json!(&self))
    }
}

// -------------- Callbacks --------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CallbackConfig {
    pub name: String,
    pub namespace: Option<String>,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}
impl CallbackConfig {
    pub fn id(&self) -> String {
        format!(
            "{}{}",
            self.namespace
                .as_ref()
                .map(|n| format!("{n}__"))
                .unwrap_or_default(),
            &self.name
        )
    }
}
