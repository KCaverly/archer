use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::Serialize;

use bytes::Bytes;
use eventsource_stream::{EventStream, Eventsource};
use futures_lite::StreamExt;

#[derive(Clone, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

pub type CompletionModelID = String;
pub type CompletionProviderID = String;

#[async_trait]
pub trait CompletionModel: Sync + Send {
    fn get_display_name(&self) -> String;
    async fn get_completion(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>>;
}

pub trait CompletionProvider: Sync {
    fn load() -> Self
    where
        Self: Sized;
    fn has_credentials(&self) -> bool;
    fn list_models(&self) -> Vec<Box<dyn CompletionModel>>;
    fn default_model(&self) -> Box<dyn CompletionModel>;
    fn get_model(&self, model_id: CompletionModelID) -> Option<Box<dyn CompletionModel>>;
}

#[async_trait]
pub trait CompletionResult: Send + Sync {
    async fn poll(&mut self);
    async fn get_status(&mut self) -> CompletionStatus;
    async fn get_stream(
        &mut self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = (String, String, String)> + Send>>>;
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub enum CompletionStatus {
    Starting,
    Processing,
    Failed,
    Canceled,
    Succeeded,
}
