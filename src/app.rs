use arboard::{Clipboard, LinuxClipboardKind, SetExtLinux};
use std::sync::Arc;

use async_channel::Sender;
use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use indexmap::IndexMap;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use replicate_rs::predictions::PredictionStatus;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::{
    action::Action,
    agent::{
        completion::{create_prediction, get_completion, CompletionModel},
        conversation::{Conversation, ConversationManager},
        message::{Message, Role},
    },
    components::{
        conversation_selector::ConversationSelector, input::MessageInput,
        model_selector::ModelSelector, viewer::Viewer, Component,
    },
    config::Config,
    mode::Mode,
    tui::{self, Frame, Tui},
};

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub components: Vec<Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub keymap: String,
    pub conversation: Conversation,
    pub manager: ConversationManager,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let conversation = Conversation::new();
        let keymap =
            " i: insert; k: focus viewer; m: change model; c: change convo; q: quit; ".to_string();
        let viewer = Viewer::new(false);
        let input = MessageInput::new(true, keymap.clone());
        let config = Config::new()?;
        let mode = Mode::Input;
        let model_selector = ModelSelector::new();
        let conversation_selector = ConversationSelector::default();
        let conversation_manager = ConversationManager::default();
        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![
                Box::new(viewer),
                Box::new(input),
                Box::new(model_selector),
                Box::new(conversation_selector),
            ],
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_mode: mode,
            last_tick_key_events: Vec::new(),
            keymap,
            conversation,
            manager: conversation_manager,
        })
    }

    pub fn set_keymap(&mut self) {
        self.keymap = match self.mode {
            Mode::Input => " i: insert; k: focus viewer; m: change model; c: change convo; q: quit; ",
            Mode::Viewer => " i: insert; j: focus input; m: change model; c: change convo; q; quit; ",
            Mode::ActiveInput => " enter: send message; esc: exit input mode; ",
            Mode::ActiveViewer => {
                " j: select next; k: select prev; c: copy; f: maximize; esc: exit scroll mode; "
            }
            Mode::ModelSelector => {
                " j: select next; k: select prev; enter: select model; m: close; "
            }
            Mode::MessageViewer => " j: scroll down; k: scroll up; esc: see all messages; ",
            Mode::ConversationManager => {
                " j: select next; k: select prev; n: new convo; enter: load convo; esc: close panel; "
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
        let title_model = CompletionModel::Mistral7bInstructV01;
        let system_prompt = "You are a helpful assistant, who title user queries.";
        let prompt = format!("Given a message, from the user, you are required to produce a short title for the message.

An example is as follows:
User: Please walk me through the 3 hardest parts to learning rust
Answer: Hard parts of learning rust

Please only provide the title and nothing else, keep the answer succinct, under 10 words preferably.

The message to title is as follows:
User: {}
", first_message);

        let messages = vec![
            Message::system_message(system_prompt.to_string(), Some(title_model)),
            Message::user_message(prompt.to_string(), Some(title_model)),
        ];

        tokio::spawn(async move {
            if let Some(result) = get_completion(title_model, messages).await.ok() {
                action_tx.send(Action::SetTitle(result.content)).await.ok();
            }
        });
    }

    fn load_conversation(&mut self, conversation: Conversation) {
        self.conversation = conversation;
    }

    fn new_conversation(&mut self) {
        let convo = Conversation::new();
        self.conversation = convo;
    }

    fn send_message(&mut self, message: Message, action_tx: Sender<Action>) {
        let first_message = self.conversation.messages.len() == 0;
        let model = message.model;
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
                            role: Role::Assistant,
                            content: "".to_string(),
                            status: Some(PredictionStatus::Starting),
                            model: Some(model.clone()),
                        },
                    ))
                    .await
                    .ok();

                messages.push(message);

                let prediction = create_prediction(&model, messages).await;
                match prediction {
                    Ok(mut prediction) => 'outer: loop {
                        prediction.reload().await.ok();
                        let status = prediction.get_status().await;
                        match status {
                            PredictionStatus::Starting => {
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            }
                            PredictionStatus::Failed | PredictionStatus::Canceled => {
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
                                            role: Role::Assistant,
                                            content: content.clone(),
                                            status: Some(status),
                                            model: Some(model.clone()),
                                        },
                                    ))
                                    .await
                                    .ok();
                            }
                            PredictionStatus::Succeeded | PredictionStatus::Processing => {
                                let stream = prediction.get_stream().await;
                                match stream {
                                    Ok(mut stream) => {
                                        while let Some(event) = stream.next().await {
                                            match event {
                                                Ok(event) => {
                                                    if event.event == "done" {
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
                                                                    role: Role::Assistant,
                                                                    content,
                                                                    status: Some(
                                                                        PredictionStatus::Succeeded,
                                                                    ),
                                                                    model: Some(model.clone()),
                                                                },
                                                            ))
                                                            .await
                                                            .ok();

                                                        action_tx
                                                            .send(Action::SaveConversation)
                                                            .await
                                                            .ok();
                                                        break 'outer;
                                                    }

                                                    content_map.insert(event.id, event.data);
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
                                                                role: Role::Assistant,
                                                                content,
                                                                status: Some(
                                                                    PredictionStatus::Processing,
                                                                ),
                                                                model: Some(model.clone()),
                                                            },
                                                        ))
                                                        .await
                                                        .ok();
                                                }
                                                Err(err) => {
                                                    action_tx
                                                        .send(Action::StreamMessage(
                                                            recv_uuid,
                                                            Message {
                                                                role: Role::Assistant,
                                                                content: err.to_string(),
                                                                status: Some(
                                                                    PredictionStatus::Failed,
                                                                ),
                                                                model: Some(model.clone()),
                                                            },
                                                        ))
                                                        .await
                                                        .ok();
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    },
                    Err(err) => {
                        todo!();
                    }
                }
            }
        });
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, action_rx) = async_channel::unbounded();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        // tui.mouse(true);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }

        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }

        for component in self.components.iter_mut() {
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
                for component in self.components.iter_mut() {
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
                    Action::SendMessage(message) => self.send_message(message, action_tx.clone()),
                    Action::ReceiveMessage(uuid, message) => self.receive_message(uuid, message),
                    Action::StreamMessage(uuid, message) => self.stream_message(uuid, message),
                    Action::FocusConversation => self.conversation.focus(),
                    Action::UnfocusConversation => self.conversation.unfocus(),
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
                    Action::AddConversationToManager(convo) => {
                        self.manager.add_conversation(convo);
                    }
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        let conversation = &self.conversation;
                        let manager = &self.manager;
                        tui.draw(|f| {
                            let rect = f.size();
                            let mut viewer_layout: Rect;
                            let input_layout: Rect;
                            let mut selector_layout: Option<Rect> = None;

                            let layout1 = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(vec![
                                    Constraint::Percentage(85),
                                    Constraint::Percentage(15),
                                ])
                                .split(rect);

                            viewer_layout = layout1[0];
                            input_layout = layout1[1];

                            if self.mode == Mode::ModelSelector
                                || self.mode == Mode::ConversationManager
                            {
                                let layout2 = Layout::default()
                                    .direction(Direction::Horizontal)
                                    .constraints(vec![
                                        Constraint::Percentage(70),
                                        Constraint::Percentage(3),
                                    ])
                                    .split(viewer_layout);
                                viewer_layout = layout2[0];
                                selector_layout = Some(layout2[1]);
                            }

                            let r =
                                self.components[0].draw(f, viewer_layout, conversation, manager);
                            if let Err(e) = r {
                                let action_tx = action_tx.clone();
                                tokio::spawn(async move {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .await
                                        .unwrap();
                                });
                            }

                            let r = self.components[1].draw(f, input_layout, conversation, manager);
                            if let Err(e) = r {
                                let action_tx = action_tx.clone();
                                tokio::spawn(async move {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .await
                                        .unwrap();
                                });
                            }

                            if let Some(selector_layout) = selector_layout {
                                match self.mode {
                                    Mode::ConversationManager => {
                                        let r = self.components[3].draw(
                                            f,
                                            selector_layout,
                                            conversation,
                                            manager,
                                        );
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
                                    Mode::ModelSelector => {
                                        let r = self.components[2].draw(
                                            f,
                                            selector_layout,
                                            conversation,
                                            manager,
                                        );
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
                                    _ => {}
                                }
                            }
                        })?;
                    }
                    Action::Render => {
                        let conversation = &self.conversation;
                        let manager = &self.manager;
                        tui.draw(|f| {
                            let rect = f.size();

                            let mut viewer_layout: Rect;
                            let input_layout: Rect;
                            let mut selector_layout: Option<Rect> = None;

                            let layout1 = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(vec![
                                    Constraint::Percentage(85),
                                    Constraint::Percentage(15),
                                ])
                                .split(rect);

                            viewer_layout = layout1[0];
                            input_layout = layout1[1];
                            if self.mode == Mode::ModelSelector
                                || self.mode == Mode::ConversationManager
                            {
                                let layout2 = Layout::default()
                                    .direction(Direction::Horizontal)
                                    .constraints(vec![
                                        Constraint::Percentage(70),
                                        Constraint::Percentage(30),
                                    ])
                                    .split(viewer_layout);
                                viewer_layout = layout2[0];
                                selector_layout = Some(layout2[1]);
                            }

                            let r =
                                self.components[0].draw(f, viewer_layout, conversation, manager);
                            if let Err(e) = r {
                                let action_tx = action_tx.clone();
                                tokio::spawn(async move {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .await
                                        .unwrap();
                                });
                            }

                            let r = self.components[1].draw(f, input_layout, conversation, manager);
                            if let Err(e) = r {
                                let action_tx = action_tx.clone();
                                tokio::spawn(async move {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .await
                                        .unwrap();
                                });
                            }

                            if let Some(selector_layout) = selector_layout {
                                match self.mode {
                                    Mode::ConversationManager => {
                                        let r = self.components[3].draw(
                                            f,
                                            selector_layout,
                                            conversation,
                                            manager,
                                        );
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
                                    Mode::ModelSelector => {
                                        let r = self.components[2].draw(
                                            f,
                                            selector_layout,
                                            conversation,
                                            manager,
                                        );
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
                                    _ => {}
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
                    Action::SwitchMode(mode) => {
                        self.set_mode(mode);
                        action_tx
                            .send(Action::SwitchKeymap(self.keymap.clone()))
                            .await?;
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
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
