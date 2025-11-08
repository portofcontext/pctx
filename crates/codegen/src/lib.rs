pub mod case;
pub mod format;
pub mod schema_type;
pub mod typegen;
pub mod utils;

use indexmap::IndexMap;
use schemars::schema::Schema;
use thiserror::Error;

pub type SchemaDefinitions = IndexMap<String, Schema>;

pub type CodegenResult<T> = Result<T, CodegenError>;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Type generation error: {0}")]
    TypeGen(String),
}

pub fn generate_docstring(content: &str) -> String {
    let mut lines = vec!["/**".to_string()];

    let replace_pat = regex::Regex::new(r"\*\/").expect("invalid docstring replace_pat");
    for line in content.split('\n') {
        // in the unlikely event that the description has a `*/`
        // ending the typescript docstring, we add escapes
        lines.push(format!("* {}", replace_pat.replace_all(line, "*-/")))
    }

    lines.push("*/".into());

    lines.join("\n")
}
