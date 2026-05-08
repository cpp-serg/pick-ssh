use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{AddHostField, App, SftpStatus};
use crate::theme::{Theme, theme_name_by_index};
use crate::tree::FlatItemKind;

pub fn draw(f: &mut Frame, app: &mut App, theme: &Theme) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(f.area());

    // Filter input
    let filter_text = format!("> {}", app.filter);
    let status_owned;
    let title = if let Some(ref msg) = app.status_msg {
        status_owned = msg.clone();
        status_owned.as_str()
    } else if app.confirm_clear_pins {
        "Clear all pins? Press Alt+Shift+B to confirm"
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
                Span::raw("Edit properties"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Alt+b    ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Bookmark (pin/unpin)"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Alt+Shift+B ",
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
                    "Ctrl+e   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Edit config entry in $EDITOR"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+n   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Add new SSH host"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+d   ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Duplicate selected entry"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Alt+m    ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Move entry to another file"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Ctrl+Spc ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Remote path completion"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Alt+T    ",
                    Style::default()
                        .fg(theme.accent_filter)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Next theme  (Alt+Shift+T = prev)"),
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

    // SSH test popup is drawn last (below) so it sits on top of any other popup.

    // SFTP path completion popup
    if app.show_sftp {
        let area = f.area();
        let popup_width = area.width * 7 / 10;
        let popup_height = area.height * 6 / 10;
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };
        f.render_widget(Clear, popup_area);

        let chunks =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(popup_area);

        // Input bar: host:path
        let input_line = Line::from(vec![
            Span::styled(
                format!("{}:", app.sftp_host),
                Style::default()
                    .fg(theme.accent_header)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.sftp_path_input.as_str(),
                Style::default().fg(theme.accent_filter),
            ),
        ]);
        let input_title = match &app.sftp_status {
            SftpStatus::Connecting => "Connecting...".to_string(),
            SftpStatus::Ready => format!("Remote Path ({} entries)", app.sftp_filtered.len()),
            SftpStatus::Error(msg) => format!("Error: {msg}"),
        };
        let input_border_color = match &app.sftp_status {
            SftpStatus::Error(_) => theme.accent_header,
            _ => theme.bg_hl,
        };
        let input_widget = Paragraph::new(input_line)
            .style(Style::default().fg(theme.fg).bg(theme.bg))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(input_title)
                    .border_style(Style::default().fg(input_border_color))
                    .style(Style::default().bg(theme.bg)),
            );
        f.render_widget(input_widget, chunks[0]);

        // Cursor after path text
        let host_prefix_len = app.sftp_host.len() + 1; // "host:"
        f.set_cursor_position((
            chunks[0].x + 1 + host_prefix_len as u16 + app.sftp_path_input.len() as u16,
            chunks[0].y + 1,
        ));

        // Entry list
        let items: Vec<ListItem> = app
            .sftp_filtered
            .iter()
            .enumerate()
            .map(|(vi, &idx)| {
                let entry = &app.sftp_entries[idx];
                let is_selected = vi == app.sftp_selected;
                let label = if entry.is_dir {
                    format!("{}/", entry.name)
                } else {
                    entry.name.clone()
                };
                let style = if is_selected {
                    Style::default()
                        .fg(theme.fg_bright)
                        .bg(theme.bg_hl)
                        .add_modifier(Modifier::BOLD)
                } else if entry.is_dir {
                    Style::default().fg(theme.accent_header)
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
                ListItem::new(Line::from(vec![prefix, Span::styled(label, style)]))
            })
            .collect();

        if matches!(app.sftp_status, SftpStatus::Connecting) && items.is_empty() {
            let msg = Paragraph::new("  Getting listing...")
                .style(Style::default().fg(theme.fg_dim).bg(theme.bg))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.bg_hl))
                        .style(Style::default().bg(theme.bg)),
                );
            f.render_widget(msg, chunks[1]);
        } else {
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.bg_hl))
                    .style(Style::default().bg(theme.bg)),
            );
            f.render_stateful_widget(list, chunks[1], &mut app.sftp_list_state);
        }
    }

    // Add-host form
    if app.show_add_host {
        draw_add_host(f, app, theme);
    }

    // SSH test popup — drawn last so it floats above all other popups (e.g. the
    // add-host form, when the user presses Ctrl+T from inside it).
    if app.show_ssh_test {
        draw_ssh_test(f, app, theme);
    }
}

fn draw_ssh_test(f: &mut Frame, app: &mut App, theme: &Theme) {
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

fn draw_add_host(f: &mut Frame, app: &mut App, theme: &Theme) {
    let area = f.area();
    let popup_width = (area.width * 7 / 10).max(60).min(area.width);
    // 7 fields × 1 row each = 7, + padding/borders + picker list + status row
    let popup_height = (area.height * 8 / 10).max(20).min(area.height);
    let popup_area = Rect {
        x: area.width.saturating_sub(popup_width) / 2,
        y: area.height.saturating_sub(popup_height) / 2,
        width: popup_width,
        height: popup_height,
    };
    f.render_widget(Clear, popup_area);

    let title = if app.add_form.confirm_create_file.is_some() {
        "Create new file?"
    } else if app.add_form.is_move {
        "Move SSH host to file"
    } else if app.add_form.edit_original.is_some() {
        "Edit SSH host"
    } else {
        "Add new SSH host"
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(theme.accent_header))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Layout: 7 field rows + picker area + status row
    let row_h = 1;
    let n_fields = AddHostField::ALL.len() as u16;
    let picker_h = if app.add_form.active() == AddHostField::File
        && !app.add_form.file_filtered.is_empty()
    {
        (app.add_form.file_filtered.len() as u16 + 2).min(8)
    } else {
        0
    };
    let constraints = [
        Constraint::Length(n_fields * row_h),
        Constraint::Length(picker_h),
        Constraint::Length(2),
        Constraint::Min(0),
    ];
    let chunks = Layout::vertical(constraints).split(inner);

    // Render fields.
    let active = app.add_form.active();
    let lines: Vec<Line> = AddHostField::ALL
        .iter()
        .map(|&fld| {
            let is_active = fld == active;
            let label_style = Style::default()
                .fg(if is_active {
                    theme.accent_filter
                } else {
                    theme.fg_dim
                })
                .add_modifier(if is_active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            let value = app.add_form.field(fld);
            let value_style = if is_active {
                Style::default().fg(theme.fg_bright).bg(theme.bg_hl)
            } else {
                Style::default().fg(theme.fg)
            };
            let marker = if is_active { "▸ " } else { "  " };
            Line::from(vec![
                Span::styled(marker, Style::default().fg(theme.accent_marker)),
                Span::styled(format!("{:14}", fld.label()), label_style),
                Span::styled(format!(" {value} "), value_style),
            ])
        })
        .collect();
    let fields_widget = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    f.render_widget(fields_widget, chunks[0]);

    // Cursor on the active field's value.
    let active_idx = app.add_form.active_field as u16;
    let active_value = app.add_form.field(active);
    f.set_cursor_position((
        chunks[0].x + 2 + 14 + 1 + active_value.chars().count() as u16,
        chunks[0].y + active_idx,
    ));

    // File picker list (only when file field active).
    if picker_h > 0 {
        let items: Vec<ListItem> = app
            .add_form
            .file_filtered
            .iter()
            .enumerate()
            .map(|(vi, &i)| {
                let path = &app.add_form.available_files[i];
                let label = display_path(path);
                let is_selected = vi == app.add_form.file_picker_idx;
                let style = if is_selected {
                    Style::default()
                        .fg(theme.fg_bright)
                        .bg(theme.bg_hl)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.fg)
                };
                ListItem::new(Line::from(Span::styled(format!("  {label}"), style)))
            })
            .collect();
        let list = List::new(items).block(
            Block::default()
                .borders(Borders::TOP)
                .title("Existing config files")
                .border_style(Style::default().fg(theme.bg_hl))
                .style(Style::default().bg(theme.bg)),
        );
        f.render_widget(list, chunks[1]);
    }

    // Status row: error / hint / confirm prompt.
    let status_text = if let Some(ref path) = app.add_form.confirm_create_file {
        format!(
            "Create file at {}?  [y]es / [n]o",
            display_path(path)
        )
    } else if let Some(ref e) = app.add_form.error {
        e.clone()
    } else {
        "Tab next  Esc cancel  Enter save  Ctrl+T test  Ctrl+U clear field".to_string()
    };
    let status_color = if app.add_form.error.is_some() || app.add_form.confirm_create_file.is_some()
    {
        theme.accent_header
    } else {
        theme.fg_dim
    };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color).bg(theme.bg));
    f.render_widget(status, chunks[2]);
}

fn display_path(p: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = p.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    p.display().to_string()
}
