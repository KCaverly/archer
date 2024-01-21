mod replicate;

use replicate::Replicate;

use super::completion::{CompletionProvider, CompletionProviderID};
use std::collections::BTreeMap;

pub struct CompletionProviderLibrary {
    providers: BTreeMap<CompletionProviderID, Box<dyn CompletionProvider>>,
}

impl CompletionProviderLibrary {
    pub fn get_provider(
        &self,
        provider_id: CompletionProviderID,
    ) -> Option<&Box<dyn CompletionProvider>> {
        self.providers.get(&provider_id)
    }

    pub fn default_provider(&self) -> Option<&Box<dyn CompletionProvider>> {
        self.providers.values().next()
    }
}

lazy_static! {
    pub static ref COMPLETION_PROVIDERS: CompletionProviderLibrary = {
        let mut providers = BTreeMap::<CompletionProviderID, Box<dyn CompletionProvider>>::new();
        providers.insert("replicate".to_string(), Box::new(Replicate::default()));
        CompletionProviderLibrary { providers }
    };
}
