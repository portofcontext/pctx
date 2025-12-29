use pctx_benchmarks::download_dataset;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");

    println!("Downloading MCP-Bench dataset to {}", data_dir.display());

    download_dataset(&data_dir).await?;

    println!("\nDataset download complete!");
    println!("Files are located in: {}", data_dir.display());

    Ok(())
}
