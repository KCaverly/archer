use std::fmt;

use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};

use crate::agent::{completion::CompletionModel, message::Message};

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
    FocusViewer,
    FocusInput,
    ActivateInput,
    DeactivateInput,
    SendMessage(Message),
    ReceiveMessage(Message),
    StreamMessage(Message),
    ActivateViewer,
    DeactivateViewer,
    SelectNextMessage,
    SelectPreviousMessage,
    DeleteSelectedMessage,
    ToggleModelSelector,
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
                    "FocusViewer" => Ok(Action::FocusViewer),
                    "FocusInput" => Ok(Action::FocusInput),
                    "ActivateInput" => Ok(Action::ActivateInput),
                    "DeactivateInput" => Ok(Action::DeactivateInput),
                    "ActivateViewer" => Ok(Action::ActivateViewer),
                    "DeactivateViewer" => Ok(Action::DeactivateViewer),
                    "SelectPreviousMessage" => Ok(Action::SelectPreviousMessage),
                    "SelectNextMessage" => Ok(Action::SelectNextMessage),
                    "DeleteSelectedMessage" => Ok(Action::DeleteSelectedMessage),
                    "ToggleModelSelector" => Ok(Action::ToggleModelSelector),

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