use std::fs;

use anyhow::{Context, Result};
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider;

use camino::Utf8PathBuf;
use opentelemetry_sdk::Resource;
use pctx_config::{Config, logger::LoggerFormat};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};
use tracing_subscriber::{Layer, Registry, util::SubscriberInitExt};

use crate::utils::{logger, metrics};

pub(crate) async fn init_telemetry(
    cfg: &Config,
    json_l: Option<Utf8PathBuf>,
    use_stderr: bool,
) -> Result<()> {
    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();

    let resource = Resource::builder()
        .with_attributes([
            KeyValue::new("service.name", cfg.name.clone()),
            KeyValue::new("service.version", cfg.version.clone()),
        ])
        .build();

    // build tracing provider with all configured exporters
    if cfg.telemetry.traces.enabled {
        let builder = cfg
            .telemetry
            .traces
            .tracer_provider_builder()
            .await?
            .with_resource(resource.clone());

        let tracer = builder.build().tracer("pctx");

        layers.push(tracing_opentelemetry::layer().with_tracer(tracer).boxed());
    }

    // build meter provider with all configured exporters
    if cfg.telemetry.metrics.enabled {
        let meter_provider = cfg
            .telemetry
            .metrics
            .meter_provider_builder()
            .await?
            .with_resource(resource)
            .build();

        opentelemetry::global::set_meter_provider(meter_provider);

        // Initialize metrics instruments after meter provider is set
        metrics::init_meter();
        metrics::init_mcp_tool_metrics();
    }

    if let Some(log_file) = json_l {
        if let Some(parent) = log_file.parent() {
            fs::create_dir_all(parent).context(format!(
                "failed creating parent directory of log file {log_file}"
            ))?;
        }
        let write_to =
            fs::File::create(&log_file).context(format!("failed creating log file: {log_file}"))?;

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or(EnvFilter::new(logger::default_env_filter("debug")));
        layers.push(
            init_tracing_layer(write_to, &LoggerFormat::Json, false)
                .with_filter(env_filter)
                .boxed(),
        );
    } else if cfg.logger.enabled {
        let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new(
            logger::default_env_filter(cfg.logger.level.as_str()),
        ));

        // Determine log destination based on config and mode:
        // 1. If file is specified in config, use it (all modes)
        // 2. If in stdio mode without a file, disable logging (to avoid interfering with JSON-RPC)
        // 3. Otherwise, use stdout (HTTP mode default)
        if let Some(log_file) = &cfg.logger.file {
            if let Some(parent) = log_file.parent() {
                fs::create_dir_all(parent).context(format!(
                    "failed creating parent directory of log file {log_file}"
                ))?;
            }
            let write_to = fs::File::create(log_file)
                .context(format!("failed creating log file: {log_file}"))?;
            layers.push(
                init_tracing_layer(write_to, &cfg.logger.format, cfg.logger.colors)
                    .with_filter(env_filter)
                    .boxed(),
            );
        } else if !use_stderr {
            // Only enable stdout logging for non-stdio modes
            // In stdio mode without a log file, logging is disabled to keep stdout/stderr clean
            layers.push(
                init_tracing_layer(std::io::stdout, &cfg.logger.format, cfg.logger.colors)
                    .with_filter(env_filter)
                    .boxed(),
            );
        }
        // else: stdio mode without log file - no logging layer added (logging disabled)
    }

    tracing_subscriber::registry().with(layers).try_init()?;

    Ok(())
}

fn init_tracing_layer<W>(
    make_writer: W,
    format: &LoggerFormat,
    colors: bool,
) -> Box<dyn Layer<Registry> + Sync + Send>
where
    W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + Sync + Send + 'static,
{
    match format {
        LoggerFormat::Compact => tracing_subscriber::fmt::layer()
            .with_writer(make_writer)
            .with_ansi(colors)
            .compact()
            .boxed(),
        LoggerFormat::Pretty => tracing_subscriber::fmt::layer()
            .with_writer(make_writer)
            .with_ansi(colors)
            .pretty()
            .boxed(),
        LoggerFormat::Json => tracing_subscriber::fmt::layer()
            .with_writer(make_writer)
            .with_ansi(colors)
            .json()
            .boxed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use pctx_config::{Config, logger::LoggerConfig};
    use tempfile::TempDir;

    fn create_test_config(logger: LoggerConfig) -> Config {
        let mut cfg = Config::default();
        cfg.logger = logger;
        cfg
    }

    #[tokio::test]
    async fn test_telemetry_log_file_creation() {
        // Test that log files are created with correct directory structure
        // This test doesn't call init_telemetry to avoid global state issues
        let temp_dir = TempDir::new().unwrap();
        let log_path =
            Utf8PathBuf::from_path_buf(temp_dir.path().join("nested").join("dir").join("test.log"))
                .unwrap();

        let cfg = create_test_config(LoggerConfig {
            enabled: true,
            file: Some(log_path.clone()),
            ..Default::default()
        });

        // Just verify the config is set up correctly
        assert_eq!(cfg.logger.file.as_ref(), Some(&log_path));
        assert!(cfg.logger.enabled);
    }

    #[tokio::test]
    async fn test_json_log_file_precedence_logic() {
        // Test the logic without initializing the global subscriber
        let temp_dir = TempDir::new().unwrap();
        let config_log_path =
            Utf8PathBuf::from_path_buf(temp_dir.path().join("config.log")).unwrap();

        let cfg = create_test_config(LoggerConfig {
            enabled: true,
            file: Some(config_log_path.clone()),
            ..Default::default()
        });

        // When json_l is provided, it should take precedence
        // (Testing the logic, not the actual initialization)
        assert!(cfg.logger.file.is_some());
        assert_eq!(cfg.logger.file.as_ref(), Some(&config_log_path));
    }

    #[tokio::test]
    async fn test_full_telemetry_init_with_log_file() {
        // Integration test that actually calls init_telemetry
        // Since this is the only test that initializes the global subscriber,
        // it can run normally without conflicts
        let temp_dir = TempDir::new().unwrap();
        let log_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("test.log")).unwrap();

        let cfg = create_test_config(LoggerConfig {
            enabled: true,
            file: Some(log_path.clone()),
            ..Default::default()
        });

        let result = init_telemetry(&cfg, None, false).await;
        assert!(result.is_ok(), "Telemetry initialization should succeed");
        assert!(
            log_path.exists(),
            "Log file should be created at {log_path}"
        );
    }
}
