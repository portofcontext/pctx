use indexmap::IndexSet;
use schemars::schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    CodegenResult, SchemaDefinitions, generate_docstring,
    schema_type::{ObjectSchemaType, SchemaType},
    utils::get_description,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchemaData {
    name: String,
    doc_string: Option<String>,
    properties: Vec<ObjectPropertyData>,
    additional_props_sig: Option<String>,
}
impl ObjectSchemaData {
    pub fn collect(schema: &Schema, defs: &SchemaDefinitions) -> CodegenResult<Vec<Self>> {
        let mut visited_refs = IndexSet::new();
        let mut collected = vec![];
        Self::_collect(schema, defs, &mut visited_refs, &mut collected)?;

        Ok(collected)
    }

    pub fn new(obj_st: &ObjectSchemaType, defs: &SchemaDefinitions) -> CodegenResult<Self> {
        let mut properties = vec![];
        for (prop_name, prop_schema) in &obj_st.obj.properties {
            let prop_st = SchemaType::from(prop_schema);
            let required = obj_st.obj.required.contains(prop_name);
            let prop_data = ObjectPropertyData {
                name: prop_name.clone(),
                sig: prop_st.type_signature(required, defs)?,
                doc_string: get_description(&prop_schema.clone().into_object(), defs)?
                    .map(|desc| generate_docstring(&desc)),
                required,
                nullable: prop_st.is_nullable(),
            };
            properties.push(prop_data)
        }

        let additional_props_sig = if let Some(add_props) = &obj_st.obj.additional_properties {
            Some(SchemaType::from(*add_props.clone()).type_signature(false, defs)?)
        } else {
            None
        };

        Ok(Self {
            name: obj_st.type_name.clone(),
            doc_string: get_description(&obj_st.schema_obj, defs)?
                .map(|desc| generate_docstring(&desc)),
            properties,
            additional_props_sig,
        })
    }

    fn _collect(
        schema: &Schema,
        defs: &SchemaDefinitions,
        visited: &mut IndexSet<String>,
        collected: &mut Vec<ObjectSchemaData>,
    ) -> CodegenResult<()> {
        match SchemaType::from(schema) {
            SchemaType::Reference(ref_st) => {
                // track for circular references
                let is_new = visited.insert(ref_st.ref_key.clone());
                if is_new {
                    let followed = ref_st.follow(defs)?;
                    Self::_collect(&followed, defs, visited, collected)?;
                }
            }
            SchemaType::Object(obj_st) => {
                // Collect for type generation
                collected.push(Self::new(&obj_st, defs)?);

                // collect child schemas
                for (_, prop_schema) in &obj_st.obj.properties {
                    Self::_collect(prop_schema, defs, visited, collected)?;
                }
                if let Some(add_props) = &obj_st.obj.additional_properties {
                    Self::_collect(add_props, defs, visited, collected)?;
                }
            }
            SchemaType::Map(map_st) => {
                Self::_collect(&map_st.value_schema, defs, visited, collected)?;
            }
            SchemaType::Array(array_st) => {
                Self::_collect(&array_st.item_schema, defs, visited, collected)?;
            }
            SchemaType::Union(union_st) => {
                for union_schema in &union_st.union_schemas {
                    Self::_collect(union_schema, defs, visited, collected)?;
                }
            }
            SchemaType::Any(_)
            | SchemaType::Boolean(_)
            | SchemaType::Number(_)
            | SchemaType::String(_)
            | SchemaType::Enum(_)
            | SchemaType::Integer(_) => {
                // these types are effectively leaves (no child schemas)
                // and do not require type generation
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPropertyData {
    name: String,
    doc_string: Option<String>,
    sig: String,
    required: bool,
    nullable: bool,
}
