use futures::StreamExt;
use lazy_static::lazy_static;
use ratatui::widgets::block::Title;
use regex::Regex;
use std::str::from_utf8;
use std::time::{Duration, Instant};
use std::{fmt, fs};
use textwrap::core::Word;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::WordSeparator;
use uuid::Uuid;

use color_eyre::eyre::Result;
use indexmap::IndexMap;
use ratatui::{prelude::*, widgets::*};
use replicate_rs::predictions::PredictionStatus;

use super::Component;
use crate::mode::Mode;
use crate::styles::{
    ACTIVE_COLOR, ASSISTANT_COLOR, FOCUSED_COLOR, SYSTEM_COLOR, UNFOCUSED_COLOR, USER_COLOR,
};
use crate::{action::Action, tui::Frame};
use archer::ai::conversation::{Conversation, ConversationManager};
use async_channel::Sender;

use crate::config::{Config, KeyBindings};
use archer::ai::completion::{CompletionStatus, Message as CompletionMessage, MessageRole};

lazy_static! {
    static ref WHITESPACE_RE: Regex = Regex::new(r"\s*[^\s]+").unwrap();
}

#[derive(Clone, Default)]
enum ViewerState {
    Active,
    #[default]
    Unfocused,
}

#[derive(Default)]
pub struct Viewer {
    command_tx: Option<Sender<Action>>,
    config: Config,
    state: ViewerState,
    visible_start: usize,
    visible_end: usize,
    visible_total: usize,
    sticky_scroll: bool,
    visible_height: usize,
    scrollable: bool,
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn get_title_line<'a>(&self, message: &CompletionMessage, width: usize) -> Line<'a> {
        let mut title_spans = Vec::new();
        match message.role {
            MessageRole::System => title_spans.push((
                " System".to_string(),
                Style::default().fg(SYSTEM_COLOR).bold(),
            )),
            MessageRole::User => {
                title_spans.push((" User".to_string(), Style::default().fg(USER_COLOR).bold()))
            }
            MessageRole::Assistant => {
                title_spans.push((
                    " Assistant".to_string(),
                    Style::default().fg(ASSISTANT_COLOR).bold(),
                ));

                title_spans.push((
                    format!(": {}", message.clone().metadata.model_id),
                    Style::default().fg(ASSISTANT_COLOR),
                ));

                let (status_str, color) = match message.metadata.status {
                    CompletionStatus::Starting => (" Starting...", Color::LightBlue),
                    CompletionStatus::Processing => (" Processing...", Color::LightGreen),
                    CompletionStatus::Succeeded => (" Succeeded", Color::LightGreen),
                    CompletionStatus::Failed => (" Failed", Color::LightRed),
                    CompletionStatus::Canceled => (" Canceled", Color::LightRed),
                };

                let total_span_chars: usize = title_spans
                    .iter()
                    .map(|(span, _)| span.len())
                    .sum::<usize>()
                    + status_str.len()
                    + 3;

                let pad_chars = width.max(total_span_chars) - total_span_chars;
                let mut pad = String::new();
                for _ in 0..pad_chars {
                    pad.push(' ');
                }

                title_spans.push((pad, Style::default()));
                title_spans.push((status_str.to_string(), Style::default().fg(color)));
            }
        }

        Line::from(
            title_spans
                .into_iter()
                .map(|(span, style)| Span::styled(span, style))
                .collect::<Vec<Span>>(),
        )
    }

    pub fn get_lines_from_content<'a>(&self, content: &'a str, width: usize) -> Vec<Line<'a>> {
        let visible_width = width.max(4) - 4;
        let mut lines = vec![Line::styled("", Style::default())];

        let content = content.trim_matches('\n');

        for line in content.lines() {
            let words = WordSeparator::Custom(split_sentence)
                .find_words(line)
                .collect::<Vec<_>>();

            let subs = lines_to_strings(
                wrap_optimal_fit(&words, &[visible_width as f64], &Penalties::new()).unwrap(),
            );

            for mut sub in subs {
                if !sub.starts_with(' ') {
                    sub = format!(" {sub}");
                }

                lines.push(Line::styled(sub, Style::default().fg(Color::White)));
            }
        }

        lines
    }

    pub fn get_visible_ranges(&mut self) -> (usize, usize) {
        if self.sticky_scroll {
            self.visible_end = self.visible_total;
            self.visible_start = self.visible_end.max(self.visible_height) - self.visible_height;
        }

        (self.visible_start, self.visible_end)
    }

    pub fn get_visible_messages<'a>(
        &'a mut self,
        conversation: &'a Conversation,
        width: usize,
    ) -> VisibleMessages {
        let mut messages = Vec::new();
        for (id, message) in &conversation.messages {
            let mut lines = vec![self.get_title_line(&message, width)];
            let content = message.content.trim();
            lines.extend(self.get_lines_from_content(content, width));

            messages.push(VisibleMessage {
                lines,
                role: message.role.clone(),
                uuid: id.clone(),
            });
        }

        let messages = VisibleMessages {
            messages: messages.clone(),
        };

        self.visible_total = messages.total_len().max(1) - 1;
        if self.visible_total > self.visible_height {
            self.scrollable = true;
        } else {
            self.scrollable = false;
        }

        messages
    }
}

impl Component for Viewer {
    fn register_action_handler(&mut self, tx: Sender<Action>) -> anyhow::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> anyhow::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> anyhow::Result<Option<Action>> {
        match action {
            Action::SwitchMode(mode) => match mode {
                Mode::Input => {
                    self.state = ViewerState::Unfocused;
                }
                Mode::ActiveViewer => {
                    self.state = ViewerState::Active;
                }
                Mode::ModelSelector => {
                    self.state = ViewerState::Unfocused;
                }
                Mode::ActiveInput | Mode::ConversationManager => {
                    self.state = ViewerState::Unfocused;
                }
            },
            Action::ScrollUp => {
                if self.scrollable {
                    if self.visible_end > self.visible_height {
                        self.visible_start = self.visible_start.max(1) - 1;
                        self.visible_end = self.visible_end.max(1) - 1;
                    }
                }

                self.sticky_scroll = false;
            }
            Action::ScrollDown => {
                if self.scrollable {
                    if self.visible_end < self.visible_total {
                        self.visible_end += 1;
                        self.visible_start += 1;
                    }
                }
                self.sticky_scroll = false;
            }
            Action::ReceiveMessage(..)
            | Action::StreamMessage(..)
            | Action::LoadSelectedConversation => {
                self.sticky_scroll = true;
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
            .style(
                Style::default()
                    .fg(match self.state {
                        ViewerState::Active => ACTIVE_COLOR,
                        _ => UNFOCUSED_COLOR,
                    })
                    .bg(Color::Black),
            );
        f.render_widget(block.clone(), rect);

        let inner = rect.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        });

        self.visible_height = (inner.height - 1) as usize;
        let message_width = (inner.width.min(105) - 1) as usize;

        let selected_uuid = match self.state {
            ViewerState::Active => conversation.get_selected_uuid(),
            _ => None,
        };

        let state = self.state.clone();
        let visible_height = self.visible_height.clone();
        let (mut visible_start, visible_end) = self.get_visible_ranges();
        let messages = self.get_visible_messages(conversation, message_width);
        let total_len = messages.total_len();

        let (visible_start, visible_end) = match state {
            ViewerState::Active => {
                if let Some(mut visible_end) = messages.length_at_uuid(selected_uuid) {
                    if visible_end < visible_height {
                        visible_start = 0;
                        visible_end = visible_height;
                    } else {
                        visible_start = visible_end.max(visible_height) - visible_height;
                    }

                    (visible_start, visible_end)
                } else {
                    (visible_start, visible_end)
                }
            }
            _ => (visible_start, visible_end),
        };

        messages.render(
            f,
            inner,
            visible_start,
            visible_end,
            message_width as u16,
            selected_uuid,
        );

        self.visible_end = visible_end;

        if self.scrollable {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            let mut scrollbar_state =
                ScrollbarState::new(self.visible_total).position(self.visible_end);
            f.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
        }
        Ok(())
    }
}

#[derive(Clone)]
struct VisibleMessage<'a> {
    lines: Vec<Line<'a>>,
    role: MessageRole,
    uuid: Uuid,
}

#[derive(Clone)]
pub struct VisibleMessages<'a> {
    messages: Vec<VisibleMessage<'a>>,
}

enum RenderState {
    Full,
    TruncatedTop,
    TruncatedBottom,
    TruncatedTopAndBottom,
}

impl<'a> VisibleMessages<'a> {
    fn total_len(&self) -> usize {
        self.messages
            .iter()
            .map(|x| x.lines.iter().len() + 2)
            .sum::<usize>()
    }

    fn length_at_uuid(&self, uuid: Option<Uuid>) -> Option<usize> {
        if let Some(uuid) = uuid {
            let mut length = 0;
            for message in &self.messages {
                length += message.lines.iter().len() + 2;

                if message.uuid == uuid {
                    return Some(length);
                }
            }
        }
        None
    }

    fn render(
        &self,
        f: &mut Frame<'_>,
        rect: Rect,
        visible_start: usize,
        visible_end: usize,
        width: u16,
        selected_uuid: Option<Uuid>,
    ) {
        let mut y = rect.y;

        let mut i = 0;
        for message in &self.messages {
            let mut message_lines = Vec::new();

            let mut top_border = false;
            let mut bottom_border = false;

            for (idx, line) in message.lines.iter().enumerate() {
                if idx == 0 {
                    if i >= visible_start && i <= visible_end {
                        top_border = true;
                    }
                    i += 1;
                }

                if i >= visible_start && i <= visible_end {
                    message_lines.push(line.clone());
                }
                i += 1;

                if idx == message.lines.iter().len() - 1 {
                    if i >= visible_start && i <= visible_end {
                        bottom_border = true;
                    }
                    i += 1;
                }
            }

            let (borders, border_height) = {
                if top_border && bottom_border {
                    (
                        Borders::TOP | Borders::BOTTOM | Borders::LEFT | Borders::RIGHT,
                        2,
                    )
                } else if top_border {
                    (Borders::TOP | Borders::LEFT | Borders::RIGHT, 1)
                } else if bottom_border {
                    (Borders::BOTTOM | Borders::LEFT | Borders::RIGHT, 1)
                } else {
                    (Borders::LEFT | Borders::RIGHT, 0)
                }
            };

            let message_len = message_lines.iter().len();
            let message_color = if let Some(selected_uuid) = selected_uuid {
                if message.uuid == selected_uuid {
                    ACTIVE_COLOR
                } else {
                    FOCUSED_COLOR
                }
            } else {
                FOCUSED_COLOR
            };

            let block = Block::default()
                .borders(borders)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(message_color));
            let paragraph = Paragraph::new(Text::from(message_lines)).block(block);

            let height = (message_len + border_height) as u16;
            let x = match message.role {
                MessageRole::Assistant => rect.width - width,
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

fn split_sentence(line: &str) -> Box<dyn Iterator<Item = Word<'_>> + '_> {
    let words = WHITESPACE_RE
        .find_iter(line)
        .map(|word| Word::from(word.as_str()));
    Box::new(words.into_iter())
}

fn lines_to_strings(lines: Vec<&[Word<'_>]>) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.iter().map(|word| &**word).collect::<Vec<_>>().join(""))
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_split_sentence() {
        assert_eq!(
            split_sentence("    this is a sentence")
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
            vec!["    this", " is", " a", " sentence"]
        );
    }
}
