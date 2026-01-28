use pctx_codegen::ToolSet;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use super::{
    SECONDARY, TERTIARY, TEXT_COLOR,
    app::{App, FocusPanel},
};

pub(super) fn ui(f: &mut Frame, app: &mut App) {
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
        Span::styled("PCTX ", Style::default().fg(SECONDARY).bold()),
        Span::styled("Dev Mode", Style::default().fg(TEXT_COLOR)),
    ];
    let title_widget = Paragraph::new(Line::from(title))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(title_widget, chunks[0]);

    // Url

    let url_span = if app.server_ready {
        Span::styled(
            format!("{} [c]", app.get_server_url()),
            Style::default().fg(TERTIARY).bold(),
        )
    } else {
        Span::raw("")
    };
    let url_widget = Paragraph::new(Line::from(url_span))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(url_widget, chunks[1]);

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

    if let Some(err) = &app.error {
        let placeholder = Paragraph::new(err.clone())
            .block(
                Block::default()
                    .borders(Borders::all())
                    .border_style(border_style)
                    .title("Error"),
            )
            .style(Style::default().red())
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
        return;
    } else if !app.server_ready {
        // Show starting state when server is starting or reloading
        let placeholder = Paragraph::new("Starting server...")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .style(Style::default().yellow())
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
        return;
    }

    let total_tools: usize = app.tools.tool_sets().iter().map(|s| s.tools.len()).sum();
    let title = format!("MCP Tools [{total_tools} total]");

    // Sort servers alphabetically by name
    let mut sorted: Vec<ToolSet> = app.tools.tool_sets().iter().cloned().collect();
    sorted.sort_by_key(|s| s.name.clone());

    if sorted.is_empty() {
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
                Span::raw("Server will restart automatically"),
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
    let num_servers = sorted.len();
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

    for (idx, tool_set) in sorted.iter().enumerate() {
        let mut items: Vec<ListItem> = Vec::new();

        // Server header
        let status = if tool_set.tools.is_empty() {
            "!"
        } else {
            "✓"
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{status} "), Style::default().fg(TERTIARY)),
            Span::styled(
                &tool_set.name,
                Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
            ),
        ])));

        // Sort tools by usage count (descending)
        let mut tools_with_usage: Vec<_> = tool_set
            .tools
            .iter()
            .map(|tool| {
                let usage_key = format!("{}::{}", tool_set.name, tool.name);
                let usage_count = app.tool_usage.get(&usage_key).map_or(0, |u| u.count);
                (tool, usage_count)
            })
            .collect();
        tools_with_usage.sort_by(|a, b| b.1.cmp(&a.1));

        // Track the starting index for this server's tools
        let tools_start_index = global_tool_index;

        // Render sorted tools
        for (tool, usage_count) in tools_with_usage {
            let is_selected_tool = app.selected_tool_index == Some(global_tool_index);

            let mut spans = vec![Span::styled(&tool.fn_name, Style::default().fg(TERTIARY))];

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

        let namespace_title = format!("{} ({} tools)", tool_set.name, tool_set.tools.len());

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
    if let Some((tool_set, tool)) = app.get_selected_tool() {
        let usage_key = format!("{}::{}", tool_set.name, tool.name);
        let usage = app.tool_usage.get(&usage_key);

        let mut lines: Vec<Line> = vec![
            // Tool header
            Line::from(vec![
                Span::styled(
                    "Server: ",
                    Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&tool_set.name),
            ]),
            Line::from(vec![
                Span::styled(
                    "Function: ",
                    Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&tool.fn_name),
            ]),
            Line::from(vec![
                Span::styled(
                    "Tool Name: ",
                    Style::default().fg(TERTIARY).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&tool.name),
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
        lines.push(Line::from(format!("  {}", tool.input_signature)));
        lines.push(Line::from(""));

        // Output type
        lines.push(Line::from(vec![Span::styled(
            "Output Type:",
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(format!("  {}", tool.output_signature)));
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

        let start_idx = app.detail_scroll_offset.min(lines.len().saturating_sub(1));
        let end_idx = (start_idx + visible_height).min(lines.len());
        let visible_lines: Vec<Line> = lines[start_idx..end_idx].to_vec();

        let detail = Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(SECONDARY))
                    .title(format!(
                        "Tool Detail - {} [{}/{}]",
                        tool.name,
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
    // Read and render the CLI.md documentation
    const CLI_DOCS: &str = include_str!("../../../../../../docs/CLI.md");

    // Split into lines
    let all_lines: Vec<&str> = CLI_DOCS.lines().collect();
    let total_lines = all_lines.len();

    // Apply scroll
    let visible_height = area.height.saturating_sub(2) as usize;
    let start_idx = app.detail_scroll_offset.min(total_lines.saturating_sub(1));
    let end_idx = (start_idx + visible_height).min(total_lines);

    // Create text with only the visible lines
    let visible_text = all_lines[start_idx..end_idx].join("\n");

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
    if app.server_ready {
        help_text.push(Span::raw("[c] Copy URL  "));
    }

    let back = Span::raw("[⌫/Esc] Back  ");
    let scroll = Span::raw("[↑/↓] Scroll  ");
    let fast_scroll = Span::raw("[PgUp/PgDn] Fast Scroll  ");
    let select_text = Span::raw("[Mouse] Select Text  ");
    let docs = Span::raw("[d] Docs  ");
    let filter_level = Span::raw("[f] Filter Level  ");
    let switch_panel = Span::raw("[Tab] Switch Panel  ");
    let navigate = Span::raw("[↑/↓] Navigate  ");
    let switch_namespace = Span::raw("[←/→] Switch Namespace  ");
    let view_details = Span::raw("[↵ Enter] View Details  ");

    match app.focused_panel {
        FocusPanel::ToolDetail => {
            help_text.extend([back, scroll, fast_scroll]);
        }
        FocusPanel::Documentation => {
            help_text.extend([back, scroll, fast_scroll, select_text]);
        }
        FocusPanel::Logs => {
            help_text.extend([docs, switch_panel, navigate, filter_level]);
        }
        FocusPanel::Tools => {
            help_text.extend([docs, switch_panel, navigate, switch_namespace, view_details]);
        }
    }

    let footer = Paragraph::new(Line::from(help_text))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    f.render_widget(footer, area);
}
