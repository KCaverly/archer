mod chatml;
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
    prompt: String,
    system_prompt: String,
    prompt_template: String,
    full_prompt: String,
}

#[derive(Debug)]
pub enum PromptTemplateVariant {
    ChatML,
}

impl PromptTemplateVariant {
    fn get_template(&self) -> Box<dyn PromptTemplate> {
        match self {
            PromptTemplateVariant::ChatML => Box::new(chatml::ChatMLTemplate::default()),
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
