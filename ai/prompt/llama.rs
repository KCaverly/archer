use super::{PromptResult, PromptTemplate};
use crate::ai::completion::{Message, MessageRole};

#[derive(Default)]
pub struct LlamaTemplate {}

impl PromptTemplate for LlamaTemplate {
    fn generate_prompt(&self, messages: &Vec<Message>) -> PromptResult {
        let mut first_message = true;
        let prompt_template =
            "<s>[INST] <<SYS>>\n{system_prompt}\n<</SYS>>\n\n{prompt}".to_string();
        let mut system_prompt = String::new();
        let mut prompt = "<s>".to_string();
        for message in messages {
            match message.role {
                MessageRole::System => system_prompt.push_str(message.content.as_str()),
                MessageRole::User => {
                    if !first_message {
                        prompt.push_str(format!("<s>").as_str());
                    }
                    prompt
                        .push_str(format!("[INST] {} [/INST]", message.content.as_str()).as_str());
                    first_message = false;
                }
                MessageRole::Assistant => {
                    prompt.push_str(format!("{}</s>", message.content).as_str());
                }
            }
        }

        let full_prompt = prompt_template
            .replace("{system_prompt}", system_prompt.as_str())
            .replace("{prompt}", prompt.as_str());

        PromptResult {
            prompt,
            system_prompt,
            prompt_template,
            full_prompt,
        }
    }
}
