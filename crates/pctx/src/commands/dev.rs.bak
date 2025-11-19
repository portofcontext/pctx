use anyhow::Result;
use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
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
use pctx_config::{Config, logger::LogLevel};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

// Brand colors
#[allow(dead_code)]
const PRIMARY: Color = Color::Rgb(0, 43, 86); // #002B56
const SECONDARY: Color = Color::Rgb(24, 66, 137); // #184289
const TERTIARY: Color = Color::Rgb(30, 105, 105); // #1E6969
const TEXT_COLOR: Color = Color::Rgb(1, 46, 88); // #012E58
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

use crate::{
    commands::start::StartCmd,
    mcp::{PctxMcp, upstream::UpstreamMcp},
};

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
}

type ServerControl = Arc<
    Mutex<
        Option<(
            tokio::task::JoinHandle<()>,
            tokio::sync::oneshot::Sender<()>,
        )>,
    >,
>;

#[derive(Debug, Clone, Deserialize)]
struct LogEntry {
    timestamp: DateTime<Utc>,
    level: LogLevel,
    #[allow(unused)]
    target: String,
    fields: LogEntryFields,
}
impl LogEntry {
    fn prefix(&self) -> String {
        self.level.as_str().to_uppercase()
    }

    fn color(&self) -> Color {
        match &self.level {
            LogLevel::Trace => Color::LightMagenta,
            LogLevel::Debug => SECONDARY,
            LogLevel::Info => TERTIARY,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::Red,
        }
    }

    fn tui_line(&'_ self, level: LogLevel) -> Line<'_> {
        let time_str = self.timestamp.format("%H:%M:%S").to_string();
        let mut parts = vec![Span::styled(
            format!("[{time_str}] "),
            Style::default().fg(Color::DarkGray),
        )];
        if level <= LogLevel::Debug {
            parts.push(Span::styled(
                self.target.clone(),
                Style::default().fg(Color::DarkGray),
            ));
        }
        parts.extend([
            Span::styled(
                format!("[{}] ", self.prefix()),
                Style::default()
                    .fg(self.color())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(self.fields.message.clone()),
        ]);

        Line::from(parts)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LogEntryFields {
    #[serde(default)]
    message: String,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone)]
enum AppMessage {
    ServerConnected(String, Vec<UpstreamMcp>),
    ServerFailed(String, String),
    ServerStarted,
    ServerStopped,
    ConfigChanged,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusPanel {
    Tools,
    Logs,
    ToolDetail,
    Documentation,
}

#[derive(Debug, Clone)]
struct ToolUsage {
    #[allow(dead_code)]
    tool_name: String,
    #[allow(dead_code)]
    server_name: String,
    count: usize,
    last_used: DateTime<Utc>,
    code_snippets: Vec<String>,
}

struct App {
    logs: Vec<LogEntry>,
    upstream_servers: Vec<UpstreamMcp>,
    server_running: bool,
    host: String,
    port: u16,
    start_time: Option<Instant>,
    log_scroll_offset: usize,
    log_file_path: Utf8PathBuf,
    log_file_pos: u64,

    // UI State
    focused_panel: FocusPanel,
    log_filter: LogLevel,
    #[allow(dead_code)]
    tools_list_state: ListState,
    selected_tool_index: Option<usize>,
    selected_namespace_index: usize, // Index of currently selected namespace/server
    detail_scroll_offset: usize,

    // Tool usage tracking
    tool_usage: HashMap<String, ToolUsage>,

    // Panel boundaries for mouse click detection
    tools_rect: Option<Rect>,
    logs_rect: Option<Rect>,
    namespace_rects: Vec<Rect>, // Rectangles for each namespace column
    docs_rect: Option<Rect>,    // Rectangle for documentation column
}

impl App {
    fn new(host: String, port: u16, log_file_path: Utf8PathBuf) -> Self {
        Self {
            logs: Vec::new(),
            upstream_servers: Vec::new(),
            server_running: false,
            host,
            port,
            start_time: None,
            log_scroll_offset: 0,
            log_file_path,
            log_file_pos: 0,
            focused_panel: FocusPanel::Logs,
            log_filter: LogLevel::Info,
            tools_list_state: ListState::default(),
            selected_tool_index: None,
            selected_namespace_index: 0,
            detail_scroll_offset: 0,
            tool_usage: HashMap::new(),
            tools_rect: None,
            logs_rect: None,
            namespace_rects: Vec::new(),
            docs_rect: None,
        }
    }

    fn get_server_url(&self) -> String {
        format!("http://{}:{}/mcp", self.host, self.port)
    }

    fn copy_server_url_to_clipboard(&self) -> Result<()> {
        let url = self.get_server_url();
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                clipboard.set_text(&url)?;
                tracing::info!("Copied server URL to clipboard: {}", url);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to access clipboard: {:?}", e);
                anyhow::bail!("Failed to access clipboard: {e}")
            }
        }
    }

    fn read_new_logs(&mut self) -> Result<()> {
        let Ok(file) = File::open(&self.log_file_path) else {
            return Ok(()); // File doesn't exist yet, that's fine
        };

        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(self.log_file_pos))?;

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                // Track tool usage from logs
                self.track_tool_usage(&entry);

                self.logs.push(entry);

                // Keep scroll at bottom (offset 0 = most recent) when new log arrives
                // Only if user hasn't scrolled up (offset > 0)
                if self.log_scroll_offset == 0 {
                    // Stay at bottom
                    self.log_scroll_offset = 0;
                }
            }
        }

        // Update position
        if let Ok(metadata) = std::fs::metadata(&self.log_file_path) {
            self.log_file_pos = metadata.len();
        }

        Ok(())
    }

    fn track_tool_usage(&mut self, entry: &LogEntry) {
        // Look for code execution logs that contain upstream tool calls
        if let Some(code_from_llm) = entry
            .fields
            .extra
            .get("code_from_llm")
            .and_then(|v| v.as_str())
        {
            tracing::trace!(
                "Found code_from_llm field (length={}), checking for tool usage. Servers available: {}",
                code_from_llm.len(),
                self.upstream_servers.len()
            );

            // Parse the code to find upstream tool calls like "Banking.getAccountBalance"
            // Pattern: namespace.methodName(
            for server in &self.upstream_servers {
                let namespace_pattern = format!("{}.", server.namespace);
                tracing::trace!(
                    "Checking for server '{}' with namespace pattern '{}' in code",
                    server.name,
                    namespace_pattern
                );

                if code_from_llm.contains(&namespace_pattern) {
                    tracing::trace!(
                        "✓ Found {} namespace in code_from_llm, checking {} tools",
                        server.namespace,
                        server.tools.len()
                    );

                    // Find all method calls for this server
                    for (fn_name, tool) in &server.tools {
                        // Check if this function is called in the code
                        let method_pattern = format!("{}.{}(", server.namespace, fn_name);
                        tracing::trace!(
                            "Checking for method pattern '{}' for tool '{}'",
                            method_pattern,
                            tool.tool_name
                        );

                        if code_from_llm.contains(&method_pattern) {
                            tracing::trace!(
                                "✓ Found tool usage: {}.{} (tool_name={})",
                                server.namespace,
                                fn_name,
                                tool.tool_name
                            );

                            // Extract a snippet of the call
                            if let Some(idx) = code_from_llm.find(&method_pattern) {
                                let snippet_start = idx.saturating_sub(10);
                                let snippet_end =
                                    (idx + method_pattern.len() + 50).min(code_from_llm.len());
                                let code_snippet = code_from_llm[snippet_start..snippet_end]
                                    .lines()
                                    .next()
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();

                                let key = format!("{}::{}", server.name, tool.tool_name);

                                self.tool_usage
                                    .entry(key.clone())
                                    .and_modify(|usage| {
                                        usage.count += 1;
                                        usage.last_used = entry.timestamp;
                                        if !code_snippet.is_empty()
                                            && !usage.code_snippets.contains(&code_snippet)
                                        {
                                            usage.code_snippets.push(code_snippet.clone());
                                        }
                                    })
                                    .or_insert_with(|| ToolUsage {
                                        tool_name: tool.tool_name.clone(),
                                        server_name: server.name.clone(),
                                        count: 1,
                                        last_used: entry.timestamp,
                                        code_snippets: if code_snippet.is_empty() {
                                            vec![]
                                        } else {
                                            vec![code_snippet]
                                        },
                                    });

                                tracing::trace!("✓ Tracked tool usage for key: {}", key);
                            }
                        }
                    }
                } else {
                    tracing::trace!(
                        "Namespace pattern '{}' not found in code_from_llm",
                        namespace_pattern
                    );
                }
            }
        }
    }

    fn reprocess_logs_for_tool_usage(&mut self) {
        // Re-read the entire log file and process all entries for tool usage
        let Ok(file) = File::open(&self.log_file_path) else {
            return;
        };

        let reader = BufReader::new(file);

        for line in reader.lines() {
            let Ok(line) = line else {
                continue;
            };

            if line.is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                self.track_tool_usage(&entry);
            }
        }
    }

    fn filtered_logs(&self) -> Vec<&LogEntry> {
        self.logs
            .iter()
            .filter(|l| self.log_filter <= l.level)
            .collect()
    }

    fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::ServerConnected(_name, upstreams) => {
                self.upstream_servers = upstreams;

                // Re-process all existing logs now that we have server metadata
                tracing::info!(
                    "ServerConnected: {} servers available. Re-processing existing logs for tool usage tracking.",
                    self.upstream_servers.len()
                );
                self.reprocess_logs_for_tool_usage();
            }
            AppMessage::ServerFailed(_name, _error) => {
                // Error is logged, nothing else to do
            }
            AppMessage::ServerStarted => {
                self.server_running = true;
                self.start_time = Some(Instant::now());
            }
            AppMessage::ServerStopped => {
                self.server_running = false;
            }
            AppMessage::ConfigChanged => {
                tracing::info!("Configuration file changed, reloading servers...");
                // Clear existing servers - they will be repopulated when reconnection completes
                self.upstream_servers.clear();
                self.selected_tool_index = None;
                self.selected_namespace_index = 0;
            }
        }
    }

    fn scroll_logs_up(&mut self) {
        // Scroll up = go back in time = increase offset
        let filtered_count = self.filtered_logs().len();
        if self.log_scroll_offset < filtered_count.saturating_sub(1) {
            self.log_scroll_offset += 1;
        }
    }

    fn scroll_logs_down(&mut self) {
        // Scroll down = go forward in time = decrease offset (0 = most recent)
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
    }

    fn cycle_log_filter(&mut self) {
        self.log_filter = match self.log_filter {
            LogLevel::Debug => LogLevel::Info,
            LogLevel::Info => LogLevel::Warn,
            LogLevel::Warn => LogLevel::Error,
            LogLevel::Error | LogLevel::Trace => LogLevel::Debug,
        };
        self.log_scroll_offset = 0;
    }

    fn next_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusPanel::Tools => FocusPanel::Logs,
            FocusPanel::Logs => FocusPanel::Tools,
            FocusPanel::ToolDetail => FocusPanel::ToolDetail, // Stay in detail view
            FocusPanel::Documentation => FocusPanel::Documentation, // Stay in docs view
        };
    }

    fn prev_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusPanel::Tools => FocusPanel::Logs,
            FocusPanel::Logs => FocusPanel::Tools,
            FocusPanel::ToolDetail => FocusPanel::ToolDetail, // Stay in detail view
            FocusPanel::Documentation => FocusPanel::Documentation, // Stay in docs view
        };
    }

    fn show_tool_detail(&mut self) {
        if self.selected_tool_index.is_some() {
            self.focused_panel = FocusPanel::ToolDetail;
            self.detail_scroll_offset = 0;
        }
    }

    fn show_documentation(&mut self) {
        self.focused_panel = FocusPanel::Documentation;
        self.detail_scroll_offset = 0;
    }

    fn close_tool_detail(&mut self) {
        self.focused_panel = FocusPanel::Tools;
    }

    fn close_documentation(&mut self) {
        self.focused_panel = FocusPanel::Tools;
    }

    fn scroll_detail_up(&mut self) {
        // Scroll faster (3 lines at a time) for better UX
        self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(3);
    }

    fn scroll_detail_down(&mut self) {
        // Scroll faster (3 lines at a time) for better UX
        self.detail_scroll_offset += 3;
    }

    fn scroll_tools_down(&mut self) {
        // Sort servers alphabetically (same as rendering)
        let mut sorted_servers: Vec<_> = self.upstream_servers.iter().collect();
        sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

        if sorted_servers.is_empty() {
            return;
        }

        // Get current namespace's tool count
        if self.selected_namespace_index >= sorted_servers.len() {
            return;
        }

        let current_server = sorted_servers[self.selected_namespace_index];
        let tools_in_namespace = current_server.tools.len();
        if tools_in_namespace == 0 {
            return;
        }

        // Calculate global indices for this namespace
        let namespace_start_idx: usize = sorted_servers
            .iter()
            .take(self.selected_namespace_index)
            .map(|s| s.tools.len())
            .sum();
        let namespace_end_idx = namespace_start_idx + tools_in_namespace - 1;

        let current = self.selected_tool_index.unwrap_or(namespace_start_idx);

        // Only move down if we're within this namespace
        if current < namespace_end_idx {
            self.selected_tool_index = Some(current + 1);
        }
    }

    fn scroll_tools_up(&mut self) {
        // Sort servers alphabetically (same as rendering)
        let mut sorted_servers: Vec<_> = self.upstream_servers.iter().collect();
        sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

        if sorted_servers.is_empty() {
            return;
        }

        // Get current namespace's start index
        if self.selected_namespace_index >= sorted_servers.len() {
            return;
        }

        let namespace_start_idx: usize = sorted_servers
            .iter()
            .take(self.selected_namespace_index)
            .map(|s| s.tools.len())
            .sum();

        let Some(current) = self.selected_tool_index else {
            return;
        };

        // Only move up if we're within this namespace
        if current > namespace_start_idx {
            self.selected_tool_index = Some(current - 1);
        }
    }

    fn move_to_next_namespace(&mut self) {
        if self.upstream_servers.is_empty() {
            return;
        }

        // Sort servers alphabetically (same as rendering)
        let mut sorted_servers: Vec<_> = self.upstream_servers.iter().collect();
        sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

        let num_namespaces = sorted_servers.len();
        if num_namespaces == 0 {
            return;
        }

        // Move to next namespace (wrap around)
        self.selected_namespace_index = (self.selected_namespace_index + 1) % num_namespaces;

        // Select first tool in new namespace
        self.select_first_tool_in_current_namespace();
    }

    fn move_to_prev_namespace(&mut self) {
        if self.upstream_servers.is_empty() {
            return;
        }

        // Sort servers alphabetically (same as rendering)
        let mut sorted_servers: Vec<_> = self.upstream_servers.iter().collect();
        sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

        let num_namespaces = sorted_servers.len();
        if num_namespaces == 0 {
            return;
        }

        // Move to previous namespace (wrap around)
        self.selected_namespace_index = if self.selected_namespace_index == 0 {
            num_namespaces - 1
        } else {
            self.selected_namespace_index - 1
        };

        // Select first tool in new namespace
        self.select_first_tool_in_current_namespace();
    }

    fn select_first_tool_in_current_namespace(&mut self) {
        // Sort servers alphabetically (same as rendering)
        let mut sorted_servers: Vec<_> = self.upstream_servers.iter().collect();
        sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

        if self.selected_namespace_index >= sorted_servers.len() {
            self.selected_tool_index = None;
            return;
        }

        // Calculate the index of the first tool in the selected namespace
        let mut tool_index = 0;
        for (idx, server) in sorted_servers.iter().enumerate() {
            if idx == self.selected_namespace_index {
                // Found our namespace, set to first tool
                if server.tools.is_empty() {
                    self.selected_tool_index = None;
                } else {
                    self.selected_tool_index = Some(tool_index);
                }
                return;
            }
            tool_index += server.tools.len();
        }
    }

    fn get_selected_tool(
        &self,
    ) -> Option<(&UpstreamMcp, String, &crate::mcp::upstream::UpstreamTool)> {
        let idx = self.selected_tool_index?;
        let mut counter = 0;

        // Sort servers alphabetically (same as rendering)
        let mut sorted_servers: Vec<_> = self.upstream_servers.iter().collect();
        sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

        for server in sorted_servers {
            // Sort tools by usage count (same as rendering)
            let mut tools_with_usage: Vec<_> = server
                .tools
                .iter()
                .map(|(fn_name, tool)| {
                    let usage_key = format!("{}::{}", server.name, tool.tool_name);
                    let usage_count = self.tool_usage.get(&usage_key).map_or(0, |u| u.count);
                    (fn_name, tool, usage_count)
                })
                .collect();
            tools_with_usage.sort_by(|a, b| b.2.cmp(&a.2));

            for (tool_name, tool, _usage_count) in tools_with_usage {
                if counter == idx {
                    return Some((server, tool_name.clone(), tool));
                }
                counter += 1;
            }
        }

        None
    }

    fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Always check the back button first (available in all views)
        if let Some(rect) = self.docs_rect
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
        {
            // If in docs or tool detail view, go back; otherwise show docs
            match self.focused_panel {
                FocusPanel::Documentation => self.close_documentation(),
                FocusPanel::ToolDetail => self.close_tool_detail(),
                _ => self.show_documentation(),
            }
            return;
        }

        // Don't handle other panel clicks when in detail or docs view
        // (to allow text selection in those views)
        if self.focused_panel == FocusPanel::ToolDetail
            || self.focused_panel == FocusPanel::Documentation
        {
            return;
        }

        // Check which panel was clicked
        if let Some(rect) = self.tools_rect
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
        {
            self.focused_panel = FocusPanel::Tools;

            // Check which namespace was clicked within the tools panel
            for (idx, namespace_rect) in self.namespace_rects.iter().enumerate() {
                if x >= namespace_rect.x
                    && x < namespace_rect.x + namespace_rect.width
                    && y >= namespace_rect.y
                    && y < namespace_rect.y + namespace_rect.height
                {
                    // Switch to the clicked namespace
                    self.selected_namespace_index = idx;
                    self.select_first_tool_in_current_namespace();
                    break;
                }
            }

            return;
        }

        if let Some(rect) = self.logs_rect
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
        {
            self.focused_panel = FocusPanel::Logs;
        }
    }

    fn handle_mouse_scroll(&mut self, x: u16, y: u16, scroll_up: bool) {
        // Handle scroll in tool detail view
        if self.focused_panel == FocusPanel::ToolDetail {
            if scroll_up {
                self.scroll_detail_up();
            } else {
                self.scroll_detail_down();
            }
            return;
        }

        // Handle scroll in documentation view
        if self.focused_panel == FocusPanel::Documentation {
            if scroll_up {
                self.scroll_detail_up();
            } else {
                self.scroll_detail_down();
            }
            return;
        }

        // Check if scrolling in tools panel
        if let Some(rect) = self.tools_rect
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
        {
            if scroll_up {
                self.scroll_tools_up();
            } else {
                self.scroll_tools_down();
            }
            return;
        }

        // Check if scrolling in logs panel
        if let Some(rect) = self.logs_rect
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
        {
            if scroll_up {
                self.scroll_logs_up();
            } else {
                self.scroll_logs_down();
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    // If in detail view, show full-screen tool detail
    if app.focused_panel == FocusPanel::ToolDetail {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Tool detail
                Constraint::Length(4), // Footer
            ])
            .split(f.area());

        render_header(f, app, chunks[0]);
        render_tool_detail(f, app, chunks[1]);
        render_footer(f, app, chunks[2]);
        return;
    }

    // If in documentation view, show full-screen documentation
    if app.focused_panel == FocusPanel::Documentation {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Documentation
                Constraint::Length(4), // Footer
            ])
            .split(f.area());

        render_header(f, app, chunks[0]);
        render_documentation(f, app, chunks[1]);
        render_footer(f, app, chunks[2]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(4), // Footer
        ])
        .split(f.area());

    // Header
    render_header(f, app, chunks[0]);

    // Main content area - split into top and bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Top: Tools + HeatMap
            Constraint::Percentage(40), // Bottom: Logs
        ])
        .split(chunks[1]);

    // Store panel boundaries for mouse click detection
    app.tools_rect = Some(main_chunks[0]);
    app.logs_rect = Some(main_chunks[1]);

    // Render panels
    render_tools_panel(f, app, main_chunks[0]);
    render_logs_panel(f, app, main_chunks[1]);

    // Footer with help text
    render_footer(f, app, chunks[2]);
}

fn render_header(f: &mut Frame, app: &mut App, area: Rect) {
    // Create a 3-column layout: Title | Server | Docs
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),    // Title (flexible)
            Constraint::Length(50), // Server URL
            Constraint::Length(12), // Docs button
        ])
        .split(area);

    // Title
    let title = vec![
        Span::styled(
            "PCTX ",
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Dev Mode", Style::default().fg(TEXT_COLOR)),
    ];
    let title_widget = Paragraph::new(Line::from(title))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(title_widget, chunks[0]);

    // Server URL
    let server_url = if app.server_running {
        app.get_server_url()
    } else {
        String::new()
    };

    let server_content = if app.server_running {
        vec![Span::styled(
            format!("{server_url} [c]"),
            Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![Span::styled(
            "Starting...",
            Style::default().fg(Color::Yellow),
        )]
    };
    let server_widget = Paragraph::new(Line::from(server_content))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(server_widget, chunks[1]);

    // Docs/Back button with keyboard shortcut hint
    // In ToolDetail view: show "Back" (goes to Tools)
    // In Documentation view: show "Back" (goes to Tools)
    // In Tools/Logs: show "Docs" (opens documentation)
    let (docs_text, docs_color) = match app.focused_panel {
        FocusPanel::ToolDetail => ("[d] Back", TERTIARY),
        FocusPanel::Documentation => ("[d] Back", TERTIARY),
        _ => ("[d] Docs", SECONDARY),
    };
    let docs_content = vec![Span::styled(
        docs_text,
        Style::default().fg(docs_color).add_modifier(Modifier::BOLD),
    )];
    let docs_widget = Paragraph::new(Line::from(docs_content))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(docs_widget, chunks[2]);

    // Store docs button rectangle for click detection
    app.docs_rect = Some(chunks[2]);
}

fn render_tools_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let is_focused = app.focused_panel == FocusPanel::Tools;
    let border_style = if is_focused {
        Style::default().fg(SECONDARY)
    } else {
        Style::default()
    };

    let total_tools: usize = app.upstream_servers.iter().map(|s| s.tools.len()).sum();
    let title = format!("MCP Tools [{total_tools} total]");

    // Sort servers alphabetically by name
    let mut sorted_servers: Vec<_> = app.upstream_servers.iter().collect();
    sorted_servers.sort_by(|a, b| a.name.cmp(&b.name));

    // Show loading state when server is starting or reloading
    if sorted_servers.is_empty() && !app.server_running {
        let placeholder = Paragraph::new("Loading MCP servers...")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
        return;
    }

    // Show "Reloading..." when server is running but no servers (config reload in progress)
    if sorted_servers.is_empty() && app.server_running {
        let placeholder = Paragraph::new("Reloading servers...")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            )
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
        return;
    }

    if sorted_servers.is_empty() {
        let help_lines = vec![
            Line::from(vec![Span::styled(
                "No MCP servers connected",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "To add upstream MCP servers:",
                Style::default().fg(TEXT_COLOR),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("1. ", Style::default().fg(SECONDARY)),
                Span::raw("Edit your "),
                Span::styled(
                    "pctx.json",
                    Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" config file"),
            ]),
            Line::from(vec![
                Span::styled("2. ", Style::default().fg(SECONDARY)),
                Span::raw("Add servers to the "),
                Span::styled("\"upstreams\"", Style::default().fg(TERTIARY)),
                Span::raw(" array"),
            ]),
            Line::from(vec![
                Span::styled("3. ", Style::default().fg(SECONDARY)),
                Span::raw("Config will reload automatically"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Example config:",
                Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                r#"  "upstreams": [{"#,
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                r"    {",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                r#"      "name": "my-server","#,
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                r#"      "url": "http://localhost:3000""#,
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                r"    }",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                r"  ]",
                Style::default().fg(Color::DarkGray),
            )]),
        ];

        let placeholder = Paragraph::new(help_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            )
            .alignment(Alignment::Left);
        f.render_widget(placeholder, area);
        return;
    }

    // Create horizontal layout for namespaces
    let num_servers = sorted_servers.len();
    let percentage_per_server = 100 / num_servers as u16;
    let constraints: Vec<Constraint> = (0..num_servers)
        .map(|_| Constraint::Percentage(percentage_per_server))
        .collect();

    let namespace_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    // Store namespace rectangles for mouse click detection
    app.namespace_rects = namespace_chunks.iter().copied().collect();

    // Render each namespace in its own column
    let mut global_tool_index = 0;

    for (idx, server) in sorted_servers.iter().enumerate() {
        let mut items: Vec<ListItem> = Vec::new();

        // Server header
        let server_status = if server.tools.is_empty() { "!" } else { "✓" };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{server_status} "), Style::default().fg(TERTIARY)),
            Span::styled(
                &server.name,
                Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
            ),
        ])));

        // Sort tools by usage count (descending)
        let mut tools_with_usage: Vec<_> = server
            .tools
            .iter()
            .map(|(fn_name, tool)| {
                let usage_key = format!("{}::{}", server.name, tool.tool_name);
                let usage_count = app.tool_usage.get(&usage_key).map_or(0, |u| u.count);
                (fn_name, tool, usage_count)
            })
            .collect();
        tools_with_usage.sort_by(|a, b| b.2.cmp(&a.2));

        // Track the starting index for this server's tools
        let tools_start_index = global_tool_index;

        // Render sorted tools
        for (fn_name, _tool, usage_count) in tools_with_usage {
            let is_selected_tool = app.selected_tool_index == Some(global_tool_index);

            let mut spans = vec![Span::styled(
                fn_name.as_str(),
                Style::default().fg(TERTIARY),
            )];

            // Add usage count in gray if > 0
            if usage_count > 0 {
                spans.push(Span::styled(
                    format!(" ({usage_count} calls)"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            // Add (enter) hint for selected tool
            if is_selected_tool && is_focused {
                spans.push(Span::styled(
                    " [enter]",
                    Style::default().fg(TERTIARY).add_modifier(Modifier::DIM),
                ));
            }

            items.push(ListItem::new(Line::from(spans)));
            global_tool_index += 1;
        }

        let namespace_title = format!("{} ({} tools)", server.name, server.tools.len());

        // Check if a tool in this namespace is selected
        let selected_in_this_namespace = app
            .selected_tool_index
            .filter(|&idx| idx >= tools_start_index && idx < global_tool_index)
            .map(|idx| idx - tools_start_index + 1); // +1 to account for header row

        let mut list_state = ListState::default();
        if let Some(local_idx) = selected_in_this_namespace {
            list_state.select(Some(local_idx));
        }

        // Highlight border of active namespace
        let namespace_border_style = if is_focused && idx == app.selected_namespace_index {
            Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD)
        } else {
            border_style
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(namespace_border_style)
                    .title(namespace_title),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(list, namespace_chunks[idx], &mut list_state);
    }
}

fn render_logs_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_panel == FocusPanel::Logs;
    let border_style = if is_focused {
        Style::default().fg(SECONDARY)
    } else {
        Style::default()
    };

    let filtered_logs = app.filtered_logs();
    let visible_height = area.height.saturating_sub(2) as usize;

    // Show most recent logs at the bottom
    let total_logs = filtered_logs.len();
    let end_idx = total_logs.saturating_sub(app.log_scroll_offset);
    let start_idx = end_idx.saturating_sub(visible_height);

    let log_items: Vec<Line> = filtered_logs[start_idx..end_idx]
        .iter()
        .map(|l| l.tui_line(app.log_filter))
        .collect();

    let title = format!(
        "Logs [Filter: {} - {}/{}]",
        app.log_filter.as_str().to_uppercase(),
        filtered_logs.len(),
        app.logs.len()
    );

    let logs = Paragraph::new(log_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(logs, area);
}

fn render_tool_detail(f: &mut Frame, app: &App, area: Rect) {
    if let Some((server, tool_name, tool)) = app.get_selected_tool() {
        let usage_key = format!("{}::{}", server.name, tool.tool_name);
        let usage = app.tool_usage.get(&usage_key);

        let mut lines: Vec<Line> = vec![
            // Tool header
            Line::from(vec![
                Span::styled(
                    "Server: ",
                    Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&server.name),
            ]),
            Line::from(vec![
                Span::styled(
                    "Function: ",
                    Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&tool_name),
            ]),
            Line::from(vec![
                Span::styled(
                    "Tool Name: ",
                    Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&tool.tool_name),
            ]),
            Line::from(""),
        ];

        // Description
        if let Some(desc) = &tool.description {
            lines.push(Line::from(vec![Span::styled(
                "Description:",
                Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(Span::raw(desc)));
            lines.push(Line::from(""));
        }

        // Usage stats
        if let Some(usage) = usage {
            lines.push(Line::from(vec![Span::styled(
                "Usage Stats:",
                Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(format!("  Calls: {}", usage.count)));
            lines.push(Line::from(format!(
                "  Last used: {}",
                usage.last_used.format("%Y-%m-%d %H:%M:%S")
            )));
            lines.push(Line::from(""));

            if !usage.code_snippets.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "Example Usage:",
                    Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
                )]));
                for snippet in &usage.code_snippets {
                    lines.push(Line::from(format!("  {snippet}")));
                }
                lines.push(Line::from(""));
            }
        }

        // Input type
        lines.push(Line::from(vec![Span::styled(
            "Input Type:",
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(format!("  {}", tool.input_type)));
        lines.push(Line::from(""));

        // Output type
        lines.push(Line::from(vec![Span::styled(
            "Output Type:",
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(format!("  {}", tool.output_type)));
        lines.push(Line::from(""));

        // TypeScript types
        lines.push(Line::from(vec![Span::styled(
            "TypeScript Definition:",
            Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
        )]));
        for line in tool.types.lines() {
            lines.push(Line::from(format!("  {line}")));
        }

        // Apply scroll
        let visible_height = area.height.saturating_sub(2) as usize;
        let start_idx = app.detail_scroll_offset;
        let end_idx = (start_idx + visible_height).min(lines.len());
        let visible_lines: Vec<Line> = lines[start_idx..end_idx].to_vec();

        let detail = Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(SECONDARY))
                    .title(format!(
                        "Tool Detail - {} [{}/{}]",
                        tool_name,
                        app.detail_scroll_offset + 1,
                        lines.len()
                    )),
            )
            .wrap(Wrap { trim: false });

        f.render_widget(detail, area);
    } else {
        let placeholder = Paragraph::new("No tool selected")
            .block(Block::default().borders(Borders::ALL).title("Tool Detail"))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
    }
}

fn render_documentation(f: &mut Frame, app: &App, area: Rect) {
    // Read and render the CLI.md documentation with nice markdown formatting
    const CLI_DOCS: &str = include_str!("../../../../docs/CLI.md");

    // Convert markdown to styled Text using tui-markdown
    let markdown_text = tui_markdown::from_str(CLI_DOCS);

    // Get all lines from the rendered markdown
    let all_lines: Vec<Line> = markdown_text.lines.clone();
    let total_lines = all_lines.len();

    // Apply scroll
    let visible_height = area.height.saturating_sub(2) as usize;
    let start_idx = app.detail_scroll_offset.min(total_lines.saturating_sub(1));
    let end_idx = (start_idx + visible_height).min(total_lines);

    // Create a new Text with only the visible lines
    let visible_text = ratatui::text::Text::from(all_lines[start_idx..end_idx].to_vec());

    let docs = Paragraph::new(visible_text)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "CLI Documentation [{}-{}/{}]",
            start_idx + 1,
            end_idx,
            total_lines
        )))
        .wrap(Wrap { trim: false });

    f.render_widget(docs, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut help_text = vec![Span::raw("[q] Quit  ")];

    // Always show copy URL if server is running
    if app.server_running {
        help_text.push(Span::raw("[c] Copy URL  "));
    }

    match app.focused_panel {
        FocusPanel::ToolDetail => {
            help_text.push(Span::raw("[d/Esc] Back  "));
            help_text.push(Span::raw("[↑/↓] Scroll  "));
            help_text.push(Span::raw("[PgUp/PgDn] Fast Scroll  "));
        }
        FocusPanel::Documentation => {
            help_text.push(Span::raw("[d/Esc] Back  "));
            help_text.push(Span::raw("[↑/↓] Scroll  "));
            help_text.push(Span::raw("[PgUp/PgDn] Fast Scroll  "));
            help_text.push(Span::raw("[Mouse] Select Text  "));
        }
        FocusPanel::Logs => {
            help_text.push(Span::raw("[d] Docs  "));
            help_text.push(Span::raw("[Tab] Switch Panel  "));
            help_text.push(Span::raw("[↑/↓] Navigate  "));
            help_text.push(Span::raw("[f] Filter Level  "));
        }
        FocusPanel::Tools => {
            help_text.push(Span::raw("[d] Docs  "));
            help_text.push(Span::raw("[Tab] Switch Panel  "));
            help_text.push(Span::raw("[↑/↓] Navigate  "));
            help_text.push(Span::raw("[←/→] Switch Namespace  "));
            help_text.push(Span::raw("[Enter] View Details  "));
        }
    }

    let footer = Paragraph::new(Line::from(help_text))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    f.render_widget(footer, area);
}

// Spawns the PctxMcp server task
// Returns (server_handle, shutdown_sender)
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
        let upstream = if cfg.servers.is_empty() {
            tracing::warn!(
                "No MCP servers configured, add servers with 'pctx add <name> <url>' and PCTX Dev Mode will refresh"
            );
            vec![]
        } else {
            let loaded = match StartCmd::load_upstream(&cfg).await {
                Ok(u) => u,
                Err(e) => {
                    tx.send(AppMessage::ServerFailed(
                        "Failed loading upstream MCPs".into(),
                        e.to_string(),
                    ))
                    .ok();
                    vec![]
                }
            };

            if loaded.is_empty() {
                tracing::warn!(
                    "Failed loading all configured MCP servers, add servers with 'pctx add <name> <url>' or edit {} and PCTX Dev Mode will refresh",
                    cfg.path()
                );
            }
            loaded
        };

        // Send connected message with all upstreams
        tx.send(AppMessage::ServerConnected(
            "all".to_string(),
            upstream.clone(),
        ))
        .ok();

        // Start PCTX server
        tx.send(AppMessage::ServerStarted).ok();

        // Run server with shutdown signal
        let pctx_mcp = PctxMcp::new(cfg.clone(), upstream, &host, port, false);

        if let Err(_e) = pctx_mcp
            .serve_with_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            // Error is already logged via tracing
        }

        tx.send(AppMessage::ServerStopped).ok();
    });

    (handle, shutdown_tx)
}

impl DevCmd {
    pub(crate) async fn handle(&self, cfg: Config) -> Result<Config> {
        // Set up terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
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
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
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
                    execute!(io::stdout(), EnableMouseCapture)?;
                } else {
                    execute!(io::stdout(), DisableMouseCapture)?;
                }
                mouse_capture_enabled = should_capture;
            }
        }

        // Draw UI
        {
            let mut app = app.lock().unwrap();
            terminal.draw(|f| ui(f, &mut app))?;
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
                            KeyCode::Esc => {
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
                                if app.server_running {
                                    let _ = app.copy_server_url_to_clipboard();
                                }
                            }
                            KeyCode::Char('d') => {
                                // 'd' behavior depends on context:
                                // - In Documentation: go back to tools
                                // - In ToolDetail: go back to tools (same as Esc)
                                // - In Tools/Logs: open documentation
                                match app.focused_panel {
                                    FocusPanel::Documentation => app.close_documentation(),
                                    FocusPanel::ToolDetail => app.close_tool_detail(),
                                    _ => app.show_documentation(),
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
                            tracing::error!("Failed to reload config: {:?}", e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::upstream::{UpstreamMcp, UpstreamTool};
    use indexmap::IndexMap;
    use serde_json::json;
    use url::Url;

    fn create_test_server() -> UpstreamMcp {
        let mut tools = IndexMap::new();

        tools.insert(
            "getAccountBalance".to_string(),
            UpstreamTool {
                tool_name: "get_account_balance".to_string(),
                title: Some("Get Account Balance".to_string()),
                description: Some("Retrieves the balance for an account".to_string()),
                fn_name: "getAccountBalance".to_string(),
                input_type: "GetAccountBalanceInput".to_string(),
                output_type: "GetAccountBalanceOutput".to_string(),
                types: "interface GetAccountBalanceInput { account_id: string }".to_string(),
            },
        );

        tools.insert(
            "freezeAccount".to_string(),
            UpstreamTool {
                tool_name: "freeze_account".to_string(),
                title: Some("Freeze Account".to_string()),
                description: Some("Freezes an account".to_string()),
                fn_name: "freezeAccount".to_string(),
                input_type: "FreezeAccountInput".to_string(),
                output_type: "FreezeAccountOutput".to_string(),
                types: "interface FreezeAccountInput { account_id: string }".to_string(),
            },
        );

        UpstreamMcp {
            name: "banking".to_string(),
            namespace: "Banking".to_string(), // PascalCase namespace
            description: "Banking MCP Server".to_string(),
            url: Url::parse("http://localhost:3000").unwrap(),
            tools,
            registration: json!({}),
        }
    }

    #[test]
    fn test_track_tool_usage_with_banking_namespace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = Utf8PathBuf::from_path_buf(temp_dir.path().join("test.jsonl")).unwrap();

        let mut app = App::new("localhost".to_string(), 8080, log_file);

        // Add the test server
        app.upstream_servers.push(create_test_server());

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
        app.upstream_servers.push(create_test_server());

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
        app.upstream_servers.push(create_test_server());

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
