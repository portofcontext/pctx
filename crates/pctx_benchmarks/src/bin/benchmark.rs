use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "benchmark")]
#[command(about = "Run MCP-Bench benchmarks with pctx", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download MCP-Bench dataset
    Download,

    /// Run MCP-Bench with LLM (requires Python and pctx-py)
    Mcp {
        /// OpenRouter API key (can also use OPENROUTER_API_KEY env var)
        #[arg(long)]
        openrouter_key: Option<String>,

        /// Model to use
        #[arg(long, default_value = "deepseek/deepseek-chat")]
        model: String,

        /// Dataset to use
        #[arg(long, default_value = "data/mcpbench_tasks_single_runner_format.json")]
        dataset: String,

        /// Maximum number of tasks to run
        #[arg(long, default_value = "5")]
        max_tasks: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Download => {
            println!("Downloading MCP-Bench dataset...");
            let status = Command::new("cargo")
                .args(["run", "--bin", "download_dataset", "-p", "pctx_benchmarks"])
                .status()?;

            if !status.success() {
                anyhow::bail!("Failed to download dataset");
            }
        }
        Commands::Mcp {
            openrouter_key,
            model,
            dataset,
            max_tasks,
        } => {
            let openrouter_key = openrouter_key
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .expect(
                    "OpenRouter API key required (--openrouter-key or OPENROUTER_API_KEY env var)",
                );

            let script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("scripts")
                .join("run_mcp_bench.py");

            if !script_path.exists() {
                anyhow::bail!("Python script not found at: {}", script_path.display());
            }

            // Check and install Python dependencies if needed
            println!("Checking Python dependencies...");
            let check_status = Command::new("python3")
                .args(["-c", "import pctx_client; from langchain_openai import ChatOpenAI; from langchain_core.messages import HumanMessage"])
                .output();

            match check_status {
                Ok(output) if !output.status.success() => {
                    println!("Installing Python dependencies...");
                    let install_status = Command::new("pip")
                        .args(["install", "-q", "pctx", "langchain-openai", "langchain-core"])
                        .status()?;

                    if !install_status.success() {
                        anyhow::bail!("Failed to install Python dependencies");
                    }
                    println!("✓ Dependencies installed\n");
                }
                Err(_) => {
                    eprintln!("\n❌ Error: python3 not found in PATH.");
                    anyhow::bail!("Python 3.8+ required");
                }
                _ => {
                    println!("✓ Dependencies OK\n");
                }
            }
            println!("Running MCP-Bench with model: {}", model);
            println!("Dataset: {}", dataset);
            println!("Max tasks: {}\n", max_tasks);

            let status = Command::new("python3")
                .arg(&script_path)
                .arg("--openrouter-key")
                .arg(&openrouter_key)
                .arg("--model")
                .arg(&model)
                .arg("--dataset")
                .arg(&dataset)
                .arg("--max-tasks")
                .arg(max_tasks.to_string())
                .status()?;

            if !status.success() {
                anyhow::bail!("Benchmark failed");
            }
        }
    }

    Ok(())
}
