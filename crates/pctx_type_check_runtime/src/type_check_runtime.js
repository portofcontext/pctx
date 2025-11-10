// PCTX Type Check Runtime - TypeScript Compiler Integration
//
// This runtime provides TypeScript type checking using the bundled TypeScript compiler.
// The typescript.min.js file is loaded before this script runs.

// Import the TypeScript compiler (already loaded by the extension)
import * as tsModule from "ext:pctx_type_check_snapshot/typescript.min.js";

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

interface MCPServerConfig {
  name: string;
  url: string;
}

interface MCPToolCall {
  name: string;
  tool: string;
  arguments?: any;
}

declare function registerMCP(config: MCPServerConfig): void;
declare function callMCPTool<T = any>(call: MCPToolCall): Promise<T>;

declare const REGISTRY: {
  has(name: string): boolean;
  get(name: string): MCPServerConfig | undefined;
  delete(name: string): boolean;
  clear(): void;
};
`;

const LIB_ES_BASIC = `
interface Console {
  log(...args: any[]): void;
  error(...args: any[]): void;
  warn(...args: any[]): void;
  info(...args: any[]): void;
  debug(...args: any[]): void;
}
declare const console: Console;

interface PromiseConstructor {
  new <T>(executor: (resolve: (value: T) => void, reject: (reason?: any) => void) => void): Promise<T>;
  resolve<T>(value: T | PromiseLike<T>): Promise<T>;
  reject<T = never>(reason?: any): Promise<T>;
  all<T>(values: Iterable<T | PromiseLike<T>>): Promise<Awaited<T>[]>;
  race<T>(values: Iterable<T | PromiseLike<T>>): Promise<Awaited<T>>;
}
declare const Promise: PromiseConstructor;

interface Promise<T> {
  then<TResult1 = T, TResult2 = never>(
    onFulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | null,
    onRejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | null
  ): Promise<TResult1 | TResult2>;
  catch<TResult = never>(
    onRejected?: ((reason: any) => TResult | PromiseLike<TResult>) | null
  ): Promise<T | TResult>;
  finally(onFinally?: (() => void) | null): Promise<T>;
}

interface PromiseLike<T> {
  then<TResult1 = T, TResult2 = never>(
    onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | null,
    onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | null
  ): PromiseLike<TResult1 | TResult2>;
}

type Awaited<T> = T extends PromiseLike<infer U> ? U : T;

interface Iterable<T> {
  [Symbol.iterator](): Iterator<T>;
}

interface Iterator<T, TReturn = any, TNext = undefined> {
  next(...args: [] | [TNext]): IteratorResult<T, TReturn>;
  return?(value?: TReturn): IteratorResult<T, TReturn>;
  throw?(e?: any): IteratorResult<T, TReturn>;
}

interface IteratorResult<T, TReturn = any> {
  done: boolean;
  value: T | TReturn;
}

interface Symbol {
  readonly [Symbol.toStringTag]: string;
}

interface SymbolConstructor {
  readonly iterator: symbol;
  readonly toStringTag: symbol;
}

declare const Symbol: SymbolConstructor;

interface Array<T> extends Iterable<T> {
  length: number;
  push(...items: T[]): number;
  pop(): T | undefined;
  shift(): T | undefined;
  unshift(...items: T[]): number;
  map<U>(callbackfn: (value: T, index: number, array: T[]) => U, thisArg?: any): U[];
  filter(predicate: (value: T, index: number, array: T[]) => unknown, thisArg?: any): T[];
  reduce<U>(callbackfn: (previousValue: U, currentValue: T, currentIndex: number, array: T[]) => U, initialValue: U): U;
  forEach(callbackfn: (value: T, index: number, array: T[]) => void, thisArg?: any): void;
  find(predicate: (value: T, index: number, obj: T[]) => unknown, thisArg?: any): T | undefined;
  some(predicate: (value: T, index: number, array: T[]) => unknown, thisArg?: any): boolean;
  every(predicate: (value: T, index: number, array: T[]) => unknown, thisArg?: any): boolean;
  join(separator?: string): string;
  slice(start?: number, end?: number): T[];
  [Symbol.iterator](): Iterator<T>;
}

type Record<K extends string | number | symbol, T> = {
  [P in K]: T;
};

interface Response {
  ok: boolean;
  status: number;
  statusText: string;
  headers: any;
  text(): Promise<string>;
  json(): Promise<any>;
}

declare function fetch(url: string, init?: any): Promise<Response>;

interface Error {
  name: string;
  message: string;
  stack?: string;
}

interface ErrorConstructor {
  new (message?: string): Error;
  (message?: string): Error;
  readonly prototype: Error;
}

declare const Error: ErrorConstructor;

interface JSON {
  parse(text: string, reviver?: (key: any, value: any) => any): any;
  stringify(value: any, replacer?: (key: string, value: any) => any, space?: string | number): string;
}

declare const JSON: JSON;

type PropertyKey = string | number | symbol;

interface Object {
  toString(): string;
  valueOf(): Object;
  hasOwnProperty(v: PropertyKey): boolean;
}

interface ObjectConstructor {
  new (value?: any): Object;
  (value?: any): any;
  keys(o: object): string[];
  values(o: object): any[];
  entries(o: object): [string, any][];
  assign<T, U>(target: T, source: U): T & U;
}

declare const Object: ObjectConstructor;
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
    files.set("lib.deno.d.ts", LIB_DENO_NS);
    files.set("lib.es.d.ts", LIB_ES_BASIC);

    // Create a custom compiler host
    const compilerHost = {
      getSourceFile: (fileName, languageVersion) => {
        const sourceText = files.get(fileName);
        if (sourceText !== undefined) {
          return ts.createSourceFile(fileName, sourceText, languageVersion, true);
        }
        // Return undefined for files we don't have
        return undefined;
      },
      getDefaultLibFileName: () => "lib.es.d.ts",
      writeFile: () => { },
      getCurrentDirectory: () => "/",
      getDirectories: () => [],
      fileExists: (fileName) => files.has(fileName),
      readFile: (fileName) => files.get(fileName),
      getCanonicalFileName: (fileName) => fileName,
      useCaseSensitiveFileNames: () => true,
      getNewLine: () => "\n",
    };

    // Create a program
    const program = ts.createProgram({
      rootNames: [fileName, "lib.es.d.ts", "lib.deno.d.ts"],
      options: {
        target: ts.ScriptTarget.ES2020,
        module: ts.ModuleKind.ES2020,
        strict: true,
        noEmit: true,
        skipLibCheck: false,
        noLib: true,
      },
      host: compilerHost,
    });

    // Get all diagnostics
    const allDiagnostics = [
      ...program.getSyntacticDiagnostics(),
      ...program.getSemanticDiagnostics(),
    ];

    // Filter and format diagnostics
    for (const diagnostic of allDiagnostics) {
      // Skip certain diagnostic codes that are not relevant for runtime execution
      if (diagnostic.code === 2580) continue; // "Cannot find name 'console'"
      if (diagnostic.code === 2584) continue; // "Cannot find name 'console'. Do you need to change your target library?"
      if (diagnostic.code === 2583) continue; // "Cannot find name 'Promise'"
      if (diagnostic.code === 2585) continue; // "Cannot find name 'AsyncIterable'"
      if (diagnostic.code === 2591) continue; // "Cannot find name 'AsyncIterator'"
      if (diagnostic.code === 2318) continue; // "Cannot find global type 'Promise'"
      if (diagnostic.code === 7006) continue; // "Parameter implicitly has an 'any' type"
      if (diagnostic.code === 7053) continue; // "Element implicitly has an 'any' type" (dynamic object access)
      if (diagnostic.code === 18046) continue; // "'e' is of type 'unknown'"
      if (diagnostic.code === 2304 && diagnostic.messageText.toString().includes("console")) continue;

      let message = ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
      let line = undefined;
      let column = undefined;

      if (diagnostic.file && diagnostic.start !== undefined) {
        const pos = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
        line = pos.line + 1;
        column = pos.character + 1;
      }

      diagnostics.push({
        message,
        line,
        column,
        severity: diagnostic.category === ts.DiagnosticCategory.Error ? "error" : "warning",
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
