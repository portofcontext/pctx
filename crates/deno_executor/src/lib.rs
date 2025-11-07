mod deno_execute;
mod ts_go_check;

pub use deno_execute::{ExecutionError as RuntimeError, execute_code};
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

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
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
    let check_result = check(code)?;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub severity: String,
    pub code: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
}

/// Check TypeScript code and return structured diagnostics if there are problems
///
/// This function performs TypeScript type checking with typescript-go:
/// - Syntax validation
/// - TypeScript parsing
/// - Type inference and checking
/// - Detects type mismatches (e.g., `const x: number = "string"`)
///
/// The typescript-go binary is automatically downloaded during build and bundled with the crate.
///
/// # Arguments
/// * `code` - The TypeScript code snippet to check
///
/// # Returns
/// * `Ok(CheckResult)` - Contains type diagnostics and success status
///
/// # Errors
/// * `ParseError` - If the code cannot be parsed
/// * `InternalError` - If typescript-go execution fails
/// * `IoError` - If file I/O fails
///
/// # Examples
/// ```
/// use deno_executor::check;
///
/// // This will pass - types match
/// let code = r#"const greeting: string = "hello";"#;
/// let result = check(code).expect("check should not fail");
/// assert!(result.success);
/// ```
pub fn check(code: &str) -> Result<CheckResult> {
    let binary_path = ts_go_check::get_tsgo_binary_path()
        .ok_or_else(|| DenoExecutorError::InternalError(
            "typescript-go binary not found. This should not happen - please report this build issue.".to_string()
        ))?;

    ts_go_check::check_with_tsgo(code, &binary_path)
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests;
