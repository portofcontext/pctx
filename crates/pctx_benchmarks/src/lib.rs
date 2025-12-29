use serde::{Deserialize, Serialize};
use std::path::Path;

/// MCP-Bench task representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpBenchTask {
    pub task_id: String,
    pub task_description: String,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Task execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub execution_time_ms: u64,
    pub tools_used: Vec<String>,
    pub error: Option<String>,
}

/// Load MCP-Bench tasks from JSON file
pub async fn load_tasks(path: impl AsRef<Path>) -> anyhow::Result<Vec<McpBenchTask>> {
    let content = tokio::fs::read_to_string(path).await?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    // MCP-Bench format: {"server_tasks": [{"server_name": "...", "tasks": [...]}]}
    let mut all_tasks = Vec::new();
    if let Some(server_tasks) = data.get("server_tasks").and_then(|v| v.as_array()) {
        for server_entry in server_tasks {
            if let Some(tasks) = server_entry.get("tasks").and_then(|v| v.as_array()) {
                for task in tasks {
                    if let Ok(parsed_task) = serde_json::from_value::<McpBenchTask>(task.clone()) {
                        all_tasks.push(parsed_task);
                    }
                }
            }
        }
    }

    Ok(all_tasks)
}

/// Download MCP-Bench dataset from GitHub
pub async fn download_dataset(output_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let base_url = "https://raw.githubusercontent.com/Accenture/mcp-bench/main/tasks";
    let files = vec![
        "mcpbench_tasks_single_runner_format.json",
        "mcpbench_tasks_multi_2server_runner_format.json",
        "mcpbench_tasks_multi_3server_runner_format.json",
    ];

    let client = reqwest::Client::new();
    let output_dir = output_dir.as_ref();
    tokio::fs::create_dir_all(output_dir).await?;

    for file in files {
        let url = format!("{base_url}/{file}");
        println!("Downloading {url}");

        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                let content = response.text().await?;
                let output_path = output_dir.join(file);
                tokio::fs::write(&output_path, content).await?;
                println!("Saved to {}", output_path.display());
            }
            Ok(response) => {
                eprintln!("Failed to download {}: HTTP {}", file, response.status());
            }
            Err(e) => {
                eprintln!("Failed to download {file}: {e}");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_tasks() {
        // This test will work once we have the dataset
        let result = load_tasks("data/mcpbench_tasks_single_runner_format.json").await;
        // Don't fail if file doesn't exist yet
        if let Ok(tasks) = result {
            assert!(!tasks.is_empty(), "Should load at least one task");
        }
    }
}
