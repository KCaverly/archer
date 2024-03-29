use arboard::{Clipboard, LinuxClipboardKind, SetExtLinux};
use archer::ai::{
    completion::{
        CompletionModelID, CompletionProvider, CompletionProviderID, CompletionStatus, Message,
        MessageMetadata, MessageRole,
    },
    config::{Profile, ARCHER_CONFIG},
    providers::{get_model, COMPLETION_PROVIDERS},
};
use std::sync::Arc;

use async_channel::Sender;
use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use eventsource_stream::Eventsource;
use futures::{pin_mut, StreamExt};
use indexmap::IndexMap;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use replicate_rs::predictions::PredictionStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::{
    action::Action,
    components::{
        conversation_selector::ConversationSelector, input::MessageInput,
        model_selector::ModelSelector, viewer::Viewer, Component,
    },
    config::Config,
    mode::Mode,
    tui::{self, Frame, Tui},
};
use archer::ai::conversation::{Conversation, ConversationManager};

#[derive(PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum AppPanel {
    Input,
    Viewer,
    ModelSelector,
    ConversationManager,
}

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub components: HashMap<AppPanel, Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub keymap: String,
    pub conversation: Conversation,
    pub manager: ConversationManager,
    pub active_profile: Profile,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> anyhow::Result<Self> {
        let profile = ARCHER_CONFIG.profiles.get(0).unwrap();
        let conversation = Conversation::new(profile.clone());
        let keymap =
            " i: insert; k: focus viewer; m: change model; c: change convo; q: quit; ".to_string();

        let mut components = HashMap::<AppPanel, Box<dyn Component>>::new();
        components.insert(AppPanel::Viewer, Box::new(Viewer::new()));
        components.insert(
            AppPanel::Input,
            Box::new(MessageInput::new(true, keymap.clone())),
        );
        components.insert(AppPanel::ModelSelector, Box::new(ModelSelector::new()));
        components.insert(
            AppPanel::ConversationManager,
            Box::new(ConversationSelector::default()),
        );
        let config = Config::new()?;
        let mode = Mode::Input;
        let conversation_manager = ConversationManager::default();

        Ok(Self {
            tick_rate,
            frame_rate,
            components,
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_mode: mode,
            last_tick_key_events: Vec::new(),
            keymap,
            conversation,
            manager: conversation_manager,
            active_profile: profile.clone(),
        })
    }

    pub fn set_keymap(&mut self) {
        self.keymap = match self.mode {
            Mode::Input => " i: insert; v: focus viewer; j: scroll down; k: scroll up; m: change model; c: change convo; q: quit; ",
            Mode::ActiveInput => " enter: send message; ctrl+n: new line; esc: exit input mode; ",
            Mode::ActiveViewer => {
                " j: select next; k: select prev; c: copy; esc: exit scroll mode; "
            }
            Mode::ModelSelector => {
                " j: select next; k: select prev; enter: select model; m: close; "
            }
            Mode::ConversationManager => {
                " j: select next; k: select prev; n: new convo; enter: load convo; d: delete convo; esc: close panel; "
            }
        }
        .to_string();
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.last_mode = self.mode;
        self.mode = mode;
        self.set_keymap();
    }

    fn receive_message(&mut self, uuid: Uuid, message: Message) {
        self.conversation.add_message(uuid, message);
    }

    fn stream_message(&mut self, uuid: Uuid, message: Message) {
        self.conversation.replace_message(uuid, message);
    }

    fn update_title(&mut self, action_tx: Sender<Action>, first_message: String) {
        let model_config = ARCHER_CONFIG.default_title_model.clone();

        let system_prompt = "You are a helpful assistant, who title user queries.";
        let prompt = format!(
            "Given a message, from the user, please produce a short title for the message.

For example if the user asked:
What are the 3 hardest parts to learning rust.

You should respond with 'Hardest parts of Rust'

Another example is, if the user asked:
What is the most popular car color?

You should response with 'White'

Please do not respond with anything else except the title.

The users message is:

{}

Please provide a title for the user message above.
Please keep the answer succinct, less than ten words long.",
            first_message
        );

        let messages = vec![
            Message {
                role: MessageRole::System,
                content: system_prompt.to_string(),
                metadata: Some(MessageMetadata {
                    model_config: model_config.clone(),
                    status: CompletionStatus::Succeeded,
                }),
            },
            Message {
                role: MessageRole::User,
                content: prompt.to_string(),
                metadata: Some(MessageMetadata {
                    model_config: model_config.clone(),
                    status: CompletionStatus::Succeeded,
                }),
            },
        ];

        tokio::spawn(async move {
            if let Some(model) = get_model(&model_config).ok() {
                if let Some(Some(mut result)) = model
                    .get_completion(messages)
                    .await
                    .ok()
                    .map(|mut x| x.get_content().ok())
                {
                    result = result.trim_matches('"').trim_end_matches('"').to_string();

                    action_tx.send(Action::SetTitle(result)).await.ok();
                }
            }
        });
    }

    fn load_conversation(&mut self, conversation: Conversation) {
        self.conversation = conversation;
    }

    fn new_conversation(&mut self) {
        let convo = Conversation::new(self.active_profile.clone());
        self.conversation = convo;
    }

    fn send_message(&mut self, message: Message, profile: Profile, action_tx: Sender<Action>) {
        let first_message = self.conversation.has_no_user_messages();
        let provider = COMPLETION_PROVIDERS
            .get_provider(&message.clone().metadata.unwrap().model_config.provider_id)
            .unwrap();
        let model = provider
            .get_model(&message.clone().metadata.unwrap().model_config)
            .ok();
        let mut messages = self
            .conversation
            .messages
            .values()
            .map(|x| x.clone())
            .collect::<Vec<Message>>();

        let input_uuid = self.conversation.generate_message_id();
        let recv_uuid = self.conversation.generate_message_id();

        tokio::spawn(async move {
            action_tx
                .send(Action::ReceiveMessage(input_uuid, message.clone()))
                .await
                .ok();

            if first_message {
                action_tx
                    .send(Action::UpdateTitle(message.content.clone()))
                    .await
                    .ok();
            }

            if let Some(model) = model {
                let mut content_map = IndexMap::<String, String>::new();
                action_tx
                    .send(Action::ReceiveMessage(
                        recv_uuid,
                        Message {
                            role: MessageRole::Assistant,
                            content: "".to_string(),
                            metadata: Some(MessageMetadata {
                                model_config: message
                                    .metadata
                                    .as_ref()
                                    .unwrap()
                                    .model_config
                                    .clone(),
                                status: CompletionStatus::Starting,
                            }),
                        },
                    ))
                    .await
                    .ok();

                messages.push(message.clone());

                let completion_result = model.start_streaming(messages).await;

                match completion_result {
                    Ok(mut result) => 'outer: loop {
                        result.poll().await;
                        let status = result.get_status().await;
                        match status {
                            CompletionStatus::Starting => {
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            }
                            CompletionStatus::Failed | CompletionStatus::Canceled => {
                                let content = content_map
                                    .values()
                                    .into_iter()
                                    .map(|x| x.as_str())
                                    .collect::<Vec<&str>>()
                                    .join("");

                                action_tx
                                    .send(Action::StreamMessage(
                                        recv_uuid,
                                        Message {
                                            role: MessageRole::Assistant,
                                            content: content.clone(),
                                            metadata: message.clone().metadata,
                                        },
                                    ))
                                    .await
                                    .ok();
                            }
                            CompletionStatus::Succeeded | CompletionStatus::Processing => {
                                let stream = result.get_stream().await;
                                match stream {
                                    Ok(mut stream) => {
                                        while let Some((event, id, data)) = stream.next().await {
                                            if event == "done" {
                                                let content = content_map
                                                    .values()
                                                    .into_iter()
                                                    .map(|x| x.as_str())
                                                    .collect::<Vec<&str>>()
                                                    .join("");

                                                action_tx
                                                    .send(Action::StreamMessage(
                                                        recv_uuid,
                                                        Message {
                                                            role: MessageRole::Assistant,
                                                            content,
                                                            metadata: Some(MessageMetadata {
                                                                model_config: message
                                                                    .metadata
                                                                    .clone()
                                                                    .unwrap()
                                                                    .model_config,
                                                                status: CompletionStatus::Succeeded,
                                                            }),
                                                        },
                                                    ))
                                                    .await
                                                    .ok();

                                                action_tx.send(Action::SaveConversation).await.ok();
                                                break 'outer;
                                            }

                                            content_map.insert(id, data);
                                            let content = content_map
                                                .values()
                                                .into_iter()
                                                .map(|x| x.as_str())
                                                .collect::<Vec<&str>>()
                                                .join("");
                                            action_tx
                                                .send(Action::StreamMessage(
                                                    recv_uuid,
                                                    Message {
                                                        role: MessageRole::Assistant,
                                                        content,
                                                        metadata: Some(MessageMetadata {
                                                            model_config: message
                                                                .metadata
                                                                .as_ref()
                                                                .unwrap()
                                                                .model_config
                                                                .clone(),
                                                            status: CompletionStatus::Processing,
                                                        }),
                                                    },
                                                ))
                                                .await
                                                .ok();
                                        }
                                    }
                                    Err(err) => {
                                        action_tx
                                            .send(Action::StreamMessage(
                                                recv_uuid,
                                                Message {
                                                    role: MessageRole::Assistant,
                                                    content: err.to_string(),
                                                    metadata: message.clone().metadata,
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
                        todo!();
                    }
                }
            } else {
            }
        });
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let (action_tx, action_rx) = async_channel::unbounded();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        // tui.mouse(true);
        tui.enter()?;

        for component in self.components.values_mut() {
            component.register_action_handler(action_tx.clone())?;
        }

        for component in self.components.values_mut() {
            component.register_config_handler(self.config.clone())?;
        }

        for component in self.components.values_mut() {
            component.init(tui.size()?)?;
        }

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit).await?,
                    tui::Event::Tick => action_tx.send(Action::Tick).await?,
                    tui::Event::Render => action_tx.send(Action::Render).await?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y)).await?,
                    tui::Event::Key(key) => {
                        if let Some(keymap) = self.config.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                log::info!("Got action: {action:?}");
                                action_tx.send(action.clone()).await?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    log::info!("Got action: {action:?}");
                                    action_tx.send(action.clone()).await?;
                                }
                            }
                        };
                    }
                    _ => {}
                }
                for component in self.components.values_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action).await?;
                    }
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match action.clone() {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    }
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::NewConversation => self.new_conversation(),
                    Action::SendMessage(message, profile) => {
                        self.send_message(message, profile, action_tx.clone())
                    }
                    Action::ReceiveMessage(uuid, message) => self.receive_message(uuid, message),
                    Action::StreamMessage(uuid, message) => self.stream_message(uuid, message),
                    Action::SelectNextMessage => self.conversation.select_next_message(),
                    Action::SelectPreviousMessage => self.conversation.select_prev_message(),
                    Action::SetTitle(title) => {
                        self.conversation.title = Some(title);
                        action_tx.send(Action::SaveConversation).await.ok();
                        self.manager.update_conversation(self.conversation.clone());
                    }
                    Action::UpdateTitle(first_message) => {
                        self.update_title(action_tx.clone(), first_message)
                    }
                    Action::DeleteSelectedMessage => {
                        self.conversation.delete_selected_message();
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
                    Action::SaveConversation => {
                        self.conversation.save().ok();
                    }
                    Action::SelectNextConversation => {
                        self.manager.select_next_conversation();
                    }
                    Action::SelectPreviousConversation => {
                        self.manager.select_prev_conversation();
                    }
                    Action::LoadSelectedConversation => {
                        self.manager.activate_selected_conversation();
                        if let Some(convo) = self.manager.load_selected_conversation().ok() {
                            self.load_conversation(convo);
                        }
                    }
                    Action::DeleteSelectedConversation => {
                        if let Some(id) = self.manager.get_selected_uuid().ok() {
                            let file_path = self.manager.get_file_path(&id);
                            self.manager.remove_conversation(&id);
                            tokio::spawn(async move {
                                tokio::fs::remove_file(&file_path).await.ok();
                            });
                        }
                    }
                    Action::AddConversationToManager(convo) => {
                        self.manager.add_conversation(convo);
                    }
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        let conversation = &self.conversation;
                        let manager = &self.manager;
                        tui.draw(|f| {
                            let rect = f.size();

                            let mut layouts = HashMap::<AppPanel, Rect>::new();

                            // Generate a top/bottom split
                            let vertical_panels = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(vec![
                                    Constraint::Percentage(85),
                                    Constraint::Percentage(15),
                                ])
                                .split(rect);

                            // Input Panel is always visible
                            layouts.insert(AppPanel::Input, vertical_panels[1]);

                            // If ModelSelector or ConversationSelector is not the current mode
                            // the ViewerComponent makes up the entire top half
                            match self.mode {
                                Mode::Input | Mode::ActiveInput | Mode::ActiveViewer => {
                                    layouts.insert(AppPanel::Viewer, vertical_panels[0]);
                                }
                                _ => {
                                    let available_width = vertical_panels[0].width as f32;
                                    let min_width: f32 = 75.0;

                                    let panel_percentage: u16 =
                                        ((min_width / available_width).min(1.0).max(0.3) * 100.0)
                                            as u16;

                                    if panel_percentage == 100 {
                                        match self.mode {
                                            Mode::ModelSelector => {
                                                layouts.insert(
                                                    AppPanel::ModelSelector,
                                                    vertical_panels[0],
                                                );
                                            }
                                            Mode::ConversationManager => {
                                                layouts.insert(
                                                    AppPanel::ConversationManager,
                                                    vertical_panels[0],
                                                );
                                            }
                                            _ => {}
                                        }
                                    } else {
                                        let horizontal_panels = Layout::default()
                                            .direction(Direction::Horizontal)
                                            .constraints(vec![
                                                Constraint::Percentage(100 - panel_percentage),
                                                Constraint::Percentage(panel_percentage),
                                            ])
                                            .split(vertical_panels[0]);

                                        layouts.insert(AppPanel::Viewer, horizontal_panels[0]);
                                        match self.mode {
                                            Mode::ConversationManager => {
                                                layouts.insert(
                                                    AppPanel::ConversationManager,
                                                    horizontal_panels[1],
                                                );
                                            }
                                            Mode::ModelSelector => {
                                                layouts.insert(
                                                    AppPanel::ModelSelector,
                                                    horizontal_panels[1],
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            for (panel, layout_rect) in layouts.into_iter() {
                                if let Some(component) = self.components.get_mut(&panel) {
                                    let r = component.draw(f, layout_rect, conversation, manager);
                                    if let Err(e) = r {
                                        let action_tx = action_tx.clone();
                                        tokio::spawn(async move {
                                            action_tx
                                                .send(Action::Error(format!(
                                                    "Failed to draw: {:?}",
                                                    e
                                                )))
                                                .await
                                                .unwrap();
                                        });
                                    }
                                }
                            }
                        })?;
                    }
                    Action::Render => {
                        let conversation = &self.conversation;
                        let manager = &self.manager;
                        tui.draw(|f| {
                            let rect = f.size();

                            let mut layouts = HashMap::<AppPanel, Rect>::new();

                            // Generate a top/bottom split
                            let vertical_panels = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(vec![
                                    Constraint::Percentage(85),
                                    Constraint::Percentage(15),
                                ])
                                .split(rect);

                            // Input Panel is always visible
                            layouts.insert(AppPanel::Input, vertical_panels[1]);

                            // If ModelSelector or ConversationSelector is not the current mode
                            // the ViewerComponent makes up the entire top half
                            match self.mode {
                                Mode::Input | Mode::ActiveInput | Mode::ActiveViewer => {
                                    layouts.insert(AppPanel::Viewer, vertical_panels[0]);
                                }
                                _ => {
                                    let available_width = vertical_panels[0].width as f32;
                                    let min_width: f32 = 75.0;

                                    let panel_percentage: u16 =
                                        ((min_width / available_width).min(1.0).max(0.3) * 100.0)
                                            as u16;

                                    if panel_percentage == 100 {
                                        match self.mode {
                                            Mode::ModelSelector => {
                                                layouts.insert(
                                                    AppPanel::ModelSelector,
                                                    vertical_panels[0],
                                                );
                                            }
                                            Mode::ConversationManager => {
                                                layouts.insert(
                                                    AppPanel::ConversationManager,
                                                    vertical_panels[0],
                                                );
                                            }
                                            _ => {}
                                        }
                                    } else {
                                        let horizontal_panels = Layout::default()
                                            .direction(Direction::Horizontal)
                                            .constraints(vec![
                                                Constraint::Percentage(100 - panel_percentage),
                                                Constraint::Percentage(panel_percentage),
                                            ])
                                            .split(vertical_panels[0]);

                                        layouts.insert(AppPanel::Viewer, horizontal_panels[0]);
                                        match self.mode {
                                            Mode::ConversationManager => {
                                                layouts.insert(
                                                    AppPanel::ConversationManager,
                                                    horizontal_panels[1],
                                                );
                                            }
                                            Mode::ModelSelector => {
                                                layouts.insert(
                                                    AppPanel::ModelSelector,
                                                    horizontal_panels[1],
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            for (panel, layout_rect) in layouts.into_iter() {
                                if let Some(component) = self.components.get_mut(&panel) {
                                    let r = component.draw(f, layout_rect, conversation, manager);
                                    if let Err(e) = r {
                                        let action_tx = action_tx.clone();
                                        tokio::spawn(async move {
                                            action_tx
                                                .send(Action::Error(format!(
                                                    "Failed to draw: {:?}",
                                                    e
                                                )))
                                                .await
                                                .unwrap();
                                        });
                                    }
                                }
                            }
                        })?;
                    }
                    Action::RevertMode => {
                        action_tx
                            .send(Action::SwitchMode(self.last_mode))
                            .await
                            .ok();
                    }
                    Action::SwitchProfile(profile) => {
                        self.active_profile = profile.clone();
                        self.conversation.set_profile(profile);
                    }
                    Action::SwitchMode(mode) => {
                        self.set_mode(mode);
                        action_tx
                            .send(Action::SwitchKeymap(self.keymap.clone()))
                            .await?;
                    }
                    _ => {}
                }
                for component in self.components.values_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action).await?
                    };
                }
            }
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume).await?;
                tui = tui::Tui::new()?
                    .tick_rate(self.tick_rate)
                    .frame_rate(self.frame_rate);
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }
}
