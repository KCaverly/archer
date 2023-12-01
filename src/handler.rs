use crate::app::{App, AppResult, CurrentFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Handles the key events and updates the state of [`App`].
pub async fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match app.current_focus {
        CurrentFocus::Input { insert } => {
            if insert {
                match key_event.code {
                    KeyCode::Char(value) => {
                        app.user_input.push(value);
                    }
                    KeyCode::Backspace => {
                        app.user_input.pop();
                    }
                    KeyCode::Enter => {
                        app.send_command().await;
                    }
                    KeyCode::Esc => {
                        app.exit_input();
                    }
                    _ => {}
                }
            } else {
                match key_event.code {
                    KeyCode::Char('i') => {
                        app.enter_input();
                    }
                    KeyCode::Char('k') => {
                        app.focus_viewer();
                    }
                    _ => {}
                }
            }
        }
        CurrentFocus::Viewer => match key_event.code {
            KeyCode::Char('j') => {
                app.focus_input();
            }
            _ => {}
        },
    }

    // Focus agnostic key bindings
    match key_event.code {
        // Exit application on `Ctrl-C`
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.quit();
            }
        }

        _ => {}
    }
    Ok(())
}
