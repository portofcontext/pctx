mod schema_data;

use std::collections::HashSet;

use handlebars::Handlebars;
use indexmap::IndexMap;
use schemars::schema::{RootSchema, Schema};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    CodegenResult, SchemaDefinitions, case::Case, format::format_ts, schema_type::SchemaType,
    typegen::schema_data::ObjectSchemaData, utils::assign_type_names,
};

static TYPES_TEMPLATE: &str = include_str!("./types.handlebars");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypegenResult {
    pub types_generated: usize,
    pub type_signature: String,
    pub all_optional: bool,
    pub types: String,
}

pub fn generate_types(root_schema: RootSchema, type_name: &str) -> CodegenResult<TypegenResult> {
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
        all_optional: is_all_optional(&schema, &defs)?,
    })
}

fn is_all_optional(schema: &Schema, defs: &SchemaDefinitions) -> CodegenResult<bool> {
    // follow top schema until no longer ref
    let mut schema_type = SchemaType::from(schema);
    let mut visited = HashSet::new();
    loop {
        if let SchemaType::Reference(ref_st) = &schema_type {
            let is_new = visited.insert(ref_st.ref_key.clone());
            if is_new {
                let followed = ref_st.follow(defs)?;
                schema_type = SchemaType::from(followed)
            } else {
                // circular ref
                break;
            }
        } else {
            break;
        }
    }

    // "all optional" means {} is a valid default
    // therefore all maps & objects with no required fields
    // satisfy this
    match schema_type {
        SchemaType::Map(_) => Ok(true),
        SchemaType::Object(obj_st) => Ok(obj_st.obj.required.is_empty()),
        _ => Ok(false),
    }
}
