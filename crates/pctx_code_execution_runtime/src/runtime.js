// PCTX Runtime - MCP Client and Console Capturing

// Import timers first - sets up setTimeout/setInterval globals
import "ext:pctx_runtime_snapshot/timers.js";

// Now import just-bash (can use setTimeout)
import { Bash } from "ext:pctx_runtime_snapshot/just-bash/bundle.js";

const core = Deno.core;
const ops = core.ops;

// Debug: log available ops
const availableOps = Object.keys(ops).filter((k) => k.startsWith("op_"));
if (availableOps.length === 0) {
  console.error(
    "WARNING: No ops available! This indicates an extension loading issue.",
  );
} else {
  // Only log MCP-related ops to reduce noise
  const mcpOps = availableOps.filter((k) => k.includes("mcp"));
  if (mcpOps.length > 0) {
    console.log("Available MCP ops:", mcpOps);
  }
}

// ============================================================================
// CONSOLE OUTPUT CAPTURING
// ============================================================================

// Helper function to format console arguments
function formatConsoleArgs(...args) {
  return args
    .map((arg) => {
      if (typeof arg === "string") return arg;
      if (arg === null) return "null";
      if (arg === undefined) return "undefined";
      try {
        return JSON.stringify(arg);
      } catch {
        return String(arg);
      }
    })
    .join(" ");
}

// Set up console output capturing
globalThis.__stdout = [];
globalThis.__stderr = [];

// Override console.log to capture stdout
console.log = (...args) => {
  globalThis.__stdout.push(formatConsoleArgs(...args));
};

// Override console.error to capture stderr
console.error = (...args) => {
  globalThis.__stderr.push(formatConsoleArgs(...args));
};

// console.warn goes to stderr
console.warn = (...args) => {
  globalThis.__stderr.push(formatConsoleArgs(...args));
};

// console.info and console.debug go to stdout
console.info = (...args) => {
  globalThis.__stdout.push(formatConsoleArgs(...args));
};

console.debug = (...args) => {
  globalThis.__stdout.push(formatConsoleArgs(...args));
};

// ============================================================================
// MCP & Callback Operations
// ============================================================================

/**
 * Call an MCP tool
 * @template T
 * @param {Object} call - Tool call configuration
 * @param {string} call.serverName - Name of the registered MCP server
 * @param {string} call.toolName - Name of the registered tool to call
 * @param {Object?} [call.arguments] - Arguments to pass to the tool
 * @returns {Promise<T>} The tool's response
 */
export async function callMCPTool(call) {
  return await ops.op_call_mcp_tool(
    call.serverName,
    call.toolName,
    call.arguments,
  );
}

/**
 * Call an MCP tool
 * @template T
 * @param {Object} call - Tool call configuration
 * @param {string} call.id - ID of the callback
 * @param {Object?} [call.arguments] - Arguments to pass to the callback
 * @returns {Promise<T>} The tool's response
 */
export async function invokeCallback(call) {
  return await ops.op_invoke_callback(call.id, call.arguments);
}

/**
 * Fetch with host-based permissions
 * @param {string} url - URL to fetch
 * @param {Object} [options] - Fetch options (method, headers, body)
 * @returns {Promise<{status: number, headers: Object, body: string}>}
 */
async function fetch(url, options) {
  return await ops.op_fetch(url, options);
}

// Make APIs available globally for convenience (matching original behavior)
globalThis.callMCPTool = callMCPTool;
globalThis.invokeCallback = invokeCallback;
globalThis.fetch = fetch;
globalThis.justBash = Bash; // lowercase to avoid any clashes with namespaces
