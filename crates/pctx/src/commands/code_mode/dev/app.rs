use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    time::Instant,
};

use anyhow::Result;
use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use pctx_codegen::{Tool, ToolSet};
use pctx_config::{Config, logger::LogLevel};
use ratatui::{layout::Rect, widgets::ListState};

use super::log_entry::LogEntry;
use pctx_code_mode::CodeMode;

// -------- APP STATE & CONTROLS ---------

#[derive(Clone)]
pub(super) enum AppMessage {
    ServerStarting,
    ServerReady(Config, CodeMode),
    ServerFailed(String),
    ServerStopped,
    ConfigChanged,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum FocusPanel {
    Namespaces,
    Logs,
    ToolDetail,
    Documentation,
}

#[derive(Debug, Clone)]
pub(super) struct ToolUsage {
    #[allow(dead_code)]
    pub(super) tool_name: String,
    #[allow(dead_code)]
    pub(super) server_name: String,
    pub(super) count: usize,
    pub(super) last_used: DateTime<Utc>,
    pub(super) code_snippets: Vec<String>,
}

pub(super) struct App {
    pub(super) logs: Vec<LogEntry>,
    pub(super) code_mode: CodeMode,
    pub(super) config: Config,
    pub(super) server_ready: bool,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) start_time: Option<Instant>,
    pub(super) log_scroll_offset: usize,
    pub(super) log_visible_height: usize,
    pub(super) log_file_path: Utf8PathBuf,
    pub(super) log_file_pos: u64,

    // UI State
    pub(super) error: Option<String>,
    pub(super) focused_panel: FocusPanel,
    pub(super) log_filter: LogLevel,
    #[allow(dead_code)]
    pub(super) tools_list_state: ListState,
    pub(super) selected_tool_index: Option<usize>,
    pub(super) selected_namespace_index: usize, // Index of currently selected namespace/server
    pub(super) detail_scroll_offset: usize,
    pub(super) detail_max_scroll: usize,

    // Tool usage tracking
    pub(super) tool_usage: HashMap<String, ToolUsage>,

    // Panel boundaries for mouse click detection
    pub(super) tools_rect: Option<Rect>,
    pub(super) logs_rect: Option<Rect>,
    pub(super) namespace_rects: Vec<Rect>, // Rectangles for each namespace column
    pub(super) docs_rect: Option<Rect>,    // Rectangle for documentation column
    pub(super) url_rect: Option<Rect>,     // Rectangle for server URL box

    // Clipboard feedback
    pub(super) copied_at: Option<Instant>,
}

impl App {
    pub(super) fn new(host: String, port: u16, log_file_path: Utf8PathBuf) -> Self {
        Self {
            logs: Vec::new(),
            code_mode: CodeMode::default(), // set on first ServerReady event
            config: Config::default(),      // set on first ServerReady event
            server_ready: false,
            host,
            port,
            start_time: None,
            error: None,
            log_scroll_offset: 0,
            log_visible_height: 0,
            log_file_path,
            log_file_pos: 0,
            focused_panel: FocusPanel::Namespaces,
            log_filter: LogLevel::Info,
            tools_list_state: ListState::default(),
            selected_tool_index: None,
            selected_namespace_index: 0,
            detail_scroll_offset: 0,
            detail_max_scroll: 0,
            tool_usage: HashMap::new(),
            tools_rect: None,
            logs_rect: None,
            namespace_rects: Vec::new(),
            docs_rect: None,
            url_rect: None,
            copied_at: None,
        }
    }

    pub(super) fn get_server_url(&self) -> String {
        format!("http://{}:{}/mcp", self.host, self.port)
    }

    pub(super) fn copy_server_url_to_clipboard(&self) -> Result<()> {
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

    pub(super) fn read_new_logs(&mut self) -> Result<()> {
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

    // TODO: track tool usage from pctx_registry logs.
    pub(super) fn track_tool_usage(&mut self, entry: &LogEntry) {
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
                self.code_mode.tool_sets().len()
            );

            // Parse the code to find upstream tool calls like "await Banking.getAccountBalance(" (fs or catalog mode)
            // or "await invoke({ name: 'banking__get_account_balance'," (sidecar)
            // Pattern: namespace.methodName(
            for tool_set in self.code_mode.tool_sets() {
                tracing::trace!(
                    "Checking for server '{:?}' for usage in code",
                    &tool_set.name
                );

                for tool in &tool_set.tools {
                    let namespace_pattern = regex::Regex::new(&format!(
                        r"await\s+{}\.{}\(",
                        tool_set.pascal_namespace(),
                        &tool.fn_name
                    ))
                    .unwrap();
                    let sidecar_pattern = regex::Regex::new(&format!(
                        r#"await\s+invoke\(\s*\{{\s*["']?name["']?:\s*["']{}["']"#,
                        tool.id(tool_set.name.as_deref())
                    ))
                    .unwrap();

                    tracing::trace!(
                        "Checking for tool '{}' (namespace_pattern={}, sidecar_pattern={})",
                        &tool.name,
                        namespace_pattern,
                        sidecar_pattern,
                    );

                    // Only one pattern will ever match (namespace vs sidecar style),
                    // but there may be multiple call-sites in the code.
                    let matches: Vec<_> = {
                        let ns = namespace_pattern
                            .find_iter(code_from_llm)
                            .collect::<Vec<_>>();
                        if !ns.is_empty() {
                            ns
                        } else {
                            sidecar_pattern.find_iter(code_from_llm).collect()
                        }
                    };

                    for m in matches {
                        let snippet_start = m.start().saturating_sub(10);
                        let snippet_end = (m.end() + 50).min(code_from_llm.len());
                        let code_snippet = code_from_llm[snippet_start..snippet_end]
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        let key = tool.id(tool_set.name.as_deref());

                        self.tool_usage
                            .entry(key.clone())
                            .and_modify(|usage| {
                                usage.count += 1;
                                tracing::trace!("usage count: {}", usage.count);
                                usage.last_used = entry.timestamp;
                                if !code_snippet.is_empty()
                                    && !usage.code_snippets.contains(&code_snippet)
                                {
                                    usage.code_snippets.push(code_snippet.clone());
                                }
                            })
                            .or_insert_with(|| ToolUsage {
                                tool_name: tool.name.clone(),
                                server_name: tool_set.name.clone().unwrap_or_default(),
                                count: 1,
                                last_used: entry.timestamp,
                                code_snippets: if code_snippet.is_empty() {
                                    vec![]
                                } else {
                                    vec![code_snippet]
                                },
                            });

                        tracing::debug!("✓ Tracked tool usage for key: {key}");
                    }
                }
            }
        }
    }

    pub(super) fn reprocess_logs_for_tool_usage(&mut self) {
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

    pub(super) fn filtered_logs(&self) -> Vec<&LogEntry> {
        self.logs
            .iter()
            .filter(|l| self.log_filter <= l.level && !l.fields.message.is_empty())
            .collect()
    }

    pub(super) fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::ServerReady(cfg, code_mode) => {
                self.server_ready = true;
                self.error = None;
                self.code_mode = code_mode;
                self.config = cfg;

                // Auto-select the first tool
                let has_tools = self
                    .code_mode
                    .tool_sets()
                    .iter()
                    .any(|ts| !ts.tools.is_empty());
                if has_tools && self.selected_tool_index.is_none() {
                    self.selected_tool_index = Some(0);
                }

                // Re-process all existing logs now that we have server metadata
                tracing::info!(
                    "Server Connected: {} servers available. Re-processing existing logs for tool usage tracking.",
                    self.code_mode.tool_sets().len()
                );
                self.reprocess_logs_for_tool_usage();
            }
            AppMessage::ServerFailed(err) => {
                tracing::error!("{err}");
                self.server_ready = false;
                self.error = Some(err);
            }
            AppMessage::ServerStarting => {
                self.server_ready = false;
                self.start_time = Some(Instant::now());
            }
            AppMessage::ServerStopped => {
                self.server_ready = false;
            }
            AppMessage::ConfigChanged => {
                tracing::info!("Configuration file changed, reloading servers...");
                // Clear existing servers - they will be repopulated when reconnection completes
                self.code_mode = CodeMode::default();
                self.selected_tool_index = None;
                self.selected_namespace_index = 0;
            }
        }
    }

    pub(super) fn scroll_logs_up(&mut self) {
        // Scroll up = go back in time = increase offset
        // Stop when the first log is already in frame
        let filtered_count = self.filtered_logs().len();
        let max_offset = filtered_count.saturating_sub(self.log_visible_height);
        if self.log_scroll_offset < max_offset {
            self.log_scroll_offset += 1;
        }
    }

    pub(super) fn scroll_logs_down(&mut self) {
        // Scroll down = go forward in time = decrease offset (0 = most recent)
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
    }

    pub(super) fn cycle_log_filter(&mut self) {
        self.log_filter = match self.log_filter {
            LogLevel::Debug => LogLevel::Info,
            LogLevel::Info => LogLevel::Warn,
            LogLevel::Warn => LogLevel::Error,
            LogLevel::Error | LogLevel::Trace => LogLevel::Debug,
        };
        self.log_scroll_offset = 0;
    }

    pub(super) fn next_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusPanel::Namespaces => FocusPanel::Logs,
            FocusPanel::Logs => FocusPanel::Namespaces,
            FocusPanel::ToolDetail => FocusPanel::ToolDetail, // Stay in detail view
            FocusPanel::Documentation => FocusPanel::Documentation, // Stay in docs view
        };
    }

    pub(super) fn prev_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusPanel::Namespaces => FocusPanel::Logs,
            FocusPanel::Logs => FocusPanel::Namespaces,
            FocusPanel::ToolDetail => FocusPanel::ToolDetail, // Stay in detail view
            FocusPanel::Documentation => FocusPanel::Documentation, // Stay in docs view
        };
    }

    pub(super) fn show_tool_detail(&mut self) {
        if self.selected_tool_index.is_some() {
            self.focused_panel = FocusPanel::ToolDetail;
            self.detail_scroll_offset = 0;
        }
    }

    pub(super) fn show_documentation(&mut self) {
        self.focused_panel = FocusPanel::Documentation;
        self.detail_scroll_offset = 0;
    }

    pub(super) fn close_tool_detail(&mut self) {
        self.focused_panel = FocusPanel::Namespaces;
    }

    pub(super) fn close_documentation(&mut self) {
        self.focused_panel = FocusPanel::Namespaces;
    }

    pub(super) fn scroll_detail_up(&mut self) {
        // Scroll faster (3 lines at a time) for better UX
        self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(3);
    }

    pub(super) fn scroll_detail_down(&mut self) {
        // Scroll faster (3 lines at a time) for better UX
        self.detail_scroll_offset = (self.detail_scroll_offset + 3).min(self.detail_max_scroll);
    }

    pub(super) fn scroll_tools_down(&mut self) {
        // Sort servers alphabetically (same as rendering)
        let mut sorted: Vec<ToolSet> = self.code_mode.tool_sets().iter().cloned().collect();
        sorted.sort_by_key(|s| s.name.clone());

        if sorted.is_empty() {
            return;
        }

        // Get current namespace's tool count
        if self.selected_namespace_index >= sorted.len() {
            return;
        }

        let current_server = &sorted[self.selected_namespace_index];
        let tools_in_namespace = current_server.tools.len();
        if tools_in_namespace == 0 {
            return;
        }

        // Calculate global indices for this namespace
        let namespace_start_idx: usize = sorted
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

    pub(super) fn scroll_tools_up(&mut self) {
        // Sort servers alphabetically (same as rendering)
        let mut sorted: Vec<ToolSet> = self.code_mode.tool_sets().iter().cloned().collect();
        sorted.sort_by_key(|s| s.name.clone());

        if sorted.is_empty() {
            return;
        }

        // Get current namespace's start index
        if self.selected_namespace_index >= sorted.len() {
            return;
        }

        let namespace_start_idx: usize = sorted
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

    pub(super) fn move_to_next_namespace(&mut self) {
        if self.code_mode.tool_sets().is_empty() {
            return;
        }

        // Sort servers alphabetically (same as rendering)
        let mut sorted: Vec<ToolSet> = self.code_mode.tool_sets().iter().cloned().collect();
        sorted.sort_by_key(|s| s.name.clone());

        let num_namespaces = sorted.len();
        if num_namespaces == 0 {
            return;
        }

        // Move to next namespace (wrap around)
        self.selected_namespace_index = (self.selected_namespace_index + 1) % num_namespaces;

        // Select first tool in new namespace
        self.select_first_tool_in_current_namespace();
    }

    pub(super) fn move_to_prev_namespace(&mut self) {
        if self.code_mode.tool_sets().is_empty() {
            return;
        }

        // Sort servers alphabetically (same as rendering)
        let mut sorted: Vec<ToolSet> = self.code_mode.tool_sets().iter().cloned().collect();
        sorted.sort_by_key(|s| s.name.clone());

        let num_namespaces = sorted.len();
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

    pub(super) fn select_first_tool_in_current_namespace(&mut self) {
        // Sort servers alphabetically (same as rendering)
        let mut sorted: Vec<ToolSet> = self.code_mode.tool_sets().iter().cloned().collect();
        sorted.sort_by_key(|s| s.name.clone());

        if self.selected_namespace_index >= sorted.len() {
            self.selected_tool_index = None;
            return;
        }

        // Calculate the index of the first tool in the selected namespace
        let mut tool_index = 0;
        for (idx, server) in sorted.iter().enumerate() {
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

    pub(super) fn get_selected_tool(&self) -> Option<(ToolSet, Tool)> {
        let idx = self.selected_tool_index?;
        let mut counter = 0;

        // Sort servers alphabetically (same as rendering)
        let mut sorted: Vec<ToolSet> = self.code_mode.tool_sets().iter().cloned().collect();
        sorted.sort_by_key(|s| s.name.clone());

        for tool_set in sorted {
            // Sort tools by usage count (same as rendering)
            let mut tools_with_usage: Vec<_> = tool_set
                .tools
                .iter()
                .map(|tool| {
                    let usage_key = tool.id(tool_set.name.as_deref());
                    let usage_count = self.tool_usage.get(&usage_key).map_or(0, |u| u.count);
                    (tool.clone(), usage_count)
                })
                .collect();
            tools_with_usage.sort_by(|a, b| b.1.cmp(&a.1));

            for (tool, _usage_count) in tools_with_usage {
                if counter == idx {
                    return Some((tool_set, tool));
                }
                counter += 1;
            }
        }

        None
    }

    pub(super) fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Check URL box click — copy to clipboard
        if let Some(rect) = self.url_rect
            && self.server_ready
            && x >= rect.x
            && x < rect.x + rect.width
            && y >= rect.y
            && y < rect.y + rect.height
        {
            let _ = self.copy_server_url_to_clipboard();
            self.copied_at = Some(Instant::now());
            return;
        }

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
            self.focused_panel = FocusPanel::Namespaces;

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

    pub(super) fn handle_mouse_scroll(&mut self, x: u16, y: u16, scroll_up: bool) {
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
