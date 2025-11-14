// PCTX Runtime - MCP Client and Console Capturing

const core = Deno.core;
const ops = core.ops;

// Debug: log available ops
const availableOps = Object.keys(ops).filter(k => k.startsWith('op_'));
if (availableOps.length === 0) {
    console.error("WARNING: No ops available! This indicates an extension loading issue.");
} else {
    // Only log MCP-related ops to reduce noise
    const mcpOps = availableOps.filter(k => k.includes('mcp'));
    if (mcpOps.length > 0) {
        console.log("Available MCP ops:", mcpOps);
    }
}

// ============================================================================
// CONSOLE OUTPUT CAPTURING
// ============================================================================

// Helper function to format console arguments
function formatConsoleArgs(...args) {
    return args.map(arg => {
        if (typeof arg === 'string') return arg;
        if (arg === null) return 'null';
        if (arg === undefined) return 'undefined';
        try {
            return JSON.stringify(arg);
        } catch {
            return String(arg);
        }
    }).join(' ');
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
// MCP CLIENT API
// ============================================================================

/**
 * Register an MCP server
 * @param {Object} config - MCP server configuration
 * @param {string} config.name - Unique name for the MCP server
 * @param {string} config.url - URL of the MCP server
 */
export function registerMCP(config) {
    return ops.op_register_mcp(config);
}

/**
 * Call an MCP tool
 * @template T
 * @param {Object} call - Tool call configuration
 * @param {string} call.name - Name of the registered MCP server
 * @param {string} call.tool - Name of the tool to call
 * @param {Object} [call.arguments] - Arguments to pass to the tool
 * @returns {Promise<T>} The tool's response
 */
export async function callMCPTool(call) {
    return await ops.op_call_mcp_tool(call);
}

/**
 * MCP Registry singleton - provides access to registered servers
 */
export const REGISTRY = {
    /**
     * Check if an MCP server is registered
     * @param {string} name - Name of the MCP server
     * @returns {boolean} True if registered
     */
    has(name) {
        return ops.op_mcp_has(name);
    },

    /**
     * Get an MCP server configuration
     * @param {string} name - Name of the MCP server
     * @returns {Object|undefined} Server configuration or undefined
     */
    get(name) {
        return ops.op_mcp_get(name);
    },

    /**
     * Delete an MCP server configuration
     * @param {string} name - Name of the MCP server
     * @returns {boolean} True if deleted, false if not found
     */
    delete(name) {
        return ops.op_mcp_delete(name);
    },

    /**
     * Clear all MCP server configurations
     */
    clear() {
        ops.op_mcp_clear();
    }
};

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
globalThis.registerMCP = registerMCP;
globalThis.callMCPTool = callMCPTool;
globalThis.REGISTRY = REGISTRY;
globalThis.fetch = fetch;
