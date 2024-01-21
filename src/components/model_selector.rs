use archer::ai::completion::CompletionModel;
use archer::ai::providers::COMPLETION_PROVIDERS;
use color_eyre::eyre::Result;
use futures::StreamExt;
use ratatui::{prelude::*, widgets::*};
use replicate_rs::predictions::PredictionStatus;
use std::collections::HashMap;
use std::fmt;
use std::str::from_utf8;
use std::time::Instant;
use strum::IntoEnumIterator; // 0.17.1

use textwrap::core::Word;
use textwrap::wrap_algorithms::{wrap_optimal_fit, Penalties};
use textwrap::WordSeparator;

use super::Component;
use crate::styles::{
    ACTIVE_COLOR, ASSISTANT_COLOR, FOCUSED_COLOR, SYSTEM_COLOR, UNFOCUSED_COLOR, USER_COLOR,
};
use crate::{action::Action, tui::Frame};
use archer::ai::conversation::{Conversation, ConversationManager};
use async_channel::Sender;

use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct ModelSelector {
    command_tx: Option<Sender<Action>>,
    config: Config,
    selected_model: usize,
    models: Vec<Box<dyn CompletionModel>>,
}

impl ModelSelector {
    pub fn new() -> Self {
        let provider = COMPLETION_PROVIDERS
            .get_provider("replicate".to_string())
            .unwrap();
        let models = provider.list_models();
        Self {
            selected_model: 0,
            models,
            ..Default::default()
        }
    }
    fn select_next_model(&mut self) {
        if self.selected_model <= self.models.len() {
            self.selected_model += 1;
        }
    }

    fn select_previous_model(&mut self) {
        if self.selected_model > 0 {
            self.selected_model -= 1;
        } else {
        }
    }

    fn get_selected_model(&mut self) -> String {
        self.models[self.selected_model].get_display_name()
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
            Action::SelectNextModel => self.select_next_model(),
            Action::SelectPreviousModel => self.select_previous_model(),
            Action::SwitchToSelectedModel => {
                let selected_model = self.get_selected_model();
                let action_tx = self.command_tx.clone().unwrap();
                tokio::spawn(async move {
                    action_tx
                        .send(Action::SwitchModel("replicate".to_string(), selected_model))
                        .await
                        .ok();
                });
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
        let mut items = Vec::new();
        for model in &self.models {
            items.push(ListItem::new(Line::from(vec![Span::styled(
                model.get_display_name(),
                Style::default(),
            )])))
        }

        let paragraph = List::new(items)
            .block(
                Block::default()
                    .title(" Select Model ")
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

        let mut list_state = ListState::default().with_selected(Some(self.selected_model));
        f.render_stateful_widget(paragraph, rect, &mut list_state);
        Ok(())
    }
}
