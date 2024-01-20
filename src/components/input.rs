use std::time::Instant;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode};
use ratatui::widgets::block::{Position, Title};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, Key, TextArea};

use super::Component;
// use crate::agent::completion::CompletionModel;
use crate::agent::conversation::{Conversation, ConversationManager};
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;
use crate::styles::{ACTIVE_COLOR, FOCUSED_COLOR, UNFOCUSED_COLOR};
use crate::{action::Action, tui::Frame};
use archer::ai::completion::{CompletionModel, CompletionStatus, MessageMetadata};
use archer::ai::completion::{Message, MessageRole};
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
    active_model: Box<dyn CompletionModel>,
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
        let default_provider = COMPLETION_PROVIDERS.default_provider().unwrap();
        let default_model = default_provider.default_model();
        Self {
            command_tx: None,
            config: Config::default(),
            state,
            keymap,
            active_model: default_model,
            textarea: TextArea::default(),
        }
    }
}

impl Component for MessageInput<'static> {
    fn register_action_handler(&mut self, tx: Sender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        if self.state == InputState::Active {
            match key.code {
                KeyCode::Enter => {
                    let content = self.textarea.lines().join("\n");
                    if content.len() > 0 {
                        let action = Action::SendMessage(Message {
                            role: MessageRole::User,
                            content,
                            metadata: MessageMetadata {
                                provider_id: "replicate".to_string(),
                                model_id: self.active_model.get_display_name(),
                                status: CompletionStatus::Succeeded,
                            },
                        });
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

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
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
            Action::SwitchModel(provider_id, model_id) => {
                if let Some(provider) = COMPLETION_PROVIDERS.get_provider(provider_id) {
                    if let Some(model) = provider.get_model(model_id) {
                        self.active_model = model;
                    }
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
        let display_name = self.active_model.get_display_name();
        let block = Block::default()
            .title(Title::from(format!(" Message ({display_name}) ")).alignment(Alignment::Left))
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
