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

            // Any keypress clears a transient status message.
            app.status_msg = None;

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

            // Add-host form
            if app.show_add_host {
                handle_add_host_keys(app, key);
                return Ok(());
            }

            // SFTP path completion popup
            if app.show_sftp {
                match key.code {
                    KeyCode::Esc => {
                        app.show_sftp = false;
                        app.sftp_chosen_path = None;
                    }
                    KeyCode::Up => {
                        if app.sftp_selected > 0 {
                            app.sftp_selected -= 1;
                            app.sftp_list_state.select(Some(app.sftp_selected));
                        }
                    }
                    KeyCode::Down => {
                        if app.sftp_selected + 1 < app.sftp_filtered.len() {
                            app.sftp_selected += 1;
                            app.sftp_list_state.select(Some(app.sftp_selected));
                        }
                    }
                    KeyCode::Enter => app.sftp_accept_selected(),
                    KeyCode::Tab => {
                        // Accept current path as-is
                        app.sftp_chosen_path =
                            Some(format!("{}:{}", app.sftp_host, app.sftp_path_input));
                        app.show_sftp = false;
                        app.should_quit = true;
                    }
                    KeyCode::Backspace => {
                        if !app.sftp_path_input.is_empty() {
                            app.sftp_path_input.pop();
                            app.sftp_on_input_changed();
                        }
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.sftp_path_input.clear();
                        app.sftp_on_input_changed();
                    }
                    KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Delete back to previous /
                        let len = app.sftp_path_input.len().saturating_sub(1);
                        if let Some(pos) = app.sftp_path_input[..len].rfind('/') {
                            app.sftp_path_input.truncate(pos + 1);
                            app.sftp_on_input_changed();
                        }
                    }
                    KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.sftp_show_hidden = !app.sftp_show_hidden;
                        app.sftp_update_filter();
                    }
                    KeyCode::Char(c) => {
                        app.sftp_path_input.push(c);
                        app.sftp_on_input_changed();
                    }
                    _ => {}
                }
                return Ok(());
            }

            // Dismiss "clear all pins?" on any key except the confirming Alt+Shift+B
            if app.confirm_clear_pins {
                let is_confirm = key.code == KeyCode::Char('b')
                    && key.modifiers.contains(KeyModifiers::ALT)
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
                // Themes: Alt+T = next, Alt+Shift+T = previous.
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::ALT)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    app.theme_index = (app.theme_index + THEMES.len() - 1) % THEMES.len();
                    app.theme_changed = true;
                    pinned::save_theme(theme_name_by_index(app.theme_index));
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.theme_index = (app.theme_index + 1) % THEMES.len();
                    app.theme_changed = true;
                    pinned::save_theme(theme_name_by_index(app.theme_index));
                }
                // Pin reorder kept on Alt+J/K.
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.move_pin_up();
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.move_pin_down();
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_edit_props();
                }
                // Alt+Shift+B confirms / triggers clear-all-pins.
                // (Alt-mod, not Ctrl-mod, so it doesn't collide with the tmux
                // prefix Ctrl+B.)
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    if app.confirm_clear_pins {
                        app.clear_pins();
                    } else if !app.pinned.is_empty() {
                        app.confirm_clear_pins = true;
                    }
                }
                // Alt+B toggles pin (Bookmark).
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.confirm_clear_pins = false;
                    app.toggle_pin();
                }
                // Alt+M (Ctrl+M is ASCII CR / indistinguishable from Enter in
                // legacy terminal mode).
                KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::ALT) => {
                    app.start_move_host();
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_ssh_test();
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_edit();
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_add_host();
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_duplicate_host();
                }
                KeyCode::Null
                | KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.start_sftp_completion();
                }
                KeyCode::Char('\0') => {
                    app.start_sftp_completion();
                }
                KeyCode::F(2) => {
                    app.start_sftp_completion();
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
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) {
                    app.show_help = false;
                }
            } else if app.show_sftp {
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        if app.sftp_selected > 0 {
                            app.sftp_selected -= 1;
                            app.sftp_list_state.select(Some(app.sftp_selected));
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if app.sftp_selected + 1 < app.sftp_filtered.len() {
                            app.sftp_selected += 1;
                            app.sftp_list_state.select(Some(app.sftp_selected));
                        }
                    }
                    _ => {}
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

fn handle_add_host_keys(app: &mut App, key: crossterm::event::KeyEvent) {
    // Confirmation prompt for creating a new file: y/n only.
    if app.add_form.confirm_create_file.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => match app.add_host_confirm_create() {
                Ok(()) => app.finish_add_host(),
                Err(msg) => app.add_form.error = Some(msg),
            },
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.add_host_cancel_create();
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => app.add_host_close(),
        KeyCode::Tab => app.add_host_cycle_field(true),
        KeyCode::BackTab => app.add_host_cycle_field(false),
        KeyCode::Up => app.add_host_nav_up(),
        KeyCode::Down => app.add_host_nav_down(),
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.add_host_test_connection();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.add_host_clear_field();
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.add_host_close();
        }
        KeyCode::Enter => match app.add_host_try_save() {
            Ok(true) => app.finish_add_host(),
            Ok(false) => {
                // Confirmation prompt is now active; UI will render it.
            }
            Err(_) => {
                // Validation error stays in app.add_form.error; UI shows it.
            }
        },
        KeyCode::Backspace => app.add_host_backspace(),
        KeyCode::Char(c) => app.add_host_input(c),
        _ => {}
    }
}
