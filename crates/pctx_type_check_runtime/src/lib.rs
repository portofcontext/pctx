//! # PCTX Type Check
//!
//! An isolated TypeScript type checking runtime powered by Deno and the official TypeScript compiler.
//!
//! ## Overview
//!
//! This crate provides a sandboxed environment for performing full semantic type checking on
//! TypeScript code. It embeds the TypeScript 5.3.3 compiler in a V8 snapshot for fast startup
//! and uses a separate Deno runtime instance to ensure complete isolation.
//!
//! ## Features
//!
//! - **Full Semantic Analysis**: Uses the official TypeScript compiler for complete type checking
//! - **Isolated Runtime**: Each type check runs in its own sandboxed Deno runtime
//! - **Fast Startup**: TypeScript compiler embedded in V8 snapshot (~20s build time, instant runtime)
//! - **JavaScript Compatible**: Filters TypeScript-only errors to allow valid JavaScript code
//! - **Async Support**: Provides both sync and async APIs
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use pctx_type_check_runtime::type_check;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Type check TypeScript code
//! let code = r#"
//!     const x: number = 42;
//!     const y: string = "hello";
//!     export default x + y.length;
//! "#;
//!
//! let result = type_check(code).await?;
//! if result.success {
//!     println!("Type check passed!");
//! } else {
//!     for diagnostic in result.diagnostics {
//!         println!("{}: {}", diagnostic.severity, diagnostic.message);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Filtering
//!
//! The type checker automatically filters out errors that don't affect runtime execution,
//! such as:
//! - Missing console/Promise declarations
//! - Implicit `any` types
//! - Dynamic object access
//! - Unknown exception types
//!
//! This allows JavaScript code to type-check successfully while still catching real type errors.
//!
//! ## Performance
//!
//! - **Build Time**: ~20 seconds (one-time cost to create V8 snapshot with TypeScript compiler)
//! - **Runtime**: ~40-60ms per type check for typical code
//! - **Memory**: Isolated runtime per check, cleaned up automatically
//!
//! ## Snapshot Details
//!
//! The crate exports a pre-compiled V8 snapshot that includes:
//! - TypeScript 5.3.3 compiler (9MB source)
//! - Type checking runtime logic
//! - Minimal lib.d.ts definitions for common types
//!
//! The snapshot is embedded at compile time and accessible via [`TYPE_CHECK_SNAPSHOT`].

use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use futures::lock::Mutex;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use thiserror::Error;

/// Result type alias for type checking operations
pub type Result<T> = std::result::Result<T, TypeCheckError>;

/// Errors that can occur during type checking
#[derive(Debug, Error)]
pub enum TypeCheckError {
    /// Internal error in the type checking runtime
    #[error("Internal type check error: {0}")]
    InternalError(String),

    /// Error parsing the TypeScript code
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// A single type checking diagnostic (error or warning)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    /// Human-readable error message
    pub message: String,
    /// Line number where the error occurred (1-indexed)
    pub line: Option<usize>,
    /// Column number where the error occurred (1-indexed)
    pub column: Option<usize>,
    /// Severity level: "error" or "warning"
    pub severity: String,
    /// TypeScript diagnostic code (e.g., 2322 for type mismatch)
    pub code: Option<u32>,
}

/// Result of a type checking operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    /// Whether the code passed type checking (no errors)
    pub success: bool,
    /// List of diagnostics found during type checking
    pub diagnostics: Vec<Diagnostic>,
}

/// Pre-compiled V8 snapshot containing the TypeScript compiler
///
/// This snapshot is created at build time and includes:
/// - TypeScript 5.3.3 compiler (full semantic analysis)
/// - Type checking runtime with lib.d.ts definitions
/// - MCP SDK type definitions
///
/// The snapshot is ~10MB and loads instantly at runtime.
pub static TYPE_CHECK_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/PCTX_TYPE_CHECK_SNAPSHOT.bin"));

// Define the type check extension
deno_core::extension!(
    pctx_type_check_snapshot,
    esm_entry_point = "ext:pctx_type_check_snapshot/type_check_runtime.js",
    esm = [ dir "src", "type_check_runtime.js" ],
);

// Global mutex to serialize type checking operations and prevent V8 race conditions
static TYPE_CHECK_MUTEX: std::sync::LazyLock<Mutex<()>> = std::sync::LazyLock::new(|| {
    // Initialize V8 platform once
    deno_core::JsRuntime::init_platform(None, false);
    Mutex::new(())
});

/// Type check TypeScript code using an isolated Deno runtime with TypeScript compiler
///
/// This creates a separate Deno runtime with the TypeScript compiler snapshot to perform
/// full semantic type checking in a sandboxed environment.
///
/// # Arguments
///
/// * `code` - The TypeScript code to type check
///
/// # Returns
///
/// Returns a [`CheckResult`] containing whether the check passed and any diagnostics.
///
/// # Errors
///
/// Returns [`TypeCheckError::ParseError`] if the code has syntax errors.
/// Returns [`TypeCheckError::InternalError`] if the type checking runtime fails.
///
/// # Example
///
/// ```rust,no_run
/// use pctx_type_check_runtime::type_check;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let code = r#"
///     const x: number = "string"; // Type error!
/// "#;
///
/// let result = type_check(code).await?;
/// assert!(!result.success);
/// assert_eq!(result.diagnostics.len(), 1);
/// # Ok(())
/// # }
/// ```
pub async fn type_check(code: &str) -> Result<CheckResult> {
    // First do a quick syntax check with deno_ast
    let parse_result = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: deno_ast::ModuleSpecifier::parse("file:///check.ts")
            .map_err(|e| TypeCheckError::InternalError(e.to_string()))?,
        text: code.into(),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    });

    // If syntax parsing fails, return immediately
    if let Err(diagnostic) = parse_result {
        return Ok(CheckResult {
            success: false,
            diagnostics: vec![Diagnostic {
                message: diagnostic.to_string(),
                line: None,
                column: None,
                severity: "error".to_string(),
                code: None,
            }],
        });
    }

    // Create an isolated runtime with the type check snapshot
    // Serialize runtime creation to prevent V8 race conditions
    let mut js_runtime = {
        let _guard = TYPE_CHECK_MUTEX.lock().await;
        JsRuntime::new(RuntimeOptions {
            module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
            startup_snapshot: Some(TYPE_CHECK_SNAPSHOT),
            extensions: vec![pctx_type_check_snapshot::init()],
            ..Default::default()
        })
    };

    // Call the type checking function from the runtime
    let code_json =
        serde_json::to_string(code).map_err(|e| TypeCheckError::InternalError(e.to_string()))?;

    let check_script = format!(
        r"
        (function() {{
            const code = {code_json};
            return globalThis.typeCheckCode(code);
        }})()
        "
    );

    let result = js_runtime
        .execute_script("<type_check>", check_script)
        .map_err(|e| TypeCheckError::InternalError(e.to_string()))?;

    // Extract the result using v8 scope
    let check_result = {
        deno_core::scope!(scope, &mut js_runtime);
        let local = deno_core::v8::Local::new(scope, result);
        deno_core::serde_v8::from_v8::<CheckResult>(scope, local)
            .map_err(|e| TypeCheckError::InternalError(e.to_string()))?
    };

    Ok(check_result)
}

/// Filters diagnostics to only include errors that indicate runtime failures
///
/// This function determines whether a TypeScript diagnostic represents a real problem that
/// would cause the code to fail at runtime, versus TypeScript-only strictness errors.
///
/// # Filtered Error Codes
///
/// The following TypeScript error codes are considered irrelevant and will return `false`:
/// - `2307`: Cannot find module (module resolution)
/// - `2304`: Cannot find name 'require'
/// - `7016`: Could not find declaration file
/// - `2580`, `2585`, `2591`: Promise/console not found (runtime provides these)
/// - `7006`, `7053`, `7005`, `7034`: Implicit any types (JavaScript compatibility)
/// - `18046`: Variable of type 'unknown' (reduce operations)
/// - `2362`, `2363`: Arithmetic operation strictness
///
/// # Arguments
///
/// * `diagnostic` - The diagnostic to check
///
/// # Returns
///
/// Returns `true` if the error is relevant (would cause runtime failure), `false` otherwise.
///
/// # Example
///
/// ```rust
/// use pctx_type_check_runtime::{Diagnostic, is_relevant_error};
///
/// // Type mismatch - relevant error
/// let type_error = Diagnostic {
///     message: "Type 'string' is not assignable to type 'number'.".to_string(),
///     line: Some(1),
///     column: Some(1),
///     severity: "error".to_string(),
///     code: Some(2322),
/// };
/// assert!(is_relevant_error(&type_error));
///
/// // Console not found - irrelevant (runtime provides it)
/// let console_error = Diagnostic {
///     message: "Cannot find name 'console'.".to_string(),
///     line: Some(1),
///     column: Some(1),
///     severity: "error".to_string(),
///     code: Some(2580),
/// };
/// assert!(!is_relevant_error(&console_error));
/// ```
pub fn is_relevant_error(diagnostic: &Diagnostic) -> bool {
    // Ignore certain TypeScript errors that aren't helpful for validation
    let ignored_codes = [
        2307,  // Cannot find module
        2304,  // Cannot find name 'require'
        7016,  // Could not find declaration file
        2580,  // Cannot find name 'console'
        2585,  // 'Promise' only refers to a type, but is being used as a value
        2591,  // Cannot find name 'Promise'
        7006,  // Parameter implicitly has an 'any' type (SDK is typed)
        7053,  // Element implicitly has an 'any' type (dynamic object access)
        7005,  // Variable implicitly has an 'any[]' type
        7034,  // Variable implicitly has type 'any[]' in some locations
        18046, // Variable is of type 'unknown' (reduce operations)
        2362,  // Left-hand side of arithmetic operation must be number/any
        2363,  // Right-hand side of arithmetic operation must be number/any
    ];

    match diagnostic.code {
        Some(code) => !ignored_codes.contains(&code),
        None => true, // No code is present is an error
    }
}

/// Returns the crate version
///
/// # Example
///
/// ```rust
/// use pctx_type_check_runtime::version;
///
/// println!("pctx_type_check version: {}", version());
/// ```
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_type_check_valid_code() {
        let code = r"const x: number = 42;";
        let result = type_check(code).await.expect("type check should not fail");
        assert!(result.success);
        assert!(result.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn test_type_check_syntax_error() {
        let code = r"const x: number = ;";
        let result = type_check(code).await.expect("type check should not fail");
        assert!(!result.success);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn test_is_relevant_error_function() {
        // Relevant error (type mismatch TS2322)
        let relevant = Diagnostic {
            message: "Type 'string' is not assignable to type 'number'.".to_string(),
            line: Some(1),
            column: Some(1),
            severity: "error".to_string(),
            code: Some(2322),
        };
        assert!(is_relevant_error(&relevant), "TS2322 should be relevant");

        // Irrelevant error (console TS2580)
        let irrelevant_console = Diagnostic {
            message: "Cannot find name 'console'.".to_string(),
            line: Some(1),
            column: Some(1),
            severity: "error".to_string(),
            code: Some(2580),
        };
        assert!(
            !is_relevant_error(&irrelevant_console),
            "TS2580 should be irrelevant"
        );

        // Irrelevant error (Promise TS2591)
        let irrelevant_promise = Diagnostic {
            message: "Cannot find name 'Promise'.".to_string(),
            line: Some(1),
            column: Some(1),
            severity: "error".to_string(),
            code: Some(2591),
        };
        assert!(
            !is_relevant_error(&irrelevant_promise),
            "TS2591 should be irrelevant"
        );

        // Irrelevant error (implicit any TS7006)
        let irrelevant_implicit_any = Diagnostic {
            message: "Parameter implicitly has an 'any' type.".to_string(),
            line: Some(1),
            column: Some(1),
            severity: "error".to_string(),
            code: Some(7006),
        };
        assert!(
            !is_relevant_error(&irrelevant_implicit_any),
            "TS7006 should be irrelevant"
        );

        // Error without code should be relevant
        let no_code = Diagnostic {
            message: "Some error".to_string(),
            line: Some(1),
            column: Some(1),
            severity: "error".to_string(),
            code: None,
        };
        assert!(
            is_relevant_error(&no_code),
            "Errors without code should be relevant"
        );
    }
}
