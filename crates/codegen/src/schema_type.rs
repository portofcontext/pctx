use std::fmt::Display;

use schemars::schema::{
    InstanceType, ObjectValidation, Schema, SchemaObject, SingleOrVec, SubschemaValidation,
};
use serde::{Deserialize, Serialize};

use crate::{
    CodegenResult, SchemaDefinitions,
    utils::{self, anything_schema},
};

pub static X_TYPE_NAME: &str = "x-type-name";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefSchemaType {
    pub ref_key: String,
    pub schema_obj: SchemaObject,
    pub nullable: bool,
}
impl RefSchemaType {
    pub fn follow(&self, defs: &SchemaDefinitions) -> CodegenResult<Schema> {
        defs.get(&self.ref_key)
            .cloned()
            .ok_or(crate::CodegenError::TypeGen(format!(
                "Failed following JSON schema reference, `#/$defs/{}` does not exist",
                &self.ref_key
            )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnySchemaType {
    pub nullable: bool,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanSchemaType {
    pub nullable: bool,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberSchemaType {
    pub nullable: bool,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringSchemaType {
    pub nullable: bool,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumSchemaType {
    pub nullable: bool,
    pub options: Vec<serde_json::Value>,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerSchemaType {
    pub nullable: bool,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchemaType {
    pub nullable: bool,
    pub type_name: String,
    pub obj: ObjectValidation,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSchemaType {
    pub nullable: bool,
    pub value_schema: Schema,
    pub obj: ObjectValidation,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArraySchemaType {
    pub nullable: bool,
    pub item_schema: Schema,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnionSchemaType {
    pub nullable: bool,
    pub union_schemas: Vec<Schema>,
    pub schema_obj: SchemaObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum SchemaType {
    Reference(RefSchemaType),
    Any(AnySchemaType),
    Boolean(BooleanSchemaType),
    Number(NumberSchemaType),
    String(StringSchemaType),
    Enum(EnumSchemaType),
    Integer(IntegerSchemaType),
    Object(ObjectSchemaType),
    Map(MapSchemaType),
    Array(ArraySchemaType),
    Union(UnionSchemaType),
}

impl Display for SchemaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typ = match self {
            SchemaType::Any(_) => "any",
            SchemaType::Reference(_) => "ref",
            SchemaType::Boolean(_) => "bool",
            SchemaType::Number(_) => "float",
            SchemaType::String(_) => "str",
            SchemaType::Enum(_) => "enum",
            SchemaType::Integer(_) => "int",
            SchemaType::Object(_) => "obj",
            SchemaType::Map(_) => "map",
            SchemaType::Array(_) => "arr",
            SchemaType::Union(_) => "union",
        };

        write!(f, "{typ}")
    }
}

impl SchemaType {
    pub fn is_any(&self) -> bool {
        matches!(self, SchemaType::Any(_))
    }

    pub fn is_ref(&self) -> bool {
        matches!(self, SchemaType::Reference(_))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, SchemaType::Boolean(_))
    }

    pub fn is_num(&self) -> bool {
        matches!(self, SchemaType::Number(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, SchemaType::Integer(_))
    }

    pub fn is_str(&self) -> bool {
        matches!(self, SchemaType::String(_))
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, SchemaType::Enum(_))
    }

    pub fn is_obj(&self) -> bool {
        matches!(self, SchemaType::Object(_))
    }

    pub fn is_map(&self) -> bool {
        matches!(self, SchemaType::Map(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, SchemaType::Array(_))
    }

    pub fn is_union(&self) -> bool {
        matches!(self, SchemaType::Union(_))
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            SchemaType::Integer(_)
                | SchemaType::Number(_)
                | SchemaType::Boolean(_)
                | SchemaType::String(_)
        )
    }

    pub fn is_nullable(&self) -> bool {
        match self {
            SchemaType::Any(AnySchemaType { nullable, .. })
            | SchemaType::Boolean(BooleanSchemaType { nullable, .. })
            | SchemaType::Integer(IntegerSchemaType { nullable, .. })
            | SchemaType::Number(NumberSchemaType { nullable, .. })
            | SchemaType::Enum(EnumSchemaType { nullable, .. })
            | SchemaType::String(StringSchemaType { nullable, .. })
            | SchemaType::Object(ObjectSchemaType { nullable, .. })
            | SchemaType::Map(MapSchemaType { nullable, .. })
            | SchemaType::Union(UnionSchemaType { nullable, .. })
            | SchemaType::Array(ArraySchemaType { nullable, .. })
            | SchemaType::Reference(RefSchemaType { nullable, .. }) => *nullable,
        }
    }

    pub fn schema_obj(&self) -> &SchemaObject {
        match self {
            SchemaType::Any(AnySchemaType { schema_obj, .. })
            | SchemaType::Boolean(BooleanSchemaType { schema_obj, .. })
            | SchemaType::Integer(IntegerSchemaType { schema_obj, .. })
            | SchemaType::Number(NumberSchemaType { schema_obj, .. })
            | SchemaType::Enum(EnumSchemaType { schema_obj, .. })
            | SchemaType::String(StringSchemaType { schema_obj, .. })
            | SchemaType::Object(ObjectSchemaType { schema_obj, .. })
            | SchemaType::Map(MapSchemaType { schema_obj, .. })
            | SchemaType::Union(UnionSchemaType { schema_obj, .. })
            | SchemaType::Array(ArraySchemaType { schema_obj, .. })
            | SchemaType::Reference(RefSchemaType { schema_obj, .. }) => schema_obj,
        }
    }

    pub fn type_signature(
        &self,
        required: bool,
        defs: &SchemaDefinitions,
    ) -> CodegenResult<String> {
        let mut sig: String = match self {
            SchemaType::Reference(ref_schema_type) => {
                let followed = ref_schema_type.follow(defs)?;
                SchemaType::from(followed).type_signature(required, defs)?
            }
            SchemaType::Any(_) => "any".into(),
            SchemaType::Boolean(_) => "boolean".into(),
            SchemaType::Integer(_) | SchemaType::Number(_) => "number".into(),
            SchemaType::String(_) => "string".into(),
            SchemaType::Enum(EnumSchemaType { options, .. }) => options
                .iter()
                .map(|o| o.to_string())
                .collect::<Vec<String>>()
                .join(" | "),
            SchemaType::Object(ObjectSchemaType { type_name, .. }) => type_name.clone(),
            SchemaType::Map(MapSchemaType { value_schema, .. }) => format!(
                "{{ [key: string]: {val_sig} }}",
                val_sig = SchemaType::from(value_schema).type_signature(false, defs)?
            ),
            SchemaType::Array(ArraySchemaType { item_schema, .. }) => format!(
                "{item_sig}[]",
                item_sig = SchemaType::from(item_schema).type_signature(true, defs)?
            ),
            SchemaType::Union(UnionSchemaType { union_schemas, .. }) => union_schemas
                .iter()
                .map(|s| SchemaType::from(s).type_signature(true, defs))
                .collect::<CodegenResult<Vec<String>>>()?
                .join(" | "),
        };

        if self.is_nullable() {
            sig = format!("{sig} | null")
        }
        if !required {
            sig = format!("{sig} | undefined")
        }
        Ok(sig)
    }
}

impl From<&Schema> for SchemaType {
    fn from(schema: &Schema) -> Self {
        if let Schema::Object(obj) = &schema {
            return SchemaType::from(obj);
        }
        // Fallback for non-object schemas
        SchemaType::Any(AnySchemaType {
            nullable: false,
            schema_obj: SchemaObject::default(),
        })
    }
}

impl From<Schema> for SchemaType {
    fn from(schema: Schema) -> Self {
        SchemaType::from(&schema)
    }
}

impl From<&SchemaObject> for SchemaType {
    fn from(obj: &SchemaObject) -> Self {
        // handle sub schemas first
        if let Some(ref sub) = obj.subschemas {
            return handle_union(obj, sub);
        }

        // Handle reference type
        if let Some(ref ref_name) = obj.reference {
            let ref_key = ref_name
                .split('/')
                .next_back()
                .unwrap_or_default()
                .to_string();
            return SchemaType::Reference(RefSchemaType {
                ref_key,
                schema_obj: obj.clone(),
                nullable: check_nullable(&obj.instance_type),
            });
        }

        // Process based on instance type
        let (instance_type, nullable) = match &obj.instance_type {
            Some(SingleOrVec::Single(typ)) => (**typ, false),
            Some(SingleOrVec::Vec(types)) => {
                let non_null: Vec<_> = types
                    .iter()
                    .filter(|t| !matches!(**t, InstanceType::Null))
                    .collect();

                if non_null.is_empty() {
                    return SchemaType::Any(AnySchemaType {
                        nullable: !types.is_empty(),
                        schema_obj: obj.clone(),
                    });
                } else if non_null.len() > 1 {
                    // multiple non-null types, reprocess as oneOf
                    let mut one_of = vec![];
                    for t in types {
                        let mut o = obj.clone();
                        o.instance_type = Some(SingleOrVec::Single(Box::new(*t)));
                        one_of.push(Schema::Object(o));
                    }
                    return SchemaType::from(Schema::Object(SchemaObject {
                        subschemas: Some(Box::new(SubschemaValidation {
                            one_of: Some(one_of),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }));
                } else {
                    (*non_null[0], types.len() > 1)
                }
            }
            None => {
                return SchemaType::Any(AnySchemaType {
                    nullable: false,
                    schema_obj: obj.clone(),
                });
            }
        };

        match instance_type {
            InstanceType::Boolean => SchemaType::Boolean(BooleanSchemaType {
                nullable,
                schema_obj: obj.clone(),
            }),
            InstanceType::Object => handle_object_type(obj, nullable),
            InstanceType::Array => handle_array_type(obj, nullable),
            InstanceType::Number => handle_number_types(obj, nullable, false),
            InstanceType::Integer => handle_number_types(obj, nullable, true),
            InstanceType::String => handle_string_type(obj, nullable),
            _ => SchemaType::Any(AnySchemaType {
                nullable: false,
                schema_obj: obj.clone(),
            }),
        }
    }
}

fn handle_union(
    obj: &SchemaObject,
    subschema: &schemars::schema::SubschemaValidation,
) -> SchemaType {
    let options = match (&subschema.one_of, &subschema.any_of) {
        (Some(opts), None) | (None, Some(opts)) => opts,
        _ => {
            // currently allOf is not support
            return SchemaType::Any(AnySchemaType {
                nullable: false,
                schema_obj: obj.clone(),
            });
        }
    };

    let (non_null_options, nullable) = extract_non_null_schemas(options);
    if non_null_options.is_empty() {
        SchemaType::Any(AnySchemaType {
            nullable,
            schema_obj: obj.clone(),
        })
    } else {
        SchemaType::Union(UnionSchemaType {
            nullable,
            union_schemas: non_null_options,
            schema_obj: obj.clone(),
        })
    }
}

fn handle_number_types(obj: &SchemaObject, nullable: bool, is_int: bool) -> SchemaType {
    if let Some(ref enum_vals) = obj.enum_values {
        let options: Vec<serde_json::Value> = enum_vals
            .iter()
            .filter_map(|val| val.as_number().map(|_| val.clone()))
            .collect();

        if !options.is_empty() {
            return SchemaType::Enum(EnumSchemaType {
                nullable,
                options,
                schema_obj: obj.clone(),
            });
        }
    }

    if is_int {
        SchemaType::Integer(IntegerSchemaType {
            nullable,
            schema_obj: obj.clone(),
        })
    } else {
        SchemaType::Number(NumberSchemaType {
            nullable,
            schema_obj: obj.clone(),
        })
    }
}

fn handle_string_type(obj: &SchemaObject, nullable: bool) -> SchemaType {
    if let Some(ref enum_vals) = obj.enum_values {
        let options: Vec<serde_json::Value> = enum_vals
            .iter()
            .filter_map(|val| val.as_str().filter(|s| !s.is_empty()).map(|_| val.clone()))
            .collect();

        if !options.is_empty() {
            return SchemaType::Enum(EnumSchemaType {
                nullable,
                options,
                schema_obj: obj.clone(),
            });
        }
    }
    SchemaType::String(StringSchemaType {
        nullable,
        schema_obj: obj.clone(),
    })
}

fn handle_object_type(obj: &SchemaObject, nullable: bool) -> SchemaType {
    if let Some(obj_validation) = &obj.object {
        if obj_validation.properties.is_empty() {
            let value_schema = obj_validation
                .additional_properties
                .clone()
                .map(|a| *a)
                .unwrap_or(utils::anything_schema());
            SchemaType::Map(MapSchemaType {
                nullable,
                value_schema,
                schema_obj: obj.clone(),
                obj: *obj_validation.clone(),
            })
        } else {
            SchemaType::Object(ObjectSchemaType {
                nullable,
                type_name: obj
                    .extensions
                    .get(X_TYPE_NAME)
                    .map(|e| e.as_str().map(String::from).unwrap_or_default())
                    .unwrap_or_default(),
                obj: *obj_validation.clone(),
                schema_obj: obj.clone(),
            })
        }
    } else {
        SchemaType::Map(MapSchemaType {
            nullable,
            value_schema: utils::anything_schema(),
            schema_obj: obj.clone(),
            obj: ObjectValidation {
                additional_properties: Some(Box::new(utils::anything_schema())),
                ..Default::default()
            },
        })
    }
}

fn handle_array_type(obj: &SchemaObject, nullable: bool) -> SchemaType {
    if let Some(ref arr) = obj.array {
        let item_schema = match arr.items.clone() {
            Some(SingleOrVec::Single(s)) => *s,
            Some(SingleOrVec::Vec(s)) => Schema::Object(SchemaObject {
                subschemas: Some(Box::new(SubschemaValidation {
                    one_of: Some(s),
                    ..Default::default()
                })),
                ..Default::default()
            }),
            None => anything_schema(),
        };
        SchemaType::Array(ArraySchemaType {
            nullable,
            item_schema,
            schema_obj: obj.clone(),
        })
    } else {
        SchemaType::Any(AnySchemaType {
            nullable,
            schema_obj: obj.clone(),
        })
    }
}

fn extract_non_null_schemas(schemas: &[Schema]) -> (Vec<Schema>, bool) {
    let mut non_null = vec![];
    let mut has_null = false;

    for schema in schemas {
        if is_null_schema(schema) {
            has_null = true;
        } else {
            non_null.push(schema.clone());
        }
    }

    (non_null, has_null)
}

fn check_nullable(instance_type: &Option<SingleOrVec<InstanceType>>) -> bool {
    match instance_type {
        Some(SingleOrVec::Single(typ)) => matches!(**typ, InstanceType::Null),
        Some(SingleOrVec::Vec(types)) => types.iter().any(|t| matches!(t, InstanceType::Null)),
        None => false,
    }
}

fn is_null_schema(s: &Schema) -> bool {
    if let Schema::Object(obj) = s {
        if let Some(ref instance_type) = obj.instance_type {
            match instance_type {
                SingleOrVec::Single(typ) => matches!(**typ, InstanceType::Null),
                SingleOrVec::Vec(types) => {
                    types.len() == 1 && matches!(types.first(), Some(InstanceType::Null))
                }
            }
        } else {
            false
        }
    } else {
        false
    }
}
