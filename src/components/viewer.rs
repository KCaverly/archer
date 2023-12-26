use futures::StreamExt;
use ratatui::widgets::block::Title;
use std::str::from_utf8;
use std::time::{Duration, Instant};
use std::{fmt, fs};
use textwrap::core::Word;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::WordSeparator;

use color_eyre::eyre::Result;
use indexmap::IndexMap;
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
    state: ViewerState,
    current_scroll: usize,
    visible_start: usize,
    visible_end: usize,
    visible_total: usize,
}

impl Viewer {
    pub fn new(focused: bool) -> Self {
        let state = if focused {
            ViewerState::Focused
        } else {
            ViewerState::Unfocused
        };

        Self {
            state,
            ..Default::default()
        }
    }

    pub fn get_visible_messages<'a>(&'a self, conversation: &'a Conversation) -> VisibleMessages {
        let mut messages = Vec::new();
        for (_, message) in &conversation.messages {
            let mut lines = vec![];
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

            for line in message.content.split("\n") {
                lines.push(Line::from(vec![Span::styled(
                    line.clone(),
                    Style::default().fg(Color::White),
                )]));
            }

            messages.push(VisibleMessage {
                lines,
                role: message.role.clone(),
            });
        }

        VisibleMessages {
            messages: messages.clone(),
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
            Action::SwitchMode(mode) => match mode {
                Mode::Viewer => {
                    self.state = ViewerState::Focused;
                    if let Some(action_tx) = self.command_tx.clone() {
                        tokio::spawn(async move {
                            action_tx.send(Action::UnfocusConversation).await.ok()
                        });
                    }
                }
                Mode::ActiveViewer => {
                    self.state = ViewerState::Active;
                    if let Some(action_tx) = self.command_tx.clone() {
                        tokio::spawn(async move {
                            action_tx.send(Action::FocusConversation).await.ok()
                        });
                    }
                }
                Mode::ModelSelector => {
                    self.state = ViewerState::Unfocused;
                    if let Some(action_tx) = self.command_tx.clone() {
                        tokio::spawn(async move {
                            action_tx.send(Action::UnfocusConversation).await.ok()
                        });
                    }
                }
                Mode::Input | Mode::ActiveInput | Mode::ConversationManager => {
                    self.state = ViewerState::Unfocused;
                }
                Mode::MessageViewer => {
                    self.state = ViewerState::Maximized;
                }
            },
            Action::ScrollUp => {
                if self.visible_start > 0 {
                    self.visible_start -= 1;
                    self.visible_end -= 1;
                }
            }
            Action::ScrollDown => {
                if self.visible_end < self.visible_total {
                    self.visible_end += 1;
                    self.visible_start += 1;
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(
        &mut self,
        f: &mut Frame<'_>,
        rect: Rect,
        conversation: &Conversation,
        manager: &ConversationManager,
    ) -> Result<()> {
        let block = Block::default()
            .title("Viewer")
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .style(Style::default().fg(FOCUSED_COLOR).bg(Color::Black));
        f.render_widget(block.clone(), rect);

        let inner = rect.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        });

        let max_height = inner.height;

        if self.visible_start == self.visible_end {
            self.visible_end = self.visible_start + inner.height as usize - 6;
        }

        let messages = self.get_visible_messages(conversation);
        let total_len = messages.total_len();
        messages.render(f, inner, self.visible_start, self.visible_end);

        self.visible_total = total_len + 5;

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state =
            ScrollbarState::new(self.visible_total).position(self.visible_end);
        f.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
        Ok(())
    }
}

#[derive(Clone)]
struct VisibleMessage<'a> {
    lines: Vec<Line<'a>>,
    role: Role,
}

#[derive(Clone)]
pub struct VisibleMessages<'a> {
    messages: Vec<VisibleMessage<'a>>,
}

enum RenderState {
    Preceeding,
    Full,
    Trailing,
}

impl<'a> VisibleMessages<'a> {
    fn total_len(&self) -> usize {
        self.messages.iter().map(|x| x.lines.iter().len()).sum()
    }

    fn render(&self, f: &mut Frame<'_>, rect: Rect, visible_start: usize, visible_end: usize) {
        let mut y = rect.y;

        let mut i = 0;
        let width = 100;
        for message in &self.messages {
            let mut message_lines = Vec::new();
            let mut first_in_message = false;
            for (idx, line) in message.lines.iter().enumerate() {
                if i >= visible_start && i <= visible_end {
                    if idx == 0 {
                        first_in_message = true;
                    }
                    message_lines.push(line.clone());
                }
                i += 1;
            }

            let state = if message_lines.len() == message.lines.iter().len() {
                RenderState::Full
            } else if first_in_message {
                RenderState::Trailing
            } else {
                RenderState::Preceeding
            };

            let message_len = message_lines.iter().len();
            if message_len > 0 {
                let block = match state {
                    RenderState::Full => Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                    RenderState::Preceeding => Block::default()
                        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                        .border_type(BorderType::Rounded),
                    RenderState::Trailing => Block::default()
                        .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP)
                        .border_type(BorderType::Rounded),
                };

                let paragraph = Paragraph::new(Text::from(message_lines)).block(block);

                let height = (message_len + 2) as u16;
                let x = match message.role {
                    Role::Assistant => rect.width - width,
                    _ => rect.x,
                };

                let message_rect = Rect {
                    x,
                    y,
                    width,
                    height,
                };

                y += height;

                f.render_widget(paragraph, message_rect);
            }
        }
    }
}

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
