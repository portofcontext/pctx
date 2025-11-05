use deno_core::{JsRuntime, PollEventLoopOptions, RuntimeOptions, error::AnyError};
use serde::{Deserialize, Serialize};
use std::pin::pin;

// Embed the Zod library at compile time
const ZOD_SOURCE: &str = include_str!("../js/zod.min.mjs");

/// Transpile TypeScript code to JavaScript
fn transpile_typescript(code: &str) -> Result<String, AnyError> {
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: deno_ast::ModuleSpecifier::parse("file:///execute.ts")?,
        text: code.into(),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })?;

    let transpiled = parsed.transpile(
        &deno_ast::TranspileOptions {
            imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
            ..Default::default()
        },
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions {
            source_map: deno_ast::SourceMapOption::None,
            inline_sources: false,
            ..Default::default()
        },
    )?;

    Ok(transpiled.into_source().text)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub message: String,
    pub stack: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<ExecutionError>,
    pub stdout: String,
    pub stderr: String,
}

/// Execute TypeScript/JavaScript code with Deno runtime
///
/// This function executes code in an isolated Deno runtime with Zod pre-loaded.
/// The runtime supports:
/// - Zod validation library available as: `import { z } from "zod"`
/// - ES modules and dynamic imports
/// - Full TypeScript support
///
/// # Arguments
/// * `code` - The TypeScript/JavaScript code to execute
///
/// # Returns
/// * `Ok(ExecuteResult)` - Contains execution result or error information
///
/// # Errors
/// * Returns error if runtime initialization fails
///
/// # Examples
/// ```no_run
/// use sdk_runner::execute;
///
/// # async fn example() {
/// let code = r#"
///     import { z } from "zod";
///     const schema = z.object({ name: z.string() });
///     const result = schema.parse({ name: "test" });
///     result
/// "#;
/// let result = execute(code).await.expect("execution should not fail");
/// assert!(result.success);
/// # }
/// ```
pub async fn execute_code(code: &str) -> Result<ExecuteResult, AnyError> {
    // Create a custom module loader that provides Zod
    struct ZodModuleLoader;

    impl deno_core::ModuleLoader for ZodModuleLoader {
        fn resolve(
            &self,
            specifier: &str,
            referrer: &str,
            _kind: deno_core::ResolutionKind,
        ) -> Result<deno_core::ModuleSpecifier, deno_core::error::ModuleLoaderError> {
            if specifier == "zod" {
                return deno_core::resolve_url("internal:zod")
                    .map_err(|e| deno_core::error::ModuleLoaderError::generic(e.to_string()));
            }
            deno_core::resolve_import(specifier, referrer)
                .map_err(|e| deno_core::error::ModuleLoaderError::generic(e.to_string()))
        }

        fn load(
            &self,
            module_specifier: &deno_core::ModuleSpecifier,
            _maybe_referrer: Option<&deno_core::ModuleLoadReferrer>,
            _load_options: deno_core::ModuleLoadOptions,
        ) -> deno_core::ModuleLoadResponse {
            let specifier_str = module_specifier.as_str();

            if specifier_str == "internal:zod" {
                let module_source = deno_core::ModuleSource::new(
                    deno_core::ModuleType::JavaScript,
                    deno_core::ModuleSourceCode::String(ZOD_SOURCE.to_string().into()),
                    module_specifier,
                    None,
                );
                return deno_core::ModuleLoadResponse::Sync(Ok(module_source));
            }

            let error = deno_core::error::ModuleLoaderError::generic(format!(
                "Module not found: {specifier_str}"
            ));
            deno_core::ModuleLoadResponse::Sync(Err(error))
        }
    }

    // Create a new Deno runtime with custom module loader
    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(std::rc::Rc::new(ZodModuleLoader)),
        ..Default::default()
    });

    // Inject console capture code
    let console_setup = r"
        globalThis.__stdout = [];
        globalThis.__stderr = [];

        const originalLog = console.log;
        const originalError = console.error;
        const originalWarn = console.warn;
        const originalInfo = console.info;

        console.log = (...args) => {
            const msg = args.map(arg => {
                if (typeof arg === 'object') {
                    try { return JSON.stringify(arg); }
                    catch { return String(arg); }
                }
                return String(arg);
            }).join(' ');
            globalThis.__stdout.push(msg);
        };

        console.error = (...args) => {
            const msg = args.map(arg => {
                if (typeof arg === 'object') {
                    try { return JSON.stringify(arg); }
                    catch { return String(arg); }
                }
                return String(arg);
            }).join(' ');
            globalThis.__stderr.push(msg);
        };

        console.warn = console.error;
        console.info = console.log;
        "
    .to_string();

    runtime.execute_script("<console_setup>", console_setup)?;

    // Transpile TypeScript to JavaScript
    let transpiled_code = match transpile_typescript(code) {
        Ok(js_code) => js_code,
        Err(e) => {
            return Ok(ExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: format!("TypeScript transpilation error: {e}"),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
    };

    // Load and execute the code as a module
    let module_specifier = deno_core::resolve_url("file:///execute.ts")?;

    let mod_id = match runtime
        .load_main_es_module_from_code(&module_specifier, transpiled_code)
        .await
    {
        Ok(id) => id,
        Err(e) => {
            return Ok(ExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: e.to_string(),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
    };

    // Evaluate the module
    let eval_result = runtime.mod_evaluate(mod_id);

    // Run the event loop to completion
    match runtime
        .run_event_loop(PollEventLoopOptions::default())
        .await
    {
        Ok(()) => {}
        Err(e) => {
            return Ok(ExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: e.to_string(),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
    }

    // Check evaluation result and get initial success/error state
    let (success, error) = match eval_result.await {
        Ok(()) => (true, None),
        Err(e) => {
            let error_string = e.to_string();
            (
                false,
                Some(ExecutionError {
                    message: error_string.clone(),
                    stack: Some(error_string),
                }),
            )
        }
    };

    // Get v8 globals before creating the handle scope
    let capture_script = r"
        ({
            stdout: globalThis.__stdout || [],
            stderr: globalThis.__stderr || []
        })
    ";
    let console_output_global = runtime
        .execute_script("<capture_output>", capture_script.to_string())
        .ok();
    let module_namespace_global = if success {
        runtime.get_module_namespace(mod_id).ok()
    } else {
        None
    };

    // Now create handle scope and extract values from the globals
    let main_context = runtime.main_context();
    let handle_scope_storage = pin!(deno_core::v8::HandleScope::new(runtime.v8_isolate()));
    let handle_scope = &mut handle_scope_storage.init();
    let context = deno_core::v8::Local::new(handle_scope, main_context);
    let context_scope = &mut deno_core::v8::ContextScope::new(handle_scope, context);

    // Extract console output
    let (stdout, stderr) = if let Some(captured) = console_output_global {
        let local = deno_core::v8::Local::new(context_scope, captured);

        if let Ok(output_obj) =
            deno_core::serde_v8::from_v8::<serde_json::Value>(context_scope, local)
        {
            let stdout_lines = output_obj["stdout"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();

            let stderr_lines = output_obj["stderr"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();

            (stdout_lines, stderr_lines)
        } else {
            (String::new(), String::new())
        }
    } else {
        (String::new(), String::new())
    };

    // Extract default export value
    let output = module_namespace_global.and_then(|module_namespace| {
        let namespace = deno_core::v8::Local::new(context_scope, module_namespace);
        let default_key = deno_core::v8::String::new(context_scope, "default")?;

        namespace
            .get(context_scope, default_key.into())
            .and_then(|default_value| {
                // Check if the value is undefined (no explicit default export)
                if default_value.is_undefined() {
                    return None;
                }

                deno_core::serde_v8::from_v8::<serde_json::Value>(context_scope, default_value).ok()
            })
    });

    Ok(ExecuteResult {
        success,
        output,
        error,
        stdout,
        stderr,
    })
}
