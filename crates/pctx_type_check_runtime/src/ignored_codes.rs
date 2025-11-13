// Shared TypeScript diagnostic codes that are ignored during type checking
//
// These codes represent TypeScript errors that don't affect runtime execution
// and are safe to ignore when validating code for the PCTX runtime.

/// TypeScript diagnostic codes that should be ignored during type checking
///
/// This list is synchronized between:
/// - Rust: Used in `is_relevant_error()` to filter diagnostics
/// - JavaScript: Embedded in `type_check_runtime.js` to filter at the source
/// - Tests: Used to verify filtering behavior
///
/// Each code includes a comment explaining why it's ignored.
pub const IGNORED_DIAGNOSTIC_CODES: &[u32] = &[
    2307,  // Cannot find module - module resolution handled by runtime
    2304,  // Cannot find name 'require' - not used in ESM
    7016,  // Could not find declaration file - not needed for runtime
    2580,  // Cannot find name 'console' - provided by runtime
    2585,  // 'Promise' only refers to a type - provided by runtime
    2591,  // Cannot find name 'Promise' - provided by runtime
    2693,  // 'Array' only refers to a type - provided by runtime
    7006,  // Parameter implicitly has an 'any' type - JS compatibility
    7053,  // Element implicitly has an 'any' type - dynamic object access is valid
    7005,  // Variable implicitly has an 'any[]' type - JS compatibility
    7034,  // Variable implicitly has type 'any[]' - JS compatibility
    18046, // Variable is of type 'unknown' - reduce operations work at runtime
    2362,  // Left-hand side of arithmetic operation - runtime handles coercion
    2363,  // Right-hand side of arithmetic operation - runtime handles coercion
];

/// Get a human-readable description for why a diagnostic code is ignored
pub fn get_ignore_reason(code: u32) -> Option<&'static str> {
    match code {
        2307 => Some("Module resolution handled by runtime"),
        2304 => Some("require() not used in ESM"),
        7016 => Some("Declaration files not needed for runtime"),
        2580 => Some("console provided by runtime"),
        2585 => Some("Promise provided by runtime"),
        2591 => Some("Promise provided by runtime"),
        2693 => Some("Array provided by runtime"),
        7006 => Some("Implicit any allowed for JS compatibility"),
        7053 => Some("Dynamic object access is valid JS"),
        7005 => Some("Implicit any[] allowed for JS compatibility"),
        7034 => Some("Implicit any[] allowed for JS compatibility"),
        18046 => Some("Unknown type in reduce works at runtime"),
        2362 => Some("Runtime handles arithmetic coercion"),
        2363 => Some("Runtime handles arithmetic coercion"),
        _ => None,
    }
}
