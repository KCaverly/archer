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
        conversation::Conversation,
        message::{Message, Role},
    },
    components::{
        conversation_manager::ConversationSelector, input::MessageInput,
        model_selector::ModelSelector, viewer::Viewer, Component,
    },
    config::Config,
    mode::Mode,
    tui,
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
        let conversation_manager = ConversationSelector::default();
        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![
                Box::new(viewer),
                Box::new(input),
                Box::new(model_selector),
                Box::new(conversation_manager),
            ],
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_mode: mode,
            last_tick_key_events: Vec::new(),
            keymap,
            conversation,
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

    fn send_message(&mut self, message: Message, action_tx: Sender<Action>) {
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
                                                            .send(Action::SaveActiveConversation)
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
                    Action::SendMessage(message) => self.send_message(message, action_tx.clone()),
                    Action::ReceiveMessage(uuid, message) => self.receive_message(uuid, message),
                    Action::StreamMessage(uuid, message) => self.stream_message(uuid, message),
                    Action::Resize(w, h) => {
                        // This isnt painting with the same render layout as below, so we should
                        // reconcile this at some point.
                        // todo!();
                        // tui.resize(Rect::new(0, 0, w, h))?;
                        // tui.draw(|f| {
                        //     for component in self.components.iter_mut() {
                        //         let r = component.draw(f, f.size());
                        //         if let Err(e) = r {
                        //             let action_tx = action_tx.clone();
                        //             tokio::spawn(async move {
                        //                 action_tx
                        //                     .send(Action::Error(format!("Failed to draw: {:?}", e)))
                        //                     .await
                        //                     .unwrap();
                        //             });
                        //         }
                        //     }
                        // })?;
                    }
                    Action::Render => {
                        let conversation = &self.conversation;
                        tui.draw(|f| {
                            let rect = f.size();

                            let mut viewer_layout: Rect;
                            let input_layout: Rect;
                            let mut selector_layout: Option<Rect> = None;

                            let layout1 = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(vec![
                                    Constraint::Percentage(90),
                                    Constraint::Percentage(10),
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

                            let r = self.components[0].draw(f, viewer_layout, conversation);
                            if let Err(e) = r {
                                let action_tx = action_tx.clone();
                                tokio::spawn(async move {
                                    action_tx
                                        .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                        .await
                                        .unwrap();
                                });
                            }

                            let r = self.components[1].draw(f, input_layout, conversation);
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
