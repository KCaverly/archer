use futures::StreamExt;
use std::fmt;
use std::str::from_utf8;
use std::time::Instant;
use textwrap::core::Word;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::WordSeparator;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use replicate_rs::predictions::PredictionStatus;

use super::Component;
use crate::agent::completion::stream_completion;
use crate::agent::conversation::Conversation;
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
    conversation: Conversation,
    active: bool,
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
                self.conversation.add_message(message);
            }
            Action::StreamMessage(message) => {
                // Simply replace the last message
                self.conversation.replace_last_message(message);
            }
            Action::ActivateViewer => {
                self.active = true;
                self.conversation.focus();
            }
            Action::DeactivateViewer => {
                self.active = false;
                self.conversation.unfocus();
            }
            Action::SelectNextMessage => {
                self.conversation.select_next_message();
            }
            Action::SelectPreviousMessage => {
                self.conversation.select_prev_message();
            }
            Action::DeleteSelectedMessage => {
                self.conversation.delete_selected_message();
            }
            Action::SendMessage(message) => {
                // Lets clean this up at some point
                // I don't think this cloning is ideal
                let model = message.model.clone();
                let action_tx = self.command_tx.clone().unwrap();
                let mut messages = self.conversation.messages.clone();
                tokio::spawn(async move {
                    action_tx
                        .send(Action::ReceiveMessage(message.clone()))
                        .await
                        .ok();

                    if let Some(model) = model {
                        let mut content = String::new();

                        action_tx
                            .send(Action::ReceiveMessage(Message {
                                role: Role::Assistant,
                                content: content.clone(),
                                status: Some(PredictionStatus::Starting),
                                model: Some(model.clone()),
                            }))
                            .await
                            .ok();
                        messages.push(message);

                        let stream = stream_completion(&model, messages).await;
                        match stream {
                            Ok((status, mut stream)) => {
                                while let Some(event) = stream.next().await {
                                    match event {
                                        Ok(event) => {
                                            if event.event == "done" {
                                                break;
                                            }
                                            content.push_str(event.data.as_str());
                                            action_tx
                                                .send(Action::StreamMessage(Message {
                                                    role: Role::Assistant,
                                                    content: content.clone(),
                                                    status: None,
                                                    model: Some(model.clone()),
                                                }))
                                                .await
                                                .ok();
                                        }
                                        Err(err) => {
                                            panic!("{:?}", err);
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                panic!("{err}");
                            }
                        }
                    }
                });
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
        let mut message_items = Vec::new();
        for message in &self.conversation.messages {
            let mut message_lines = Vec::new();
            match message.role {
                Role::System => message_lines.push(Line::from(vec![Span::styled(
                    "System",
                    Style::default().fg(SYSTEM_COLOR).bold(),
                )])),
                Role::User => message_lines.push(Line::from(vec![Span::styled(
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

                    message_lines.push(Line::from(spans));
                }
            }

            for line in message.content.split("\n") {
                let words = WordSeparator::AsciiSpace
                    .find_words(line)
                    .collect::<Vec<_>>();
                let subs = lines_to_strings(
                    wrap_optimal_fit(&words, &[rect.width as f64 - 2.0], &Penalties::new())
                        .unwrap(),
                );

                for sub in subs {
                    message_lines.push(Line::from(vec![Span::styled(
                        sub,
                        Style::default().fg(Color::White),
                    )]));
                }
            }

            message_items.push(ListItem::new(Text::from(message_lines)));
        }

        let vertical_scroll = 0;
        let list = List::new(message_items)
            .block(
                Block::default()
                    .title("Conversation")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(Style::default().fg(if self.active {
                        ACTIVE_COLOR
                    } else if self.focused {
                        FOCUSED_COLOR
                    } else {
                        UNFOCUSED_COLOR
                    }))
                    .bg(Color::Black),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::ITALIC)
                    .bg(Color::DarkGray),
            )
            .highlight_symbol("");

        let mut list_state = ListState::default().with_selected(self.conversation.selected_message);
        f.render_stateful_widget(list, layout[0], &mut list_state);

        Ok(())
    }
}
//
// Helper to convert wrapped lines to a Vec<String>.
fn lines_to_strings(lines: Vec<&[Word<'_>]>) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|word| &**word)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
}
