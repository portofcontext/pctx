mod schema_data;

use handlebars::Handlebars;
use indexmap::IndexMap;
use schemars::schema::{RootSchema, Schema};
use serde_json::json;

use crate::{
    CodegenResult, SchemaDefinitions, case::Case, format::format_ts, schema_type::SchemaType,
    typegen::schema_data::ObjectSchemaData, utils::assign_type_names,
};

static TYPES_TEMPLATE: &str = include_str!("./types.handlebars");

pub struct TypegenResult {
    pub types_generated: usize,
    pub type_signature: String,
    pub types: String,
}
pub fn generate_types(
    json_schema: serde_json::Value,
    type_name: &str,
) -> CodegenResult<TypegenResult> {
    let root_schema: RootSchema = serde_json::from_value(json_schema)?;

    // ensure all objects have type names
    let mut defs: SchemaDefinitions = IndexMap::new();
    for (ref_key, s) in root_schema.definitions {
        // TODO: clashing type names?
        let type_name = Case::Pascal.sanitize(format!("{type_name} {ref_key}"));
        defs.insert(ref_key, assign_type_names(s, &type_name));
    }
    let schema = assign_type_names(
        Schema::Object(root_schema.schema),
        &Case::Pascal.sanitize(type_name),
    );

    // collect and generate types with handlebars
    let to_generate = ObjectSchemaData::collect(&schema, &defs)?;
    let types = Handlebars::new()
        .render_template(TYPES_TEMPLATE, &json!({"objects": to_generate}))
        .unwrap();

    Ok(TypegenResult {
        types: format_ts(&types),
        types_generated: to_generate.len(),
        type_signature: SchemaType::from(&schema).type_signature(true, &defs)?,
    })
}
