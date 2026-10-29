# pctx OpenTelemetry Example

This directory contains a complete setup for testing pctx's OpenTelemetry (OTLP) functionality with traces and metrics collection.

## Overview

The Docker Compose setup provides:

- **OpenTelemetry Collector**: Receives OTLP data from pctx and exports to downstream services
- **Tempo**: Distributed tracing backend for storing and querying traces
- **Prometheus**: Metrics storage and querying
- **Grafana**: Visualization dashboard for traces and metrics
- **Memcached**: Cache for Tempo

## Getting Started

### 1. Start the Telemetry Stack

```bash
cd examples/telemetry
docker compose up -d
```

This will start all services:

- OTLP Collector: `http://localhost:4318` (HTTP) and `localhost:4317` (gRPC)
- Grafana: `http://localhost:3000`
- Prometheus: `http://localhost:9090`
- Tempo: `http://localhost:3200`

### 2. Configure pctx

Create a pctx configuration file with telemetry enabled. Here's an example configuration:

```json
{
  "name": "pctx-opentelemetry",
  "version": "0.1.0",
  "logger": {
    "enabled": true,
    "format": "pretty",
    "level": "info"
  },
  "telemetry": {
    "traces": {
      "enabled": true,
      "exporters": [
        {
          "name": "tempo",
          "url": "http://localhost:4318/v1/traces",
          "protocol": "http"
        }
      ]
    },
    "metrics": {
      "enabled": true,
      "exporters": [
        {
          "name": "prometheus",
          "url": "http://localhost:4318/v1/metrics",
          "protocol": "http"
        }
      ]
    }
  }
}
```

### 3. Run pctx

Start the pctx MCP server with your configuration file:

```bash
pctx code-mode start --config path/to/your/pctx.json
```

**_Note: you can also configure the pctx session server (`pctx code-mode serve` to export OpenTelemetry) with the same config, just `servers` will be ignored_**

### 4. View Telemetry Data

#### Grafana Dashboard

1. Open [http://localhost:3000](http://localhost:3000) in your browser
2. Navigate to "Explore" in the left sidebar
3. Select "Tempo" as the data source to view traces
4. Select "Prometheus" to view metrics (when enabled)

#### Prometheus

Direct access to metrics: [http://localhost:9090](http://localhost:9090)

You can also query metrics directly in Prometheus or via Grafana. Here are some example queries:

**MCP tool call rate (calls per second):**

```promql
rate(mcp_tool_calls_total[5m])
```

**MCP tool error rate:**

```promql
rate(mcp_tool_errors_total[5m])
```

**MCP tool call duration (95th percentile):**

```promql
histogram_quantile(0.95, rate(mcp_tool_call_duration_ms_bucket[5m]))
```

**MCP tool list duration (average):**

```promql
rate(mcp_tool_list_duration_ms_sum[5m])
/
rate(mcp_tool_list_duration_ms_count[5m])
```

**MCP tool call duration (average by tool):**

```promql
sum(rate(mcp_tool_call_duration_ms_sum[5m])) by (tool_name)
/
sum(rate(mcp_tool_call_duration_ms_count[5m])) by (tool_name)
```

**Total MCP tool calls:**

```promql
sum(mcp_tool_calls_total)
```

## Configuration Options

See the [Configuration Guide](../../docs/config.md) for details on how to configure OpenTelemetry exporters with pctx.

## Stopping the Stack

```bash
docker compose down
```

To remove volumes and start fresh:

```bash
docker compose down -v
```

## Persistent Data

The following data is persisted in Docker volumes:

- Tempo traces: `tempo-data`
- Prometheus metrics: `prometheus-data`
- Grafana dashboards and settings: `grafana-data`

## Troubleshooting

### No traces appearing in Grafana

1. Check that pctx is running with telemetry enabled
2. Verify the OTLP Collector is receiving data: `docker compose logs otel-collector`
3. Check Tempo logs: `docker compose logs tempo`

### Connection refused errors

Ensure all services are running:

```bash
docker compose ps
```

All services should show "Up" status.
