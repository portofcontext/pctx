//! Python stub generation for pctx tools.
//!
//! Generates `.pyi`-style stub strings that can be passed directly to
//! `pctx_python_runtime::ExecuteOptions::with_stubs()`.  The stubs are consumed
//! by the monty-type-checking layer (Astral `ty_python_semantic`) — they are
//! never executed by the monty interpreter itself, so they may use full Python
//! typing syntax even though monty doesn't support classes at runtime.
//!
//! # Type mapping
//!
//! | JSON Schema type | Python annotation        |
//! |---|---|
//! | any / empty      | `Any`                    |
//! | boolean          | `bool`                   |
//! | integer          | `int`                    |
//! | number           | `float`                  |
//! | string           | `str`                    |
//! | string enum      | `Literal["a", "b"]`      |
//! | number/int enum  | `Literal[1, 2]`          |
//! | object (named)   | `dict[str, Any]`         |
//! | map (additional) | `dict[str, V]`           |
//! | array            | `list[T]`                |
//! | union            | `T1 \| T2`               |
//! | nullable         | `T \| None`              |
//!
//! Named object types collapse to `dict[str, Any]` because monty can't
//! instantiate Python classes — the LLM always works with plain dicts.

use std::collections::{HashMap, HashSet};

use schemars::schema::Schema;

use crate::{
    CodegenResult, SchemaDefinitions, Tool, ToolSet,
    case::Case,
    schema_type::{EnumSchemaType, SchemaType},
    tools::ToolVariant,
};

// ── identifier sanitization ───────────────────────────────────────────────────

/// Convert a property/tool name to a valid Python identifier.
///
/// 1. Convert to snake_case.
/// 2. Append `_` if the result is a Python keyword (would be a syntax error).
///
/// PEP 8 recommends the trailing-underscore convention for keyword conflicts
/// (e.g. `from_`, `import_`, `class_`).
fn sanitize_python_param(name: &str) -> String {
    let mut s = Case::Snake.sanitize(name);
    if is_python_keyword(&s) {
        s.push('_');
    }
    s
}

/// Returns `true` if `s` is a Python 3 keyword.
///
/// Source: `keyword.kwlist` from CPython 3.12. The capitalised keywords
/// (`False`, `True`, `None`) are omitted because snake_case conversion
/// already lower-cases them (`false`, `true`, `none`), which are safe.
fn is_python_keyword(s: &str) -> bool {
    matches!(
        s,
        "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

// ── multi-toolset stub generation ────────────────────────────────────────────

/// Mapping from a resolved Python function name to its callback registry key.
///
/// Used by the session server to remap a `CallbackRegistry`
/// (where keys are `CallbackConfig::id()`, i.e. `"namespace.toolName"`)
/// to a Python-keyed registry (where keys are the resolved `py_name`).
pub struct PythonToolMapping {
    /// The Python function name after collision resolution.
    pub py_name: String,
    /// The callback registry key (`CallbackConfig::id()` = `"namespace.toolName"`).
    pub callback_id: String,
    /// Maps each transformed Python parameter name back to its original JSON Schema
    /// property name.  Empty when all parameter names are unchanged (the common case).
    ///
    /// Used by the session server to reverse-map kwargs before invoking the callback,
    /// so the callback receives the original schema keys regardless of any sanitization
    /// applied to make the names valid Python identifiers.
    pub param_renames: HashMap<String, String>,
}

/// Compute parameter renames for a single tool.
///
/// For each input schema property whose name changes after `sanitize_python_param`,
/// records a `(python_name, original_schema_name)` pair.
fn compute_param_renames(tool: &Tool) -> HashMap<String, String> {
    let mut renames = HashMap::new();
    if let Some(root) = &tool.input_schema {
        let defs = collect_defs(root);
        let schema = Schema::Object(root.schema.clone());
        collect_renames(&schema, &defs, &mut renames);
    }
    renames
}

/// Recursively walk a schema and record any property names that change
/// after `sanitize_python_param`.
fn collect_renames(schema: &Schema, defs: &SchemaDefinitions, out: &mut HashMap<String, String>) {
    match SchemaType::from(schema) {
        SchemaType::Object(obj_st) => {
            for (prop_name, _) in &obj_st.obj.properties {
                let py = sanitize_python_param(prop_name);
                if py != *prop_name {
                    out.insert(py, prop_name.clone());
                }
            }
        }
        SchemaType::Reference(ref_st) => {
            if let Ok(followed) = ref_st.follow(defs) {
                collect_renames(&followed, defs, out);
            }
        }
        _ => {}
    }
}

/// Resolve Python function name mappings for all *callback-variant* tools across
/// multiple toolsets, applying collision resolution across namespaces.
///
/// Returns the mappings that let the session server remap a TypeScript-keyed
/// `CallbackRegistry` (keys `"namespace.toolName"`) to a Python-keyed one
/// (keys = Python function names).
///
/// MCP-variant tools are excluded — they cannot be called from the Python sandbox.
///
/// # Collision handling
///
/// 1. Bare snake_case tool name when unique across all toolsets.
/// 2. `snake_namespace_toolname` when two tools share the same bare name.
/// 3. Suffix `_2`, `_3`, … for any remaining collisions.
pub fn generate_mappings_for_toolsets(toolsets: &[ToolSet]) -> Vec<PythonToolMapping> {
    // Collect callback-variant tools only
    let all: Vec<(&ToolSet, &Tool)> = toolsets
        .iter()
        .flat_map(|ts| {
            ts.tools
                .iter()
                .filter(|t| matches!(t.variant, ToolVariant::Callback))
                .map(move |t| (ts, t))
        })
        .collect();

    // Compute bare candidate names and count occurrences
    let candidates: Vec<String> = all
        .iter()
        .map(|(_, t)| sanitize_python_param(&t.name))
        .collect();

    let mut counts: HashMap<String, usize> = HashMap::new();
    for c in &candidates {
        *counts.entry(c.clone()).or_insert(0) += 1;
    }

    // Resolve final Python names, tracking used names to handle persistent collisions
    let mut used: HashSet<String> = HashSet::new();
    let mut mappings: Vec<PythonToolMapping> = Vec::new();

    for ((ts, tool), candidate) in all.iter().zip(candidates.iter()) {
        let base = if counts[candidate] > 1 {
            // Collision: prefix with snake_case namespace
            sanitize_python_param(&format!("{}_{}", ts.name, tool.name))
        } else {
            candidate.clone()
        };

        // Guard against any remaining persistent collisions
        let mut py_name = base.clone();
        let mut suffix = 2usize;
        while used.contains(&py_name) {
            py_name = format!("{base}_{suffix}");
            suffix += 1;
        }
        used.insert(py_name.clone());

        mappings.push(PythonToolMapping {
            py_name,
            callback_id: format!("{}.{}", ts.name, tool.name),
            param_renames: compute_param_renames(tool),
        });
    }

    mappings
}

/// Generate Python stubs for all *callback-variant* tools across multiple toolsets,
/// resolving name collisions across namespaces.
///
/// Returns `(stubs_string, mappings)` where:
/// - `stubs_string` is ready to pass to `ExecuteOptions::with_stubs()`
/// - `mappings` maps each Python function name back to its `(namespace, tool_name)`
///
/// MCP-variant tools are excluded — they cannot be called from the Python sandbox.
///
/// # Errors
///
/// Returns `CodegenError::TypeGen` on schema traversal failure.
pub fn generate_stubs_for_toolsets(
    toolsets: &[ToolSet],
) -> CodegenResult<(String, Vec<PythonToolMapping>)> {
    let all: Vec<(&ToolSet, &Tool)> = toolsets
        .iter()
        .flat_map(|ts| {
            ts.tools
                .iter()
                .filter(|t| matches!(t.variant, ToolVariant::Callback))
                .map(move |t| (ts, t))
        })
        .collect();

    let mappings = generate_mappings_for_toolsets(toolsets);

    let mut out = "from typing import Any, Literal\n".to_owned();
    for ((_, tool), mapping) in all.iter().zip(mappings.iter()) {
        out.push('\n');
        out.push_str(&tool_stub_named(tool, &mapping.py_name)?);
    }

    Ok((out, mappings))
}

// ── per-tool stub generation ─────────────────────────────────────────────────

/// Generate a `.pyi`-style stub string for a slice of tools.
///
/// The returned string is ready to pass to
/// `pctx_python_runtime::ExecuteOptions::with_stubs()`.
///
/// # Errors
///
/// Returns `CodegenError::TypeGen` if schema traversal fails (e.g. a
/// `$ref` that points to a missing definition).
pub fn generate_stubs(tools: &[Tool]) -> CodegenResult<String> {
    let mut out = "from typing import Any, Literal\n".to_owned();
    for tool in tools {
        out.push('\n');
        out.push_str(&tool_stub(tool)?);
    }
    Ok(out)
}

// ── internals ────────────────────────────────────────────────────────────────

fn tool_stub(tool: &Tool) -> CodegenResult<String> {
    tool_stub_named(tool, &sanitize_python_param(&tool.name))
}

/// Generate a stub for `tool` using `py_name` as the Python function name.
///
/// The name is used as-is (no further sanitization), so the caller is
/// responsible for providing a valid Python identifier.
fn tool_stub_named(tool: &Tool, py_name: &str) -> CodegenResult<String> {
    let params = match &tool.input_schema {
        Some(root) => {
            let defs = collect_defs(root);
            let schema = Schema::Object(root.schema.clone());
            build_params(&schema, &defs)?
        }
        None => String::new(),
    };

    let ret = match &tool.output_schema {
        Some(root) => {
            let defs = collect_defs(root);
            let schema = Schema::Object(root.schema.clone());
            return_annotation(&SchemaType::from(&schema), &defs)?
        }
        None => "Any".to_owned(),
    };

    let mut stub = format!("def {py_name}({params}) -> {ret}:\n");
    if let Some(desc) = &tool.description {
        // Escape any triple-quote sequences that would close the docstring
        let safe = desc.replace("\"\"\"", "\\\"\\\"\\\"");
        stub.push_str(&format!("    \"\"\"{safe}\"\"\"\n"));
    }
    stub.push_str("    ...\n");

    Ok(stub)
}

/// Extract `$defs` / `definitions` from a `RootSchema` into a flat map.
fn collect_defs(root: &schemars::schema::RootSchema) -> SchemaDefinitions {
    root.definitions
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Build the Python parameter list string from an input schema.
///
/// If the top-level schema is an Object, each property becomes an individual
/// keyword argument (required properties first, optional ones after with
/// `= None` defaults).  Any other schema type is passed as a single `input`
/// parameter.
fn build_params(schema: &Schema, defs: &SchemaDefinitions) -> CodegenResult<String> {
    match SchemaType::from(schema) {
        SchemaType::Object(obj_st) => {
            let mut required_params: Vec<String> = vec![];
            let mut optional_params: Vec<String> = vec![];

            for (prop_name, prop_schema) in &obj_st.obj.properties {
                let py_name = sanitize_python_param(prop_name);
                let is_required = obj_st.obj.required.contains(prop_name);
                let annotation = python_annotation(&SchemaType::from(prop_schema), defs)?;

                if is_required {
                    required_params.push(format!("{py_name}: {annotation}"));
                } else {
                    // Avoid doubling `| None` if the type is already nullable.
                    let param_ann = if annotation.ends_with("| None") {
                        annotation
                    } else {
                        format!("{annotation} | None")
                    };
                    optional_params.push(format!("{py_name}: {param_ann} = None"));
                }
            }

            required_params.extend(optional_params);
            Ok(required_params.join(", "))
        }
        SchemaType::Reference(ref_st) => {
            let followed = ref_st.follow(defs)?;
            build_params(&followed, defs)
        }
        _ => {
            // Non-object input: a single `input` parameter
            let annotation = python_annotation(&SchemaType::from(schema), defs)?;
            Ok(format!("input: {annotation}"))
        }
    }
}

/// Choose the return type annotation for an output schema type.
///
/// Named objects always collapse to `dict[str, Any]` since the LLM works
/// with plain Python dicts.  Map types carry through the value annotation.
fn return_annotation(schema_type: &SchemaType, defs: &SchemaDefinitions) -> CodegenResult<String> {
    match schema_type {
        // Both Object and Map surface as dict — Object loses named-field info
        // (acceptable: monty can't instantiate classes anyway).
        SchemaType::Object(_) => Ok("dict[str, Any]".to_owned()),
        SchemaType::Map(_) => python_annotation(schema_type, defs),
        _ => python_annotation(schema_type, defs),
    }
}

/// Map a `SchemaType` to a Python type annotation string.
///
/// Nullable types automatically get `| None` appended.
fn python_annotation(schema_type: &SchemaType, defs: &SchemaDefinitions) -> CodegenResult<String> {
    let base: String = match schema_type {
        SchemaType::Reference(ref_st) => {
            let followed = ref_st.follow(defs)?;
            // Recurse without propagating the outer nullable flag — the
            // followed type carries its own nullability.
            return python_annotation(&SchemaType::from(followed), defs);
        }
        SchemaType::Any(_) => "Any".to_owned(),
        SchemaType::Boolean(_) => "bool".to_owned(),
        SchemaType::Integer(_) => "int".to_owned(),
        SchemaType::Number(_) => "float".to_owned(),
        SchemaType::String(_) => "str".to_owned(),

        SchemaType::Enum(EnumSchemaType { options, .. }) => {
            // serde_json::Value::to_string() formats strings with quotes and
            // numbers without — exactly what Python Literal[] needs.
            let literals = options
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("Literal[{literals}]")
        }

        // Named objects collapse to dict since monty can't instantiate classes.
        SchemaType::Object(_) => "dict[str, Any]".to_owned(),

        SchemaType::Map(map_st) => {
            let val_ann = python_annotation(&SchemaType::from(&map_st.value_schema), defs)?;
            format!("dict[str, {val_ann}]")
        }

        SchemaType::Array(arr_st) => {
            let item_ann = python_annotation(&SchemaType::from(&arr_st.item_schema), defs)?;
            format!("list[{item_ann}]")
        }

        SchemaType::Union(union_st) => {
            let parts: CodegenResult<Vec<String>> = union_st
                .union_schemas
                .iter()
                .map(|s| python_annotation(&SchemaType::from(s), defs))
                .collect();
            parts?.join(" | ")
        }
    };

    Ok(if schema_type.is_nullable() {
        format!("{base} | None")
    } else {
        base
    })
}
