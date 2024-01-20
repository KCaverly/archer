use async_trait::async_trait;

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Clone)]
pub(crate) struct Message {
    pub(crate) role: MessageRole,
    pub(crate) content: String,
}

#[async_trait]
pub(crate) trait CompletionModel {
    fn get_display_name(&self) -> String;
    async fn get_completion(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>>;
}

pub(crate) trait CompletionProvider {
    fn load() -> Self
    where
        Self: Sized;
    fn has_credentials(&self) -> bool;
    fn list_models(&self) -> Vec<Box<dyn CompletionModel>>;
}

pub(crate) trait CompletionResult {
    fn poll(&self);
    fn get_status(&self) -> CompletionStatus;
    fn get_message(&self) -> Option<Message>;
}

pub(crate) enum CompletionStatus {
    Starting,
    Processing,
    Failed,
    Canceled,
    Succeeded,
}
