use anyhow::anyhow;
use replicate_rs::predictions::{PredictionClient, PredictionStatus};

use replicate_rs::config::ReplicateConfig;
use serde::Serialize;

#[derive(Serialize)]
struct Llama70b {
    prompt: String,
    system_prompt: String,
}

pub async fn get_response(prompt: &str) -> anyhow::Result<String> {
    let config = ReplicateConfig::new()?;
    let client = PredictionClient::from(config);

    let input = Llama70b {
        prompt: prompt.to_string(),
        system_prompt: "You are a helpful assistant named bond. Please answer the user's query, speaking in plain language.".to_string(),
    };

    let mut prediction = client
        .create("meta", "llama-2-70b-chat", Box::new(input))
        .await?;

    loop {
        match prediction.status {
            PredictionStatus::Succeeded => {
                return anyhow::Ok(
                    prediction
                        .output
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect::<String>(),
                );
            }
            PredictionStatus::Starting | PredictionStatus::Processing => {
                prediction.reload().await?;
            }
            _ => {
                return Err(anyhow!("prediction failed!"));
            }
        }
    }
}
