use anyhow::anyhow;
use archer::ai::completion::CompletionModel;
use archer::ai::config::{ModelConfig, Profile, ARCHER_CONFIG};
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
enum Tab {
    #[default]
    Models,
    Profiles,
}

impl Tab {
    fn get_id(&self) -> usize {
        match self {
            Tab::Models => 0,
            Tab::Profiles => 1,
        }
    }
}

#[derive(Default)]
pub struct ModelSelector {
    command_tx: Option<Sender<Action>>,
    config: Config,
    selected_provider: CompletionProviderID,
    selected_model: HashMap<CompletionProviderID, (usize, Vec<ModelConfig>)>,
    selected_profile: (usize, Vec<Profile>),
    selected_tab: Tab,
}

impl ModelSelector {
    pub fn new() -> Self {
        let provider_id = ARCHER_CONFIG.default_completion_model.provider_id.clone();
        let provider = COMPLETION_PROVIDERS.get_provider(&provider_id).unwrap();
        let mut selected_model = HashMap::<CompletionProviderID, (usize, Vec<ModelConfig>)>::new();
        let models = provider.list_models();
        selected_model.insert(provider_id.clone(), (0, models));
        let selected_profiles = ARCHER_CONFIG.profiles.clone();
        Self {
            selected_model,
            selected_provider: provider_id,
            selected_profile: (0, selected_profiles),
            ..Default::default()
        }
    }
    fn select_next(&mut self) {
        match self.selected_tab {
            Tab::Models => {
                if let Some((selected_idx, models)) =
                    self.selected_model.get_mut(&self.selected_provider)
                {
                    if selected_idx < &mut (models.len() - 1) {
                        *selected_idx += 1;
                    }
                }
            }
            Tab::Profiles => {
                if self.selected_profile.0 < (self.selected_profile.1.len() - 1) {
                    self.selected_profile.0 += 1;
                }
            }
        }
    }

    fn select_previous(&mut self) {
        match self.selected_tab {
            Tab::Models => {
                if let Some((selected_idx, _)) =
                    self.selected_model.get_mut(&self.selected_provider)
                {
                    if selected_idx > &mut (0 as usize) {
                        *selected_idx -= 1;
                    }
                }
            }
            Tab::Profiles => {
                if self.selected_profile.0 > (0 as usize) {
                    self.selected_profile.0 -= 1;
                }
            }
        }
    }

    fn get_selected_model_config(&self) -> anyhow::Result<ModelConfig> {
        if let Some(Some(model)) = self
            .selected_model
            .get(&self.selected_provider)
            .map(|x| x.1.get(x.0))
        {
            anyhow::Ok(model.clone())
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
            Action::NextTab => {
                self.selected_tab = match self.selected_tab {
                    Tab::Models => Tab::Profiles,
                    Tab::Profiles => Tab::Models,
                };
            }
            Action::PrevProvider => {
                let prev_provider = COMPLETION_PROVIDERS.prev_provider(&self.selected_provider);
                if let Some(provider) = COMPLETION_PROVIDERS.get_provider(&prev_provider) {
                    let models = provider.list_models();

                    if !self.selected_model.contains_key(&prev_provider) {
                        self.selected_model
                            .insert(prev_provider.clone(), (0, models));
                    }
                    self.selected_provider = prev_provider;
                }
            }
            Action::NextProvider => {
                let next_provider = COMPLETION_PROVIDERS.next_provider(&self.selected_provider);
                if let Some(provider) = COMPLETION_PROVIDERS.get_provider(&next_provider) {
                    let models = provider.list_models();

                    if !self.selected_model.contains_key(&next_provider) {
                        self.selected_model
                            .insert(next_provider.clone(), (0, models));
                    }
                    self.selected_provider = next_provider;
                }
            }
            Action::SelectNextInConfigList => self.select_next(),
            Action::SelectPreviousInConfigList => self.select_previous(),
            Action::SwitchToSelectedItem => match self.selected_tab {
                Tab::Profiles => {
                    let selected_profile = self
                        .selected_profile
                        .1
                        .get(self.selected_profile.0)
                        .unwrap()
                        .clone();

                    let action_tx = self.command_tx.clone().unwrap();
                    tokio::spawn(async move {
                        action_tx
                            .send(Action::SwitchProfile(selected_profile))
                            .await
                            .ok();
                    });
                }
                Tab::Models => {
                    let selected_model = self.get_selected_model_config()?;
                    let action_tx = self.command_tx.clone().unwrap();
                    tokio::spawn(async move {
                        action_tx
                            .send(Action::SwitchModel(selected_model))
                            .await
                            .ok();
                        action_tx
                            .send(Action::SwitchMode(Mode::ActiveInput))
                            .await
                            .ok();
                    });
                }
            },
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

        let tabs_panel = vertical_panels[0];
        let second_panel = vertical_panels[1];

        let bottom = ((vertical_panels[1].height as f32 - 3.0) / (vertical_panels[1].height as f32)
            * 100.0) as u16;
        let top = 100 - bottom;

        let models_panels = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(top),
                Constraint::Percentage(bottom),
            ])
            .split(second_panel.inner(&Margin::new(0, 0)));

        let provider_panel = models_panels[0];
        let models_panel = models_panels[1];

        let tabs = Tabs::new(vec!["Models", "Profiles"])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Black))
                    .style(Style::default().fg(Color::Gray).bg(Color::Black)),
            )
            .select(self.selected_tab.get_id());

        f.render_widget(tabs, vertical_panels[0]);

        match self.selected_tab {
            Tab::Models => {
                let provider_symbol = if COMPLETION_PROVIDERS
                    .get_provider(&self.selected_provider)
                    .map(|x| x.has_credentials())
                    .unwrap_or(false)
                {
                    "(API Key Available)"
                } else {
                    "(API Key Missing)"
                };

                let paragraph = Paragraph::new(format!(
                    " Provider: {} {}",
                    self.selected_provider, provider_symbol
                ))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        // .border_style(Style::default().fg(Color::Black))
                        .style(Style::default().fg(ACTIVE_COLOR).bg(Color::Black)),
                );
                f.render_widget(paragraph, provider_panel);

                let mut items = Vec::new();

                for model in self
                    .selected_model
                    .get(&self.selected_provider)
                    .map(|x| x.1.clone())
                    .unwrap_or(Vec::new())
                {
                    items.push(ListItem::new(Line::from(vec![Span::styled(
                        model.model_id.clone(),
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

                if let Some((selected_id, _)) = self.selected_model.get(&self.selected_provider) {
                    let mut list_state = ListState::default().with_selected(Some(*selected_id));
                    f.render_stateful_widget(paragraph, models_panel, &mut list_state);
                }
            }
            Tab::Profiles => {
                let mut items = Vec::new();
                for profile in &ARCHER_CONFIG.profiles {
                    items.push(ListItem::new(Line::from(vec![Span::styled(
                        profile.name.clone(),
                        Style::default(),
                    )])));
                }

                let paragraph = List::new(items)
                    .block(
                        Block::default()
                            .title(" Select Profile ")
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

                let mut list_state =
                    ListState::default().with_selected(Some(self.selected_profile.0));
                f.render_stateful_widget(paragraph, second_panel, &mut list_state);
            }
        }

        Ok(())
    }
}
