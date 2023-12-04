use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    action::Action,
    agent::{
        completion::{get_completion, stream_completion, CompletionModel},
        message::{Message, Role},
    },
    components::{input::MessageInput, viewer::Viewer, Component},
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
    pub last_tick_key_events: Vec<KeyEvent>,
    pub messages: Vec<Message>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let viewer = Viewer::new(false);
        let input = MessageInput::new(true);
        let config = Config::new()?;
        let mode = Mode::Input;
        Ok(Self {
            tick_rate,
            frame_rate,
            components: vec![Box::new(viewer), Box::new(input)],
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
            messages: Vec::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = async_channel::unbounded();

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
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    let action_tx = action_tx.clone();
                                    tokio::spawn(async move {
                                        action_tx
                                            .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                            .await
                                            .unwrap();
                                    });
                                }
                            }
                        })?;
                    }
                    Action::Render => {
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    let action_tx = action_tx.clone();
                                    tokio::spawn(async move {
                                        action_tx
                                            .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                            .await
                                            .unwrap();
                                    });
                                }
                            }
                        })?;
                    }
                    Action::FocusViewer => {
                        self.mode = Mode::Viewer;
                    }
                    Action::FocusInput => {
                        self.mode = Mode::Input;
                    }
                    Action::ActivateInput => {
                        self.mode = Mode::ActiveInput;
                    }
                    Action::DeactivateInput => {
                        self.mode = Mode::Input;
                    }
                    Action::SendMessage(message) => {
                        // Lets clean this up at some point
                        // I don't think this cloning is ideal
                        let action_tx = action_tx.clone();
                        let mut messages = self.messages.clone();
                        tokio::spawn(async move {
                            action_tx
                                .send(Action::ReceiveMessage(message.clone()))
                                .await
                                .ok();

                            let mut content = String::new();
                            action_tx
                                .send(Action::ReceiveMessage(Message {
                                    role: Role::Assistant,
                                    content: content.clone(),
                                }))
                                .await
                                .ok();

                            messages.push(message);

                            let stream = stream_completion(CompletionModel::Yi34B, messages).await;
                            match stream {
                                Ok(mut stream) => {
                                    while let Some(event) = stream.next().await {
                                        match event {
                                            Ok(event) => {
                                                if event.event == "done" {
                                                    break;
                                                }
                                                content.push_str(event.data.as_str());
                                                action_tx
                                                    .send(Action::StreamMessage(Message {
                                                        role: Role::Assistant,
                                                        content: content.clone(),
                                                    }))
                                                    .await
                                                    .ok();
                                            }
                                            Err(err) => {
                                                panic!("{:?}", err);
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    panic!("{err}");
                                }
                            }
                        });
                    }
                    Action::ReceiveMessage(message) => {
                        self.messages.push(message);
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
