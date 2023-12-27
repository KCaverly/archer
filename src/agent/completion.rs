use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::agent::message::{Message, Role};
use anyhow::anyhow;
use eventsource_stream::EventStream;
use futures::stream;
use replicate_rs::config::ReplicateConfig;
use replicate_rs::predictions::{Prediction, PredictionClient, PredictionStatus};
use serde_json::json;
use strum_macros::EnumIter; // 0.17.1

#[derive(Deserialize, Copy, EnumIter, Default, Eq, PartialEq, Debug, Clone, Serialize)]
pub enum CompletionModel {
    #[default]
    NousHermes_2_Yi34b,
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

impl CompletionModel {
    pub fn get_model_details(&self) -> (String, String) {
        match self {
            CompletionModel::NousHermes_2_Yi34b => (
                "kcaverly".to_string(),
                "nous-hermes-2-yi-34b-gguf".to_string(),
            ),
            CompletionModel::Dolphin2_5Mixtral8x7b => (
                "kcaverly".to_string(),
                "dolphin-2.5-mixtral-8x7b-gguf".to_string(),
            ),
            CompletionModel::Dolphin2_6Mixtral8x7b => (
                "kcaverly".to_string(),
                "dolphin-2.6-mixtral-8x7b-gguf".to_string(),
            ),
            CompletionModel::Yi34bChat => ("01-ai".to_string(), "yi-34b-chat".to_string()),
            CompletionModel::Llama2_13bChat => ("meta".to_string(), "llama-2-13b-chat".to_string()),
            CompletionModel::Llama2_70bChat => ("meta".to_string(), "llama-2-70b-chat".to_string()),
            CompletionModel::Llama2_7bChat => ("meta".to_string(), "llama-2-7b-chat".to_string()),
            CompletionModel::Mistral7bInstructV01 => (
                "mistralai".to_string(),
                "mistral-7b-instruct-v0.1".to_string(),
            ),
            CompletionModel::Codellama34bInstruct => {
                ("meta".to_string(), "codellama-34b-instruct".to_string())
            }
            CompletionModel::DeepseekCoder6_7bInstruct => (
                "kcaverly".to_string(),
                "deepseek-coder-6.7b-instruct".to_string(),
            ),
            CompletionModel::DeepseekCoder33bInstructGGUF => (
                "kcaverly".to_string(),
                "deepseek-coder-33b-instruct-gguf".to_string(),
            ),
        }
    }

    pub fn get_inputs(&self, messages: &Vec<Message>) -> serde_json::Value {
        match self {
            CompletionModel::Dolphin2_5Mixtral8x7b
            | CompletionModel::Dolphin2_6Mixtral8x7b
            | CompletionModel::NousHermes_2_Yi34b => {
                let mut system_prompt = "You are a helpful AI assistant, running in Llmit a Terminal chat Interface built by kcaverly.".to_string();
                let mut prompt = String::new();

                for message in messages {
                    match message.role {
                        Role::System => {
                            system_prompt.push_str(message.content.as_str());
                        }
                        Role::User => {
                            prompt.push_str(
                                format!(
                                    "<|im_start|>user\n{}<|im_end|>\n",
                                    message.content.as_str()
                                )
                                .as_str(),
                            );
                        }
                        Role::Assistant => {
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
            CompletionModel::Yi34bChat => {
                let mut prompt = String::new();
                for message in messages {
                    let content = &message.content;
                    let role = match message.role {
                        Role::System => "system",
                        Role::Assistant => "assistant",
                        Role::User => "user",
                    };

                    prompt.push_str(format!("\n<im_start|>{role}\n{content}<|im_end|>").as_str());
                }

                prompt.push_str("<|im_start|>assistant");

                json!({"prompt": prompt, "prompt_template": "{prompt}"})
            }
            CompletionModel::DeepseekCoder6_7bInstruct => {
                let mut message_objects = Vec::new();
                for message in messages {
                    let role = match message.role {
                        Role::System => "system",
                        Role::Assistant => "assistant",
                        Role::User => "user",
                    };
                    message_objects.push(json!({"role": role, "content": message.content}));
                }

                let message_str = serde_json::to_string(&message_objects).unwrap();

                json!({"messages": message_str})
            }
            CompletionModel::DeepseekCoder33bInstructGGUF => {
                let mut prompt = String::new();
                let mut last_role = Role::System;
                for message in messages {
                    if message.role == last_role {
                        prompt.push_str(format!("\n{}", message.content).as_str());
                    } else {
                        match message.role {
                            Role::User => {
                                prompt.push_str(
                                    format!("\n### Instruction: {}", message.content).as_str(),
                                );
                            }
                            Role::Assistant => prompt
                                .push_str(format!("\n### Response: {}", message.content).as_str()),
                            _ => {}
                        }
                    }
                    last_role = message.role.clone();
                }

                if last_role != Role::Assistant {
                    prompt.push_str("\n### Response: ");
                }

                json!({"prompt": prompt, "prompt_template": "{system_prompt}{prompt}"})
            }
            CompletionModel::Mistral7bInstructV01 => {
                let mut prompt = "<s>".to_string();
                for message in messages {
                    let content = &message.content;
                    match message.role {
                        Role::User => {
                            prompt.push_str(format!("[INST] {content} [/INST]").as_str());
                        }
                        Role::Assistant => prompt.push_str(format!(" {content} </s>").as_str()),
                        Role::System => {}
                    }
                }

                json!({"prompt": prompt})
            }
            CompletionModel::Llama2_13bChat
            | CompletionModel::Llama2_70bChat
            | CompletionModel::Llama2_7bChat
            | CompletionModel::Codellama34bInstruct => {
                let mut system_prompt = String::new();
                let mut prompt = String::new();

                for message in messages {
                    let content = &message.content;
                    match message.role {
                        Role::System => {
                            system_prompt.push_str(format!("{content}\n").as_str());
                        }
                        Role::User => {
                            prompt.push_str(format!("{content} [/INST] ").as_str());
                        }
                        Role::Assistant => {
                            prompt.push_str(format!("{content}</s><s>[INST] ").as_str());
                        }
                    }
                }

                json!({"prompt": prompt, "system_prompt": system_prompt, "prompt_template": "[INST] <<SYS>>\n{{system_prompt}}\n<</SYS>>\n\n{{prompt}}", "max_new_tokens": 4000 })
            }
        }
    }
}

pub async fn get_completion(
    model: CompletionModel,
    messages: Vec<Message>,
) -> anyhow::Result<Message> {
    // Generate Prompt
    let inputs = model.get_inputs(&messages);
    let model_details = model.get_model_details();

    let config = ReplicateConfig::new()?;
    let client = PredictionClient::from(config);

    let mut prediction = client
        .create(
            model_details.0.as_str(),
            model_details.1.as_str(),
            inputs,
            false,
        )
        .await?;

    loop {
        match prediction.status {
            PredictionStatus::Succeeded => {
                if let Some(output) = prediction.output {
                    let content = output
                        .as_array()
                        .ok_or(anyhow!("output is unexpected"))?
                        .iter()
                        .map(|x| x.as_str().unwrap())
                        .collect::<String>();
                    return anyhow::Ok(Message {
                        role: Role::Assistant,
                        content,
                        status: None,
                        model: Some(model),
                    });
                } else {
                    panic!("output error");
                }
            }
            PredictionStatus::Failed | PredictionStatus::Canceled => {
                panic!("prediction failed or was canceled");
            }
            _ => {}
        }
        prediction.reload().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    }
}

pub async fn create_prediction(
    model: &CompletionModel,
    messages: Vec<Message>,
) -> anyhow::Result<Prediction> {
    let model_details = model.get_model_details();
    let inputs = model.get_inputs(&messages);
    let config = ReplicateConfig::new()?;
    let client = PredictionClient::from(config);

    anyhow::Ok(
        client
            .create(
                model_details.0.as_str(),
                model_details.1.as_str(),
                inputs,
                true,
            )
            .await?,
    )
}
