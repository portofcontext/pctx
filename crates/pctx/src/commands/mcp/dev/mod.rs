mod app;
mod log_entry;
mod renderers;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use notify::{RecursiveMode, Watcher, recommended_watcher};
use pctx_config::Config;
use ratatui::{Terminal, backend::CrosstermBackend, style::Color};
use tokio::sync::mpsc;

use crate::commands::mcp::start::StartCmd;
use app::{App, AppMessage, FocusPanel};
use pctx_mcp_server::PctxMcpServer;

#[allow(unused)]
const PRIMARY: Color = Color::Rgb(0, 43, 86); // #002B56
const SECONDARY: Color = Color::Rgb(24, 66, 137); // #184289
const TERTIARY: Color = Color::Rgb(30, 105, 105); // #1E6969
const TEXT_COLOR: Color = Color::Rgb(1, 46, 88); // #012E58

type ServerControl = Arc<
    Mutex<
        Option<(
            tokio::task::JoinHandle<()>,
            tokio::sync::oneshot::Sender<()>,
        )>,
    >,
>;

#[derive(Debug, Clone, Parser)]
pub struct DevCmd {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host address to bind to (use 0.0.0.0 for external access)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Path to JSONL log file
    #[arg(long, default_value = "pctx-dev.jsonl")]
    pub log_file: Utf8PathBuf,

    /// Serve MCP over stdio instead of HTTP
    #[arg(long)]
    pub stdio: bool,
}

impl DevCmd {
    pub(crate) async fn handle(&self, cfg: Config) -> Result<Config> {
        if self.stdio {
            anyhow::bail!("Dev mode does not support stdio. Use `pctx mcp start --stdio` instead.");
        }

        // Set up terminal
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create app state
        let app = Arc::new(Mutex::new(App::new(
            self.host.clone(),
            self.port,
            self.log_file.clone(),
        )));

        // Channel for sending messages to the UI
        let (tx, mut rx) = mpsc::unbounded_channel::<AppMessage>();

        // Spawn initial server task
        let (server_handle, shutdown_tx) =
            spawn_server_task(cfg.clone(), tx.clone(), self.host.clone(), self.port);

        // Store server control in Arc<Mutex<>> so we can replace it on config reload
        let server_control: ServerControl =
            Arc::new(Mutex::new(Some((server_handle, shutdown_tx))));

        // Spawn config file watcher task
        let tx_watcher = tx.clone();
        let config_path = cfg.path();
        let watcher_handle = tokio::task::spawn_blocking(move || {
            let (watch_tx, watch_rx) = std::sync::mpsc::channel();

            let mut watcher = match recommended_watcher(watch_tx) {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!("Failed to create file watcher: {:?}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(config_path.as_std_path(), RecursiveMode::NonRecursive) {
                tracing::error!("Failed to watch config file: {:?}", e);
                return;
            }

            tracing::info!("Watching config file for changes: {}", config_path);

            // Use recv_timeout so we can check periodically and exit cleanly
            loop {
                match watch_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(res) => match res {
                        Ok(event) => {
                            // Filter for modify events to avoid duplicate notifications
                            if event.kind.is_modify()
                                && tx_watcher.send(AppMessage::ConfigChanged).is_err()
                            {
                                // Channel closed, exit loop
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("File watch error: {:?}", e);
                        }
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Check if tx is still valid
                        if tx_watcher.is_closed() {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        // Watcher dropped, exit loop
                        break;
                    }
                }
            }
        });

        // Run the UI
        let result = run_ui(
            &mut terminal,
            &app,
            &mut rx,
            &tx,
            &cfg.path(),
            &server_control,
            &self.host,
            self.port,
        );

        // Cleanup terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        // Send shutdown signal to server
        let shutdown_and_handle = {
            let mut control = server_control.lock().unwrap();
            control.take()
        };

        if let Some((handle, shutdown_tx)) = shutdown_and_handle {
            let _ = shutdown_tx.send(());

            // Wait for server to stop gracefully (with timeout)
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // Drop tx to signal watchers to exit (by closing the channel)
        drop(tx);

        // Wait for watchers to exit
        let _ = tokio::time::timeout(Duration::from_secs(1), watcher_handle).await;

        result?;

        Ok(cfg)
    }
}

#[allow(clippy::too_many_arguments)]
fn run_ui(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &Arc<Mutex<App>>,
    rx: &mut mpsc::UnboundedReceiver<AppMessage>,
    tx: &mpsc::UnboundedSender<AppMessage>,
    config_path: &camino::Utf8PathBuf,
    server_control: &ServerControl,
    host: &str,
    port: u16,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    // Track background tasks so we can abort them on exit
    let mut background_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // Track if mouse capture is currently enabled
    let mut mouse_capture_enabled = true;

    loop {
        // Check if we should toggle mouse capture based on focused panel
        {
            let app = app.lock().unwrap();
            let should_capture = !matches!(
                app.focused_panel,
                FocusPanel::Documentation | FocusPanel::ToolDetail
            );

            if should_capture != mouse_capture_enabled {
                if should_capture {
                    execute!(std::io::stdout(), EnableMouseCapture)?;
                } else {
                    execute!(std::io::stdout(), DisableMouseCapture)?;
                }
                mouse_capture_enabled = should_capture;
            }
        }

        // Draw UI
        {
            let mut app = app.lock().unwrap();
            terminal.draw(|f| renderers::ui(f, &mut app))?;
        }

        // Handle events
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        let mut app = app.lock().unwrap();
                        match key.code {
                            KeyCode::Char('q') => {
                                break;
                            }
                            KeyCode::Esc | KeyCode::Backspace => {
                                if app.focused_panel == FocusPanel::ToolDetail {
                                    app.close_tool_detail();
                                } else if app.focused_panel == FocusPanel::Documentation {
                                    app.close_documentation();
                                } else {
                                    break;
                                }
                            }
                            KeyCode::Enter => {
                                if app.focused_panel == FocusPanel::Tools {
                                    app.show_tool_detail();
                                }
                            }
                            KeyCode::Tab => {
                                app.next_panel();
                            }
                            KeyCode::BackTab => {
                                app.prev_panel();
                            }
                            KeyCode::Up => match app.focused_panel {
                                FocusPanel::Logs => app.scroll_logs_up(),
                                FocusPanel::Tools => app.scroll_tools_up(),
                                FocusPanel::ToolDetail => app.scroll_detail_up(),
                                FocusPanel::Documentation => app.scroll_detail_up(),
                            },
                            KeyCode::Down => match app.focused_panel {
                                FocusPanel::Logs => app.scroll_logs_down(),
                                FocusPanel::Tools => app.scroll_tools_down(),
                                FocusPanel::ToolDetail => app.scroll_detail_down(),
                                FocusPanel::Documentation => app.scroll_detail_down(),
                            },
                            KeyCode::PageUp => match app.focused_panel {
                                FocusPanel::ToolDetail | FocusPanel::Documentation => {
                                    // Scroll by 10 lines for page up
                                    for _ in 0..10 {
                                        app.scroll_detail_up();
                                    }
                                }
                                _ => {}
                            },
                            KeyCode::PageDown => match app.focused_panel {
                                FocusPanel::ToolDetail | FocusPanel::Documentation => {
                                    // Scroll by 10 lines for page down
                                    for _ in 0..10 {
                                        app.scroll_detail_down();
                                    }
                                }
                                _ => {}
                            },
                            KeyCode::Left => {
                                if app.focused_panel == FocusPanel::Tools {
                                    app.move_to_prev_namespace();
                                }
                            }
                            KeyCode::Right => {
                                if app.focused_panel == FocusPanel::Tools {
                                    app.move_to_next_namespace();
                                }
                            }
                            KeyCode::Char('f') if app.focused_panel == FocusPanel::Logs => {
                                app.cycle_log_filter();
                            }
                            KeyCode::Char('c') => {
                                if app.server_ready {
                                    let _ = app.copy_server_url_to_clipboard();
                                }
                            }
                            KeyCode::Char('d') => {
                                // open / close docs
                                if app.focused_panel == FocusPanel::Documentation {
                                    app.close_documentation();
                                } else {
                                    app.show_documentation();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    let mut app = app.lock().unwrap();
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            app.handle_mouse_click(mouse.column, mouse.row);
                        }
                        MouseEventKind::ScrollUp => {
                            app.handle_mouse_scroll(mouse.column, mouse.row, true);
                        }
                        MouseEventKind::ScrollDown => {
                            app.handle_mouse_scroll(mouse.column, mouse.row, false);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Process messages from server task
        while let Ok(msg) = rx.try_recv() {
            // Handle ConfigChanged specially - need to restart server with new config
            if matches!(msg, AppMessage::ConfigChanged) {
                // First, update the app state to clear servers
                {
                    let mut app = app.lock().unwrap();
                    app.handle_message(msg);
                }

                // Shutdown the existing server and spawn a new one
                let tx_reload = tx.clone();
                let config_path_clone = config_path.clone();
                let server_control_clone = server_control.clone();
                let host_clone = host.to_string();
                let port_clone = port;

                let task_handle = tokio::spawn(async move {
                    // 1. Stop the existing server
                    tracing::info!("Stopping existing server for config reload...");
                    let old_server = {
                        let mut control = server_control_clone.lock().unwrap();
                        control.take() // Take ownership of the old server control
                    };

                    if let Some((old_handle, old_shutdown_tx)) = old_server {
                        // Send shutdown signal
                        let _ = old_shutdown_tx.send(());

                        // Wait for server to stop (with timeout)
                        let _ = tokio::time::timeout(Duration::from_secs(5), old_handle).await;
                        tracing::info!("Old server stopped");
                    }

                    // 2. Reload config
                    match Config::load(&config_path_clone) {
                        Ok(new_cfg) => {
                            tracing::info!(
                                "Config reloaded, starting new server with {} upstream servers",
                                new_cfg.servers.len()
                            );

                            // 3. Spawn new server with new config
                            let (new_handle, new_shutdown_tx) = spawn_server_task(
                                new_cfg,
                                tx_reload.clone(),
                                host_clone,
                                port_clone,
                            );

                            // 4. Store new server control
                            {
                                let mut control = server_control_clone.lock().unwrap();
                                *control = Some((new_handle, new_shutdown_tx));
                            }

                            tracing::info!("New server started successfully");
                        }
                        Err(e) => {
                            tx_reload
                                .send(AppMessage::ServerFailed(format!("{e:?}")))
                                .ok();
                        }
                    }
                });

                // Track this task so we can abort it on exit
                background_tasks.push(task_handle);
            } else {
                let mut app = app.lock().unwrap();
                app.handle_message(msg);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Read new logs from JSONL file
            let mut app = app.lock().unwrap();
            let _ = app.read_new_logs();

            last_tick = Instant::now();
        }
    }

    // Abort all background tasks
    for task in background_tasks {
        task.abort();
    }

    Ok(())
}

// Spawns the PctxMcp server task
// Returns (server_handle, shutdown_sender)
async fn load_code_mode_for_dev(cfg: &Config) -> Result<pctx_code_mode::CodeMode> {
    if cfg.servers.is_empty() {
        tracing::warn!(
            "No MCP servers configured, add servers with 'pctx add <name> <url>' and PCTX Dev Mode will refresh"
        );
        Ok(pctx_code_mode::CodeMode::default())
    } else {
        let loaded = StartCmd::load_code_mode(cfg).await?;
        if loaded.tool_sets.is_empty() {
            tracing::warn!(
                "Failed loading all configured MCP servers, add servers with 'pctx add <name> <url>' or edit {} and PCTX Dev Mode will refresh",
                cfg.path()
            );
        }
        Ok(loaded)
    }
}

fn spawn_server_task(
    cfg: Config,
    tx: mpsc::UnboundedSender<AppMessage>,
    host: String,
    port: u16,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        tx.send(AppMessage::ServerStarting).ok();

        let tools = match load_code_mode_for_dev(&cfg).await {
            Ok(loaded) => loaded,
            Err(err) => {
                tx.send(AppMessage::ServerFailed(format!(
                    "Failed loading upstream MCPs: {err:?}"
                )))
                .ok();
                pctx_code_mode::CodeMode::default()
            }
        };

        // Run server with shutdown signal
        let pctx_mcp = PctxMcpServer::new(&host, port, false);

        tx.send(AppMessage::ServerReady(tools.clone())).ok();

        if let Err(e) = pctx_mcp
            .serve_with_shutdown(&cfg, tools, async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            let mut msg = e.to_string();
            if msg.starts_with("Address already in use") {
                // nicer log
                msg = format!(
                    "Address http://{host}:{port} is already in use, restart this command with the `--port` flag to select a different port"
                );
            }
            tx.send(AppMessage::ServerFailed(msg)).ok();
        }

        tx.send(AppMessage::ServerStopped).ok();
    });

    (handle, shutdown_tx)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::commands::mcp::dev::log_entry::{LogEntry, LogEntryFields};

    use super::*;
    // use crate::commands::dev::log_entry::{LogEntry, LogEntryFields};
    use chrono::Utc;
    use codegen::{Tool, ToolSet};
    use pctx_code_mode::CodeMode;
    use pctx_config::{logger::LogLevel, server::ServerConfig};
    use serde_json::json;

    fn create_pctx_tools() -> CodeMode {
        let account_schema = json!({
            "type": "object",
            "required": ["account_id", "opened_at", "balance", "status"],
            "properties": {
                "account_id": {"type": "string"},
                "opened_at": {"type": "string", "format": "date-time"},
                "balance": {"type": "number"},
                "status": {"type": "string", "enum": ["open", "frozen", "in_review"]}
            }
        });
        let tools = vec![
            Tool::new_mcp(
                "get_account_balance",
                Some("Retrieves the balance for an account".into()),
                serde_json::from_value(json!({
                    "type": "object",
                    "required": ["account_id"],
                    "properties": {
                        "account_id": {"type": "string"}
                    }
                }))
                .unwrap(),
                Some(serde_json::from_value(account_schema.clone()).unwrap()),
            )
            .unwrap(),
            Tool::new_mcp(
                "freeze_account",
                Some("Freezes an account".into()),
                serde_json::from_value(json!({
                    "type": "object",
                    "required": ["account_id"],
                    "properties": {
                        "account_id": {"type": "string"}
                    }
                }))
                .unwrap(),
                Some(serde_json::from_value(account_schema.clone()).unwrap()),
            )
            .unwrap(),
        ];

        CodeMode {
            tool_sets: vec![ToolSet::new("banking", "Banking MCP Server", tools)],
            servers: vec![ServerConfig::new(
                "banking".into(),
                "http://localhost:8080/mcp".parse().unwrap(),
            )],
            callbacks: vec![],
        }
    }

    #[test]
    fn test_track_tool_usage_with_banking_namespace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = Utf8PathBuf::from_path_buf(temp_dir.path().join("test.jsonl")).unwrap();

        let mut app = App::new("localhost".to_string(), 8080, log_file);

        // Add the test server
        app.tools = create_pctx_tools();

        // Create a log entry with code_from_llm field containing Banking.getAccountBalance
        let log_entry = LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            target: "pctx".to_string(),
            fields: LogEntryFields {
                message: "Executing code".into(),
                extra: HashMap::from_iter([(
                    "code_from_llm".to_string(),
                    json!(
                        "const balance = await Banking.getAccountBalance({ account_id: \"ACC-123\" });"
                    ),
                )]),
            },
        };

        // Track the tool usage
        app.track_tool_usage(&log_entry);

        // Verify that the tool was tracked
        let key = "banking::get_account_balance";
        assert!(
            app.tool_usage.contains_key(key),
            "Expected tool_usage to contain key '{}', but it doesn't. Keys present: {:?}",
            key,
            app.tool_usage.keys().collect::<Vec<_>>()
        );

        let usage = app.tool_usage.get(key).unwrap();
        assert_eq!(usage.count, 1);
        assert_eq!(usage.tool_name, "get_account_balance");
        assert_eq!(usage.server_name, "banking");
        assert!(!usage.code_snippets.is_empty());
    }

    #[test]
    fn test_track_tool_usage_with_freeze_account() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = Utf8PathBuf::from_path_buf(temp_dir.path().join("test.jsonl")).unwrap();

        let mut app = App::new("localhost".to_string(), 8080, log_file);

        // Add the test server
        app.tools = create_pctx_tools();

        // Create a log entry with Banking.freezeAccount
        let log_entry = LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            target: "pctx".into(),
            fields: LogEntryFields {
                message: "Executing code".into(),
                extra: HashMap::from_iter([(
                    "code_from_llm".to_string(),
                    json!("await Banking.freezeAccount({ account_id: \"ACC-555\" });"),
                )]),
            },
        };

        // Track the tool usage
        app.track_tool_usage(&log_entry);

        // Verify that the tool was tracked
        let key = "banking::freeze_account";
        assert!(
            app.tool_usage.contains_key(key),
            "Expected tool_usage to contain key '{}', but it doesn't. Keys present: {:?}",
            key,
            app.tool_usage.keys().collect::<Vec<_>>()
        );

        let usage = app.tool_usage.get(key).unwrap();
        assert_eq!(usage.count, 1);
        assert_eq!(usage.tool_name, "freeze_account");
        assert_eq!(usage.server_name, "banking");
    }

    #[test]
    fn test_track_multiple_calls() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = Utf8PathBuf::from_path_buf(temp_dir.path().join("test.jsonl")).unwrap();

        let mut app = App::new("localhost".to_string(), 8080, log_file);

        // Add the test server
        app.tools = create_pctx_tools();

        // First call
        let log_entry1 = LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            target: "pctx".into(),
            fields: LogEntryFields {
                message: "Executing code".into(),
                extra: HashMap::from_iter([(
                    "code_from_llm".to_string(),
                    json!("await Banking.getAccountBalance({ account_id: \"ACC-1\" });"),
                )]),
            },
        };

        // Second call
        let log_entry2 = LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            target: "pctx".into(),
            fields: LogEntryFields {
                message: "Executing code".into(),
                extra: HashMap::from_iter([(
                    "code_from_llm".to_string(),
                    json!("await Banking.getAccountBalance({ account_id: \"ACC-2\" });"),
                )]),
            },
        };

        app.track_tool_usage(&log_entry1);
        app.track_tool_usage(&log_entry2);

        let key = "banking::get_account_balance";
        let usage = app.tool_usage.get(key).unwrap();
        assert_eq!(usage.count, 2, "Expected count to be 2 after two calls");
        assert_eq!(
            usage.code_snippets.len(),
            2,
            "Expected 2 unique code snippets"
        );
    }
}
