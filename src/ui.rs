use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::App;
use crate::theme::{Theme, theme_name_by_index};
use crate::tree::FlatItemKind;

pub fn draw(f: &mut Frame, app: &mut App, theme: &Theme) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(f.area());

    // Filter input
    let filter_text = format!("> {}", app.filter);
    let title = if app.confirm_clear_pins {
        "Clear all pins? Press Ctrl+Shift+P to confirm"
    } else if app.theme_changed {
        theme_name_by_index(app.theme_index)
    } else {
        "Filter"
    };
    let filter = Paragraph::new(filter_text)
        .style(Style::default().fg(theme.accent_filter).bg(theme.bg))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(theme.bg_hl))
                .style(Style::default().bg(theme.bg)),
        );
    f.render_widget(filter, chunks[0]);

    // Set cursor position after filter text
    f.set_cursor_position((
        chunks[0].x + 2 + app.filter.len() as u16 + 1,
        chunks[0].y + 1,
    ));

    // Update page size and list area position from actual viewport (minus borders)
    app.page_size = (chunks[1].height as usize).saturating_sub(2);
    app.list_area_y = chunks[1].y + 1;
    app.list_area_height = chunks[1].height.saturating_sub(2);

    // Build list items in a block so the immutable borrow is dropped before render_stateful_widget
    let items: Vec<ListItem> = {
        let all_items = app.items();
        let selected = app.selected;
        app.visible
            .iter()
            .enumerate()
            .map(|(vi, &idx)| {
                let item = &all_items[idx];
                let indent = "  ".repeat(item.depth);
                let is_selected = vi == selected;

                match item.kind {
                    FlatItemKind::File => {
                        let style = Style::default()
                            .fg(theme.accent_header)
                            .add_modifier(Modifier::BOLD);
                        ListItem::new(Line::from(Span::styled(
                            format!("{indent}{}", item.label),
                            style,
                        )))
                    }
                    FlatItemKind::Host => {
                        let style = if is_selected {
                            Style::default()
                                .fg(theme.fg_bright)
                                .bg(theme.bg_hl)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.fg)
                        };

                        let prefix = if is_selected {
                            Span::styled(
                                "▸ ",
                                Style::default()
                                    .fg(theme.accent_marker)
                                    .bg(theme.bg_hl)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            Span::raw("  ")
                        };
                        let prefix_width = 2;
                        let label_text = format!("{indent}{}", item.label);
                        let visual_width = label_text.chars().count() + prefix_width;

                        // Show full_cmd (history, >2 args) at col 30, or info (config) at col 40
                        let show_cmd = item
                            .full_cmd
                            .as_ref()
                            .filter(|cmd| cmd.split_whitespace().count() > 2);
                        let (hint, col): (Option<String>, usize) = if let Some(cmd) = show_cmd {
                            (Some(cmd.clone()), 30)
                        } else if let Some(ref info) = item.info {
                            (Some(info.clone()), 40)
                        } else {
                            (None, 0)
                        };
                        if let Some(hint) = hint {
                            let pad = col.saturating_sub(visual_width);
                            let mut hint_style = Style::default().fg(theme.fg_dim);
                            let mut pad_style = Style::default();
                            if is_selected {
                                hint_style = Style::default().fg(theme.accent_info).bg(theme.bg_hl);
                                pad_style = pad_style.bg(theme.bg_hl);
                            }
                            ListItem::new(Line::from(vec![
                                prefix,
                                Span::styled(label_text, style),
                                Span::styled(" ".repeat(pad), pad_style),
                                Span::styled(hint, hint_style),
                            ]))
                        } else {
                            ListItem::new(Line::from(vec![prefix, Span::styled(label_text, style)]))
                        }
                    }
                }
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("SSH Hosts")
            .border_style(Style::default().fg(theme.bg_hl))
            .style(Style::default().bg(theme.bg)),
    );
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Help popup
    if app.show_help {
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "Up/Down  ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Navigate"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+j/k ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Navigate"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Home/End ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Jump to first/last"),
            ]),
            Line::from(vec![
                Span::styled(
                    "PgUp/Dn  ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Page up/down"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Tab      ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Select (run command)"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Enter    ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Select (print only)"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+f   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Toggle fuzzy/exact"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+u   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Clear filter"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+w   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Delete last word"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+p   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Pin/unpin entry"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+P   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Clear all pins"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Alt+j/k  ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Move pin down/up"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+t   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Test SSH connection"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Alt+r/e  ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Next/prev theme"),
            ]),
            Line::from(vec![
                Span::styled(
                    "F1       ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("This help"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Esc      ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Quit"),
            ]),
        ];
        let popup_height = lines.len() as u16 + 2; // +2 for borders
        let popup_width = 38;
        let area = f.area();
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width.min(area.width),
            height: popup_height.min(area.height),
        };
        f.render_widget(Clear, popup_area);
        let help = Paragraph::new(lines)
            .style(Style::default().fg(theme.fg).bg(theme.bg))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Keyboard Shortcuts")
                    .border_style(Style::default().fg(theme.accent_header))
                    .style(Style::default().bg(theme.bg)),
            );
        f.render_widget(help, popup_area);
    }

    // SSH test result popup
    if app.show_ssh_test {
        let area = f.area();
        let popup_width = area.width * 9 / 10;
        let popup_height = area.height * 9 / 10;
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };

        // Build lines: verbose output + status line at the end
        let mut lines: Vec<Line> = app
            .ssh_test_output
            .iter()
            .map(|s| Line::from(Span::styled(s.as_str(), Style::default().fg(theme.fg))))
            .collect();

        if let Some(success) = app.ssh_test_result {
            lines.push(Line::raw(""));
            let ts = app.ssh_test_done_ts.as_deref().unwrap_or("--:---");
            let (text, color) = if success {
                (
                    format!("{ts}  {}: CONNECTION SUCCESSFUL", app.ssh_test_host),
                    theme.accent_filter,
                )
            } else {
                (
                    format!("{ts}  {}: CONNECTION FAILED", app.ssh_test_host),
                    theme.accent_header,
                )
            };
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )));
        }

        // Count wrapped lines using word-wrap logic matching ratatui Wrap { trim: false }
        let inner_width = popup_width.saturating_sub(2) as usize;
        let inner_height = popup_height.saturating_sub(2);
        let total_lines: u16 = lines
            .iter()
            .map(|l| {
                let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                if text.is_empty() || inner_width == 0 {
                    return 1u16;
                }
                let mut rows: u16 = 1;
                let mut col: usize = 0;
                for word in text.split(' ') {
                    let wlen = word.len();
                    if col == 0 {
                        col = wlen;
                    } else if col + 1 + wlen > inner_width {
                        rows += 1;
                        col = wlen;
                    } else {
                        col += 1 + wlen;
                    }
                    // Handle words wider than the line
                    while col > inner_width {
                        rows += 1;
                        col = col.saturating_sub(inner_width);
                    }
                }
                rows
            })
            .sum();
        let max_scroll = total_lines.saturating_sub(inner_height);
        if app.ssh_test_scroll > max_scroll {
            app.ssh_test_scroll = max_scroll;
        }

        let title = match app.ssh_test_result {
            None => format!("Testing {}", app.ssh_test_host),
            Some(true) => format!("{} - Success", app.ssh_test_host),
            Some(false) => format!("{} - Failed", app.ssh_test_host),
        };
        let border_color = match app.ssh_test_result {
            Some(true) => theme.accent_filter,
            _ => theme.accent_header,
        };

        f.render_widget(Clear, popup_area);
        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(theme.fg).bg(theme.bg))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color))
                    .style(Style::default().bg(theme.bg)),
            )
            .scroll((app.ssh_test_scroll, 0));
        f.render_widget(paragraph, popup_area);
    }
}
