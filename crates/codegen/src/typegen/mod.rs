use indexmap::IndexMap;
use schemars::schema::{RootSchema, Schema};

use crate::{CodegenResult, SchemaDefinitions, case::Case, utils::assign_type_names};

pub struct TypegenResult {
    pub type_signature: String,
    pub types: Vec<String>,
}
pub fn generate_typescript_types(
    json_schema: serde_json::Value,
    type_name: &str,
) -> CodegenResult<TypegenResult> {
    let root_schema: RootSchema = serde_json::from_value(json_schema)?;

    // ensure all objects have type names
    let mut defs: SchemaDefinitions = IndexMap::new();
    for (ref_key, s) in root_schema.definitions {
        let type_name = Case::Pascal.sanitize(&ref_key);
        defs.insert(ref_key, assign_type_names(s, &type_name));
    }
    let schema = assign_type_names(Schema::Object(root_schema.schema), type_name);

    // println!("{schema:#?}");
    todo!()
}
