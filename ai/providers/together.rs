use crate::ai::completion::{
    CompletionModel, CompletionModelID, CompletionProvider, CompletionResult, Message,
};
use async_trait::async_trait;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Default)]
pub struct TogetherAI {
    api_key: Option<String>,
}

impl CompletionProvider for TogetherAI {
    fn load() -> Self {
        TogetherAI { api_key: None }
    }
    fn has_credentials(&self) -> bool {
        false
    }

    fn list_models(&self) -> Vec<Box<dyn CompletionModel>> {
        let mut models = Vec::<Box<dyn CompletionModel>>::new();
        for model in TogetherCompletionModel::iter() {
            let boxed_model = Box::new(model);
            models.push(boxed_model)
        }

        models
    }

    fn default_model(&self) -> Box<dyn CompletionModel> {
        Box::new(TogetherCompletionModel::ModelA)
    }
    fn get_model(&self, model_id: CompletionModelID) -> Option<Box<dyn CompletionModel>> {
        return None;
    }
}

#[derive(Default, EnumIter, Clone)]
enum TogetherCompletionModel {
    #[default]
    ModelA,
    ModelB,
}

#[async_trait]
impl CompletionModel for TogetherCompletionModel {
    async fn get_completion(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        todo!();
    }

    async fn start_streaming(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        todo!();
    }

    fn get_display_name(&self) -> String {
        match self {
            TogetherCompletionModel::ModelA => "ModelA".to_string(),
            TogetherCompletionModel::ModelB => "ModelB".to_string(),
        }
    }
}
