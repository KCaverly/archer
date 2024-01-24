mod chatml;
mod mistral;
use super::completion::Message;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};
use std::fmt;

pub trait PromptTemplate {
    fn generate_prompt(&self, messages: &Vec<Message>) -> PromptResult;
}

pub struct PromptResult {
    pub prompt: String,
    pub system_prompt: String,
    pub prompt_template: String,
    pub full_prompt: String,
}

#[derive(Serialize, PartialEq, Eq, Debug, Clone)]
pub enum PromptTemplateVariant {
    ChatML,
    Mistral,
}

impl PromptTemplateVariant {
    pub fn get_template(&self) -> Box<dyn PromptTemplate> {
        match self {
            PromptTemplateVariant::ChatML => Box::new(chatml::ChatMLTemplate::default()),
            PromptTemplateVariant::Mistral => Box::new(mistral::MistralTemplate::default()),
        }
    }
}

impl<'de> Deserialize<'de> for PromptTemplateVariant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ActionVisitor;

        impl<'de> Visitor<'de> for ActionVisitor {
            type Value = PromptTemplateVariant;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid string representation of Action")
            }

            fn visit_str<E>(self, value: &str) -> Result<PromptTemplateVariant, E>
            where
                E: de::Error,
            {
                match value {
                    "ChatML" => Ok(PromptTemplateVariant::ChatML),
                    "Mistral" => Ok(PromptTemplateVariant::Mistral),
                    _ => Err(E::custom(format!(
                        "Unknown Prompt Template variant: {}",
                        value
                    ))),
                }
            }
        }

        deserializer.deserialize_str(ActionVisitor)
    }
}
