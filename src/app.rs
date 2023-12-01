use std::error;

use crate::agent::get_response;

#[derive(Debug)]
pub enum CurrentFocus {
    Viewer,
    Input { insert: bool },
}

#[derive(Debug)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    pub user_input: String,
    pub current_focus: CurrentFocus,
    pub messages: Vec<Message>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            user_input: String::new(),
            current_focus: CurrentFocus::Input { insert: true },
            messages: Vec::new(),
        }
    }
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles the tick event of the terminal.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn focus_viewer(&mut self) {
        match self.current_focus {
            CurrentFocus::Input { insert } => {
                if !insert {
                    self.current_focus = CurrentFocus::Viewer;
                }
            }
            _ => {}
        }
    }

    pub fn focus_input(&mut self) {
        self.current_focus = CurrentFocus::Input { insert: false };
    }

    pub fn enter_input(&mut self) {
        match self.current_focus {
            CurrentFocus::Input { insert } => {
                if !insert {
                    self.current_focus = CurrentFocus::Input { insert: true };
                }
            }
            _ => {}
        }
    }

    pub fn exit_input(&mut self) {
        match self.current_focus {
            CurrentFocus::Input { insert } => {
                if insert {
                    self.current_focus = CurrentFocus::Input { insert: false };
                }
            }
            _ => {}
        }
    }

    pub async fn send_command(&mut self) {
        let prompt = self.user_input.clone();

        self.messages.push(Message {
            role: MessageRole::User,
            content: prompt.clone(),
        });

        let output = get_response(&prompt).await.unwrap();

        self.messages.push(Message {
            role: MessageRole::Assistant,
            content: output,
        });

        self.user_input = String::new();
    }
}
