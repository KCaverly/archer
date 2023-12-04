use replicate_rs::predictions::PredictionStatus;
use serde::Serialize;

use super::completion::CompletionModel;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub status: Option<PredictionStatus>,
    pub model: Option<CompletionModel>,
}
