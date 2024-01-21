use color_eyre::eyre::Result;
use futures::StreamExt;
use ratatui::{prelude::*, widgets::*};
use replicate_rs::predictions::PredictionStatus;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::from_utf8;
use std::time::Instant;
use strum::IntoEnumIterator; // 0.17.1
use uuid::Uuid;
use walkdir::WalkDir;

use textwrap::core::Word;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::WordSeparator;

use super::Component;
use crate::styles::{
    ACTIVE_COLOR, ASSISTANT_COLOR, FOCUSED_COLOR, SYSTEM_COLOR, UNFOCUSED_COLOR, USER_COLOR,
};
use crate::{action::Action, tui::Frame};
use archer::ai::conversation::{Conversation, ConversationManager, CONVERSATION_DIR};
use async_channel::Sender;

use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct ConversationMeta {
    path: PathBuf,
}

#[derive(Default)]
pub struct ConversationSelector {
    command_tx: Option<Sender<Action>>,
    config: Config,
}

impl Component for ConversationSelector {
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
        let mut items = Vec::new();
        for title in manager.list_titles() {
            items.push(ListItem::new(title));
        }

        let paragraph = List::new(items)
            .block(
                Block::default()
                    .title(" Load Conversation ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(Style::default().fg(ACTIVE_COLOR).bg(Color::Black)),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::ITALIC)
                    .bg(Color::DarkGray),
            )
            .highlight_symbol("");

        let mut list_state =
            ListState::default().with_selected(Some(manager.selected_conversation));
        f.render_stateful_widget(paragraph, rect, &mut list_state);
        Ok(())
    }
}
