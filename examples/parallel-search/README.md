# Parallel Search through Code Mode

Use [Parallel Search MCP](https://docs.parallel.ai/integrations/mcp/search-mcp)
for free web search and page fetching in pctx, with no account or API key.
Anonymous search uses Fast mode. Free access has rate limits.

This example registers the HTTP endpoint as `parallel`. pctx discovers its tools
and generates the `Parallel.webSearch` and `Parallel.webFetch` TypeScript functions.
The standalone configuration leaves your existing `pctx.json` unchanged. Its
`auth.headers` field sets a project User-Agent; it contains no authentication secret.

## Run the example

Install Node.js with npm, Python 3.10 or later, and pctx:

```bash
npm install -g @portofcontext/pctx@0.7.6
cd examples/parallel-search
python -m venv .venv
# On Windows: .venv\Scripts\activate
source .venv/bin/activate
python -m pip install -r requirements.txt
python search.py
```

`search.py` starts `pctx mcp start --stdio` with this example's configuration,
discovers the generated functions, and calls `execute_typescript`. Its `run()`
function searches for pctx documentation and fetches the repository page, reusing
the search session ID for the fetch. The output includes source URLs, excerpts,
and any page fetch errors. Both upstream calls use HTTP MCP. The Python script
calls Code Mode directly and needs no LLM or model credentials.

## Use with an MCP client

Configure your client to launch `pctx` with these arguments, replacing the path
with the absolute path to this example's `pctx.json`:

```json
{
  "mcpServers": {
    "pctx": {
      "command": "pctx",
      "args": [
        "--config", "/absolute/path/to/pctx/examples/parallel-search/pctx.json",
        "mcp", "start", "--stdio", "--no-banner"
      ]
    }
  }
}
```

The client can discover the generated functions and execute TypeScript using the
same `run()` body shown in [search.py](search.py). To add Parallel to an existing
setup, copy the single server entry from [pctx.json](pctx.json) into your
configuration's `servers` array, keeping your other servers.
