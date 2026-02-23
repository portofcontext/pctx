mod callback_registry;

pub use callback_registry::{CallbackFn, CallbackRegistry};

use monty::{
    DictPairs, ExcType, ExternalResult, MontyException, MontyObject, MontyRun, NoLimitTracker,
    PrintWriter, RunProgress,
};
use monty_type_checking::{SourceFile, type_check as monty_type_check};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, instrument, warn};

pub type Result<T> = std::result::Result<T, PythonRuntimeError>;

#[derive(Debug, Error)]
pub enum PythonRuntimeError {
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Options for Python code execution.
///
/// Monty is sandboxed by default — all filesystem, network, and environment
/// variable access is blocked. External functions (callbacks) are the only
/// way for Python code to interact with the host.
#[derive(Clone, Default, Debug)]
pub struct ExecuteOptions {
    /// Registry of named callbacks that Python code can call as regular functions.
    ///
    /// The names registered here are declared to monty at parse time so the
    /// interpreter knows they are external. When Python calls one of them,
    /// execution suspends, the callback is invoked with the call's arguments
    /// serialised as JSON, and the result is deserialised back into a Python
    /// value before resuming.
    pub callback_registry: CallbackRegistry,

    /// Typed `.pyi`-style stubs for the registered callbacks.
    ///
    /// When provided, the type checker uses these signatures instead of the
    /// default `(*args: Any, **kwargs: Any) -> Any` fallback. This lets you
    /// express the precise interface an LLM is expected to call so that
    /// type errors in generated code are caught before execution.
    ///
    /// Example:
    /// ```python
    /// def get_weather(city: str, units: str) -> float: ...
    /// def search(query: str, limit: int = 10) -> list[str]: ...
    /// ```
    pub stubs: Option<String>,
}

impl ExecuteOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a callback registry.
    #[must_use]
    pub fn with_callbacks(mut self, registry: CallbackRegistry) -> Self {
        self.callback_registry = registry;
        self
    }

    /// Provide typed stubs for the registered callbacks.
    ///
    /// The string should contain `.pyi`-style function signatures, one per
    /// callback. Any callback not mentioned here falls back to
    /// `(*args: Any, **kwargs: Any) -> Any` (i.e., no type checking).
    #[must_use]
    pub fn with_stubs(mut self, stubs: impl Into<String>) -> Self {
        self.stubs = Some(stubs.into());
        self
    }
}

/// The result of executing Python code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    /// Whether the code executed successfully.
    pub success: bool,

    /// Static type-checking errors reported before execution.
    ///
    /// Non-empty when `monty-type-checking` detects a type violation.
    /// Execution is skipped when this is non-empty.
    pub stderr: String,

    /// Runtime error information (parse errors, unhandled exceptions, etc.).
    pub runtime_error: Option<ExecutionError>,

    /// The final value of the last expression in the script, serialised as JSON.
    pub output: Option<serde_json::Value>,

    /// Everything written to stdout by the script via `print()`.
    pub stdout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub message: String,
}

/// Execute Python code using the monty interpreter.
///
/// Monty is a minimal, sandboxed Python interpreter with sub-millisecond startup.
/// It blocks all filesystem, network, and environment variable access by default.
/// Interaction with the host happens exclusively through named callbacks registered
/// in [`ExecuteOptions::callback_registry`].
///
/// # Arguments
/// * `code` — The Python source code to execute
/// * `options` — Execution options including the callback registry
///
/// # Returns
/// * `Ok(ExecuteResult)` — Always; parse and runtime errors appear inside the result
///
/// # Errors
/// Returns `Err(PythonRuntimeError)` only if internal tooling fails (not for Python errors).
pub async fn execute(code: &str, options: ExecuteOptions) -> Result<ExecuteResult> {
    debug!(
        code_length = code.len(),
        "Python code submitted for execution"
    );

    let callback_names: Vec<String> = options.callback_registry.ids();

    // ── Pre-flight: detect `functions.X()` anti-pattern ──────────────────────
    if let Some(err) = check_functions_namespace(code, &callback_names) {
        return Ok(ExecuteResult {
            success: false,
            stderr: err,
            runtime_error: None,
            output: None,
            stdout: String::new(),
        });
    }

    // ── Pre-flight: detect stdlib import statements ───────────────────────────
    if let Some(err) = check_imports(code) {
        return Ok(ExecuteResult {
            success: false,
            stderr: err,
            runtime_error: None,
            output: None,
            stdout: String::new(),
        });
    }

    // ── Pre-flight: auto-wrap top-level `return` statements ──────────────────
    // Top-level `return` is a syntax error in Python, but LLMs write it
    // instinctively for early exits. We transparently wrap the whole script in
    // `def run(): ...; run()` so the `return` statements become valid.
    let wrapped;
    let code = if let Some(w) = wrap_module_returns(code) {
        debug!("Auto-wrapped top-level return(s) in def run()");
        wrapped = w;
        wrapped.as_str()
    } else {
        code
    };

    // ── Static type checking ──────────────────────────────────────────────────
    // Build stubs so the type checker knows registered callbacks exist.
    // User-supplied typed stubs take precedence; any callback not covered by
    // them gets a fallback `(*args: Any, **kwargs: Any) -> Any` declaration.
    let source = SourceFile::new(code, "script.py");
    let stubs_src = build_stubs(options.stubs.as_deref(), &callback_names);
    let stubs = (!stubs_src.is_empty()).then(|| SourceFile::new(&stubs_src, "callbacks.pyi"));
    match monty_type_check(&source, stubs.as_ref()) {
        Ok(Some(diagnostics)) => {
            let stderr = enrich_python_error(&format!("{diagnostics}"));
            warn!(stderr = %stderr, "Python type check failed");
            return Ok(ExecuteResult {
                success: false,
                stderr,
                runtime_error: None,
                output: None,
                stdout: String::new(),
            });
        }
        Ok(None) => debug!("Python type check passed"),
        Err(e) => warn!(error = %e, "Type checker internal error (skipping)"),
    }

    let runner = match MontyRun::new(code.to_owned(), "script.py", vec![], callback_names.clone()) {
        Ok(r) => r,
        Err(exc) => {
            let message = format_exception(&exc);
            warn!(error = %message, "Python parse/compile error");
            let stderr = enrich_python_error(&message);
            return Ok(ExecuteResult {
                success: false,
                stderr,
                runtime_error: Some(ExecutionError { message }),
                output: None,
                stdout: String::new(),
            });
        }
    };

    debug!(callbacks = ?callback_names, "Starting Python execution");

    let mut writer = PrintWriter::Collect(String::new());

    // start() consumes the runner and returns the first RunProgress snapshot
    let first = match runner.start(vec![], NoLimitTracker, &mut writer) {
        Ok(p) => p,
        Err(exc) => {
            let stdout = writer.collected_output().unwrap_or("").to_string();
            return Ok(make_error_result(&exc, stdout));
        }
    };

    drive_to_completion(first, &mut writer, &options.callback_registry).await
}

/// Drive the monty `RunProgress` state machine to completion, servicing
/// `FunctionCall` suspensions via the callback registry.
#[instrument(skip_all)]
async fn drive_to_completion(
    first: RunProgress<NoLimitTracker>,
    writer: &mut PrintWriter<'_>,
    registry: &CallbackRegistry,
) -> Result<ExecuteResult> {
    let mut current = first;

    loop {
        match current {
            RunProgress::Complete(obj) => {
                let stdout = writer.collected_output().unwrap_or("").to_string();
                let output = monty_to_json(&obj);
                debug!("Python execution completed successfully");
                return Ok(ExecuteResult {
                    success: true,
                    stderr: String::new(),
                    runtime_error: None,
                    output: Some(output),
                    stdout,
                });
            }

            RunProgress::FunctionCall {
                function_name,
                args,
                kwargs,
                state,
                ..
            } => {
                debug!(function = %function_name, "Python called external function");

                let json_args = monty_args_to_json(&args, kwargs);
                let external_result = match registry.invoke(&function_name, json_args).await {
                    Ok(value) => ExternalResult::Return(json_to_monty(value)),
                    Err(e) => {
                        warn!(function = %function_name, error = %e, "Callback failed");
                        ExternalResult::Error(MontyException::new(ExcType::RuntimeError, Some(e)))
                    }
                };

                current = match state.run(external_result, writer) {
                    Ok(p) => p,
                    Err(exc) => {
                        let stdout = writer.collected_output().unwrap_or("").to_string();
                        return Ok(make_error_result(&exc, stdout));
                    }
                };
            }

            RunProgress::OsCall {
                function, state, ..
            } => {
                // OS calls (filesystem, network, env vars) are blocked in the sandbox.
                // Return a RuntimeError to Python so the script sees a clear message.
                warn!(function = ?function, "Blocked OS call in Python sandbox");
                let exc = MontyException::new(
                    ExcType::RuntimeError,
                    Some(format!(
                        "OS call '{function:?}' is not permitted in this sandbox"
                    )),
                );
                current = match state.run(ExternalResult::Error(exc), writer) {
                    Ok(p) => p,
                    Err(e) => {
                        let stdout = writer.collected_output().unwrap_or("").to_string();
                        return Ok(make_error_result(&e, stdout));
                    }
                };
            }

            RunProgress::ResolveFutures(_) => {
                // asyncio / native Python futures are not supported yet.
                let stdout = writer.collected_output().unwrap_or("").to_string();
                let message = "Python asyncio is not supported in this runtime".to_owned();
                return Ok(ExecuteResult {
                    success: false,
                    stderr: "Error: Python asyncio is not supported. Use synchronous callback-based tools instead.".to_owned(),
                    runtime_error: Some(ExecutionError { message }),
                    output: None,
                    stdout,
                });
            }
        }
    }
}

// ── Conversion helpers ────────────────────────────────────────────────────────

/// Convert monty call arguments to a JSON value for the callback.
///
/// Convention (mirrors how LLM-generated code naturally calls tools):
/// - kwargs only   → JSON object  `{"param": value, ...}`
/// - single arg    → the value itself (unwrapped)
/// - multiple args → JSON array   `[val, val, ...]`
/// - no args       → `None`
fn monty_args_to_json(
    args: &[MontyObject],
    kwargs: Vec<(MontyObject, MontyObject)>,
) -> Option<serde_json::Value> {
    if !kwargs.is_empty() {
        let map = kwargs
            .into_iter()
            .filter_map(|(k, v)| {
                if let MontyObject::String(key) = k {
                    Some((key, monty_to_json(&v)))
                } else {
                    None
                }
            })
            .collect::<serde_json::Map<_, _>>();
        return Some(serde_json::Value::Object(map));
    }

    match args.len() {
        0 => None,
        1 => Some(monty_to_json(&args[0])),
        _ => Some(serde_json::Value::Array(
            args.iter().map(monty_to_json).collect(),
        )),
    }
}

/// Convert a JSON value returned by a callback back into a [`MontyObject`].
fn json_to_monty(value: serde_json::Value) -> MontyObject {
    match value {
        serde_json::Value::Null => MontyObject::None,
        serde_json::Value::Bool(b) => MontyObject::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                MontyObject::Int(i)
            } else if let Some(f) = n.as_f64() {
                MontyObject::Float(f)
            } else {
                MontyObject::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => MontyObject::String(s),
        serde_json::Value::Array(items) => {
            MontyObject::List(items.into_iter().map(json_to_monty).collect())
        }
        serde_json::Value::Object(map) => {
            let pairs = map
                .into_iter()
                .map(|(k, v)| (MontyObject::String(k), json_to_monty(v)))
                .collect::<Vec<_>>();
            MontyObject::dict(DictPairs::from(pairs))
        }
    }
}

/// Convert a [`MontyObject`] into a [`serde_json::Value`].
///
/// Primitive types map directly. Collections are handled recursively.
/// Types without a clean JSON equivalent (dataclasses, etc.) are rendered
/// via Python's `repr()`.
fn monty_to_json(obj: &MontyObject) -> serde_json::Value {
    match obj {
        MontyObject::None => serde_json::Value::Null,
        MontyObject::Bool(b) => serde_json::Value::Bool(*b),
        MontyObject::Int(i) => serde_json::json!(*i),
        MontyObject::BigInt(n) => serde_json::Value::String(n.to_string()),
        MontyObject::Float(f) => serde_json::json!(*f),
        MontyObject::String(s) => serde_json::Value::String(s.clone()),
        MontyObject::Bytes(b) => {
            serde_json::Value::Array(b.iter().map(|byte| serde_json::json!(*byte)).collect())
        }
        MontyObject::List(items) => {
            serde_json::Value::Array(items.iter().map(monty_to_json).collect())
        }
        MontyObject::Tuple(items) => {
            serde_json::Value::Array(items.iter().map(monty_to_json).collect())
        }
        MontyObject::Set(items) => {
            serde_json::Value::Array(items.iter().map(monty_to_json).collect())
        }
        MontyObject::FrozenSet(items) => {
            serde_json::Value::Array(items.iter().map(monty_to_json).collect())
        }
        // Fall back to Python repr() for Dict, Dataclass, etc.
        other => serde_json::Value::String(other.py_repr()),
    }
}

/// Build the stubs string passed to the type checker.
///
/// If `custom` stubs are supplied by the caller they are used as-is (they
/// should declare the full signatures). For every registered callback that
/// does NOT appear in the custom stubs we append a permissive fallback so
/// the type checker doesn't flag it as an unresolved reference.
fn build_stubs(custom: Option<&str>, names: &[String]) -> String {
    use std::fmt::Write as _;

    if names.is_empty() && custom.is_none() {
        return String::new();
    }

    let mut out = "from typing import Any\n".to_owned();

    if let Some(c) = custom {
        out.push_str(c);
        if !c.ends_with('\n') {
            out.push('\n');
        }
        // Append Any-fallbacks for callbacks not mentioned in the custom stubs
        for name in names {
            if !c.contains(&format!("def {name}(")) {
                let _ = writeln!(out, "def {name}(*args: Any, **kwargs: Any) -> Any: ...");
            }
        }
    } else {
        for name in names {
            let _ = writeln!(out, "def {name}(*args: Any, **kwargs: Any) -> Any: ...");
        }
    }

    out
}

fn format_exception(exc: &MontyException) -> String {
    format!("{exc:?}")
}

fn make_error_result(exc: &MontyException, stdout: String) -> ExecuteResult {
    let message = format_exception(exc);
    warn!(error = %message, "Python runtime exception");
    let stderr = enrich_python_error(&message);
    ExecuteResult {
        success: false,
        stderr,
        runtime_error: Some(ExecutionError { message }),
        output: None,
        stdout,
    }
}

/// Wrap the module in `def run(): ...; run()` if it contains a module-level `return`.
///
/// Top-level `return` is a syntax error in standard Python, but LLMs routinely
/// write it as an early-exit idiom — both at column 0 and inside `if`/`for`/`while`
/// blocks that are themselves at module scope. We use a scope-aware scanner:
/// only `def` and `class` statements create new scopes; control-flow blocks
/// (`if`, `for`, `while`, `try`, `with`) do not.
///
/// Returns `None` if no module-level `return` is found (common case, zero cost).
fn wrap_module_returns(code: &str) -> Option<String> {
    if !has_module_level_return(code) {
        return None;
    }

    let indented = code
        .lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("    {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!("def run():\n{indented}\nrun()"))
}

/// Return `true` if the source contains a `return` at module scope.
///
/// Tracks `def` / `async def` / `class` scopes by indentation: when a line's
/// leading-whitespace count falls back to or below the indent of the enclosing
/// `def`/`class`, that scope is considered closed. A `return` found while the
/// scope stack is empty is a module-level return.
fn has_module_level_return(code: &str) -> bool {
    // Stack of indentation levels for open def/class scopes.
    let mut scope_indent_stack: Vec<usize> = vec![];

    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - trimmed.len();

        // Close any scopes whose def/class indent >= current indent.
        while let Some(&top) = scope_indent_stack.last() {
            if indent <= top {
                scope_indent_stack.pop();
            } else {
                break;
            }
        }

        // def / async def / class open a new scope.
        if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
        {
            scope_indent_stack.push(indent);
            continue; // The def line itself never contains a module-level return.
        }

        // A `return` with an empty scope stack is at module scope.
        if scope_indent_stack.is_empty()
            && (trimmed == "return"
                || trimmed.starts_with("return ")
                || trimmed.starts_with("return\t"))
        {
            return true;
        }
    }

    false
}

/// Scan source for `import` / `from X import` statements.
///
/// Returns a directive error listing the offending lines and concrete alternatives,
/// because the monty sandbox does not support any standard-library imports.
fn check_imports(code: &str) -> Option<String> {
    let mut offenders: Vec<(usize, String)> = vec![];
    for (i, line) in code.lines().enumerate() {
        let t = line.trim();
        if !t.starts_with('#')
            && (t.starts_with("import ") || (t.starts_with("from ") && t.contains(" import ")))
        {
            offenders.push((i + 1, t.to_owned()));
        }
    }
    if offenders.is_empty() {
        return None;
    }

    let lines = offenders
        .iter()
        .map(|(n, s)| format!("  Line {n}: {s}"))
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "Error: Import statements are not available in this execution mode.\n\
        {lines}\n\
        \n\
        Alternatives:\n\
        - `map(fn, lst)` → list comprehension: `[fn(x) for x in lst]`\n\
        - `filter(fn, lst)` → list comprehension: `[x for x in lst if fn(x)]`\n\
        - `datetime` → use string values from tool responses directly\n\
        - `re` → use `str.startswith()`, `str.endswith()`, or `'substring' in s`\n\
        - `json` → tool results are already parsed Python objects, no JSON parsing needed\n\
        - `functools` / `itertools` → use list comprehensions and built-in functions"
    ))
}

/// Scan source for `functions.X` attribute-access patterns.
///
/// Returns an error string (with "did you mean" suggestions where possible)
/// when the pattern is detected, because `functions` is never a valid object
/// in the monty sandbox — tools must be called directly.
fn check_functions_namespace(code: &str, callback_names: &[String]) -> Option<String> {
    const PREFIX: &str = "functions.";
    let mut found: Vec<&str> = vec![];
    let mut start = 0;
    while let Some(pos) = code[start..].find(PREFIX) {
        let abs = start + pos + PREFIX.len();
        let rest = &code[abs..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            let name = &rest[..end];
            if !found.contains(&name) {
                found.push(name);
            }
        }
        start += pos + PREFIX.len();
    }

    if found.is_empty() {
        return None;
    }

    let (known, unknown): (Vec<&&str>, Vec<&&str>) = found
        .iter()
        .partition(|&&n| callback_names.iter().any(|c| c == n));

    let mut msg =
        "Error: `functions` is not a valid object — tools are bare global functions.\n".to_owned();

    if !known.is_empty() {
        let suggestions = known
            .iter()
            .map(|n| format!("`{n}(...)`"))
            .collect::<Vec<_>>()
            .join(", ");
        msg.push_str(&format!("Did you mean: {suggestions}?\n"));
    }
    if !unknown.is_empty() {
        let names = unknown
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ");
        msg.push_str(&format!(
            "Unknown functions referenced via `functions.`: {names}. Use `list_functions()` to see what is available.\n"
        ));
    }
    msg.push_str("Call tools directly: `tool_name(param=value)`.");
    Some(msg)
}

/// Post-process an error message to append actionable hints for common mistakes.
fn enrich_python_error(message: &str) -> String {
    let mut out = message.to_owned();

    // Top-level `return` statement (syntax error or "outside of a function")
    // Most common eval failure: 179 occurrences.
    if message.contains("return") {
        let lower = message.to_lowercase();
        if lower.contains("outside")
            || lower.contains("invalid-syntax")
            || lower.contains("invalid syntax")
            || lower.contains("syntaxerror")
        {
            out.push_str(
                "\n\
                \nFIX: Module-level code cannot use `return`. Two options:\
                \n\
                \nOption 1 — drop the `return`; the last expression is the output automatically:\
                \n  REMOVE:  return result\
                \n  KEEP:    result\
                \n\
                \nOption 2 — wrap in a function for early exits:\
                \n  def run():\
                \n      data = get_something(...)\
                \n      if not data:\
                \n          return \"not found\"\
                \n      return data\
                \n  run()",
            );
        }
    }

    // invalid-argument-type where the found type contains None in a union — typically
    // caused by dict.get() which returns Any | None (or Unknown | ... | None) (136 occurrences).
    if message.contains("invalid-argument-type") && message.contains("| None") {
        out.push_str(
            "\nHint: This usually happens when using `.get('key')` on a tool result — \
            `.get()` returns `Any | None`. \
            Use `result['key']` when the key is guaranteed to exist, \
            or `result.get('key') or ''` to provide a non-None string fallback.",
        );
    }

    // Unresolved references to Python builtins not available in monty (64 occurrences).
    // The type checker emits `error[unresolved-reference]: Name `X` used when not defined`.
    if message.contains("unresolved-reference") {
        // Table of (backtick-wrapped name in error, alternative description).
        const BUILTIN_HINTS: &[(&str, &str)] = &[
            ("`map`", "a list comprehension: `[f(x) for x in lst]`"),
            (
                "`filter`",
                "a list comprehension: `[x for x in lst if pred(x)]`",
            ),
            (
                "`next`",
                "index access `lst[0]` or a comprehension with a conditional",
            ),
            (
                "`hasattr`",
                "`'key' in obj` for dicts, or direct attribute access",
            ),
            ("`locals`", "explicitly naming each variable you need"),
            ("`vars`", "explicitly naming each variable you need"),
            (
                "`getattr`",
                "`obj['key']` for dicts or direct attribute access",
            ),
        ];
        for (name, hint) in BUILTIN_HINTS {
            if message.contains(name) {
                out.push_str(&format!(
                    "\nHint: {name} is not available in this runtime. Use {hint} instead."
                ));
            }
        }
    }

    out
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pctx_codegen::{RootSchema, Tool};
    use std::sync::Arc;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }

    /// Build a realistic multi-parameter tool (required + optional + enum)
    /// and a matching callback.
    ///
    /// Schema:
    ///   search(query: str, `max_results`: int, category: Literal[...] | None = None)
    ///        -> dict[str, Any]
    fn make_search_tool() -> (Tool, CallbackFn) {
        let input_schema: RootSchema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "required": ["query", "max_results"],
            "properties": {
                "query":       { "type": "string" },
                "max_results": { "type": "integer" },
                "category": {
                    "type": "string",
                    "enum": ["news", "articles", "blogs"]
                }
            }
        }))
        .unwrap();

        let output_schema: RootSchema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "required": ["results", "total"],
            "properties": {
                "results": { "type": "array", "items": { "type": "string" } },
                "total":   { "type": "integer" }
            }
        }))
        .unwrap();

        let tool = Tool::new_callback(
            "search",
            Some("Search for documents matching a query".into()),
            Some(input_schema),
            Some(output_schema),
        )
        .unwrap();

        let callback: CallbackFn = Arc::new(|args: Option<serde_json::Value>| {
            Box::pin(async move {
                let args = args.unwrap_or_default();
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let max_results = args
                    .get("max_results")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(10);
                Ok(serde_json::json!({
                    "results": [format!("Result for '{query}'")],
                    "total": max_results
                }))
            })
        });

        (tool, callback)
    }

    /// Stubs are generated from the JSON schema via `pctx_codegen`.
    /// Passing the wrong type for a required parameter is caught by the
    /// type checker before any code executes — the callback is never invoked.
    #[test]
    fn test_type_error_caught_from_generated_stubs() {
        let (tool, callback) = make_search_tool();
        let stubs = pctx_codegen::python::generate_stubs(&[tool]).unwrap();
        eprintln!("generated stubs:\n{stubs}");

        let registry = CallbackRegistry::default();
        registry.add("search", callback).unwrap();

        let opts = ExecuteOptions::new()
            .with_callbacks(registry)
            .with_stubs(stubs);

        // max_results expects int; passing a string triggers a type error.
        let result = rt()
            .block_on(execute(r#"search(query="rust", max_results="five")"#, opts))
            .unwrap();

        assert!(!result.success, "expected type error, got: {result:?}");
        assert!(
            result.runtime_error.is_none(),
            "should be caught by type checker before execution: {result:?}"
        );
        assert!(
            !result.stderr.is_empty(),
            "stderr should contain the diagnostic"
        );
        eprintln!("type diagnostic:\n{}", result.stderr);
    }

    /// Runtime exceptions must appear in `stderr` (not only in `runtime_error`)
    /// so the Python client can surface them to the model.
    #[test]
    fn test_runtime_error_surfaces_in_stderr() {
        let result = rt()
            .block_on(execute("undefined_name", ExecuteOptions::new()))
            .unwrap();
        assert!(!result.success, "expected failure, got: {result:?}");
        assert!(
            !result.stderr.is_empty(),
            "stderr must be non-empty for runtime errors, got: {result:?}"
        );
    }

    /// Parse/compile errors must appear in `stderr`.
    #[test]
    fn test_parse_error_surfaces_in_stderr() {
        let result = rt()
            .block_on(execute("def (", ExecuteOptions::new()))
            .unwrap();
        assert!(!result.success, "expected failure, got: {result:?}");
        assert!(
            !result.stderr.is_empty(),
            "stderr must be non-empty for parse errors, got: {result:?}"
        );
    }

    /// `functions.search()` where `search` IS registered → specific "did you mean" suggestion.
    #[test]
    fn test_functions_namespace_known_function_suggests_did_you_mean() {
        let (tool, callback) = make_search_tool();
        let stubs = pctx_codegen::python::generate_stubs(&[tool]).unwrap();
        let registry = CallbackRegistry::default();
        registry.add("search", callback).unwrap();
        let opts = ExecuteOptions::new()
            .with_callbacks(registry)
            .with_stubs(stubs);

        let result = rt()
            .block_on(execute(
                "functions.search(query=\"test\", max_results=5)",
                opts,
            ))
            .unwrap();
        assert!(!result.success);
        assert!(
            result.stderr.contains("`search(...)`"),
            "expected 'did you mean `search(...)`' in stderr, got:\n{}",
            result.stderr
        );
    }

    /// `functions.unknown()` where the name is NOT registered → no false suggestion.
    #[test]
    fn test_functions_namespace_unknown_function_no_false_suggestion() {
        let result = rt()
            .block_on(execute(
                "testingstuff.totally_unknown()",
                ExecuteOptions::new(),
            ))
            .unwrap();
        assert!(!result.success);
        assert!(
            result.stderr.contains("testingstuff"),
            "expected explanation about `testingstuff.*` in stderr, got:\n{}",
            result.stderr
        );
        assert!(
            !result.stderr.contains("Did you mean"),
            "should not suggest 'did you mean' for an unknown function, got:\n{}",
            result.stderr
        );
    }

    /// Top-level `return` is transparently wrapped — execution succeeds and
    /// the returned value is the module output.
    #[test]
    fn test_top_level_return_succeeds_via_auto_wrap() {
        let result = rt()
            .block_on(execute("x = 1\nreturn x", ExecuteOptions::new()))
            .unwrap();
        assert!(
            result.success,
            "expected success after auto-wrap, got: {result:?}"
        );
        assert_eq!(result.output, Some(serde_json::json!(1)));
    }

    /// `import datetime` → actionable error listing the import and alternatives.
    #[test]
    fn test_import_produces_actionable_error() {
        let result = rt()
            .block_on(execute(
                "import datetime\ndatetime.now()",
                ExecuteOptions::new(),
            ))
            .unwrap();
        assert!(!result.success);
        assert!(
            result
                .stderr
                .contains("Import statements are not available"),
            "expected import error, got:\n{}",
            result.stderr
        );
        assert!(
            result.stderr.contains("datetime"),
            "error should name the offending import, got:\n{}",
            result.stderr
        );
    }

    /// `from re import match` → same import error.
    #[test]
    fn test_from_import_produces_actionable_error() {
        let result = rt()
            .block_on(execute(
                "from re import match\nmatch('a', 'a')",
                ExecuteOptions::new(),
            ))
            .unwrap();
        assert!(!result.success);
        assert!(
            result
                .stderr
                .contains("Import statements are not available"),
            "expected import error, got:\n{}",
            result.stderr
        );
        assert!(
            result.stderr.contains("re"),
            "error should name the offending module, got:\n{}",
            result.stderr
        );
    }

    /// `return` inside an `if` block at module scope (indent > 0, no column-0 return)
    /// is also detected and auto-wrapped. This is the most common LLM pattern:
    /// `if not cust:\n    return "not found"`.
    #[test]
    fn test_return_inside_if_block_at_module_scope_auto_wraps() {
        let result = rt()
            .block_on(execute(
                "x = 0\nif not x:\n    return \"early\"\n42",
                ExecuteOptions::new(),
            ))
            .unwrap();
        assert!(
            result.success,
            "expected success after auto-wrap, got: {result:?}"
        );
        assert_eq!(result.output, Some(serde_json::json!("early")));
    }

    /// `return` inside a function body (legitimately scoped) must NOT trigger wrapping.
    #[test]
    fn test_return_inside_function_not_wrapped() {
        let result = rt()
            .block_on(execute(
                "def helper():\n    return 42\nhelper()",
                ExecuteOptions::new(),
            ))
            .unwrap();
        assert!(result.success, "expected success, got: {result:?}");
        assert_eq!(result.output, Some(serde_json::json!(42)));
    }

    /// Early exit via top-level `return` works end-to-end: the value from the
    /// first taken `return` branch becomes the module output.
    #[test]
    fn test_top_level_early_return_exits_with_correct_value() {
        // The first branch is taken (x is falsy), so we get "not found".
        let result = rt()
            .block_on(execute(
                "x = 0\nif not x:\n    return \"not found\"\nreturn \"found\"",
                ExecuteOptions::new(),
            ))
            .unwrap();
        assert!(
            result.success,
            "expected success after auto-wrap, got: {result:?}"
        );
        assert_eq!(result.output, Some(serde_json::json!("not found")));
    }

    /// When `.get()` returns `Any | None` and a `str` is required, the error
    /// should suggest using `result['key']` or `result.get('key') or ''`.
    #[test]
    fn test_none_type_error_suggests_dict_access() {
        let (tool, callback) = make_search_tool();
        let stubs = pctx_codegen::python::generate_stubs(&[tool]).unwrap();
        let registry = CallbackRegistry::default();
        registry.add("search", callback).unwrap();
        let opts = ExecuteOptions::new()
            .with_callbacks(registry)
            .with_stubs(stubs);

        // search() requires query: str, but .get() returns Any | None → type error
        let result = rt()
            .block_on(execute(
                "d = {\"query\": \"rust\", \"max_results\": 5}\nsearch(query=d.get(\"query\"), max_results=5)",
                opts,
            ))
            .unwrap();
        assert!(!result.success);
        assert!(
            result.stderr.contains("result['key']") || result.stderr.contains("get('key')"),
            "expected dict access hint in stderr, got:\n{}",
            result.stderr
        );
    }

    /// When all types are correct the type checker passes, the callback is
    /// invoked, and the Python code can use the returned dict.
    #[test]
    fn test_callback_invoked_with_correct_types() {
        let (tool, callback) = make_search_tool();
        let stubs = pctx_codegen::python::generate_stubs(&[tool]).unwrap();

        let registry = CallbackRegistry::default();
        registry.add("search", callback).unwrap();

        let opts = ExecuteOptions::new()
            .with_callbacks(registry)
            .with_stubs(stubs);

        let result = rt()
            .block_on(execute(
                "r = search(query=\"rust\", max_results=5)\nr[\"total\"]",
                opts,
            ))
            .unwrap();

        assert!(result.success, "expected success, got: {result:?}");
        assert_eq!(result.output, Some(serde_json::json!(5)));
    }

    /// Unsupported builtins (`map`, `next`, `hasattr`) get a specific alternative hint.
    #[test]
    fn test_unsupported_builtin_suggests_alternative() {
        for (code, expected_hint) in [
            ("map(str, [1, 2, 3])", "list comprehension"),
            ("next(x for x in [1])", "index access"),
            ("hasattr({}, 'key')", "'key' in obj"),
        ] {
            let result = rt().block_on(execute(code, ExecuteOptions::new())).unwrap();
            assert!(
                !result.success,
                "expected failure for `{code}`, got success"
            );
            assert!(
                result.stderr.contains(expected_hint),
                "expected hint '{expected_hint}' in stderr for `{code}`, got:\n{}",
                result.stderr
            );
        }
    }
}
