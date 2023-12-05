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
pub struct ModelSelector {
    command_tx: Option<Sender<Action>>,
    config: Config,
}

impl ModelSelector {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl Component for ModelSelector {
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
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let paragraph = Paragraph::new("")
            .block(
                Block::default()
                    .title("Select Model")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(Style::default().fg(ACTIVE_COLOR).bg(Color::Black)),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, rect);
        Ok(())
    }
}
