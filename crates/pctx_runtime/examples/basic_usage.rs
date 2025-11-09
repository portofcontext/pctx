//! Basic example of using pctx_runtime in another crate
//!
//! This example shows how to:
//! 1. Create a Deno runtime with the pctx_runtime extension
//! 2. Register an MCP server
//! 3. Use the global APIs from JavaScript
//!
//! Run with: cargo run --example basic_usage

use pctx_runtime::MCPRegistry;
use deno_core::{JsRuntime, RuntimeOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new MCP registry
    let registry = MCPRegistry::new();

    // Create a Deno runtime with the pctx_runtime extension
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![pctx_runtime::pctx_runtime::init(registry)],
        ..Default::default()
    });

    // Execute some JavaScript that uses the MCP client
    let code = r#"
        // Register an MCP server (would fail to connect, but demonstrates the API)
        registerMCP({
            name: "example-server",
            url: "http://localhost:3000"
        });

        // Check if it was registered
        const isRegistered = REGISTRY.has("example-server");
        console.log("Server registered:", isRegistered);

        // Get the configuration
        const config = REGISTRY.get("example-server");
        console.log("Server config:", JSON.stringify(config));

        // Return the result
        isRegistered
    "#;

    let result = runtime.execute_script("<example>", code)?;

    // Resolve any promises and run the event loop
    let resolved = runtime.resolve(result).await?;

    // Extract the boolean result
    let is_registered = {
        deno_core::scope!(scope, &mut runtime);
        let local = deno_core::v8::Local::new(scope, resolved);
        deno_core::serde_v8::from_v8::<bool>(scope, local)?
    };

    println!("✓ Successfully used pctx_runtime from example");
    println!("✓ MCP server registration: {}", is_registered);

    Ok(())
}
