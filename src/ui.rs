use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;
use crate::theme::{Theme, theme_name_by_index};
use crate::tree::FlatItemKind;

pub fn draw(f: &mut Frame, app: &mut App, theme: &Theme) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(f.area());

    // Filter input
    let filter_text = format!("> {}", app.filter);
    let title = if app.theme_changed {
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

    // Update page size from actual viewport height (minus borders)
    app.page_size = (chunks[1].height as usize).saturating_sub(2);

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
                            Span::styled("▸ ", Style::default().fg(theme.accent_marker).bg(theme.bg_hl).add_modifier(Modifier::BOLD))
                        } else {
                            Span::raw("  ")
                        };
                        let prefix_width = 2;
                        let label_text = format!("{indent}{}", item.label);
                        let visual_width = label_text.chars().count() + prefix_width;

                        // Show full_cmd (history, >2 args) at col 30, or info (config) at col 40
                        let show_cmd = item.full_cmd.as_ref().filter(|cmd| {
                            cmd.split_whitespace().count() > 2
                        });
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
                            ListItem::new(Line::from(vec![
                                prefix,
                                Span::styled(label_text, style),
                            ]))
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
}
