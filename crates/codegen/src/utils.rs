use schemars::schema::{
    InstanceType, ObjectValidation, Schema, SchemaObject, SingleOrVec, SubschemaValidation,
};
use serde_json::json;

use crate::{
    case::Case,
    schema_type::{
        ArraySchemaType, MapSchemaType, ObjectSchemaType, SchemaType, UnionSchemaType, X_TYPE_NAME,
    },
};

pub fn anything_schema() -> Schema {
    Schema::Object(SchemaObject::default())
}

pub fn map_schema(value_schema: &Schema) -> Schema {
    let obj = SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
        object: Some(Box::new(ObjectValidation {
            additional_properties: Some(Box::new(value_schema.clone())),
            ..Default::default()
        })),
        ..Default::default()
    };

    Schema::Object(obj)
}

/// Iterates through the provided schema, assigning unique type names recursively
pub fn assign_type_names(schema: Schema, type_name: &str) -> Schema {
    match SchemaType::from(&schema) {
        SchemaType::Object(ObjectSchemaType {
            nullable,
            schema_obj,
            obj,
            ..
        }) => {
            let mut mutable_schema_obj = schema_obj.clone();
            mutable_schema_obj.instance_type =
                Some(rebuild_instance_type(InstanceType::Object, nullable));
            mutable_schema_obj
                .extensions
                .insert(X_TYPE_NAME.to_string(), json!(type_name));

            let mut mutable_obj_validation = obj.clone();

            // Process properties
            mutable_obj_validation.properties = obj
                .properties
                .into_iter()
                .map(|(prop_name, prop_schema)| {
                    let property_type_name =
                        Case::Pascal.sanitize(&format!("{type_name} {prop_name}"));
                    (
                        prop_name.clone(),
                        assign_type_names(prop_schema, &property_type_name),
                    )
                })
                .collect();
            mutable_obj_validation.additional_properties =
                obj.additional_properties.map(|additional| {
                    let additional_class_name = format!("{type_name} AdditionalProps");

                    Box::new(assign_type_names(*additional, &additional_class_name))
                });

            mutable_schema_obj.object = Some(Box::new(mutable_obj_validation));
            Schema::Object(mutable_schema_obj)
        }
        SchemaType::Map(MapSchemaType {
            nullable,
            value_schema,
            schema_obj,
            obj,
            ..
        }) => {
            let mut mutable_schema_obj = schema_obj.clone();
            mutable_schema_obj.instance_type =
                Some(rebuild_instance_type(InstanceType::Object, nullable));
            let mut mutable_obj_validation = obj.clone();

            mutable_obj_validation.additional_properties =
                Some(Box::new(assign_type_names(value_schema, type_name)));
            mutable_schema_obj.object = Some(Box::new(mutable_obj_validation));

            Schema::Object(mutable_schema_obj)
        }
        SchemaType::Array(ArraySchemaType {
            nullable,
            item_schema,
            schema_obj,
        }) => {
            let mut mutable_schema_obj = schema_obj.clone();
            mutable_schema_obj.instance_type =
                Some(rebuild_instance_type(InstanceType::Array, nullable));
            let mut arr = schema_obj.array.unwrap_or_default();
            arr.items = Some(SingleOrVec::Single(Box::new(assign_type_names(
                item_schema,
                type_name,
            ))));
            mutable_schema_obj.array = Some(arr);

            Schema::Object(mutable_schema_obj)
        }
        SchemaType::Union(UnionSchemaType {
            nullable,
            schema_obj,
            union_schemas,
        }) => {
            let mut mutable_schema_obj = schema_obj.clone();
            let mut one_of: Vec<Schema> = union_schemas
                .into_iter()
                .enumerate()
                .map(|(i, s)| {
                    let option_type = SchemaType::from(&s);
                    let option_type_name =
                        Case::Pascal.sanitize(&format!("{type_name} {option_type} {i}"));
                    assign_type_names(s, &option_type_name)
                })
                .collect();
            if nullable {
                one_of.push(Schema::Object(SchemaObject {
                    instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Null))),
                    ..Default::default()
                }));
            }

            mutable_schema_obj.subschemas = Some(Box::new(SubschemaValidation {
                one_of: Some(one_of),
                ..Default::default()
            }));
            Schema::Object(mutable_schema_obj)
        }
        SchemaType::Any(_)
        | SchemaType::Boolean(_)
        | SchemaType::Number(_)
        | SchemaType::String(_)
        | SchemaType::Enum(_)
        | SchemaType::Integer(_)
        | SchemaType::Reference(_) => schema,
    }
}

fn rebuild_instance_type(typ: InstanceType, nullable: bool) -> SingleOrVec<InstanceType> {
    if nullable {
        SingleOrVec::Vec(vec![typ, InstanceType::Null])
    } else {
        SingleOrVec::Single(Box::new(typ))
    }
}
