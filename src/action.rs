use archer::ai::completion::Message as CompletionMessage;
use archer::ai::completion::{CompletionModelID, CompletionProviderID, CompletionStatus};
use std::fmt;
use uuid::Uuid;

use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};

use crate::{agent::conversation::Conversation, mode::Mode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Help,
    SendMessage(CompletionMessage),
    ReceiveMessage(Uuid, CompletionMessage),
    StreamMessage(Uuid, CompletionMessage),
    SelectNextMessage,
    SelectPreviousMessage,
    DeleteSelectedMessage,
    CopySelectedMessage,
    ToggleMaximized,
    RevertMode,
    SwitchMode(Mode),
    SelectNextModel,
    SelectPreviousModel,
    SwitchModel(CompletionProviderID, CompletionModelID),
    SwitchToSelectedModel,
    SwitchKeymap(String),
    SelectPreviousConversation,
    SelectNextConversation,
    LoadSelectedConversation,
    AddConversationToManager(Conversation),
    NewConversation,
    SaveConversation,
    DeleteSelectedConversation,
    SetTitle(String),
    UpdateTitle(String),
    ScrollUp,
    ScrollDown,
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ActionVisitor;

        impl<'de> Visitor<'de> for ActionVisitor {
            type Value = Action;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid string representation of Action")
            }

            fn visit_str<E>(self, value: &str) -> Result<Action, E>
            where
                E: de::Error,
            {
                match value {
                    "Tick" => Ok(Action::Tick),
                    "Render" => Ok(Action::Render),
                    "Suspend" => Ok(Action::Suspend),
                    "Resume" => Ok(Action::Resume),
                    "Quit" => Ok(Action::Quit),
                    "Refresh" => Ok(Action::Refresh),
                    "Help" => Ok(Action::Help),
                    "SelectPreviousMessage" => Ok(Action::SelectPreviousMessage),
                    "SelectNextMessage" => Ok(Action::SelectNextMessage),
                    "DeleteSelectedMessage" => Ok(Action::DeleteSelectedMessage),
                    "CopySelectedMessage" => Ok(Action::CopySelectedMessage),
                    "RevertMode" => Ok(Action::RevertMode),
                    "SelectPreviousModel" => Ok(Action::SelectPreviousModel),
                    "SelectNextModel" => Ok(Action::SelectNextModel),
                    "SwitchToSelectedModel" => Ok(Action::SwitchToSelectedModel),
                    "SelectPreviousConversation" => Ok(Action::SelectPreviousConversation),
                    "SelectNextConversation" => Ok(Action::SelectNextConversation),
                    "LoadSelectedConversation" => Ok(Action::LoadSelectedConversation),
                    "DeleteSelectedConversation" => Ok(Action::DeleteSelectedConversation),
                    "NewConversation" => Ok(Action::NewConversation),
                    "ScrollUp" => Ok(Action::ScrollUp),
                    "ScrollDown" => Ok(Action::ScrollDown),
                    data if data.starts_with("SwitchMode(") => {
                        let mode = data.trim_start_matches("SwitchMode(").trim_end_matches(")");
                        match mode {
                            "Input" => Ok(Action::SwitchMode(Mode::Input)),
                            "ActiveInput" => Ok(Action::SwitchMode(Mode::ActiveInput)),
                            "ActiveViewer" => Ok(Action::SwitchMode(Mode::ActiveViewer)),
                            "ModelSelector" => Ok(Action::SwitchMode(Mode::ModelSelector)),
                            "ConversationManager" => {
                                Ok(Action::SwitchMode(Mode::ConversationManager))
                            }
                            _ => Err(E::custom(format!("invalid Action Variant: {:?}", mode))),
                        }
                    }

                    data if data.starts_with("Error(") => {
                        let error_msg = data.trim_start_matches("Error(").trim_end_matches(")");
                        Ok(Action::Error(error_msg.to_string()))
                    }
                    data if data.starts_with("Resize(") => {
                        let parts: Vec<&str> = data
                            .trim_start_matches("Resize(")
                            .trim_end_matches(")")
                            .split(',')
                            .collect();
                        if parts.len() == 2 {
                            let width: u16 = parts[0].trim().parse().map_err(E::custom)?;
                            let height: u16 = parts[1].trim().parse().map_err(E::custom)?;
                            Ok(Action::Resize(width, height))
                        } else {
                            Err(E::custom(format!("Invalid Resize format: {}", value)))
                        }
                    }
                    _ => Err(E::custom(format!("Unknown Action variant: {}", value))),
                }
            }
        }

        deserializer.deserialize_str(ActionVisitor)
    }
}
