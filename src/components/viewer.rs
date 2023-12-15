use arboard::{Clipboard, LinuxClipboardKind, SetExtLinux};
use futures::StreamExt;
use ratatui::widgets::block::Title;
use std::str::from_utf8;
use std::time::Instant;
use std::{fmt, fs};
use textwrap::core::Word;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::WordSeparator;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use replicate_rs::predictions::PredictionStatus;

use super::Component;
use crate::agent::completion::create_prediction;
use crate::agent::conversation::{Conversation, ConversationManager};
use crate::agent::message::{Message, Role};
use crate::mode::Mode;
use crate::styles::{
    ACTIVE_COLOR, ASSISTANT_COLOR, FOCUSED_COLOR, SYSTEM_COLOR, UNFOCUSED_COLOR, USER_COLOR,
};
use crate::{action::Action, tui::Frame};
use async_channel::Sender;

use crate::config::{Config, KeyBindings};

#[derive(Default)]
enum ViewerState {
    Active,
    Focused,
    #[default]
    Unfocused,
    Maximized,
}

#[derive(Default)]
pub struct Viewer {
    command_tx: Option<Sender<Action>>,
    config: Config,
    conversation: Conversation,
    manager: ConversationManager,
    state: ViewerState,
    keymap: String,
}

impl Viewer {
    pub fn new(focused: bool, keymap: String) -> Self {
        let state = if focused {
            ViewerState::Focused
        } else {
            ViewerState::Unfocused
        };

        let mut manager = ConversationManager::default();
        let conversation = manager.new_conversation();

        Self {
            state,
            keymap,
            manager,
            conversation,
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
            Action::ReceiveMessage(id, message) => {
                self.conversation.add_message(id, message);
            }
            Action::NewConversation => {
                let convo = self.manager.new_conversation();
                self.conversation = convo;
            }
            Action::StreamMessage(id, message) => {
                // Simply replace the last message
                self.conversation.replace_message(id, message);
            }
            Action::LoadSelectedConversation => {
                if let Some(convo) = self.manager.load_selected_conversation().ok() {
                    self.conversation = convo;
                }
            }
            Action::SelectNextConversation => {
                self.manager.select_next_conversation();
            }
            Action::SelectPreviousConversation => {
                self.manager.select_prev_conversation();
            }
            Action::SwitchMode(mode) => match mode {
                Mode::Viewer => {
                    self.state = ViewerState::Focused;
                    self.conversation.unfocus();
                }
                Mode::ActiveViewer => {
                    self.state = ViewerState::Active;
                    self.conversation.focus();
                }
                Mode::ModelSelector => {
                    self.state = ViewerState::Unfocused;
                    self.conversation.unfocus();
                }
                Mode::Input | Mode::ActiveInput | Mode::ConversationManager => {
                    self.state = ViewerState::Unfocused;
                }
                Mode::MessageViewer => {
                    self.state = ViewerState::Maximized;
                }
            },
            Action::SaveActiveConversation => {
                self.conversation.save().ok();
                let convo = self.conversation.clone();
                if let Some(action_tx) = self.command_tx.clone() {
                    tokio::spawn(async move {
                        action_tx
                            .send(Action::AddConversationToManager(convo))
                            .await
                            .ok();
                    });
                }
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
            Action::SwitchKeymap(keymap) => {
                self.keymap = keymap;
            }
            Action::CopySelectedMessage => {
                let selected_message = self.conversation.get_selected_message().unwrap();

                let content = selected_message.content.clone();

                #[cfg(any(target_os = "linux"))]
                tokio::spawn(async move {
                    let mut ctx = Clipboard::new().unwrap();
                    let _ = ctx
                        .set()
                        .wait()
                        .clipboard(LinuxClipboardKind::Clipboard)
                        .text(content.clone());
                });

                let content = selected_message.content.clone();
                let mut ctx = Clipboard::new()?;
                let _ = ctx.set().text(content);
            }
            Action::SendMessage(message) => {
                // Lets clean this up at some point
                // I don't think this cloning is ideal
                let model = message.model.clone();
                let action_tx = self.command_tx.clone().unwrap();
                let mut messages =
                    Vec::from_iter(self.conversation.messages.values().map(|x| x.clone()));

                let input_uuid = self.conversation.generate_message_id();
                let recv_uuid = self.conversation.generate_message_id();

                tokio::spawn(async move {
                    action_tx
                        .send(Action::ReceiveMessage(input_uuid, message.clone()))
                        .await
                        .ok();

                    if let Some(model) = model {
                        let mut content = String::new();
                        action_tx
                            .send(Action::ReceiveMessage(
                                recv_uuid,
                                Message {
                                    role: Role::Assistant,
                                    content: content.clone(),
                                    status: Some(PredictionStatus::Starting),
                                    model: Some(model.clone()),
                                },
                            ))
                            .await
                            .ok();
                        messages.push(message);

                        let prediction = create_prediction(&model, messages.clone()).await;

                        match prediction {
                            Ok(mut prediction) => 'outer: loop {
                                prediction.reload().await.ok();
                                let status = prediction.get_status().await;
                                match status {
                                    PredictionStatus::Starting => {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(500))
                                            .await;
                                    }
                                    PredictionStatus::Canceled | PredictionStatus::Failed => {
                                        action_tx
                                            .send(Action::StreamMessage(
                                                recv_uuid,
                                                Message {
                                                    role: Role::Assistant,
                                                    content: content.clone(),
                                                    status: Some(status),
                                                    model: Some(model.clone()),
                                                },
                                            ))
                                            .await
                                            .ok();
                                    }
                                    PredictionStatus::Succeeded | PredictionStatus::Processing => {
                                        let stream = prediction.get_stream().await;
                                        match stream {
                                            Ok(mut stream) => {
                                                while let Some(event) = stream.next().await {
                                                    match event {
                                                        Ok(event) => {
                                                            if event.event == "done" {
                                                                action_tx
                                                                .send(Action::StreamMessage(
                                                                    recv_uuid,
                                                                    Message {
                                                                        role: Role::Assistant,
                                                                        content: content.clone(),
                                                                        status: Some(PredictionStatus::Succeeded),
                                                                        model: Some(model.clone()),
                                                                    },
                                                                ))
                                                                .await
                                                                .ok();

                                                                action_tx.send(Action::SaveActiveConversation).await.ok();

                                                                break 'outer;
                                                            }

                                                            content.push_str(event.data.as_str());
                                                            action_tx
                                                                .send(Action::StreamMessage(
                                                                    recv_uuid,
                                                                    Message {
                                                                        role: Role::Assistant,
                                                                        content: content.clone(),
                                                                        status: Some(PredictionStatus::Processing),
                                                                        model: Some(model.clone()),
                                                                    },
                                                                ))
                                                                .await
                                                                .ok();
                                                        }
                                                        Err(err) => {
                                                            action_tx.send(Action::StreamMessage(recv_uuid, Message { role: Role::Assistant, content: err.to_string(), status: Some(PredictionStatus::Failed), model: Some(model.clone())})).await.ok();
                                                        }
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                action_tx
                                                    .send(Action::StreamMessage(
                                                        recv_uuid,
                                                        Message {
                                                            role: Role::Assistant,
                                                            content: err.to_string(),
                                                            status: Some(PredictionStatus::Failed),
                                                            model: Some(model.clone()),
                                                        },
                                                    ))
                                                    .await
                                                    .ok();
                                            }
                                        }
                                    }
                                }
                            },
                            Err(err) => {
                                action_tx
                                    .send(Action::StreamMessage(
                                        recv_uuid,
                                        Message {
                                            role: Role::Assistant,
                                            content: err.to_string(),
                                            status: Some(PredictionStatus::Failed),
                                            model: Some(model.clone()),
                                        },
                                    ))
                                    .await;
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
        let mut visible_lines = rect.height as usize;

        match self.state {
            ViewerState::Maximized => {
                if let Some(message) = self.conversation.get_selected_message().ok() {
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

                            if let Some(status) = message.status {
                                let (status_str, color) = match status {
                                    PredictionStatus::Starting => ("Starting...", Color::Blue),
                                    PredictionStatus::Processing => {
                                        ("Processing...", Color::LightGreen)
                                    }
                                    PredictionStatus::Canceled => ("Canceled.", Color::Gray),
                                    PredictionStatus::Succeeded => {
                                        ("Succeeded.", Color::LightGreen)
                                    }
                                    PredictionStatus::Failed => ("Failed.", Color::Red),
                                };
                                spans.push(Span::styled(
                                    " - ",
                                    Style::default().fg(ASSISTANT_COLOR),
                                ));
                                spans.push(Span::styled(status_str, Style::default().fg(color)));
                            }
                            message_lines.push(Line::from(spans));
                        }
                    }

                    for line in message.content.split("\n") {
                        message_lines.push(Line::from(vec![Span::styled(
                            line,
                            Style::default().fg(Color::White),
                        )]));
                    }

                    let line_count = message_lines.len();
                    let text = Text::from(message_lines);

                    let vertical_scroll = 0;
                    let scrollbar = Scrollbar::default()
                        .orientation(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓"));
                    let mut scrollbar_state =
                        ScrollbarState::new(line_count).position(vertical_scroll);

                    let paragraph = Paragraph::new(text)
                        .block(
                            Block::default()
                                .title(
                                    Title::from(format!(" Focused Message "))
                                        .alignment(Alignment::Left),
                                )
                                .borders(Borders::ALL)
                                .border_type(BorderType::Thick)
                                .style(Style::default().fg(ACTIVE_COLOR).bg(Color::Black)),
                        )
                        .alignment(Alignment::Left)
                        .wrap(Wrap { trim: true })
                        .scroll((vertical_scroll as u16, 0));
                    f.render_widget(paragraph, rect);
                    f.render_stateful_widget(
                        scrollbar,
                        rect.inner(&Margin {
                            vertical: 1,
                            horizontal: 0,
                        }), // using a inner vertical margin of 1 unit makes the scrollbar inside the block
                        &mut scrollbar_state,
                    );
                }
            }
            _ => {
                // Render Messages
                let mut message_items = Vec::new();
                let mut line_count: usize = 0;
                for (_, message) in &self.conversation.messages {
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

                            if let Some(status) = message.status.clone() {
                                let (status_str, color) = match status {
                                    PredictionStatus::Starting => ("Starting...", Color::Blue),
                                    PredictionStatus::Processing => {
                                        ("Processing...", Color::LightGreen)
                                    }
                                    PredictionStatus::Canceled => ("Canceled.", Color::Gray),
                                    PredictionStatus::Succeeded => {
                                        ("Succeeded.", Color::LightGreen)
                                    }
                                    PredictionStatus::Failed => ("Failed.", Color::Red),
                                };
                                spans.push(Span::styled(
                                    " - ",
                                    Style::default().fg(ASSISTANT_COLOR),
                                ));
                                spans.push(Span::styled(status_str, Style::default().fg(color)));
                            }

                            message_lines.push(Line::from(spans));
                        }
                    }

                    visible_lines -= message_lines.len();

                    'outer: for line in message.content.split("\n") {
                        let words = WordSeparator::AsciiSpace
                            .find_words(line)
                            .collect::<Vec<_>>();
                        let subs = lines_to_strings(
                            wrap_optimal_fit(&words, &[rect.width as f64 - 2.0], &Penalties::new())
                                .unwrap(),
                        );

                        for sub in subs {
                            if visible_lines <= 2 {
                                message_lines.push(Line::from(vec![Span::styled(
                                    "...",
                                    Style::default().fg(Color::White),
                                )]));
                                break 'outer;
                            }

                            message_lines.push(Line::from(vec![Span::styled(
                                sub,
                                Style::default().fg(Color::White),
                            )]));

                            visible_lines -= 1;
                        }
                    }

                    let mut break_line = String::new();
                    for _ in 0..(rect.width - 2) {
                        break_line.push('-');
                    }
                    message_lines
                        .push(Line::from(vec![Span::styled(break_line, Style::default())]));

                    line_count = message_lines.len();

                    // Add seperator to the bottom of the message
                    message_items.push(ListItem::new(Text::from(message_lines)));
                }

                let vertical_scroll = 0;
                let list = List::new(message_items.clone()).block(
                    Block::default()
                        .title(Title::from(" Conversation ").alignment(Alignment::Left))
                        .title(Title::from(self.keymap.clone()).alignment(Alignment::Right))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Thick)
                        .style(Style::default().fg(match self.state {
                            ViewerState::Active | ViewerState::Maximized => ACTIVE_COLOR,
                            ViewerState::Unfocused => UNFOCUSED_COLOR,
                            ViewerState::Focused => FOCUSED_COLOR,
                        }))
                        .bg(Color::Black),
                );

                let list = match self.state {
                    ViewerState::Active => list
                        .highlight_style(
                            Style::default()
                                .add_modifier(Modifier::ITALIC)
                                .fg(Color::LightYellow),
                        )
                        .highlight_symbol(""),
                    _ => list,
                };

                let mut list_state =
                    ListState::default().with_selected(self.conversation.selected_message);

                let scrollbar = Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"));
                let mut scrollbar_state = ScrollbarState::new(line_count).position(vertical_scroll);

                f.render_stateful_widget(list, rect, &mut list_state);
                f.render_stateful_widget(
                    scrollbar,
                    rect.inner(&Margin {
                        vertical: 1,
                        horizontal: 0,
                    }), // using a inner vertical margin of 1 unit makes the scrollbar inside the block
                    &mut scrollbar_state,
                );
            }
        }

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
