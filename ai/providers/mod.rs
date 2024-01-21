mod replicate;
mod together;

use replicate::Replicate;

use crate::ai::providers::together::TogetherAI;

use super::completion::{CompletionProvider, CompletionProviderID};
use std::collections::BTreeMap;

pub const DEFAULT_COMPLETION_PROVIDER: &str = "TogetherAI";

pub struct CompletionProviderLibrary {
    providers: BTreeMap<CompletionProviderID, Box<dyn CompletionProvider>>,
}

impl CompletionProviderLibrary {
    pub fn get_provider(
        &self,
        provider_id: &CompletionProviderID,
    ) -> Option<&Box<dyn CompletionProvider>> {
        self.providers.get(provider_id)
    }

    pub fn default_provider(&self) -> Option<&Box<dyn CompletionProvider>> {
        self.providers.get(&DEFAULT_COMPLETION_PROVIDER.to_string())
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
        providers.insert("TogetherAI".to_string(), Box::new(TogetherAI::default()));
        providers.insert("Replicate".to_string(), Box::new(Replicate::default()));

        CompletionProviderLibrary { providers }
    };
}
