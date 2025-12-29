use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use pctx_benchmarks::{McpBenchTask, load_tasks};
use pctx_executor::{ExecuteOptions, execute};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

async fn run_task_benchmark(task: &McpBenchTask) -> Duration {
    let start = std::time::Instant::now();

    // For now, we'll benchmark the executor with simple TypeScript code
    // In the future, this would execute the actual MCP bench task
    let code = format!(
        r#"
        // Task: {}
        const result = {{ success: true }};
        result;
        "#,
        task.description
    );

    let _result = execute(&code, ExecuteOptions::new()).await;

    start.elapsed()
}

fn benchmark_single_tasks(c: &mut Criterion) {
    let data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("mcpbench_tasks_single_runner_format.json");

    // Try to load tasks, skip benchmark if file doesn't exist
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let tasks = match runtime.block_on(load_tasks(&data_path)) {
        Ok(tasks) => tasks,
        Err(_) => {
            eprintln!(
                "Skipping benchmark - dataset not found. Run: cargo run --bin download_dataset"
            );
            return;
        }
    };

    let mut group = c.benchmark_group("mcp_bench_single");
    group.sample_size(10); // Smaller sample size for potentially long tasks
    group.measurement_time(Duration::from_secs(30));

    // Benchmark first 5 tasks to keep things reasonable
    for task in tasks.iter().take(5) {
        group.bench_with_input(BenchmarkId::new("task", &task.id), task, |b, task| {
            b.to_async(&runtime)
                .iter(|| async { black_box(run_task_benchmark(task).await) });
        });
    }

    group.finish();
}

fn benchmark_executor_baseline(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("executor_baseline");

    group.bench_function("simple_typescript", |b| {
        b.to_async(&runtime).iter(|| async {
            let code = r#"const x: number = 42; x * 2;"#;
            black_box(execute(code, ExecuteOptions::new()).await)
        });
    });

    group.bench_function("async_typescript", |b| {
        b.to_async(&runtime).iter(|| async {
            let code = r#"
                async function test() {
                    return Promise.resolve(42);
                }
                await test();
            "#;
            black_box(execute(code, ExecuteOptions::new()).await)
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_executor_baseline, benchmark_single_tasks);
criterion_main!(benches);
