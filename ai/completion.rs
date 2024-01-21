use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use bytes::Bytes;
use eventsource_stream::{EventStream, Eventsource};
use futures_lite::StreamExt;

#[derive(Serialize, Clone, PartialEq, Eq, Debug, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub metadata: MessageMetadata,
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug, Deserialize)]
pub struct MessageMetadata {
    pub provider_id: CompletionProviderID,
    pub model_id: CompletionModelID,
    pub status: CompletionStatus,
}

pub type CompletionModelID = String;
pub type CompletionProviderID = String;

#[async_trait]
pub trait CompletionModel: Sync + Send {
    fn get_display_name(&self) -> String;
    async fn start_streaming(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>>;
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
    fn get_id(&self) -> String;
}

#[async_trait]
pub trait CompletionResult: Send + Sync {
    async fn poll(&mut self);
    async fn get_status(&mut self) -> CompletionStatus;
    async fn get_stream<'a>(
        &'a mut self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = (String, String, String)> + Send + Sync + 'a>>>;
    fn get_content(&mut self) -> anyhow::Result<String>;
}

#[derive(Deserialize, Clone, Eq, PartialEq, Debug, Serialize)]
pub enum CompletionStatus {
    Starting,
    Processing,
    Failed,
    Canceled,
    Succeeded,
}
