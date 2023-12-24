use replicate_rs::predictions::PredictionStatus;
use serde::{Deserialize, Serialize};

use super::completion::CompletionModel;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub status: Option<PredictionStatus>,
    pub model: Option<CompletionModel>,
}

impl Message {
    pub fn user_message(content: String, model: Option<CompletionModel>) -> Self {
        Message {
            role: Role::User,
            content,
            model,
            status: None,
        }
    }

    pub fn system_message(content: String, model: Option<CompletionModel>) -> Self {
        Message {
            role: Role::System,
            content,
            model,
            status: None,
        }
    }
}
