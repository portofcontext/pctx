use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_core::futures::FutureExt;
use deno_runtime::deno_core::{ModuleCodeString, error::AnyError, extension};
use deno_runtime::deno_fetch::dns::Resolver;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::{Permissions, PermissionsContainer, PermissionsOptions};
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use serde::{Deserialize, Serialize};
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use sys_traits::impls::RealSys;

// Embed the mcp-client library at compile time
const MCP_CLIENT_SOURCE: &str = include_str!("../js/mcp-client.min.mjs");

// Setup extension that imports all the web extension modules we need
extension!(
    setup_web_apis,
    esm_entry_point = "ext:setup_web_apis/setup.js",
    esm = [dir "js", "setup.js"],
);

// Custom module loader that provides mcp-client from memory
struct McpClientModuleLoader;

impl deno_runtime::deno_core::ModuleLoader for McpClientModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_runtime::deno_core::ResolutionKind,
    ) -> Result<
        deno_runtime::deno_core::ModuleSpecifier,
        deno_runtime::deno_core::error::ModuleLoaderError,
    > {
        if specifier == "mcp-client" {
            return deno_runtime::deno_core::resolve_url("internal:mcp-client").map_err(|e| {
                deno_runtime::deno_core::error::ModuleLoaderError::generic(e.to_string())
            });
        }
        deno_runtime::deno_core::resolve_import(specifier, referrer)
            .map_err(|e| deno_runtime::deno_core::error::ModuleLoaderError::generic(e.to_string()))
    }

    fn load(
        &self,
        module_specifier: &deno_runtime::deno_core::ModuleSpecifier,
        _maybe_referrer: Option<&deno_runtime::deno_core::ModuleLoadReferrer>,
        _is_dyn_import: bool,
        _requested_module_type: deno_runtime::deno_core::RequestedModuleType,
    ) -> deno_runtime::deno_core::ModuleLoadResponse {
        let specifier_str = module_specifier.as_str();

        if specifier_str == "internal:mcp-client" {
            let module_source = deno_runtime::deno_core::ModuleSource::new(
                deno_runtime::deno_core::ModuleType::JavaScript,
                deno_runtime::deno_core::ModuleSourceCode::String(ModuleCodeString::from(
                    String::from(MCP_CLIENT_SOURCE),
                )),
                module_specifier,
                None,
            );
            return deno_runtime::deno_core::ModuleLoadResponse::Sync(Ok(module_source));
        }

        let error = deno_runtime::deno_core::error::ModuleLoaderError::generic(format!(
            "Module not found: {specifier_str}"
        ));
        deno_runtime::deno_core::ModuleLoadResponse::Sync(Err(error))
    }

    fn import_meta_resolve(
        &self,
        specifier: &str,
        referrer: &str,
    ) -> Result<
        deno_runtime::deno_core::ModuleSpecifier,
        deno_runtime::deno_core::error::ModuleLoaderError,
    > {
        self.resolve(
            specifier,
            referrer,
            deno_runtime::deno_core::ResolutionKind::DynamicImport,
        )
    }

    fn prepare_load(
        &self,
        _module_specifier: &deno_runtime::deno_core::ModuleSpecifier,
        _maybe_referrer: Option<String>,
        _is_dyn_import: bool,
        _requested_module_type: deno_runtime::deno_core::RequestedModuleType,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<(), deno_runtime::deno_core::error::ModuleLoaderError>>>,
    > {
        async { Ok(()) }.boxed_local()
    }

    fn finish_load(&self) {}

    fn code_cache_ready(
        &self,
        _module_specifier: deno_runtime::deno_core::ModuleSpecifier,
        _hash: u64,
        _code_cache: &[u8],
    ) -> std::pin::Pin<Box<dyn Future<Output = ()>>> {
        async {}.boxed_local()
    }

    fn purge_and_prevent_code_cache(&self, _module_specifier: &str) {}

    fn get_source_map(&self, _file_name: &str) -> Option<std::borrow::Cow<'_, [u8]>> {
        None
    }

    fn get_source_mapped_source_line(
        &self,
        _file_name: &str,
        _line_number: usize,
    ) -> Option<String> {
        None
    }

    fn get_host_defined_options<'s>(
        &self,
        _scope: &mut deno_runtime::deno_napi::v8::PinScope<'s, '_>,
        _name: &str,
    ) -> Option<deno_runtime::deno_napi::v8::Local<'s, deno_runtime::deno_napi::v8::Data>> {
        None
    }
}

// Dummy types for npm resolution (not needed for our use case)
#[derive(Clone)]
struct DummyNpmChecker;

impl node_resolver::InNpmPackageChecker for DummyNpmChecker {
    fn in_npm_package(&self, _specifier: &deno_runtime::deno_core::ModuleSpecifier) -> bool {
        false
    }
}

#[derive(Clone)]
struct DummyNpmResolver;

impl node_resolver::NpmPackageFolderResolver for DummyNpmResolver {
    fn resolve_package_folder_from_package(
        &self,
        name: &str,
        referrer: &node_resolver::UrlOrPathRef<'_>,
    ) -> Result<std::path::PathBuf, node_resolver::errors::PackageFolderResolveError> {
        use node_resolver::UrlOrPath;
        use node_resolver::errors::{
            PackageFolderResolveError, PackageFolderResolveErrorKind, PackageFolderResolveIoError,
        };

        // Convert UrlOrPathRef to UrlOrPath using the accessor methods
        let referrer_owned = if let Ok(url) = referrer.url() {
            UrlOrPath::Url(url.clone())
        } else if let Ok(path) = referrer.path() {
            UrlOrPath::Path(path.to_path_buf())
        } else {
            // Fallback - try to create a dummy path
            UrlOrPath::Path(std::path::PathBuf::from("unknown"))
        };

        Err(PackageFolderResolveError(Box::new(
            PackageFolderResolveErrorKind::Io(PackageFolderResolveIoError {
                package_name: name.to_string(),
                referrer: referrer_owned,
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "npm packages not supported",
                ),
            }),
        )))
    }
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
/// This function executes code in an isolated Deno runtime with mcp-client pre-loaded.
/// The runtime supports:
/// - mcp-client library available as: `import { Client } from "mcp-client"`
/// - ES modules and dynamic imports
/// - Full web APIs (fetch, console, URL, etc.)
/// - Full TypeScript support (code is automatically transpiled)
///
/// # Arguments
/// * `code` - The TypeScript/JavaScript code to execute
/// * `allowed_hosts` - Optional list of hosts that network requests are allowed to access.
///   Format: "hostname:port" or just "hostname" (e.g., "localhost:3000", "api.example.com").
///   If None or empty, all network access is denied.
///
/// # Returns
/// * `Ok(ExecuteResult)` - Contains execution result or error information
///
/// # Errors
/// * Returns error if runtime initialization fails or transpilation fails
///
/// # Examples
/// ```no_run
/// use deno_executor::execute_code;
///
/// # async fn example() {
/// let code = r#"
///     import { registerMCP, callMCPTool } from "mcp-client";
///     const name: string = "test";
///     registerMCP({ name, url: "http://localhost:3000" });
///     const result = await callMCPTool({ name, tool: "echo", arguments: { message: "hello" } });
///     export default result;
/// "#;
/// let allowed_hosts = Some(vec!["localhost:3000".to_string()]);
/// let result = execute_code(code, allowed_hosts).await.expect("execution should not fail");
/// assert!(result.success);
/// # }
/// ```
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

    // Create the main module specifier
    let main_module = deno_runtime::deno_core::resolve_url("file:///execute.js")?;

    // Create permission descriptor parser using RealSys
    let sys = RealSys;
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(sys.clone()));

    // Create permissions with restricted network access
    let permissions_options = PermissionsOptions {
        allow_net: allowed_hosts,
        deny_net: None,
        allow_env: None,
        deny_env: None,
        allow_run: None,
        deny_run: None,
        allow_read: None,
        deny_read: None,
        allow_write: None,
        deny_write: None,
        allow_ffi: None,
        deny_ffi: None,
        allow_sys: None,
        deny_sys: None,
        allow_import: None,
        deny_import: None,
        prompt: false,
    };

    let permissions =
        Permissions::from_options(permission_desc_parser.as_ref(), &permissions_options)?;
    let permissions = PermissionsContainer::new(permission_desc_parser, permissions);

    // Create the MainWorker with required services
    let mut worker = MainWorker::bootstrap_from_options::<DummyNpmChecker, DummyNpmResolver, RealSys>(
        &main_module,
        WorkerServiceOptions {
            deno_rt_native_addon_loader: None,
            module_loader: Rc::new(McpClientModuleLoader),
            permissions,
            blob_store: Arc::default(),
            broadcast_channel: InMemoryBroadcastChannel::default(),
            shared_array_buffer_store: Option::default(),
            compiled_wasm_module_store: Option::default(),
            feature_checker: Arc::default(),
            node_services: Option::default(),
            npm_process_state_provider: Option::default(),
            root_cert_store_provider: Option::default(),
            fetch_dns_resolver: Resolver::default(),
            v8_code_cache: Option::default(),
            fs: Arc::new(RealFs),
            bundle_provider: None,
        },
        WorkerOptions {
            extensions: vec![setup_web_apis::init()],
            startup_snapshot: None,
            ..Default::default()
        },
    );

    // Load the transpiled JavaScript code as a side module
    let mod_id = match worker
        .js_runtime
        .load_side_es_module_from_code(&main_module, js_code)
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
    let eval_result = worker.js_runtime.mod_evaluate(mod_id);

    // Drive the module evaluation and event loop together using tokio::select!
    // Both futures need to run concurrently:
    // - mod_evaluate handles module execution and top-level await
    // - run_event_loop processes async operations (fetch, timers, etc.)
    //
    // Note: There is a known race condition where if both futures complete simultaneously,
    // tokio::select! may choose the event loop result even if mod_evaluate has an error.
    // In practice this rarely occurs because:
    // 1. Runtime errors cause mod_evaluate to fail immediately
    // 2. Successful execution with async work takes time for the event loop to complete
    // 3. The use case for pctx (server with `export default await run()`) works correctly
    let eval_with_event_loop = async {
        tokio::select! {
            eval_res = eval_result => eval_res,
            loop_res = worker.run_event_loop(false) => loop_res
        }
    };

    let eval_final_result =
        tokio::time::timeout(std::time::Duration::from_secs(10), eval_with_event_loop).await;

    let (success, error) = match eval_final_result {
        Ok(Ok(())) => (true, None),
        Ok(Err(e)) => (
            false,
            Some(ExecutionError {
                message: e.to_string(),
                stack: None,
            }),
        ),
        Err(_) => {
            return Ok(ExecuteResult {
                success: false,
                output: None,
                error: Some(ExecutionError {
                    message: "Execution timed out after 10 seconds".to_string(),
                    stack: None,
                }),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
    };

    // success and error are already set above

    // Get v8 globals before creating the handle scope
    let capture_script = r"
        ({
            stdout: globalThis.__stdout || [],
            stderr: globalThis.__stderr || []
        })
    ";
    let console_output_global = worker
        .js_runtime
        .execute_script("<capture_output>", capture_script.to_string())
        .ok();
    let module_namespace_global = if success {
        worker.js_runtime.get_module_namespace(mod_id).ok()
    } else {
        None
    };

    // Now create handle scope and extract values from the globals
    let main_context = worker.js_runtime.main_context();
    let handle_scope_storage = pin!(deno_runtime::deno_core::v8::HandleScope::new(
        worker.js_runtime.v8_isolate()
    ));
    let handle_scope = &mut handle_scope_storage.init();
    let context = deno_runtime::deno_core::v8::Local::new(handle_scope, main_context);
    let context_scope = &mut deno_runtime::deno_core::v8::ContextScope::new(handle_scope, context);

    // Extract console output
    let (stdout, stderr) = if let Some(captured) = console_output_global {
        let local = deno_runtime::deno_core::v8::Local::new(context_scope, captured);

        if let Ok(output_obj) =
            deno_runtime::deno_core::serde_v8::from_v8::<serde_json::Value>(context_scope, local)
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

    // Extract default export value - if it's a Promise, resolve it
    let output = module_namespace_global.and_then(|module_namespace| {
        let namespace = deno_runtime::deno_core::v8::Local::new(context_scope, module_namespace);
        let default_key = deno_runtime::deno_core::v8::String::new(context_scope, "default")?;

        namespace
            .get(context_scope, default_key.into())
            .and_then(|default_value| {
                // Check if the value is undefined (no explicit default export)
                if default_value.is_undefined() {
                    return None;
                }

                // Check if it's a Promise - extract the resolved value
                if default_value.is_promise() {
                    let promise = default_value.cast::<deno_runtime::deno_core::v8::Promise>();
                    if promise.state() == deno_runtime::deno_core::v8::PromiseState::Fulfilled {
                        let result = promise.result(context_scope);
                        return deno_runtime::deno_core::serde_v8::from_v8::<serde_json::Value>(
                            context_scope,
                            result,
                        )
                        .ok();
                    }
                    // Promise is not fulfilled (might be pending or rejected)
                    return None;
                }

                deno_runtime::deno_core::serde_v8::from_v8::<serde_json::Value>(
                    context_scope,
                    default_value,
                )
                .ok()
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
