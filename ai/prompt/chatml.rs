use super::{PromptResult, PromptTemplate};
use crate::ai::completion::{Message, MessageRole};

#[derive(Default)]
pub struct ChatMLTemplate {}

impl PromptTemplate for ChatMLTemplate {
    fn generate_prompt(&self, messages: &Vec<Message>) -> PromptResult {
        let prompt_template = "<|im_start|>system\n{system_prompt}<|im_end|>\n{prompt}".to_string();
        let mut system_prompt = String::new();
        let mut prompt = String::new();
        for message in messages {
            match message.role {
                MessageRole::System => system_prompt.push_str(message.content.as_str()),
                MessageRole::User => {
                    prompt.push_str(
                        format!("<|im_start|>user\n{}<|im_end|>\n", message.content.as_str())
                            .as_str(),
                    );
                }
                MessageRole::Assistant => {
                    prompt.push_str(
                        format!("<|im_start|>assistant\n{}<|im_end|>\n", message.content).as_str(),
                    );
                }
            }
        }

        prompt.push_str("<|im_start|>assistant");

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
