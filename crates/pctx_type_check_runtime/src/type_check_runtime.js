// PCTX Type Check Runtime - TypeScript Compiler Integration
//
// This runtime provides TypeScript type checking using the bundled TypeScript compiler.
// The typescript.min.js file is loaded before this script runs.

// Import the TypeScript compiler (already loaded by the extension)
import * as tsModule from "ext:pctx_type_check_snapshot/typescript.min.js";

// CODEGEN_IGNORED_CODES_PLACEHOLDER
// This placeholder is replaced at build time with the actual ignored diagnostic codes
// from src/ignored_codes.rs, ensuring Rust and JavaScript stay in sync.
//
// NOTE: TS_LIBS is injected as a global variable by the Rust runtime before
// calling typeCheckCode(). It contains all TypeScript lib.d.ts files.

// Access ts from the imported module or globalThis
const ts = tsModule.ts || tsModule.default || globalThis.ts;

if (!ts) {
  throw new Error("TypeScript compiler not loaded properly");
}

// Minimal lib.d.ts definitions for runtime environment
const LIB_DENO_NS = `
declare namespace Deno {
  export const core: any;
}

interface MCPToolProps {
  serverName: string;
  toolName: string;
  arguments?: any;
}
interface InvokeCallbackProps {
  id: string;
  arguments?: any;
}

declare function callMCPTool<T = any>(call: MCPToolProps): Promise<T>;
declare function invokeCallback<T = any>(call: InvokeCallbackProps): Promise<T>;

// Console API (from lib.dom.d.ts, but needed for runtime)
interface Console {
  log(...data: any[]): void;
  error(...data: any[]): void;
  warn(...data: any[]): void;
  info(...data: any[]): void;
  debug(...data: any[]): void;
  trace(...data: any[]): void;
  assert(condition?: boolean, ...data: any[]): void;
  clear(): void;
  count(label?: string): void;
  countReset(label?: string): void;
  dir(item?: any, options?: any): void;
  dirxml(...data: any[]): void;
  group(...data: any[]): void;
  groupCollapsed(...data: any[]): void;
  groupEnd(): void;
  table(tabularData?: any, properties?: string[]): void;
  time(label?: string): void;
  timeEnd(label?: string): void;
  timeLog(label?: string, ...data: any[]): void;
  timeStamp(label?: string): void;
}

declare var console: Console;
`;

/**
 * Type check TypeScript code using the full TypeScript compiler
 *
 * @param {string} code - The TypeScript code to check
 * @returns {{success: boolean, diagnostics: Array<{message: string, line?: number, column?: number, severity: string, code?: number}>}}
 */
function typeCheckCode(code) {
  const diagnostics = [];

  try {
    // Create a virtual file system for the TypeScript compiler
    const fileName = "check.ts";
    const files = new Map();
    files.set(fileName, code);

    // Add all TypeScript lib files to the virtual file system
    for (const [libName, libContent] of Object.entries(TS_LIBS)) {
      files.set(libName, libContent);
    }

    // Add custom lib.deno.d.ts AFTER TypeScript libs (to allow augmentation)
    files.set("lib.deno.d.ts", LIB_DENO_NS);

    // Create a custom compiler host
    const compilerHost = {
      getSourceFile: (fileName, languageVersion) => {
        const sourceText = files.get(fileName);
        if (sourceText !== undefined) {
          return ts.createSourceFile(
            fileName,
            sourceText,
            languageVersion,
            true,
          );
        }
        // Return undefined for files we don't have
        return undefined;
      },
      getDefaultLibFileName: () => "lib.es2020.d.ts",
      writeFile: () => {},
      getCurrentDirectory: () => "/",
      getDirectories: () => [],
      fileExists: (fileName) => files.has(fileName),
      readFile: (fileName) => files.get(fileName),
      getCanonicalFileName: (fileName) => fileName,
      useCaseSensitiveFileNames: () => true,
      getNewLine: () => "\n",
    };

    // TODO: more granular control over type check strictness
    const program = ts.createProgram({
      rootNames: [fileName, "lib.deno.d.ts"],
      options: {
        target: ts.ScriptTarget.ES2020,
        module: ts.ModuleKind.ES2020,
        strict: true,
        noEmit: true,
        skipLibCheck: false,
        noLib: false,
      },
      host: compilerHost,
    });

    // Get all diagnostics
    const allDiagnostics = [
      ...program.getSyntacticDiagnostics(),
      ...program.getSemanticDiagnostics(),
    ];

    // Filter and format diagnostics
    // NOTE: IGNORED_DIAGNOSTIC_CODES is generated at build time from src/ignored_codes.rs
    // to ensure Rust and JavaScript stay in sync
    for (const diagnostic of allDiagnostics) {
      // Skip diagnostic codes that are not relevant for runtime execution
      if (IGNORED_DIAGNOSTIC_CODES.includes(diagnostic.code)) continue;

      let message = ts.flattenDiagnosticMessageText(
        diagnostic.messageText,
        "\n",
      );
      let line = undefined;
      let column = undefined;

      if (diagnostic.file && diagnostic.start !== undefined) {
        const pos = diagnostic.file.getLineAndCharacterOfPosition(
          diagnostic.start,
        );
        line = pos.line + 1;
        column = pos.character + 1;
      }

      diagnostics.push({
        message,
        line,
        column,
        severity:
          diagnostic.category === ts.DiagnosticCategory.Error
            ? "error"
            : "warning",
        code: diagnostic.code,
      });
    }
  } catch (error) {
    // If type checking fails, return an internal error
    diagnostics.push({
      message: `Internal type check error: ${error.message}`,
      line: undefined,
      column: undefined,
      severity: "error",
      code: undefined,
    });
  }

  return {
    success: diagnostics.length === 0,
    diagnostics,
  };
}

// Make the type checking function available globally
globalThis.typeCheckCode = typeCheckCode;
