use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::App;
use crate::tree::FlatItemKind;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(f.area());

    // Filter input
    let filter_text = format!("> {}", app.filter);
    let filter =
        Paragraph::new(filter_text).block(Block::default().borders(Borders::ALL).title("Filter"));
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
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD);
                        ListItem::new(Line::from(Span::styled(
                            format!("{indent}{}", item.label),
                            style,
                        )))
                    }
                    FlatItemKind::Host => {
                        let style = if is_selected {
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };

                        let prefix = if is_selected { "▸ " } else { "  " };
                        let main_text = format!("{indent}{prefix}{}", item.label);

                        let show_cmd = item.full_cmd.as_ref().filter(|cmd| {
                            // Hide if it's just "ssh [user@]host" with no extra args
                            cmd.split_whitespace().count() > 2
                        });
                        if let Some(cmd) = show_cmd {
                            let pad = 30usize.saturating_sub(main_text.chars().count());
                            let cmd_style = Style::default().fg(Color::DarkGray);
                            ListItem::new(Line::from(vec![
                                Span::styled(main_text, style),
                                Span::raw(" ".repeat(pad)),
                                Span::styled(cmd.clone(), cmd_style),
                            ]))
                        } else {
                            ListItem::new(Line::from(Span::styled(main_text, style)))
                        }
                    }
                }
            })
            .collect()
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("SSH Hosts"));
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);
}
