//! # PCTX Library
//!
//! Core library for Port of Context - an MCP client SDK for connecting to upstream
//! MCP servers and executing TypeScript code in a sandboxed environment.
//!
//! ## Overview
//!
//! This library provides:
//! - Connection to upstream MCP servers
//! - Introspection of available tools/functions
//! - Type-safe TypeScript code generation
//! - Sandboxed code execution with MCP tool access
//!
//! ## Rust API
//!
//! ```rust,no_run
//! use pctx_lib::{PctxClient, SdkConfig, ServerConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create configuration
//! let config = SdkConfig {
//!     servers: vec![
//!         // ServerConfig { name: "my-server", url: ... }
//!     ],
//! };
//!
//! // Create client
//! let mut client = PctxClient::new(config);
//!
//! // Or load from config file
//! let client = PctxClient::from_config("pctx.json")?;
//!
//! // Add upstream MCP server (example, would need proper configuration)
//! // let server = ServerConfig { ... };
//! // client.add_server(&server).await?;
//!
//! // List available functions
//! let functions = client.list_functions()?;
//! println!("Available functions:\n{}", functions);
//!
//! // Execute code
//! let code = r#"
//!     async function run() {
//!         const result = await MyServer.getData({ id: 42 });
//!         return result;
//!     }
//! "#;
//! let result = client.execute(code).await?;
//! println!("Result: {:?}", result.output);
//! # Ok(())
//! # }
//! ```
//!
//! ## Python SDK (via PyO3)
//!
//! **Design Document**: See `docs/python_sdk_design.py` for complete API design and examples
//!
//! **Use Case**: Add MCP tool execution to your Anthropic/OpenAI agent workflows
//!
//! ```python
//! from pctx import Pctx
//!
//! # Initialize with config file or dict
//! pctx = Pctx.from_config("pctx.json")
//!
//! # List available functions
//! functions = pctx.functions.list()
//!
//! # Execute TypeScript code with MCP access
//! result = pctx.execute("""
//!     async function run() {
//!         const data = await MyServer.getData({ id: 42 });
//!         return data;
//!     }
//! """)
//! ```
//!
//! **Installation** (when published):
//! ```bash
//! pip install pctx
//! ```
//!
//! ## TypeScript/JavaScript SDK (via napi-rs)
//!
//! **Design Document**: See `docs/typescript_sdk_design.ts` for complete API design and examples
//!
//! **Use Case**: Add MCP tool execution to your Node.js/TypeScript applications
//!
//! ```typescript
//! import { Pctx } from '@pctx/sdk';
//!
//! // Initialize with config file or object
//! const client = await Pctx.fromConfig('pctx.json');
//!
//! // List available functions
//! const functions = client.functions.list();
//!
//! // Execute TypeScript code with MCP access
//! const result = await client.execute(`
//!   async function run() {
//!     const data = await MyServer.getData({ id: 42 });
//!     return data;
//!   }
//! `);
//! ```
//!
//! **Installation** (when published):
//! ```bash
//! npm install @pctx/sdk
//! ```
//!
//! ## Key Features
//!
//! - **Cross-Language**: Same functionality available in Rust, Python, and JavaScript/TypeScript
//! - **Type-Safe**: Auto-generated TypeScript definitions, Python type hints
//! - **Async Support**: Both synchronous and asynchronous APIs where appropriate
//! - **MCP Integration**: Seamless connection to upstream MCP servers
//! - **Sandboxed Execution**: Safe TypeScript code execution with network controls
//! - **Performance**: Native speed with Rust core, minimal overhead
//!
//! ## Distribution
//!
//! - **Python**: Published to `PyPI` as `pctx` (binary wheels for major platforms)
//! - **JavaScript**: Published to npm as `@pctx/sdk` (native modules for major platforms)

mod client;
mod config;
mod upstream;

// Re-export main types
pub use client::{ExecuteInput, GetFunctionDetailsInput, PctxClient};
pub use config::{SdkConfig, ServerConfig};
pub use upstream::{UpstreamMcp, UpstreamTool};

// Re-export CLI config for compatibility with pctx CLI
pub use pctx_config::Config as CliConfig;

// Re-export execution result
pub use deno_executor::ExecuteResult;
