mod replicate;
mod together;

use anyhow::anyhow;
use replicate::Replicate;

use crate::ai::providers::together::TogetherAI;

use super::completion::{CompletionModel, CompletionProvider, CompletionProviderID};
use super::config::ModelConfig;
use std::collections::BTreeMap;

pub struct CompletionProviderLibrary {
    providers: BTreeMap<CompletionProviderID, Box<dyn CompletionProvider>>,
}

impl CompletionProviderLibrary {
    pub fn get_provider(&self, provider_id: &String) -> Option<&Box<dyn CompletionProvider>> {
        self.providers.get(provider_id)
    }

    pub fn next_provider(&self, provider_id: &CompletionProviderID) -> CompletionProviderID {
        let mut next = false;
        for id in self.providers.keys() {
            if next {
                return id.clone();
            }

            if id == provider_id {
                next = true;
            }
        }

        let first_provider = self.providers.keys().next().clone().unwrap();
        first_provider.clone()
    }
}

lazy_static! {
    pub static ref COMPLETION_PROVIDERS: CompletionProviderLibrary = {
        let mut providers = BTreeMap::<CompletionProviderID, Box<dyn CompletionProvider>>::new();
        providers.insert("TogetherAI".to_string(), Box::new(TogetherAI::load()));
        providers.insert("Replicate".to_string(), Box::new(Replicate::load()));

        CompletionProviderLibrary { providers }
    };
}

pub fn get_model(model_config: &ModelConfig) -> anyhow::Result<Box<dyn CompletionModel>> {
    if let Some(provider) = COMPLETION_PROVIDERS.get_provider(&model_config.provider_id) {
        return provider.get_model(model_config);
    }

    Err(anyhow!("model not found"))
}
