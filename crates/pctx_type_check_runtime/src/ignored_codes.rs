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
pub(crate) const IGNORED_DIAGNOSTIC_CODES: &[u32] = &[
    2307,  // Cannot find module - module resolution handled by runtime
    2304,  // Cannot find name 'require' - not used in ESM
    7016,  // Could not find declaration file - not needed for runtime
    2318,  // Cannot find global type 'Promise' - provided by runtime
    2580,  // Cannot find name 'console' - provided by runtime
    2583,  // Cannot find name 'Promise' (with lib suggestion) - provided by runtime
    2584,  // Cannot find name 'console' (with dom suggestion) - provided by runtime
    2585,  // 'Promise' only refers to a type - provided by runtime
    2591,  // Cannot find name 'Promise' - provided by runtime
    2339,  // Property does not exist on type - runtime provides full prototypes
    2693,  // 'Array' only refers to a type - provided by runtime
    7006,  // Parameter implicitly has an 'any' type - JS compatibility
    7053,  // Element implicitly has an 'any' type - dynamic object access is valid
    7005,  // Variable implicitly has an 'any[]' type - JS compatibility
    7034,  // Variable implicitly has type 'any[]' - JS compatibility
    18046, // Variable is of type 'unknown' - reduce operations work at runtime
    2362,  // Left-hand side of arithmetic operation - runtime handles coercion
    2363,  // Right-hand side of arithmetic operation - runtime handles coercion
];
