use crate::archer_ai::completion::{
    CompletionModel, CompletionProvider, CompletionResult, CompletionStatus, Message, MessageRole,
};
use async_trait::async_trait;
use replicate_rs::config::ReplicateConfig;
use replicate_rs::predictions::{Prediction, PredictionClient, PredictionStatus};
use serde_json::json;
use std::env::var;
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
}

#[derive(Default, EnumIter)]
enum ReplicateCompletionModel {
    #[default]
    NousHermes2Yi34b,
    Dolphin2_6Mixtral8x7b,
    Dolphin2_5Mixtral8x7b,
    Yi34bChat,
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
            ReplicateCompletionModel::Mistral7bInstructV01 => {
                ("mistralai".to_string(), "mistral-7b-instruct".to_string())
            }
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

struct ReplicateCompletionResult {
    status: CompletionStatus,
    message: Option<Message>,
    prediction: Prediction,
}

impl ReplicateCompletionResult {
    fn new(prediction: Prediction) -> Self {
        let mut result = ReplicateCompletionResult {
            message: None,
            status: CompletionStatus::Starting,
            prediction,
        };

        result.update();

        return result;
    }

    fn update(&mut self) {
        match self.prediction.status {
            PredictionStatus::Succeeded | PredictionStatus::Processing => {
                if let Some(output) = self
                    .prediction
                    .output
                    .as_ref()
                    .map(|x| x.as_array())
                    .unwrap_or(None)
                {
                    self.message = Some(Message {
                        role: MessageRole::Assistant,
                        content: output
                            .iter()
                            .map(|x| x.as_str().unwrap())
                            .collect::<String>(),
                    });
                } else {
                    self.status = CompletionStatus::Failed;
                };
            }
            PredictionStatus::Failed => self.status = CompletionStatus::Failed,
            PredictionStatus::Canceled => self.status = CompletionStatus::Canceled,
            PredictionStatus::Starting => self.status = CompletionStatus::Starting,
        }
    }
}

impl CompletionResult for ReplicateCompletionResult {
    fn poll(&self) {
        todo!();
    }
    fn get_status(&self) -> CompletionStatus {
        todo!();
    }
    fn get_message(&self) -> Option<Message> {
        todo!();
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
                false,
            )
            .await?;

        anyhow::Ok(Box::new(ReplicateCompletionResult::new(prediction)))
    }
}
