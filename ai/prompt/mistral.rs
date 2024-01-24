use super::{PromptResult, PromptTemplate};
use crate::ai::completion::{Message, MessageRole};

#[derive(Default)]
pub struct MistralTemplate {}

impl PromptTemplate for MistralTemplate {
    fn generate_prompt(&self, messages: &Vec<Message>) -> PromptResult {
        let prompt_template = "{prompt}".to_string();
        let mut system_prompt = String::new();
        let mut prompt = "<s>".to_string();
        for message in messages {
            match message.role {
                MessageRole::System => system_prompt.push_str(message.content.as_str()),
                MessageRole::User => {
                    prompt
                        .push_str(format!("[INST] {} [/INST]", message.content.as_str()).as_str());
                }
                MessageRole::Assistant => {
                    prompt.push_str(format!("{}</s>", message.content).as_str());
                }
            }
        }

        let full_prompt = prompt_template.replace("{prompt}", prompt.as_str());

        PromptResult {
            prompt,
            system_prompt,
            prompt_template,
            full_prompt,
        }
    }
}
