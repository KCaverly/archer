use crate::ai::completion::{
    CompletionModel, CompletionModelID, CompletionProvider, CompletionResult, CompletionStatus,
    Message, MessageRole,
};
use anyhow::anyhow;
use async_stream::stream;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures_lite::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env::var;
use std::pin::Pin;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

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

    fn list_models(&self) -> Vec<Box<dyn CompletionModel>> {
        let mut models = Vec::<Box<dyn CompletionModel>>::new();
        for model in TogetherCompletionModel::iter() {
            let boxed_model = Box::new(model);
            models.push(boxed_model)
        }

        models
    }

    fn get_id(&self) -> String {
        "TogetherAI".to_string()
    }

    fn default_model(&self) -> Box<dyn CompletionModel> {
        Box::new(TogetherCompletionModel::DiscoLMMixtral8x7bv2)
    }
    fn get_model(&self, model_id: CompletionModelID) -> Option<Box<dyn CompletionModel>> {
        for model in TogetherCompletionModel::iter() {
            if model.get_display_name() == model_id {
                return Some(Box::new(model));
            }
        }

        None
    }
}

#[derive(Default, EnumIter, Clone, Debug)]
enum TogetherCompletionModel {
    #[default]
    DiscoLMMixtral8x7bv2,
}

impl TogetherCompletionModel {
    fn get_inputs(&self, messages: &Vec<Message>, stream: bool) -> serde_json::Value {
        let mut system_prompt = "You are a helpful AI assistant, running in Archer a Terminal chat Interface built by kcaverly.".to_string();
        let mut prompt = String::new();

        for message in messages {
            match message.role {
                MessageRole::System => {
                    system_prompt.push_str(message.content.as_str());
                }
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

        let full_prompt = format!("<|im_start|>system\n{system_prompt}<|im_end|>\n{prompt}");
        json!({"prompt": full_prompt, "model": self.get_display_name(), "temperature": 0.7, "top_p": 0.7, "top_k": 50, "max_tokens": 2000, "stop": ["<|im_start|>"], "repetition_penalty": 1, "stream_tokens": stream})
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

    fn get_display_name(&self) -> String {
        match self {
            TogetherCompletionModel::DiscoLMMixtral8x7bv2 => {
                "DiscoResearch/DiscoLM-mixtral-8x7b-v2".to_string()
            }
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
