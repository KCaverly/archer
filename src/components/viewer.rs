use std::fmt;
use std::time::Instant;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use replicate_rs::predictions::PredictionStatus;

use super::Component;
use crate::agent::message::{Message, Role};
use crate::styles::{
    ACTIVE_COLOR, ASSISTANT_COLOR, FOCUSED_COLOR, SYSTEM_COLOR, UNFOCUSED_COLOR, USER_COLOR,
};
use crate::{action::Action, tui::Frame};
use async_channel::Sender;

use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct Viewer {
    command_tx: Option<Sender<Action>>,
    config: Config,
    focused: bool,
    messages: Vec<Message>,
}

impl Viewer {
    pub fn new(focused: bool) -> Self {
        Self {
            focused,
            ..Default::default()
        }
    }
}

impl Component for Viewer {
    fn register_action_handler(&mut self, tx: Sender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::FocusInput => {
                self.focused = false;
            }
            Action::FocusViewer => {
                self.focused = true;
            }
            Action::ReceiveMessage(message) => {
                self.messages.push(message);
            }
            Action::StreamMessage(message) => {
                // Simply replace the last message
                self.messages.pop();
                self.messages.push(message);
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(rect);

        // Render Messages
        let mut lines = Vec::new();
        for message in &self.messages {
            match message.role {
                Role::System => lines.push(Line::from(vec![Span::styled(
                    "System",
                    Style::default().fg(SYSTEM_COLOR).bold(),
                )])),
                Role::User => lines.push(Line::from(vec![Span::styled(
                    "User",
                    Style::default().fg(USER_COLOR).bold(),
                )])),
                Role::Assistant => {
                    let mut spans = Vec::new();
                    spans.push(Span::styled(
                        "Assistant",
                        Style::default().fg(ASSISTANT_COLOR).bold(),
                    ));

                    if let Some(model) = &message.model {
                        let (owner, model_name) = model.get_model_details();
                        spans.push(Span::styled(
                            format!(": ({owner}/{model_name})"),
                            Style::default().fg(ASSISTANT_COLOR),
                        ));
                    }

                    lines.push(Line::from(spans));
                }
            }

            lines.push(Line::from(vec![Span::styled(
                message.content.clone(),
                Style::default().fg(Color::White),
            )]));
        }

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title("Viewer")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(
                        Style::default()
                            .fg(if self.focused {
                                FOCUSED_COLOR
                            } else {
                                UNFOCUSED_COLOR
                            })
                            .bg(Color::Black),
                    ),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, layout[0]);
        Ok(())
    }
}
