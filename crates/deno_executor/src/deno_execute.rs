use deno_runtime::deno_core;
use deno_runtime::deno_core::JsRuntime;
use deno_runtime::deno_core::ModuleCodeString;
use deno_runtime::deno_core::RuntimeOptions;
use deno_runtime::deno_core::error::AnyError;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

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

/// Execute TypeScript/JavaScript code with `pctx_runtime`
///
/// This function executes code in an isolated Deno runtime with MCP client functionality built-in.
/// The runtime is loaded from a pre-compiled snapshot for faster startup.
///
/// # Arguments
/// * `code` - The TypeScript/JavaScript code to execute
/// * `allowed_hosts` - Optional list of hosts that network requests are allowed to access
///
/// # Returns
/// * `Ok(ExecuteResult)` - Contains execution result or error information
///
/// # Errors
/// * Returns error only if internal Deno runtime initialization fails
pub async fn execute_code(
    code: &str,
    allowed_hosts: Option<Vec<String>>,
) -> Result<ExecuteResult, AnyError> {
    // Transpile TypeScript to JavaScript
    let js_code = match deno_transpiler::transpile(code, None) {
        Ok(js) => js,
        Err(e) => {
            return Ok(ExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: format!("Transpilation failed: {e}"),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
    };

    // Create MCP registry and allowed hosts for this execution
    let mcp_registry = pctx_code_execution_runtime::MCPRegistry::new();
    let allowed_hosts = pctx_code_execution_runtime::AllowedHosts::new(allowed_hosts);

    // Create JsRuntime with `pctx_runtime` snapshot and extension
    // The snapshot contains the ESM code pre-compiled, and init() registers both ops and ESM
    // Deno handles the deduplication when loading from snapshot
    let mut js_runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
        startup_snapshot: Some(pctx_code_execution_runtime::RUNTIME_SNAPSHOT),
        extensions: vec![pctx_code_execution_runtime::pctx_runtime_snapshot::init(
            mcp_registry,
            allowed_hosts,
        )],
        ..Default::default()
    });

    // Create the main module specifier
    let main_module = deno_core::resolve_url("file:///execute.js")?;

    // Load and evaluate the transpiled code as a module
    let mod_id = match js_runtime
        .load_side_es_module_from_code(&main_module, ModuleCodeString::from(js_code))
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
    let eval_future = js_runtime.mod_evaluate(mod_id);

    // Run the event loop to completion
    let event_loop_future = js_runtime.run_event_loop(deno_core::PollEventLoopOptions {
        wait_for_inspector: false,
        pump_v8_message_loop: true,
    });

    // Drive both futures together - wait for BOTH to complete
    let (eval_result, event_loop_result) = futures::join!(eval_future, event_loop_future);

    // Check for errors from either future
    let (success, error) = match (eval_result, event_loop_result) {
        (Ok(()), Ok(())) => (true, None),
        (Err(e), _) | (_, Err(e)) => (
            false,
            Some(ExecutionError {
                message: e.to_string(),
                stack: None,
            }),
        ),
    };

    // Get console output (even if there was an error)
    let capture_script = r"
        ({
            stdout: globalThis.__stdout || [],
            stderr: globalThis.__stderr || []
        })
    ";

    // Execute the capture script to get the console output
    let console_global = js_runtime
        .execute_script("<capture_output>", capture_script)
        .ok();

    // Get module namespace
    let module_namespace = if success {
        js_runtime.get_module_namespace(mod_id).ok()
    } else {
        None
    };

    // Extract console output and module exports using scope
    let (stdout, stderr, output) = {
        deno_core::scope!(scope, &mut js_runtime);

        let console_output = console_global.and_then(|global| {
            let local = deno_core::v8::Local::new(scope, global);
            deno_core::serde_v8::from_v8::<serde_json::Value>(scope, local).ok()
        });

        let stdout_str = console_output
            .as_ref()
            .and_then(|v| v["stdout"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let stderr_str = console_output
            .as_ref()
            .and_then(|v| v["stderr"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        // Extract default export from module namespace
        let output = module_namespace.and_then(|module_namespace| {
            let namespace = deno_core::v8::Local::new(scope, module_namespace);
            let default_key = deno_core::v8::String::new(scope, "default")?;

            namespace
                .get(scope, default_key.into())
                .and_then(|default_value| {
                    // Skip undefined (no default export)
                    if default_value.is_undefined() {
                        return None;
                    }

                    // Handle Promise
                    if default_value.is_promise() {
                        let promise = default_value.cast::<deno_core::v8::Promise>();
                        if promise.state() == deno_core::v8::PromiseState::Fulfilled {
                            let result = promise.result(scope);
                            return deno_core::serde_v8::from_v8(scope, result).ok();
                        }
                        return None;
                    }

                    deno_core::serde_v8::from_v8(scope, default_value).ok()
                })
        });

        (stdout_str, stderr_str, output)
    };

    Ok(ExecuteResult {
        success,
        output,
        error,
        stdout,
        stderr,
    })
}
