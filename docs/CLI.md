# Command-Line Help for `pctx`

This document contains the help content for the `pctx` command-line program.

**Command Overview:**

* [`pctx`↴](#pctx)
* [`pctx code-mode`↴](#pctx-code-mode)
* [`pctx code-mode init`↴](#pctx-code-mode-init)
* [`pctx code-mode list`↴](#pctx-code-mode-list)
* [`pctx code-mode add`↴](#pctx-code-mode-add)
* [`pctx code-mode remove`↴](#pctx-code-mode-remove)
* [`pctx code-mode start`↴](#pctx-code-mode-start)
* [`pctx code-mode dev`↴](#pctx-code-mode-dev)
* [`pctx code-mode serve`↴](#pctx-code-mode-serve)

## `pctx`

Use PCTX to run Code Mode as a session server for client SDKs, or as an MCP server that aggregates multiple MCP servers.

**Usage:** `pctx [OPTIONS] <COMMAND>`

EXAMPLES:
  # Code Mode sessions
  pctx code-mode serve
  # Code Mode MCP
  pctx code-mode init 
  pctx code-mode add my-server https://mcp.example.com
  pctx code-mode dev

  

###### **Subcommands:**

* `code-mode` — Code Mode MCP server commands (with pctx.json configuration)

###### **Options:**

* `-c`, `--config <CONFIG>` — Config file path, defaults to ./pctx.json

  Default value: `pctx.json`
* `-q`, `--quiet` — No logging except for errors
* `-v`, `--verbose` — Verbose logging (-v) or trace logging (-vv)



## `pctx code-mode`

Code Mode MCP server commands (with pctx.json configuration)

**Usage:** `pctx code-mode <COMMAND>`

###### **Subcommands:**

* `init` — Initialize pctx.json configuration file
* `list` — List MCP servers and test connections
* `add` — Add an MCP server to configuration (HTTP or stdio)
* `remove` — Remove an MCP server from configuration
* `start` — Start the PCTX MCP server (lightweight)
* `dev` — Start the PCTX MCP server with terminal UI
* `serve` — Start the Code Mode session server (scalable, used by the pctx-client SDK)



## `pctx code-mode init`

Initialize pctx.json configuration file.

**Usage:** `pctx code-mode init [OPTIONS]`

###### **Options:**

* `-y`, `--yes` — Use default values and skip interactive adding of upstream MCPs



## `pctx code-mode list`

Lists configured MCP servers and tests the connection to each.

**Usage:** `pctx code-mode list`



## `pctx code-mode add`

Add a new MCP server to the configuration. Supports both HTTP(S) URLs and stdio-based servers via the --command flag.

**Usage:** `pctx code-mode add [OPTIONS] <NAME> [URL]`

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



## `pctx code-mode remove`

Remove an MCP server from the configuration.

**Usage:** `pctx code-mode remove <NAME>`

###### **Arguments:**

* `<NAME>` — Name of the server to remove



## `pctx code-mode start`

Start the PCTX MCP server (exposes /mcp endpoint). The lightweight way to try Code Mode; use `serve` to run it at scale.

**Usage:** `pctx code-mode start [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to listen on

  Default value: `8080`
* `--host <HOST>` — Host address to bind to (use 0.0.0.0 for external access)

  Default value: `127.0.0.1`
* `--no-banner` — Don't show the server banner
* `--stdio` — Serve MCP over stdio instead of HTTP
* `--stateful-http` — Use stateful MCP sessions (incompatible with --stdio)



## `pctx code-mode dev`

Start the PCTX MCP server in development mode with an interactive terminal UI with data and logging.

**Usage:** `pctx code-mode dev [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to listen on

  Default value: `8080`
* `--host <HOST>` — Host address to bind to (use 0.0.0.0 for external access)

  Default value: `127.0.0.1`
* `--log-file <LOG_FILE>` — Path to JSONL log file

  Default value: `pctx-dev.jsonl`
* `--stdio` — Serve MCP over stdio instead of HTTP
* `--stateful-http` — Use stateful HTTP sessions (incompatible with --stdio)



## `pctx code-mode serve`

Start the Code Mode session server with no pre-configured tools. Clients like `pip install pctx-client` create sessions, register tools, and expose code-mode tools to agent libraries.

**Usage:** `pctx code-mode serve [OPTIONS]`

###### **Options:**

* `-p`, `--port <PORT>` — Port to listen on

  Default value: `8080`
* `--host <HOST>` — Host address to bind to (use 0.0.0.0 for external access)

  Default value: `127.0.0.1`
* `--session-dir <SESSION_DIR>` — Path to session storage directory

  Default value: `.pctx/sessions`
* `--allowed-origin <ALLOWED_ORIGINS>` — Allowed CORS origins. Can be specified multiple times. Defaults to localhost only (<http://localhost>, <http://127.0.0.1>, http://[`::1`]). Specify your own origins to override the default (can include or exclude localhost). Origins without explicit ports will match any port. Example: --allowed-origin <http://localhost> --allowed-origin <https://app.example.com>
* `--no-banner` — Don't show the server banner



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

