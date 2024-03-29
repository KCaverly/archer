use std::time::Instant;

use archer::ai::config::{ModelConfig, Profile, ARCHER_CONFIG};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode};
use ratatui::widgets::block::{Position, Title};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, Key, TextArea};

use super::Component;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;
use crate::styles::{ACTIVE_COLOR, FOCUSED_COLOR, UNFOCUSED_COLOR};
use crate::{action::Action, tui::Frame};
use archer::ai::completion::{CompletionModel, CompletionStatus, MessageMetadata};
use archer::ai::completion::{Message, MessageRole};
use archer::ai::conversation::{Conversation, ConversationManager};
use archer::ai::providers::COMPLETION_PROVIDERS;

use async_channel::Sender;

#[derive(Default, Eq, PartialEq)]
enum InputState {
    Focused,
    #[default]
    Unfocused,
    Active,
}

// #[derive(Default)]
pub struct MessageInput<'a> {
    command_tx: Option<Sender<Action>>,
    config: Config,
    state: InputState,
    active_model: ModelConfig,
    active_profile: Profile,
    keymap: String,
    textarea: TextArea<'a>,
}

impl MessageInput<'static> {
    pub fn new(focused: bool, keymap: String) -> Self {
        let state = if focused {
            InputState::Focused
        } else {
            InputState::Unfocused
        };

        let active_model = ARCHER_CONFIG.default_completion_model.clone();
        let active_profile = ARCHER_CONFIG.profiles.get(0).unwrap().clone();

        Self {
            command_tx: None,
            config: Config::default(),
            state,
            keymap,
            active_model,
            active_profile,
            textarea: TextArea::default(),
        }
    }
}

impl Component for MessageInput<'static> {
    fn register_action_handler(&mut self, tx: Sender<Action>) -> anyhow::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> anyhow::Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<Option<Action>> {
        if self.state == InputState::Active {
            match key.code {
                KeyCode::Enter => {
                    let content = self.textarea.lines().join("\n");
                    if content.len() > 0 {
                        let action = Action::SendMessage(
                            Message {
                                role: MessageRole::User,
                                content,
                                metadata: Some(MessageMetadata {
                                    model_config: self.active_model.clone(),
                                    status: CompletionStatus::Succeeded,
                                }),
                            },
                            self.active_profile.clone(),
                        );
                        self.textarea = TextArea::default();
                        return Ok(Some(action));
                    }
                }
                _ => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char(char) => {
                                if char == 'n' {
                                    self.textarea.insert_str("\n");
                                }
                            }
                            _ => {
                                self.textarea.input(key);
                            }
                        };
                    } else {
                        self.textarea.input(key);
                    }
                }
            }
        }

        Ok(None)
    }

    fn update(&mut self, action: Action) -> anyhow::Result<Option<Action>> {
        match action {
            Action::SwitchKeymap(keymap) => {
                self.keymap = keymap;
            }
            Action::SwitchMode(mode) => match mode {
                Mode::ActiveViewer | Mode::ModelSelector | Mode::ConversationManager => {
                    self.state = InputState::Unfocused;
                }
                Mode::Input => {
                    self.state = InputState::Focused;
                }
                Mode::ActiveInput => {
                    self.state = InputState::Active;
                }
            },
            Action::SwitchModel(model_config) => {
                self.active_model = model_config;
            }
            Action::SwitchProfile(profile) => {
                self.active_profile = profile;
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
        let display_name = self.active_model.model_id.clone();
        let profile_name = self.active_profile.name.clone();
        let block = Block::default()
            .title(
                Title::from(format!(" Message ({profile_name}: {display_name}) "))
                    .alignment(Alignment::Left),
            )
            .title(
                Title::from(self.keymap.clone())
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .style(Style::default().fg(match self.state {
                InputState::Active => ACTIVE_COLOR,
                InputState::Focused => FOCUSED_COLOR,
                InputState::Unfocused => UNFOCUSED_COLOR,
            }))
            .bg(Color::Black);

        self.textarea.set_block(block);
        self.textarea.set_cursor_line_style(Style::default());

        f.render_widget(self.textarea.widget(), rect);
        Ok(())
    }
}
