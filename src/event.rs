use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::app::App;
use crate::theme::THEMES;

pub fn handle_events(app: &mut App) -> Result<()> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(());
    }

    if let Event::Key(key) = event::read()? {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => app.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.should_quit = true;
            }
            KeyCode::Up => app.move_up(),
            KeyCode::Down => app.move_down(),
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.move_up();
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.move_down();
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.toggle_exact();
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.theme_index = (app.theme_index + 1) % THEMES.len();
                app.theme_changed = true;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.theme_index = (app.theme_index + THEMES.len() - 1) % THEMES.len();
                app.theme_changed = true;
            }
            KeyCode::Home => app.move_to_first(),
            KeyCode::End => app.move_to_last(),
            KeyCode::PageUp => app.page_up(app.page_size),
            KeyCode::PageDown => app.page_down(app.page_size),
            KeyCode::Tab => {
                app.select_print_only();
            }
            KeyCode::Enter => app.select(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter.clear();
                app.update_filter();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let trimmed = app.filter.trim_end_matches(|c: char| !c.is_whitespace());
                let trimmed = trimmed.trim_end();
                app.filter = trimmed.to_string();
                app.update_filter();
            }
            KeyCode::Backspace => {
                app.filter.pop();
                app.update_filter();
            }
            KeyCode::Char(c) => {
                app.filter.push(c);
                app.update_filter();
            }
            _ => {}
        }
    }

    Ok(())
}
