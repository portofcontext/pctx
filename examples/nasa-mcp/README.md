# NASA MCP Server with pctx

This example demonstrates a custom MCP (Model Context Protocol) server built with the **official @modelcontextprotocol/sdk** that provides access to NASA APIs, deployed with pctx.

## Features

The NASA MCP server provides 5 tools for accessing NASA data:

### Asteroid Tools (NeoWs API)
1. **search_asteroids** - Search for Near Earth Objects based on their closest approach date
2. **lookup_asteroid** - Get detailed information about a specific asteroid by ID
3. **browse_asteroids** - Browse the overall NEO dataset with pagination

### Satellite Tools (TLE API)
4. **search_satellites** - Search for satellites by name and get TLE data
5. **lookup_satellite** - Get TLE data for a specific satellite by catalog number

## Architecture

- **NASA MCP Server** (port 3000): Custom MCP server that wraps NASA APIs
- **pctx** (port 8080): Exposes the NASA MCP server to AI models

## Setup

### 1. Get a NASA API Key

Get your free API key at https://api.nasa.gov/

### 2. Install Dependencies

```bash
npm install
```

### 3. Configure Environment

Copy `.env.example` to `.env` and add your NASA API key:

```bash
cp .env.example .env
```

Edit `.env`:
```
NASA_API_KEY=your_key_here
```

## Running Locally

### Option 1: Using npm (with installed pctx)

```bash
npm start
```

This will:
1. Start the NASA MCP server on port 3000
2. Capture the auth token automatically
3. Start pctx on port 8080

### Option 2: Using local cargo build

```bash
npm start -- --local
```

This uses `cargo run` to build and run pctx from source.

### Option 3: Manual setup

Start the NASA MCP server:
```bash
node nasa-mcp-server.js
```

In another terminal, start pctx:
```bash
pctx start --port 8080 --config pctx.json
```

## Testing

Test the NASA MCP server directly:
```bash
# List available tools
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer YOUR_TOKEN' \
  -d '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":1}'

# Search for asteroids
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer YOUR_TOKEN' \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search_asteroids","arguments":{"start_date":"2024-01-01"}},"id":2}'

# Search for satellites
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer YOUR_TOKEN' \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"search_satellites","arguments":{"search":"ISS"}},"id":3}'
```

Test pctx:
```bash
curl -X POST http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}'
```

## Deployment

### Docker Compose

```bash
docker-compose up
```

### Fly.io

1. Install the Fly CLI: https://fly.io/docs/hands-on/install-flyctl/

2. Login to Fly:
```bash
fly auth login
```

3. Set your NASA API key as a secret:
```bash
fly secrets set NASA_API_KEY=your_key_here
```

4. Deploy:
```bash
fly deploy
```

Your server will be available at `https://nasa-pctx-example.fly.dev/mcp`

## API Documentation

### NASA NeoWs API
- Documentation: https://api.nasa.gov/
- Rate limits: 1000 requests per hour with API key

### TLE API
- Documentation: http://tle.ivanstanojevic.me
- No authentication required
- Data updated daily from CelesTrak

## Example Tool Calls

### Search Asteroids
```json
{
  "name": "search_asteroids",
  "arguments": {
    "start_date": "2024-01-01",
    "end_date": "2024-01-02"
  }
}
```

### Lookup Asteroid
```json
{
  "name": "lookup_asteroid",
  "arguments": {
    "asteroid_id": "3542519"
  }
}
```

### Browse Asteroids
```json
{
  "name": "browse_asteroids",
  "arguments": {
    "page": 0,
    "size": 20
  }
}
```

### Search Satellites
```json
{
  "name": "search_satellites",
  "arguments": {
    "search": "ISS"
  }
}
```

### Lookup Satellite
```json
{
  "name": "lookup_satellite",
  "arguments": {
    "satellite_id": "25544"
  }
}
```

## Troubleshooting

### Port Already in Use

If ports 3000 or 8080 are already in use, you can change them:

```bash
NASA_MCP_PORT=3001 PCTX_PORT=8081 npm start
```

### Connection Issues

Make sure both servers are running and the auth token is properly captured. Check the logs for any errors.

### API Rate Limits

The NASA API has rate limits. If you're getting 429 errors, wait a bit or use your own API key instead of DEMO_KEY.

## License

This example is provided as-is for demonstration purposes.
