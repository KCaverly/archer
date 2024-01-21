use anyhow::anyhow;
use archer::ai::completion::CompletionModel;
use archer::ai::config::ARCHER_CONFIG;
use archer::ai::providers::{COMPLETION_PROVIDERS, DEFAULT_COMPLETION_PROVIDER};
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
use crate::mode::Mode;
use crate::styles::{
    ACTIVE_COLOR, ASSISTANT_COLOR, FOCUSED_COLOR, SYSTEM_COLOR, UNFOCUSED_COLOR, USER_COLOR,
};
use crate::{action::Action, tui::Frame};
use archer::ai::completion::{CompletionModelID, CompletionProviderID};
use archer::ai::conversation::{Conversation, ConversationManager};
use async_channel::Sender;

use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct ModelSelector {
    command_tx: Option<Sender<Action>>,
    config: Config,
    selected_provider: CompletionProviderID,
    selected_model: HashMap<CompletionProviderID, usize>,
    models: Vec<Box<dyn CompletionModel>>,
}

impl ModelSelector {
    pub fn new() -> Self {
        let provider = COMPLETION_PROVIDERS
            .get_provider(&DEFAULT_COMPLETION_PROVIDER.to_string())
            .unwrap();
        let mut selected_model = HashMap::<CompletionProviderID, usize>::new();
        selected_model.insert(DEFAULT_COMPLETION_PROVIDER.to_string(), 0);
        let models = provider.list_models();
        Self {
            selected_model,
            models,
            selected_provider: DEFAULT_COMPLETION_PROVIDER.to_string(),
            ..Default::default()
        }
    }
    fn select_next_model(&mut self) {
        if let Some(mut selected_model) = self.selected_model.get_mut(&self.selected_provider) {
            if selected_model <= &mut self.models.len() {
                *selected_model += 1 as usize;
            }
        }
    }

    fn select_previous_model(&mut self) {
        if let Some(mut selected_model) = self.selected_model.get_mut(&self.selected_provider) {
            if selected_model >= &mut (0 as usize) {
                *selected_model -= 1;
            }
        }
    }

    fn get_selected_model_id(&self) -> anyhow::Result<CompletionModelID> {
        if let Some(selected_model) = self.selected_model.get(&self.selected_provider) {
            anyhow::Ok(self.models[*selected_model].get_display_name())
        } else {
            Err(anyhow!("selected model not found"))
        }
    }
}

impl Component for ModelSelector {
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
            Action::NextProvider => {
                self.selected_provider =
                    COMPLETION_PROVIDERS.next_provider(&self.selected_provider);
                if let Some(provider) = COMPLETION_PROVIDERS.get_provider(&self.selected_provider) {
                    self.models = provider.list_models();
                    self.selected_model
                        .insert(self.selected_provider.clone(), 0);
                }
            }
            Action::SelectNextModel => self.select_next_model(),
            Action::SelectPreviousModel => self.select_previous_model(),
            Action::SwitchToSelectedModel => {
                let selected_model = self.get_selected_model_id()?;
                let selected_provider = self.selected_provider.clone();
                let action_tx = self.command_tx.clone().unwrap();
                tokio::spawn(async move {
                    action_tx
                        .send(Action::SwitchModel(selected_provider, selected_model))
                        .await
                        .ok();
                    action_tx
                        .send(Action::SwitchMode(Mode::ActiveInput))
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
        let block = Block::default()
            .title(" Config ")
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .style(Style::default().fg(ACTIVE_COLOR).bg(Color::Black));

        f.render_widget(block, rect);

        let bottom = (((rect.height as f32 - 3.0) / rect.height as f32) * 100.0) as u16;
        let top = 100 - bottom;

        let vertical_panels = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(top),
                Constraint::Percentage(bottom),
            ])
            .split(rect.inner(&Margin::new(1, 1)));

        let paragraph = Paragraph::new(format!(" Provider: {} ", self.selected_provider)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Black))
                .style(Style::default().fg(Color::Gray).bg(Color::Black)),
        );

        f.render_widget(paragraph, vertical_panels[0]);

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
                    .border_type(BorderType::Rounded)
                    .style(Style::default().fg(ACTIVE_COLOR).bg(Color::Black)),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::ITALIC)
                    .bg(Color::DarkGray),
            )
            .highlight_symbol("");

        if let Some(selected_id) = self.selected_model.get(&self.selected_provider) {
            let mut list_state = ListState::default().with_selected(Some(*selected_id));
            f.render_stateful_widget(paragraph, vertical_panels[1], &mut list_state);
        }

        Ok(())
    }
}
