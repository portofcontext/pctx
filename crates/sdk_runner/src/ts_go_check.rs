use regex::Regex;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use tempfile::NamedTempFile;

use crate::{CheckResult, Diagnostic, Result};

/// Get the path to the bundled typescript-go binary
pub(crate) fn get_tsgo_binary_path() -> Option<std::path::PathBuf> {
    // Check if the build script set the binary path at compile time
    if let Some(build_path) = option_env!("TSGO_BINARY_PATH") {
        let path = std::path::PathBuf::from(build_path);
        if path.exists() {
            return Some(path);
        }
    }

    let binary_name = if cfg!(target_os = "windows") {
        "tsgo.exe"
    } else {
        "tsgo"
    };

    // (development) - fallback if build script didn't set the path
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dev_path = std::path::Path::new(manifest_dir)
        .join(".bin")
        .join(binary_name);
    if dev_path.exists() {
        return Some(dev_path);
    }

    // (production) - check next to the executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let exe_sibling = exe_dir.join(binary_name);
        if exe_sibling.exists() {
            return Some(exe_sibling);
        }
    }

    None
}

/// Perform type checking using typescript-go
pub(crate) fn check_with_tsgo(code: &str, binary_path: &std::path::Path) -> Result<CheckResult> {
    // Create a temporary file with .ts extension
    let mut temp_file = NamedTempFile::with_suffix(".ts")?;
    temp_file.write_all(code.as_bytes())?;
    temp_file.flush()?;

    let temp_path = temp_file.path();

    // Run typescript-go type checker and only check if it's valid --noEmit
    let output = Command::new(binary_path)
        .arg("--noEmit")
        .arg("--pretty")
        .arg("false")
        .arg(temp_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut diagnostics = parse_tsgo_diagnostics(&stdout);
    if diagnostics.is_empty() {
        diagnostics = parse_tsgo_diagnostics(&stderr);
    }
    let relevant_diagnostics: Vec<Diagnostic> =
        diagnostics.into_iter().filter(is_relevant_error).collect();

    Ok(CheckResult {
        success: relevant_diagnostics.is_empty(),
        diagnostics: relevant_diagnostics,
    })
}

/// Filters errors to only include SDK-related issues that indicate the code won't run correctly.
///
/// This function ignores TypeScript errors that occur in valid JavaScript code that will run fine,
/// but TypeScript's strict mode complains about. We only care about errors that indicate
/// incorrect SDK usage or code that will fail.
fn is_relevant_error(diagnostic: &Diagnostic) -> bool {
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

/// Regex to match TypeScript error format
/// Example: "file.ts(1,19): error TS2322: Type 'string' is not assignable to type 'number'."
static ERROR_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_error_regex() -> &'static Regex {
    ERROR_REGEX.get_or_init(|| {
        Regex::new(r"(?m)^[^(]+\((\d+),(\d+)\):\s+error\s+TS(\d+):\s+(.+)$")
            .expect("ERROR_REGEX should be valid")
    })
}

/// Parse typescript-go diagnostic output
///
/// TypeScript outputs diagnostics in this format:
/// ```text
/// file.ts(1,19): error TS2322: Type 'string' is not assignable to type 'number'.
/// ```
fn parse_tsgo_diagnostics(stderr: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let error_regex = get_error_regex();

    for line in stderr.lines() {
        if let Some(captures) = error_regex.captures(line) {
            let line_num = captures.get(1).and_then(|m| m.as_str().parse().ok());
            let column_num = captures.get(2).and_then(|m| m.as_str().parse().ok());
            let error_code = captures.get(3).and_then(|m| m.as_str().parse().ok());
            let message = captures.get(4).unwrap().as_str().to_string();

            diagnostics.push(Diagnostic {
                message,
                line: line_num,
                column: column_num,
                severity: "error".to_string(),
                code: error_code,
            });
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_relevant_error_function() {
        // Test the is_relevant_error function directly

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
