use deno_core::JsRuntime;
use deno_core::ModuleCodeString;
use deno_core::RuntimeOptions;
use deno_core::anyhow;
use deno_core::error::CoreError;
use futures::lock::Mutex;
use pctx_registry::PctxRegistry;
pub use pctx_type_check_runtime::{CheckResult, Diagnostic, is_relevant_error};
use pctx_type_check_runtime::{init_v8_platform, type_check};
use serde::{Deserialize, Serialize};
use std::{rc::Rc, time::SystemTime};
use thiserror::Error;
use tracing::{debug, warn};

pub mod events;

/// Process-wide mutex to serialize all V8 isolate creation and usage.
///
/// V8 isolates share platform-level state (code pages, thread pool, etc.) that is not
/// safe to access concurrently from multiple OS threads. All code that creates or uses
/// a `JsRuntime` must hold this lock for the runtime's entire lifetime.
///
/// This mutex is acquired by `execute()` and held for both type checking and code execution.
static V8_MUTEX: std::sync::LazyLock<Mutex<()>> = std::sync::LazyLock::new(|| {
    init_v8_platform();
    Mutex::new(())
});

pub type Result<T> = std::result::Result<T, DenoExecutorError>;

#[derive(Clone, Default)]
pub struct ExecuteOptions {
    pub registry: PctxRegistry,
}

impl std::fmt::Debug for ExecuteOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecuteOptions")
            .field("registry", &self.registry.ids())
            .finish()
    }
}

impl ExecuteOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the unified local registry
    ///
    /// This registry contains all local tool callbacks, and mcp server actions
    #[must_use]
    pub fn with_registry(mut self, registry: PctxRegistry) -> Self {
        self.registry = registry;
        self
    }
}

#[derive(Clone, Debug)]
pub struct ExecuteResult {
    pub success: bool,

    /// Type checking diagnostics (if any)
    pub diagnostics: Vec<Diagnostic>,

    /// Runtime error information (if execution failed)
    pub runtime_error: Option<ExecutionError>,

    /// The default export value from the module (if any)
    pub output: Option<serde_json::Value>,

    /// Standard output from execution
    pub stdout: String,

    /// Standard error from execution
    pub stderr: String,

    pub registry: PctxRegistry,

    /// Full trace of this execution flow.
    pub trace: events::ExecutionTrace,
}

#[derive(Debug, Error)]
pub enum DenoExecutorError {
    #[error("Internal check error: {0}")]
    InternalError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Type check error: {0}")]
    TypeCheckError(#[from] pctx_type_check_runtime::TypeCheckError),
}

/// Execute TypeScript code with type checking and runtime execution
///
/// This function combines type checking and execution:
/// 1. First runs TypeScript type checking via `run_type_check()`
/// 2. If type checking passes, executes transpiled code with Deno runtime
/// 3. Returns unified result with diagnostics and runtime output
///
/// # Arguments
/// * `code` - The TypeScript code to check and execute
/// * `options` - Execution options (registry with local tools and MCP server actions)
///
/// # Returns
/// * `Ok(ExecuteResult)` - Contains type diagnostics, runtime errors, stdout/stderr, and default export value
///
/// # Errors
/// * Returns `Err` only if internal tooling fails (e.g. type check infrastructure); type errors
///   and runtime errors are captured in the returned `ExecuteResult`
///
/// # Example
/// ```rust,no_run
/// use pctx_executor::{execute, ExecuteOptions};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let result = execute("const x = 1 + 1; export default x;", ExecuteOptions::new()).await?;
/// assert_eq!(result.output, Some(serde_json::json!(2)));
/// # Ok(())
/// # }
/// ```
pub async fn execute(code: &str, options: ExecuteOptions) -> Result<ExecuteResult> {
    debug!(
        code_length = code.len(),
        "Code submitted for typecheck & execution"
    );

    let started_at = SystemTime::now();

    // Acquire V8 mutex for the entire operation (type check + execution)
    // This ensures no concurrent V8 isolate usage across the process
    let _guard = V8_MUTEX.lock().await;

    let typecheck_started_at = SystemTime::now();
    let check_result = run_type_check(code)?;
    let typecheck_ended_at = SystemTime::now();

    // Check if we have diagnostics
    if !check_result.diagnostics.is_empty() {
        // Format diagnostics as rich stderr output
        let stderr = format_diagnostics(&check_result.diagnostics);

        warn!(
            runtime = "type_check",
            diagnostic = %stderr,
            diagnostic_count = check_result.diagnostics.len(),
            "Type check failed with diagnostics"
        );

        let events = vec![events::ExecutionEvent::TypeCheck(events::TypeCheckEvent {
            started_at: typecheck_started_at,
            ended_at: typecheck_ended_at,
            outcome: events::TypecheckOutcome::Failed {
                diagnostics: check_result.diagnostics.clone(),
            },
        })];

        return Ok(ExecuteResult {
            success: false,
            diagnostics: check_result.diagnostics,
            runtime_error: None,
            output: None,
            stdout: String::new(),
            stderr,
            trace: events::ExecutionTrace {
                code: code.to_string(),
                started_at,
                ended_at: SystemTime::now(),
                events,
            },
            registry: options.registry,
        });
    }

    debug!(runtime = "type_check", "Type check passed");

    let exec_result = execute_code(code, options)
        .await
        .map_err(|e| DenoExecutorError::InternalError(e.to_string()))?;

    let stderr = if let Some(ref err) = exec_result.error {
        err.message.clone()
    } else {
        String::new()
    };

    let ended_at = SystemTime::now();

    let mut events = vec![events::ExecutionEvent::TypeCheck(events::TypeCheckEvent {
        started_at: typecheck_started_at,
        ended_at: typecheck_ended_at,
        outcome: events::TypecheckOutcome::Passed,
    })];
    for registry_event in exec_result.registry.trace().events() {
        events.push(match registry_event {
            events::RegistryEvent::McpToolCall(e) => events::ExecutionEvent::McpToolCall(e),
            events::RegistryEvent::CallbackInvocation(e) => {
                events::ExecutionEvent::CallbackInvocation(e)
            }
        });
    }
    events.sort_by_key(|e| e.started_at());

    Ok(ExecuteResult {
        success: exec_result.success,
        diagnostics: Vec::new(), // No type-check diagnostics if we reach execution
        runtime_error: exec_result.error,
        output: exec_result.output,
        stdout: exec_result.stdout,
        stderr: if exec_result.stderr.is_empty() {
            stderr
        } else {
            exec_result.stderr
        },
        trace: events::ExecutionTrace {
            code: code.to_string(),
            started_at,
            ended_at,
            events,
        },
        registry: exec_result.registry,
    })
}

#[tracing::instrument(skip(code), fields(runtime = "type_check", code_length = code.len()))]
fn run_type_check(code: &str) -> Result<CheckResult> {
    let mut check_result = type_check(code)?;

    if !check_result.success && !check_result.diagnostics.is_empty() {
        // filter for only relevant diagnostics
        check_result.diagnostics = check_result
            .diagnostics
            .into_iter()
            .filter(is_relevant_error)
            .collect();
    }

    Ok(check_result)
}

/// Format diagnostics as rich stderr output with line numbers, columns, and error codes
fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| {
            let mut parts = Vec::new();

            // Add position if available
            if let Some(line) = d.line {
                if let Some(col) = d.column {
                    parts.push(format!("Line {line}, Column {col}"));
                } else {
                    parts.push(format!("Line {line}"));
                }
            }

            if let Some(code) = d.code {
                parts.push(format!("TS{code}"));
            }

            // Clean the message by removing internal file paths
            // The type checker uses "file:///check.ts" as an internal detail
            let cleaned_message = d
                .message
                .replace("file:///check.ts:", "")
                .trim()
                .to_string();

            // Build the formatted error
            if parts.is_empty() {
                cleaned_message
            } else {
                format!("{}: {}", parts.join(", "), cleaned_message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub message: String,
    pub stack: Option<String>,
}

/// Internal execution result used by `execute_code`
#[derive(Debug, Clone)]
struct InternalExecuteResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<ExecutionError>,
    pub stdout: String,
    pub stderr: String,
    pub registry: PctxRegistry,
}

/// Execute TypeScript/JavaScript code with `pctx_runtime`
///
/// This function executes code in an isolated Deno runtime with MCP client functionality built-in.
/// The runtime is loaded from a pre-compiled snapshot for faster startup.
///
/// # Arguments
/// * `code` - The TypeScript/JavaScript code to execute
/// * `allowed_hosts` - Optional list of hosts that network requests are allowed to access
/// * `mcp_configs` - Optional list of MCP server configurations to pre-register
/// * `local_tools` - Optional list of local tool definitions to pre-register
///
/// # Returns
/// * `Ok(ExecuteResult)` - Contains execution result or error information
///
/// # Errors
/// * Returns error only if internal Deno runtime initialization fails
#[tracing::instrument(
    skip(code, options),
    fields(runtime = "execution", code_length = code.len())
)]
async fn execute_code(
    code: &str,
    options: ExecuteOptions,
) -> anyhow::Result<InternalExecuteResult> {
    debug!("Starting code execution");

    // Note: V8_MUTEX is held by the caller (execute()) for the entire operation

    // Transpile TypeScript to JavaScript
    let js_code = match pctx_deno_transpiler::transpile(code, None) {
        Ok(js) => {
            debug!(
                runtime = "execution",
                transpiled_code_length = js.len(),
                "Code transpiled successfully"
            );
            js
        }
        Err(e) => {
            warn!(runtime = "execution", error = %e, "Transpilation failed");
            return Ok(InternalExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: format!("Transpilation failed: {e}"),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
                registry: options.registry,
            });
        }
    };

    // Build extensions list
    let extensions = vec![pctx_code_execution_runtime::pctx_runtime_snapshot::init(
        options.registry.clone(),
    )];

    // Create JsRuntime from `pctx_runtime` snapshot and extension
    // The snapshot contains the ESM code pre-compiled, and init() registers both ops and ESM
    // Deno handles the deduplication when loading from snapshot
    let mut js_runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
        startup_snapshot: Some(pctx_code_execution_runtime::RUNTIME_SNAPSHOT),
        extensions,
        ..Default::default()
    });

    // Create the main module specifier
    let main_module = deno_core::resolve_url("file:///execute.js")?;

    // Load and evaluate the transpiled code as a module
    debug!(main_module =? main_module, "Loading module into runtime");
    let mod_id = match js_runtime
        .load_side_es_module_from_code(&main_module, ModuleCodeString::from(js_code))
        .await
    {
        Ok(id) => {
            debug!(module_id = id, "Module loaded successfully");
            id
        }
        Err(e) => {
            warn!(error = %e, "Failed to load module");
            return Ok(InternalExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: e.to_string(),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
                registry: options.registry,
            });
        }
    };

    // Evaluate the module
    debug!("Evaluating module");
    let eval_future = js_runtime.mod_evaluate(mod_id);

    // Run the event loop to completion
    debug!("Running event loop");
    let event_loop_future = js_runtime.run_event_loop(deno_core::PollEventLoopOptions {
        wait_for_inspector: false,
        pump_v8_message_loop: true,
    });

    // Drive both futures together - wait for BOTH to complete
    let (eval_result, event_loop_result) = futures::join!(eval_future, event_loop_future);
    debug!("Eval and event loop futures resolved");

    process_execution_results(
        &mut js_runtime,
        mod_id,
        eval_result.err(),
        event_loop_result.err(),
        options.registry,
    )
}

#[tracing::instrument(skip_all)]
fn process_execution_results(
    js_runtime: &mut JsRuntime,
    mod_id: usize,
    eval_err: Option<CoreError>,
    event_loop_err: Option<CoreError>,
    registry: PctxRegistry,
) -> anyhow::Result<InternalExecuteResult> {
    // Check for errors from either future
    let (success, error) = match (eval_err, event_loop_err) {
        (None, None) => {
            debug!("Code executed successfully");
            (true, None)
        }
        (Some(e), _) | (_, Some(e)) => {
            warn!( error = %e, "Code execution failed");
            (
                false,
                Some(ExecutionError {
                    message: e.to_string(),
                    stack: None,
                }),
            )
        }
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
    deno_core::scope!(scope, js_runtime);

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
    let output: Option<serde_json::Value> = module_namespace.and_then(|module_namespace| {
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

    Ok(InternalExecuteResult {
        success,
        output,
        error,
        stdout: stdout_str,
        stderr: stderr_str,
        registry,
    })
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
