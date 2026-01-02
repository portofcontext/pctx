# Command-Line Help for `pctx`

This document contains the help content for the `pctx` command-line program.

**Command Overview:**

* [`pctx`↴](#pctx)
* [`pctx start`↴](#pctx-start)
* [`pctx mcp`↴](#pctx-mcp)
* [`pctx mcp init`↴](#pctx-mcp-init)
* [`pctx mcp list`↴](#pctx-mcp-list)
* [`pctx mcp add`↴](#pctx-mcp-add)
* [`pctx mcp remove`↴](#pctx-mcp-remove)
* [`pctx mcp start`↴](#pctx-mcp-start)
* [`pctx mcp dev`↴](#pctx-mcp-dev)

## `pctx`

Use PCTX to expose code mode either as a session based server or by aggregating multiple MCP servers into a single code mode MCP server.

**Usage:** `pctx [OPTIONS] <COMMAND>`

EXAMPLES:
  # Code Mode sessions
  pctx start
  # Code Mode MCP
  pctx mcp init 
  pctx mcp add my-server https://mcp.example.com
  pctx mcp dev

  

###### **Subcommands:**

* `start` — Start PCTX server for code mode sessions
* `mcp` — MCP server commands (with pctx.json configuration)

###### **Options:**

* `-c`, `--config <CONFIG>` — Config file path, defaults to ./pctx.json

  Default value: `pctx.json`
* `-q`, `--quiet` — No logging except for errors
* `-v`, `--verbose` — Verbose logging (-v) or trace logging (-vv)



## `pctx start`

Starts PCTX server with no pre-configured tools. Use a client library like `pip install pctx-client` to create sessions, register tools, and expose code-mode tools to agent libraries.

**Usage:** `pctx start [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to listen on

  Default value: `8080`
* `--host <HOST>` — Host address to bind to (use 0.0.0.0 for external access)

  Default value: `127.0.0.1`
* `--session-dir <SESSION_DIR>` — Path to session storage directory

  Default value: `.pctx/sessions`
* `--no-banner` — Don't show the server banner



## `pctx mcp`

MCP server commands (with pctx.json configuration)

**Usage:** `pctx mcp <COMMAND>`

###### **Subcommands:**

* `init` — Initialize pctx.json configuration file
* `list` — List MCP servers and test connections
* `add` — Add an MCP server to configuration (HTTP or stdio)
* `remove` — Remove an MCP server from configuration
* `start` — Start the PCTX MCP server
* `dev` — Start the PCTX MCP server with terminal UI



## `pctx mcp init`

Initialize pctx.json configuration file.

**Usage:** `pctx mcp init [OPTIONS]`

###### **Options:**

* `-y`, `--yes` — Use default values and skip interactive adding of upstream MCPs



## `pctx mcp list`

Lists configured MCP servers and tests the connection to each.

**Usage:** `pctx mcp list`



## `pctx mcp add`

Add a new MCP server to the configuration. Supports both HTTP(S) URLs and stdio-based servers via the --command flag.

**Usage:** `pctx mcp add [OPTIONS] <NAME> [URL]`

###### **Arguments:**

* `<NAME>` — Unique name for this server
* `<URL>` — HTTP(S) URL of the MCP server endpoint (conflicts with --command for stdio)

###### **Options:**

* `--command <COMMAND>` — Command to execute for stdio MCP server (conflicts with url)
* `--arg <ARGS>` — Arguments to pass to the stdio command (repeat for multiple)
* `--env <ENV>` — Environment variables in KEY=VALUE format (repeat for multiple)
* `-b`, `--bearer <BEARER>` — use bearer authentication to connect to HTTP MCP server using PCTX's secret string syntax.

   e.g. `--bearer '${env:BEARER_TOKEN}'`
* `-H`, `--header <HEADER>` — use custom headers to connect to HTTP MCP server using PCTX's secret string syntax. Many headers can be defined.

   e.g. `--headers 'x-api-key: ${keychain:API_KEY}'`
* `-f`, `--force` — Overrides any existing server under the same name & skips testing connection to the MCP server



## `pctx mcp remove`

Remove an MCP server from the configuration.

**Usage:** `pctx mcp remove <NAME>`

###### **Arguments:**

* `<NAME>` — Name of the server to remove



## `pctx mcp start`

Start the PCTX MCP server (exposes /mcp endpoint).

**Usage:** `pctx mcp start [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to listen on

  Default value: `8080`
* `--host <HOST>` — Host address to bind to (use 0.0.0.0 for external access)

  Default value: `127.0.0.1`
* `--no-banner` — Don't show the server banner
* `--stdio` — Serve MCP over stdio instead of HTTP



## `pctx mcp dev`

Start the PCTX MCP server in development mode with an interactive terminal UI with data and logging.

**Usage:** `pctx mcp dev [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to listen on

  Default value: `8080`
* `--host <HOST>` — Host address to bind to (use 0.0.0.0 for external access)

  Default value: `127.0.0.1`
* `--log-file <LOG_FILE>` — Path to JSONL log file

  Default value: `pctx-dev.jsonl`
* `--stdio` — Serve MCP over stdio instead of HTTP



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

