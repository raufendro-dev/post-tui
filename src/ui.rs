use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::Frame,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};

use crate::{
    app::{
        request_field_label, sidebar_label, summarize_auth, App, EditTarget, Modal, Panel,
        RequestField, ResponseTab, SidebarEntry,
    },
    models::{BodyConfig, Header, QueryParam, ResponseModel},
    theme,
};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.size();
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(area);

    render_top_bar(frame, root[0], app);
    render_main(frame, root[1], app);
    render_footer(frame, root[2], app);

    if let Some(modal) = &app.modal {
        render_modal(frame, centered_rect(72, 72, area), app, modal);
    }
}

fn render_main(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(40)])
        .split(area);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(horizontal[1]);

    render_sidebar(frame, horizontal[0], app);
    render_request(frame, right[0], app);
    render_response(frame, right[1], app);
}

fn render_top_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mode = if app.edit_target.is_some() {
        "EDIT"
    } else if app.is_sending {
        "SENDING"
    } else {
        "NORMAL"
    };
    let line = Line::from(vec![
        Span::styled(
            " post-tui ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {mode} "), Style::default().fg(theme::TEXT)),
        Span::styled(
            " ? help  q quit  r run  u curl  Enter edit/load  Ctrl+s save  Tab focus ",
            Style::default().fg(theme::MUTED),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::PANEL)),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let editing = app
        .edit_target
        .map(|target| format!("Editing {target:?}: Enter apply, Esc cancel"))
        .unwrap_or_else(|| {
            "j/k move  h/l tabs/method/auth  Enter edit/load  r run  u curl  d delete  o import  / search"
                .to_string()
        });
    let text = format!(" {} | {}", app.status_line, editing);
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme::MUTED).bg(theme::PANEL)),
        area,
    );
}

fn render_sidebar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = app
        .sidebar_entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut style = match entry {
                SidebarEntry::Collection { .. } => Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
                SidebarEntry::Folder { .. } => Style::default().fg(theme::TEXT),
                SidebarEntry::Request { .. } | SidebarEntry::Saved { .. } => {
                    Style::default().fg(theme::TEXT)
                }
                SidebarEntry::History { .. } => Style::default().fg(theme::MUTED),
            };
            if app.focus == Panel::Sidebar && index == app.selected_sidebar && app.blink_on {
                style = style.fg(theme::WARNING).add_modifier(Modifier::BOLD);
            }
            ListItem::new(Line::from(Span::styled(
                sidebar_label(entry).to_string(),
                style,
            )))
        })
        .collect::<Vec<_>>();

    let mut state = ListState::default();
    state.select(Some(app.selected_sidebar));
    let list = List::new(items)
        .block(panel_block(
            "Collections / History",
            app.focus == Panel::Sidebar,
        ))
        .highlight_style(
            Style::default()
                .bg(theme::SELECTED_BG)
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_request(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = vec![
        request_row(RequestField::Method, app, method_value(app)),
        request_row(RequestField::Url, app, app.current_request.url.clone()),
        request_row(
            RequestField::Headers,
            app,
            summarize_headers(&app.current_request.headers),
        ),
        request_row(
            RequestField::Query,
            app,
            summarize_query(&app.current_request.query_params),
        ),
        request_row(
            RequestField::Auth,
            app,
            summarize_auth(&app.current_request.auth),
        ),
        request_row(
            RequestField::Body,
            app,
            summarize_body(&app.current_request.body),
        ),
    ];

    let mut text = Text::default();
    text.lines.extend(rows);
    text.lines.push(Line::from(""));
    text.lines.push(Line::from(vec![
        Span::styled("Run: ", Style::default().fg(theme::MUTED)),
        Span::styled("r", Style::default().fg(theme::TEXT)),
        Span::styled("   Edit field: ", Style::default().fg(theme::MUTED)),
        Span::styled("Enter", Style::default().fg(theme::TEXT)),
    ]));

    if let Some(target) = app.edit_target {
        if !matches!(
            target,
            EditTarget::ImportPath | EditTarget::Search | EditTarget::ResponseSearch
        ) {
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                format!("> {}", app.input.value_with_cursor(app.blink_on)),
                Style::default().fg(theme::WARNING),
            )));
        }
    }

    frame.render_widget(
        Paragraph::new(text)
            .block(panel_block("Request Builder", app.focus == Panel::Request))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn request_row(field: RequestField, app: &App, value: String) -> Line<'static> {
    let focused = app.focus == Panel::Request && app.request_field == field;
    let marker = if focused { "›" } else { " " };
    let label_style = if focused {
        let color = if app.blink_on {
            theme::WARNING
        } else {
            theme::ACCENT
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };

    Line::from(vec![
        Span::styled(
            format!("{marker} {:<8}", request_field_label(field)),
            label_style,
        ),
        Span::styled(value, Style::default().fg(theme::TEXT)),
    ])
}

fn method_value(app: &App) -> String {
    let method = app.current_request.method.as_str();
    if app.focus == Panel::Request && app.request_field == RequestField::Method {
        format!("{method}   Arrow <- or -> to change")
    } else {
        method.to_string()
    }
}

fn render_response(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(4),
        ])
        .split(area);

    let titles = ["Pretty", "Tree", "Raw", "HTML", "Headers", "Error"]
        .iter()
        .map(|title| Line::from(*title))
        .collect::<Vec<_>>();
    let tab_index = match app.response_tab {
        ResponseTab::Pretty => 0,
        ResponseTab::JsonTree => 1,
        ResponseTab::Raw => 2,
        ResponseTab::Html => 3,
        ResponseTab::Headers => 4,
        ResponseTab::Error => 5,
    };

    frame.render_widget(
        Tabs::new(titles)
            .select(tab_index)
            .block(panel_block("Response", app.focus == Panel::Response))
            .style(Style::default().fg(theme::MUTED))
            .highlight_style(
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        chunks[0],
    );

    render_response_actions(frame, chunks[1], app);

    let body = response_text_widget(app);
    let status = response_status(app.response.as_ref());
    frame.render_widget(
        Paragraph::new(body)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .title(status),
            )
            .style(Style::default().fg(theme::TEXT))
            .scroll((app.response_scroll, 0))
            .wrap(Wrap { trim: false }),
        chunks[2],
    );
}

fn render_response_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(" j/k scroll ", Style::default().fg(theme::MUTED)),
        Span::styled(" PgUp/PgDn ", Style::default().fg(theme::MUTED)),
        Span::styled(" / search ", Style::default().fg(theme::MUTED)),
        Span::styled(
            format!(" line {} ", app.response_scroll.saturating_add(1)),
            Style::default().fg(theme::TEXT),
        ),
    ];

    if !app.response_search.is_empty() {
        let matches = response_match_count(app);
        spans.push(Span::styled(
            format!(" '{}' {} match(es) ", app.response_search, matches),
            Style::default().fg(theme::WARNING),
        ));
    }

    if app.response_tab == ResponseTab::Html {
        let enabled = app
            .response
            .as_ref()
            .and_then(|response| response.html_body.as_ref())
            .is_some();
        let style = if enabled {
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::MUTED)
        };
        spans.push(Span::styled(" [b] View in browser ", style));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::PANEL)),
        area,
    );
}

fn response_status(response: Option<&ResponseModel>) -> String {
    match response {
        Some(response) if response.error.is_some() => " Request error ".to_string(),
        Some(response) => format!(
            " {} {} | {} ms | {} B ",
            response.status.unwrap_or_default(),
            response.status_text,
            response.elapsed_ms,
            response.size_bytes
        ),
        None => " No response yet ".to_string(),
    }
}

fn response_text(
    response: Option<&ResponseModel>,
    tab: ResponseTab,
    prefer_pretty: bool,
) -> String {
    let Some(response) = response else {
        return "Send a request to see the response here.".to_string();
    };

    match tab {
        ResponseTab::Pretty if prefer_pretty => response
            .pretty_body
            .clone()
            .unwrap_or_else(|| response.body.clone()),
        ResponseTab::Pretty => response.body.clone(),
        ResponseTab::JsonTree => response
            .json_tree_body
            .clone()
            .unwrap_or_else(|| "No JSON tree view for this response.".to_string()),
        ResponseTab::Raw => response.body.clone(),
        ResponseTab::Html => response
            .html_body
            .clone()
            .unwrap_or_else(|| "No HTML view for this response.".to_string()),
        ResponseTab::Headers => response
            .headers
            .iter()
            .map(|header| format!("{}: {}", header.key, header.value))
            .collect::<Vec<_>>()
            .join("\n"),
        ResponseTab::Error => response
            .error
            .clone()
            .unwrap_or_else(|| "No error for the last response.".to_string()),
    }
}

fn response_text_widget(app: &App) -> Text<'static> {
    let body = response_text(
        app.response.as_ref(),
        app.response_tab,
        app.config.response_pretty,
    );

    match app.response_tab {
        ResponseTab::Headers | ResponseTab::JsonTree => color_key_value_lines(&body),
        ResponseTab::Pretty => color_json_like_lines(&body),
        _ => Text::from(body),
    }
}

fn color_key_value_lines(body: &str) -> Text<'static> {
    let lines = body
        .lines()
        .map(|line| {
            if let Some((key, value)) = line.split_once(':') {
                Line::from(vec![
                    Span::styled(key.to_string(), Style::default().fg(theme::ACCENT)),
                    Span::styled(": ", Style::default().fg(theme::MUTED)),
                    Span::styled(
                        value.trim_start().to_string(),
                        Style::default().fg(theme::TEXT),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme::TEXT),
                ))
            }
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn color_json_like_lines(body: &str) -> Text<'static> {
    let lines = body
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let indent_len = line.len().saturating_sub(trimmed.len());
            if trimmed.starts_with('"') {
                if let Some(colon) = trimmed.find("\":") {
                    let indent = &line[..indent_len];
                    let key = &trimmed[..colon + 1];
                    let rest = &trimmed[colon + 1..];
                    return Line::from(vec![
                        Span::raw(indent.to_string()),
                        Span::styled(key.to_string(), Style::default().fg(theme::ACCENT)),
                        Span::styled(rest.to_string(), Style::default().fg(theme::TEXT)),
                    ]);
                }
            }
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(theme::TEXT),
            ))
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn response_match_count(app: &App) -> usize {
    if app.response_search.trim().is_empty() {
        return 0;
    }
    response_text(
        app.response.as_ref(),
        app.response_tab,
        app.config.response_pretty,
    )
    .lines()
    .filter(|line| {
        line.to_ascii_lowercase()
            .contains(&app.response_search.to_ascii_lowercase())
    })
    .count()
}

fn render_modal(frame: &mut Frame<'_>, area: Rect, app: &App, modal: &Modal) {
    frame.render_widget(Clear, area);
    let title = match modal {
        Modal::Help => "Help",
        Modal::About => "About",
        Modal::Curl(_) => "Generated cURL",
        Modal::Import => "Import Collection",
        Modal::Search => "Search",
        Modal::ResponseSearch => "Search Response",
        Modal::ConfirmNewRequest => "New Request",
        Modal::ConfirmClearHistory => "Clear History",
        Modal::ConfirmDeleteSidebar(_) => "Delete Sidebar Item",
        Modal::Error(_) => "Error",
        Modal::Info(_) => "Info",
    };

    let content = match modal {
        Modal::Help => help_text(),
        Modal::About => about_text(),
        Modal::Curl(command) => format!("{command}\n\nEsc closes this message."),
        Modal::Import => format!(
            "Type the path to a Collection v2.1 JSON file.\n\nPath:\n{}\n\nEnter imports, Esc closes.",
            app.input.value_with_cursor(app.blink_on)
        ),
        Modal::Search => format!(
            "Filter collections, saved requests, and history.\n\nSearch:\n{}\n\nEnter keeps filter, Esc clears.",
            app.input.value_with_cursor(app.blink_on)
        ),
        Modal::ResponseSearch => format!(
            "Search within the active response tab.\n\nSearch:\n{}\n\nEnter jumps to the first match, Esc cancels.",
            app.input.value_with_cursor(app.blink_on)
        ),
        Modal::ConfirmNewRequest => {
            "Create a new request and clear the current builder/response?\n\nPress y to confirm, n or Esc to cancel.".to_string()
        }
        Modal::ConfirmClearHistory => {
            "Clear request history and delete all local HTML response exports?\n\nPress y to confirm, n or Esc to cancel.".to_string()
        }
        Modal::ConfirmDeleteSidebar(label) => {
            format!("Delete this sidebar item?\n\n{label}\n\nPress y to confirm, n or Esc to cancel.")
        }
        Modal::Error(message) | Modal::Info(message) => {
            format!("{message}\n\nEsc closes this message.")
        }
    };

    let border_style = match modal {
        Modal::Error(_) => Style::default().fg(theme::ERROR),
        Modal::ConfirmNewRequest | Modal::ConfirmClearHistory | Modal::ConfirmDeleteSidebar(_) => {
            Style::default().fg(theme::WARNING)
        }
        Modal::Info(_) => Style::default().fg(theme::SUCCESS),
        _ => Style::default().fg(theme::ACCENT),
    };

    frame.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .title(format!(" {title} "))
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .style(Style::default().bg(theme::BG)),
            )
            .style(Style::default().fg(theme::TEXT))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn panel_block(title: &'static str, focused: bool) -> Block<'static> {
    let style = if focused {
        Style::default().fg(theme::ACCENT)
    } else {
        Style::default().fg(theme::BORDER)
    };
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(style)
        .style(Style::default().bg(theme::BG).fg(theme::TEXT))
}

fn summarize_headers(headers: &[Header]) -> String {
    if headers.is_empty() {
        "No headers".to_string()
    } else {
        format!(
            "{} header(s)",
            headers.iter().filter(|header| header.enabled).count()
        )
    }
}

fn summarize_query(query: &[QueryParam]) -> String {
    if query.is_empty() {
        "No query params".to_string()
    } else {
        query
            .iter()
            .filter(|param| param.enabled)
            .map(|param| format!("{}={}", param.key, param.value))
            .collect::<Vec<_>>()
            .join("&")
    }
}

fn summarize_body(body: &BodyConfig) -> String {
    match body {
        BodyConfig::Empty => "Empty".to_string(),
        BodyConfig::Raw { body, .. } => format!("Raw body, {} bytes", body.len()),
    }
}

fn help_text() -> String {
    [
        "Global",
        "  ?              Open help",
        "  a              Open about",
        "  q              Quit",
        "  u              Generate cURL for current request",
        "  Tab / Shift+Tab Switch panels",
        "  r              Run current request",
        "  Enter          Edit focused field or load sidebar request",
        "  Ctrl+r         Run current request",
        "  Ctrl+s         Save current request",
        "  n              New request",
        "  o              Import collection JSON",
        "  /              Search collections/history",
        "  c              Clear history and local HTML exports",
        "  d              Delete selected sidebar item",
        "",
        "Navigation",
        "  j/k or arrows  Move through sidebar and request fields",
        "  h/l            Switch method, auth type, or response tab",
        "  PageUp/PageDown Scroll response faster",
        "  /              Search response when response panel is focused",
        "  b              Open HTML response in browser from HTML tab",
        "  Esc            Cancel edit or close modal",
        "",
        "Editors",
        "  Headers        One per line: Header-Name: value",
        "  Query params   One per line: key=value",
        "  Bearer auth    Paste token only",
        "  Basic auth     username:password",
        "  API key auth   header-name: value",
    ]
    .join("\n")
}

fn about_text() -> String {
    [
        "Created by raufendro.",
        "",
        "Thank you for using post-tui. This tool was built to make API exploration feel fast, focused, and comfortable directly from the terminal, especially for developers who enjoy keyboard-driven workflows and lightweight software. I hope it helps you test, learn, and build with more confidence.",
        "",
        "Regards, raufendro.",
        "",
        "Esc closes this message.",
    ]
    .join("\n")
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
