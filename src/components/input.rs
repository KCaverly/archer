use std::time::Instant;

use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::agent::completion::CompletionModel;
use crate::agent::message::{Message, Role};
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;
use crate::styles::{ACTIVE_COLOR, FOCUSED_COLOR, UNFOCUSED_COLOR};
use crate::{action::Action, tui::Frame};
use async_channel::Sender;

#[derive(Default, Eq, PartialEq)]
enum InputState {
    Focused,
    #[default]
    Unfocused,
    Active,
}

#[derive(Default)]
pub struct MessageInput<'a> {
    command_tx: Option<Sender<Action>>,
    config: Config,
    current_input: String,
    display_spans: Vec<Span<'a>>,
    slash_command: bool,
    state: InputState,
}

impl MessageInput<'static> {
    pub fn new(focused: bool) -> Self {
        let state = if focused {
            InputState::Focused
        } else {
            InputState::Unfocused
        };
        Self {
            state,
            ..Default::default()
        }
    }
}

impl Component for MessageInput<'static> {
    fn register_action_handler(&mut self, tx: Sender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        if self.state == InputState::Active {
            match key.code {
                KeyCode::Char(c) => {
                    if (c == '/' && self.current_input == String::new())
                        || (self.slash_command && c != ' ')
                    {
                        self.slash_command = true;
                        self.display_spans.push(Span::styled(
                            c.to_string(),
                            Style::default().fg(Color::Cyan).bold(),
                        ));
                    } else if c == ' ' {
                        self.slash_command = false;
                        self.display_spans.push(Span::styled(
                            c.to_string(),
                            Style::default().fg(Color::White),
                        ));
                    } else {
                        self.display_spans.push(Span::styled(
                            c.to_string(),
                            Style::default().fg(Color::White),
                        ));
                    }
                    self.current_input.push(c);
                }
                KeyCode::Backspace => {
                    self.display_spans.pop();
                    self.current_input.pop();
                }
                KeyCode::Enter => {
                    let action = Action::SendMessage(Message {
                        role: Role::User,
                        content: self.current_input.clone(),
                        status: None,
                        model: Some(CompletionModel::Yi34B),
                    });
                    self.display_spans = Vec::new();
                    self.current_input = String::new();
                    return Ok(Some(action));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::SwitchMode(mode) => match mode {
                Mode::Viewer | Mode::ActiveViewer | Mode::ModelSelector => {
                    self.state = InputState::Unfocused;
                }
                Mode::Input => {
                    self.state = InputState::Focused;
                }
                Mode::ActiveInput => {
                    self.state = InputState::Active;
                }
            },

            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let text = Text::from(Line::from(self.display_spans.clone()));
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title("Input")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(
                        Style::default()
                            .fg(match self.state {
                                InputState::Active => ACTIVE_COLOR,
                                InputState::Focused => FOCUSED_COLOR,
                                InputState::Unfocused => UNFOCUSED_COLOR,
                            })
                            .bg(Color::Black),
                    ),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, rect);
        Ok(())
    }
}
