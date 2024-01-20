mod replicate;
use replicate::Replicate;

use super::completion::CompletionProvider;

enum CompletionProviders {
    REPLICATE,
}

impl CompletionProviders {
    fn get_provider(&self) -> Box<dyn CompletionProvider> {
        match self {
            CompletionProviders::REPLICATE => Box::new(Replicate::default()),
        }
    }
}
