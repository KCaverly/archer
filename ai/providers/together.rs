use crate::ai::completion::{
    CompletionModel, CompletionProvider, CompletionResult, CompletionStatus, Message,
};
use crate::ai::config::{merge, ModelConfig, ARCHER_CONFIG};
use anyhow::anyhow;
use async_stream::stream;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures_lite::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env::var;
use std::pin::Pin;

#[derive(Default)]
pub struct TogetherAI {
    api_key: Option<String>,
    base_url: String,
}

impl CompletionProvider for TogetherAI {
    fn load() -> Self {
        let base_url = "https://api.together.xyz".to_string();
        if let Some(api_key) = var("TOGETHER_API_KEY").ok() {
            TogetherAI {
                api_key: Some(api_key),
                base_url,
            }
        } else {
            TogetherAI {
                api_key: None,
                base_url,
            }
        }
    }
    fn has_credentials(&self) -> bool {
        self.api_key.is_some()
    }
    fn get_model(&self, model_config: &ModelConfig) -> anyhow::Result<Box<dyn CompletionModel>> {
        if model_config.provider_id == self.get_id() {
            return anyhow::Ok(Box::new(TogetherCompletionModel::load(
                model_config.clone(),
            )));
        }
        Err(anyhow!("model_config provider does not match provider"))
    }

    fn get_id(&self) -> String {
        "TogetherAI".to_string()
    }
}

#[derive(Clone, Debug)]
struct TogetherCompletionModel {
    model_config: ModelConfig,
}

impl TogetherCompletionModel {
    pub fn load(model_config: ModelConfig) -> Self {
        TogetherCompletionModel { model_config }
    }
    pub fn get_inputs(&self, messages: &Vec<Message>, stream: bool) -> serde_json::Value {
        let template = self.model_config.template.get_template();
        let prompt = template.generate_prompt(messages);

        let inputs = json!({"prompt": prompt.full_prompt, "model": self.model_config.model_id, "temperature": 0.7, "top_p": 0.7, "top_k": 50, "max_tokens": 2000, "repetition_penalty": 1, "stream_tokens": stream});

        let inputs = if let Some(extra_args) = self.model_config.extra_args.clone() {
            merge(&inputs, &extra_args)
        } else {
            inputs
        };

        inputs
    }
}

#[async_trait]
impl CompletionModel for TogetherCompletionModel {
    async fn get_completion(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        todo!();
    }

    async fn start_streaming(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        let provider = TogetherAI::load();
        let endpoint = format!("{}/inference", provider.base_url);
        if let Some(api_key) = provider.api_key {
            let body = self.get_inputs(&messages, true);
            let client = reqwest::Client::new();
            let mut event_stream = client
                .post(endpoint)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .send()
                .await?
                .bytes_stream()
                .eventsource();

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

            let boxed_stream: Pin<Box<dyn Stream<Item = (String, String, String)> + Send + Sync>> =
                Box::pin(stream);

            anyhow::Ok(Box::new(TogetherCompletionResult {
                status: CompletionStatus::Processing,
                stream: boxed_stream,
            }))
        } else {
            Err(anyhow!("togetherai api request failed"))
        }
    }
}

struct TogetherCompletionResult {
    status: CompletionStatus,
    stream: Pin<Box<dyn Stream<Item = (String, String, String)> + Send + Sync>>,
}

#[async_trait]
impl CompletionResult for TogetherCompletionResult {
    async fn poll(&mut self) {}
    async fn get_status(&mut self) -> CompletionStatus {
        self.status.clone()
    }
    async fn get_stream<'a>(
        &'a mut self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = (String, String, String)> + Send + Sync + 'a>>>
    {
        let stream = stream! {
            let mut id = 0;
            while let Some(event) = self.stream.next().await {

                let (event_str, _, data) = &event;
                id += 1;

                let obj: anyhow::Result<TogetherStreamingEvent> = serde_json::from_str(data).map_err(|err| anyhow!(err.to_string()));
                match obj {
                    Ok(obj) => {
                        let data = obj.choices.get(0).map(|x| x.get("text")).unwrap().unwrap();
                        yield (event_str.clone(), id.to_string().clone(), data.clone())
                    }
                    _ => {
                        yield ("done".to_string(), id.to_string(), "".to_string())
                    }
                }
            }
        };

        anyhow::Ok(Box::pin(stream))
    }
    fn get_content(&mut self) -> anyhow::Result<String> {
        todo!();
    }
}

#[derive(Deserialize, Debug)]
struct TogetherStreamingEvent {
    choices: Vec<HashMap<String, String>>,
}
