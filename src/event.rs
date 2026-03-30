use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::app::App;
use crate::pinned;
use crate::theme::{THEMES, theme_name_by_index};

pub fn handle_events(app: &mut App) -> Result<()> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(());
    }

    match event::read()? {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            // Dismiss help popup on any key
            if app.show_help {
                app.show_help = false;
                return Ok(());
            }

            // SSH test popup: scrollable, dismiss on Esc only
            if app.show_ssh_test {
                match key.code {
                    KeyCode::Esc => app.show_ssh_test = false,
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.ssh_test_scroll = app.ssh_test_scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.ssh_test_scroll = app.ssh_test_scroll.saturating_add(1);
                    }
                    KeyCode::PageUp => {
                        app.ssh_test_scroll = app.ssh_test_scroll.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.ssh_test_scroll = app.ssh_test_scroll.saturating_add(10);
                    }
                    _ => {}
                }
                return Ok(());
            }

            // Dismiss "clear all pins?" on any key except the confirming Ctrl+Shift+P
            if app.confirm_clear_pins {
                let is_confirm = key.code == KeyCode::Char('p')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT);
                if !is_confirm {
                    app.confirm_clear_pins = false;
                }
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
                    pinned::save_theme(theme_name_by_index(app.theme_index));
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.theme_index = (app.theme_index + THEMES.len() - 1) % THEMES.len();
                    app.theme_changed = true;
                    pinned::save_theme(theme_name_by_index(app.theme_index));
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.move_pin_up();
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.move_pin_down();
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        if app.confirm_clear_pins {
                            app.clear_pins();
                        } else if !app.pinned.is_empty() {
                            app.confirm_clear_pins = true;
                        }
                    } else {
                        app.confirm_clear_pins = false;
                        app.toggle_pin();
                    }
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_ssh_test();
                }
                KeyCode::F(1) => {
                    app.show_help = true;
                }
                KeyCode::Home => app.move_to_first(),
                KeyCode::End => app.move_to_last(),
                KeyCode::PageUp => app.page_up(app.page_size),
                KeyCode::PageDown => app.page_down(app.page_size),
                KeyCode::Tab => app.select(),
                KeyCode::Enter => {
                    app.select_print_only();
                }
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
        Event::Mouse(mouse) => {
            if app.show_help {
                // Any mouse action dismisses help
                if matches!(mouse.kind, MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
                    app.show_help = false;
                }
            } else if app.show_ssh_test {
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        app.ssh_test_scroll = app.ssh_test_scroll.saturating_sub(3);
                    }
                    MouseEventKind::ScrollDown => {
                        app.ssh_test_scroll = app.ssh_test_scroll.saturating_add(3);
                    }
                    _ => {}
                }
            } else {
                match mouse.kind {
                    MouseEventKind::ScrollUp => app.move_up(),
                    MouseEventKind::ScrollDown => app.move_down(),
                    MouseEventKind::Down(MouseButton::Left) => {
                        app.click_select(mouse.row);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    Ok(())
}
