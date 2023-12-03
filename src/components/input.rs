use std::time::Instant;

use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::agent::message::{Message, Role};
use crate::config::{Config, KeyBindings};
use crate::styles::{ACTIVE_COLOR, FOCUSED_COLOR, UNFOCUSED_COLOR};
use crate::{action::Action, tui::Frame};
use async_channel::Sender;

#[derive(Default)]
pub struct MessageInput<'a> {
    command_tx: Option<Sender<Action>>,
    config: Config,
    active: bool,
    focused: bool,
    current_input: String,
    display_spans: Vec<Span<'a>>,
    slash_command: bool,
}

impl MessageInput<'static> {
    pub fn new(focused: bool) -> Self {
        Self {
            focused,
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
        if self.active {
            match key.code {
                KeyCode::Char(c) => {
                    if (c == '/' && self.current_input == String::new())
                        || (self.slash_command && c != ' ')
                    {
                        self.slash_command = true;
                        self.display_spans.push(Span::styled(
                            c.to_string(),
                            Style::default().fg(Color::Magenta),
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
            Action::FocusViewer => {
                self.focused = false;
                self.active = false;
            }
            Action::FocusInput => {
                self.focused = true;
                self.active = false;
            }
            Action::ActivateInput => self.active = true,
            Action::DeactivateInput => self.active = false,

            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(rect);

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
                            .fg(if self.active {
                                ACTIVE_COLOR
                            } else if self.focused {
                                FOCUSED_COLOR
                            } else {
                                UNFOCUSED_COLOR
                            })
                            .bg(Color::Black),
                    ),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, layout[1]);
        Ok(())
    }
}
