mod deno_execute;

pub use deno_execute::{ExecutionError as RuntimeError, execute_code};
pub use pctx_type_check_runtime::{CheckResult, Diagnostic, is_relevant_error, type_check};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DenoExecutorError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub success: bool,

    /// Type checking diagnostics (if any)
    pub diagnostics: Vec<Diagnostic>,

    /// Runtime error information (if execution failed)
    pub runtime_error: Option<RuntimeError>,

    /// The default export value from the module (if any)
    pub output: Option<serde_json::Value>,

    /// Standard output from execution
    pub stdout: String,

    /// Standard error from execution
    pub stderr: String,
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
/// 1. First runs TypeScript type checking via `check()`
/// 2. If type checking passes, executes code with Deno runtime
/// 3. Returns unified result with diagnostics and runtime output
///
/// # Arguments
/// * `code` - The TypeScript code to check and execute
/// * `allowed_hosts` - Optional list of hosts that network requests are allowed to access.
///   Format: "hostname:port" or just "hostname" (e.g., "localhost:3000", "api.example.com").
///   If None or empty, all network access is denied.
///
/// # Returns
/// * `Ok(ExecuteResult)` - Contains type diagnostics, runtime errors, and output
///
/// # Errors
/// * Returns error only if internal tooling fails (not for type errors or runtime errors)
///
pub async fn execute(code: &str, allowed_hosts: Option<Vec<String>>) -> Result<ExecuteResult> {
    let check_result = type_check(code).await?;
    if !check_result.success {
        return Ok(ExecuteResult {
            success: false,
            diagnostics: check_result.diagnostics,
            runtime_error: None,
            output: None,
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    let exec_result = execute_code(code, allowed_hosts)
        .await
        .map_err(|e| DenoExecutorError::InternalError(e.to_string()))?;

    let stderr = if let Some(ref err) = exec_result.error {
        err.message.clone()
    } else {
        String::new()
    };

    Ok(ExecuteResult {
        success: exec_result.success,
        diagnostics: check_result.diagnostics, // always is empty if here
        runtime_error: exec_result.error,
        output: exec_result.output,
        stdout: exec_result.stdout,
        stderr: if exec_result.stderr.is_empty() {
            stderr
        } else {
            exec_result.stderr
        },
    })
}

/// Check TypeScript code and return structured diagnostics if there are problems
///
/// This function performs TypeScript type checking using an isolated Deno runtime:
/// - Syntax validation
/// - TypeScript parsing
/// - Full semantic type checking
///
/// # Arguments
/// * `code` - The TypeScript code snippet to check
///
/// # Returns
/// * `Ok(CheckResult)` - Contains type diagnostics and success status
///
/// # Errors
/// * Returns error only if internal type checking runtime fails
///
/// # Examples
/// ```
/// use deno_executor::check;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // This will pass - valid syntax
/// let code = r#"const greeting: string = "hello";"#;
/// let result = check(code).await?;
/// assert!(result.success);
/// # Ok(())
/// # }
/// ```
pub async fn check(code: &str) -> Result<CheckResult> {
    Ok(type_check(code).await?)
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests;
