use crate::ai::completion::{
    CompletionModel, CompletionModelID, CompletionProvider, CompletionProviderID, CompletionResult,
    CompletionStatus, Message, MessageRole,
};
use async_stream::stream;
use async_trait::async_trait;
use bytes::Bytes;
use eventsource_stream::{EventStream, Eventsource};
use futures::{pin_mut, Stream, StreamExt};
use replicate_rs::config::ReplicateConfig;
use replicate_rs::predictions::{Prediction, PredictionClient, PredictionStatus};
use serde_json::json;
use std::default;
use std::env::var;
use std::pin::Pin;
use std::task::Context;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Default)]
pub struct Replicate {
    api_key: Option<String>,
}

impl CompletionProvider for Replicate {
    fn load() -> Self {
        if let Some(api_key) = var("REPLICATE_API_KEY").ok() {
            Replicate {
                api_key: Some(api_key),
            }
        } else {
            Replicate { api_key: None }
        }
    }
    fn has_credentials(&self) -> bool {
        self.api_key.is_some()
    }

    fn list_models(&self) -> Vec<Box<dyn CompletionModel>> {
        todo!();
    }

    fn default_model(&self) -> Box<dyn CompletionModel> {
        Box::new(ReplicateCompletionModel::default())
    }
    fn get_model(&self, model_id: CompletionModelID) -> Option<Box<dyn CompletionModel>> {
        for model in ReplicateCompletionModel::iter() {
            if model.get_display_name() == model_id {
                return Some(Box::new(model));
            }
        }

        return None;
    }
}

#[derive(Default, EnumIter, Clone)]
enum ReplicateCompletionModel {
    NousHermes2Yi34b,
    Dolphin2_6Mixtral8x7b,
    Dolphin2_5Mixtral8x7b,
    Yi34bChat,
    #[default]
    Llama2_13bChat,
    Llama2_70bChat,
    Llama2_7bChat,
    Mistral7bInstructV01,
    Codellama34bInstruct,
    DeepseekCoder6_7bInstruct,
    DeepseekCoder33bInstructGGUF,
}

impl ReplicateCompletionModel {
    fn get_model_details(&self) -> (String, String) {
        match self {
            ReplicateCompletionModel::NousHermes2Yi34b => (
                "kcaverly".to_string(),
                "nous-hermes-2-yi-34b-gguf".to_string(),
            ),
            ReplicateCompletionModel::Dolphin2_5Mixtral8x7b => (
                "kcaverly".to_string(),
                "dolphin-2.5-mixtral-8x7b-gguf".to_string(),
            ),
            ReplicateCompletionModel::Dolphin2_6Mixtral8x7b => (
                "kcaverly".to_string(),
                "dolphin-2.6-mixtral-8x7b-gguf".to_string(),
            ),
            ReplicateCompletionModel::Yi34bChat => ("01-ai".to_string(), "yi-34b-chat".to_string()),
            ReplicateCompletionModel::Llama2_70bChat => {
                ("meta".to_string(), "llama-2-70b-chat".to_string())
            }
            ReplicateCompletionModel::Llama2_13bChat => {
                ("meta".to_string(), "llama-2-13b-chat".to_string())
            }
            ReplicateCompletionModel::Llama2_7bChat => {
                ("meta".to_string(), "llama-2-7b-chat".to_string())
            }
            ReplicateCompletionModel::Mistral7bInstructV01 => (
                "mistralai".to_string(),
                "mistral-7b-instruct-v0.1".to_string(),
            ),
            ReplicateCompletionModel::Codellama34bInstruct => {
                ("meta".to_string(), "codellama-34b-instruct".to_string())
            }
            ReplicateCompletionModel::DeepseekCoder6_7bInstruct => (
                "kcaverly".to_string(),
                "deepseek-code-6.7b-instruct".to_string(),
            ),
            ReplicateCompletionModel::DeepseekCoder33bInstructGGUF => (
                "kcaverly".to_string(),
                "deepseek-coder-33b-instruct-gguf".to_string(),
            ),
            _ => {
                todo!()
            }
        }
    }

    fn get_inputs(&self, messages: &Vec<Message>) -> serde_json::Value {
        match self {
            ReplicateCompletionModel::Dolphin2_5Mixtral8x7b
            | ReplicateCompletionModel::Dolphin2_6Mixtral8x7b
            | ReplicateCompletionModel::NousHermes2Yi34b => {
                let mut system_prompt = "You are a helpful AI assistant, running in Archer a Terminal chat Interface built by kcaverly.".to_string();
                let mut prompt = String::new();

                for message in messages {
                    match message.role {
                        MessageRole::System => {
                            system_prompt.push_str(message.content.as_str());
                        }
                        MessageRole::User => {
                            prompt.push_str(
                                format!(
                                    "<|im_start|>user\n{}<|im_end|>\n",
                                    message.content.as_str()
                                )
                                .as_str(),
                            );
                        }
                        MessageRole::Assistant => {
                            prompt.push_str(
                                format!("<|im_start|>assistant\n{}<|im_end|>\n", message.content)
                                    .as_str(),
                            );
                        }
                    }
                }

                prompt.push_str("<|im_start|>assistant");

                json!({"prompt": prompt, "system_prompt": system_prompt, "prompt_template": "<|im_start|>system\n{system_prompt}<|im_end|>\n{prompt}"})
            }
            ReplicateCompletionModel::Yi34bChat => {
                let mut prompt = String::new();
                for message in messages {
                    let content = &message.content;
                    let role = match message.role {
                        MessageRole::System => "system",
                        MessageRole::Assistant => "assistant",
                        MessageRole::User => "user",
                    };

                    prompt.push_str(format!("\n<im_start|>{role}\n{content}<|im_end|>").as_str());
                }

                prompt.push_str("<|im_start|>assistant");

                json!({"prompt": prompt, "prompt_template": "{prompt}"})
            }
            ReplicateCompletionModel::DeepseekCoder6_7bInstruct => {
                let mut message_objects = Vec::new();
                for message in messages {
                    let role = match message.role {
                        MessageRole::System => "system",
                        MessageRole::Assistant => "assistant",
                        MessageRole::User => "user",
                    };
                    message_objects.push(json!({"role": role, "content": message.content}));
                }

                let message_str = serde_json::to_string(&message_objects).unwrap();

                json!({"messages": message_str})
            }
            ReplicateCompletionModel::DeepseekCoder33bInstructGGUF => {
                let mut prompt = String::new();
                let mut last_role = MessageRole::System;
                for message in messages {
                    if message.role == last_role {
                        prompt.push_str(format!("\n{}", message.content).as_str());
                    } else {
                        match message.role {
                            MessageRole::User => {
                                prompt.push_str(
                                    format!("\n### Instruction: {}", message.content).as_str(),
                                );
                            }
                            MessageRole::Assistant => prompt
                                .push_str(format!("\n### Response: {}", message.content).as_str()),
                            _ => {}
                        }
                    }
                    last_role = message.role.clone();
                }

                if last_role != MessageRole::Assistant {
                    prompt.push_str("\n### Response: ");
                }

                json!({"prompt": prompt, "prompt_template": "{system_prompt}{prompt}"})
            }
            ReplicateCompletionModel::Mistral7bInstructV01 => {
                let mut prompt = "<s>".to_string();
                for message in messages {
                    let content = &message.content;
                    match message.role {
                        MessageRole::User => {
                            prompt.push_str(format!("[INST] {content} [/INST]").as_str());
                        }
                        MessageRole::Assistant => {
                            prompt.push_str(format!(" {content} </s>").as_str())
                        }
                        MessageRole::System => {}
                    }
                }

                json!({"prompt": prompt})
            }
            ReplicateCompletionModel::Llama2_13bChat
            | ReplicateCompletionModel::Llama2_70bChat
            | ReplicateCompletionModel::Llama2_7bChat
            | ReplicateCompletionModel::Codellama34bInstruct => {
                let mut system_prompt = String::new();
                let mut prompt = String::new();

                for message in messages {
                    let content = &message.content;
                    match message.role {
                        MessageRole::System => {
                            system_prompt.push_str(format!("{content}\n").as_str());
                        }
                        MessageRole::User => {
                            prompt.push_str(format!("{content} [/INST] ").as_str());
                        }
                        MessageRole::Assistant => {
                            prompt.push_str(format!("{content}</s><s>[INST] ").as_str());
                        }
                    }
                }

                json!({"prompt": prompt, "system_prompt": system_prompt, "prompt_template": "[INST] <<SYS>>\n{{system_prompt}}\n<</SYS>>\n\n{{prompt}}", "max_new_tokens": 4000 })
            }
        }
    }
}

#[derive(Debug)]
struct ReplicateCompletionResult {
    prediction: Prediction,
    provider_id: CompletionProviderID,
    model_id: CompletionModelID,
}

impl ReplicateCompletionResult {
    fn new(prediction: Prediction, model_id: CompletionModelID) -> Self {
        let result = ReplicateCompletionResult {
            prediction,
            provider_id: "replicate".to_string(),
            model_id,
        };

        return result;
    }
}

#[async_trait]
impl CompletionResult for ReplicateCompletionResult {
    async fn poll(&mut self) {
        // TODO: There is a risk here for silent errors
        let _ = self.prediction.reload().await;
    }
    async fn get_status(&mut self) -> CompletionStatus {
        let status = self.prediction.get_status().await;
        match status {
            PredictionStatus::Starting => CompletionStatus::Starting,
            PredictionStatus::Failed => CompletionStatus::Failed,
            PredictionStatus::Canceled => CompletionStatus::Canceled,
            PredictionStatus::Succeeded => CompletionStatus::Succeeded,
            PredictionStatus::Processing => CompletionStatus::Processing,
        }
    }

    async fn get_stream(
        &mut self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = (String, String, String)> + Send>>> {
        let event_stream = self.prediction.get_stream().await;

        match event_stream {
            Ok(mut event_stream) => {
                let stream = stream! {
                    while let Some(event) = event_stream.next().await {
                        match event {
                            Ok(event) => {
                                yield (event.event, event.id, event.data);

                            }
                            _ => todo!(),
                        }
                    }
                };

                // pin_mut!(stream);

                let boxed_stream: Pin<Box<dyn Stream<Item = (String, String, String)> + Send>> =
                    Box::pin(stream);
                anyhow::Ok(boxed_stream)
            }
            Err(err) => panic!("{err}"),
        }
    }
}

#[async_trait]
impl CompletionModel for ReplicateCompletionModel {
    fn get_display_name(&self) -> String {
        let (model_owner, model_name) = self.get_model_details();
        format!("{model_owner}/{model_name}")
    }

    async fn get_completion(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        let inputs = self.get_inputs(&messages);
        let model_details = self.get_model_details();

        let config = ReplicateConfig::new()?;
        let client = PredictionClient::from(config);

        let prediction = client
            .create(
                model_details.0.as_str(),
                model_details.1.as_str(),
                inputs,
                true,
            )
            .await?;

        anyhow::Ok(Box::new(ReplicateCompletionResult::new(
            prediction,
            self.get_display_name(),
        )))
    }
}
