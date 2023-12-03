use crate::agent::message::{Message, Role};
use anyhow::anyhow;
use replicate_rs::config::ReplicateConfig;
use replicate_rs::predictions::{PredictionClient, PredictionStatus};
use serde_json::json;

pub enum CompletionModel {
    Yi34B,
}

impl CompletionModel {
    pub fn get_model_details(&self) -> (String, String) {
        match self {
            CompletionModel::Yi34B => ("01-ai".to_string(), "yi-34b-chat".to_string()),
        }
    }
}

pub async fn get_completion(
    model: CompletionModel,
    messages: Vec<Message>,
) -> anyhow::Result<Message> {
    // Generate Prompt
    let mut prompt = String::new();
    for message in messages {
        let content = message.content;
        match message.role {
            Role::System => {
                prompt.push_str(format!("\n<|im_start|>system\n{content}<|im_end|>").as_str());
            }
            Role::User => {
                prompt.push_str(format!("\n<|im_start|>user\n{content}<|im_end|>").as_str());
            }
            Role::Assistant => {
                prompt.push_str(format!("\n<|im_start|>assistant\n{content}<|im_end|>").as_str());
            }
        }
    }

    prompt.push_str("<|im_start|>assistant");

    let model_details = model.get_model_details();

    let config = ReplicateConfig::new()?;
    let client = PredictionClient::from(config);

    let mut prediction = client
        .create(
            model_details.0.as_str(),
            model_details.1.as_str(),
            json!({"prompt": prompt, "prompt_template": "{prompt}"}),
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
