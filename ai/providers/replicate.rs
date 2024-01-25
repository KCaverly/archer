use crate::ai::completion::{
    CompletionModel, CompletionModelID, CompletionProvider, CompletionProviderID, CompletionResult,
    CompletionStatus, Message, MessageRole,
};
use crate::ai::config::{merge, ModelConfig};
use anyhow::anyhow;
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

    fn get_model(&self, model_config: &ModelConfig) -> anyhow::Result<Box<dyn CompletionModel>> {
        if model_config.provider_id == self.get_id() {
            return anyhow::Ok(Box::new(ReplicateCompletionModel::load(
                model_config.clone(),
            )));
        }
        Err(anyhow!("model_id does not match provider"))
    }

    fn get_id(&self) -> String {
        "Replicate".to_string()
    }
}

#[derive(Clone, Debug)]
struct ReplicateCompletionModel {
    model_config: ModelConfig,
}

impl ReplicateCompletionModel {
    pub fn load(model_config: ModelConfig) -> Self {
        ReplicateCompletionModel { model_config }
    }
    pub fn get_inputs(&self, messages: &Vec<Message>) -> serde_json::Value {
        let template = self.model_config.template.get_template();
        let prompt = template.generate_prompt(messages);
        let inputs = json!({"prompt": prompt.prompt, "system_prompt": prompt.system_prompt, "prompt_template": prompt.prompt_template});

        let inputs = if let Some(extra_args) = self.model_config.extra_args.clone() {
            merge(&inputs, &extra_args)
        } else {
            inputs
        };

        inputs
    }
}

#[derive(Debug)]
struct ReplicateCompletionResult {
    prediction: Prediction,
    model_config: ModelConfig,
}

impl ReplicateCompletionResult {
    fn new(prediction: Prediction, model_config: ModelConfig) -> Self {
        let result = ReplicateCompletionResult {
            prediction,
            model_config,
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

    fn get_content(&mut self) -> anyhow::Result<String> {
        if let Some(output) = self.prediction.output.clone() {
            let content = output
                .as_array()
                .ok_or(anyhow!("output is unexpected"))?
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect::<String>();
            anyhow::Ok(content)
        } else {
            Err(anyhow!("output is invalid"))
        }
    }

    async fn get_stream<'a>(
        &'a mut self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = (String, String, String)> + Send + Sync + 'a>>>
    {
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

                let boxed_stream: Pin<
                    Box<dyn Stream<Item = (String, String, String)> + Send + Sync>,
                > = Box::pin(stream);
                anyhow::Ok(boxed_stream)
            }
            Err(err) => panic!("{err}"),
        }
    }
}

#[async_trait]
impl CompletionModel for ReplicateCompletionModel {
    async fn get_completion(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        let inputs = self.get_inputs(&messages);

        let model_details = {
            let splits = self.model_config.model_id.split("/");
            let model_details = splits.map(|x| x.to_string()).collect::<Vec<String>>();
            model_details
        };

        let config = ReplicateConfig::new()?;
        let client = PredictionClient::from(config);

        let mut prediction = client
            .create(
                model_details
                    .get(0)
                    .ok_or(anyhow!("model_id not in correct format"))?
                    .as_str(),
                model_details
                    .get(1)
                    .ok_or(anyhow!("model_id not in correct format"))?
                    .as_str(),
                inputs,
                false,
            )
            .await?;

        fn is_completed(status: &PredictionStatus) -> bool {
            match status {
                PredictionStatus::Failed
                | PredictionStatus::Succeeded
                | PredictionStatus::Canceled => true,
                _ => false,
            }
        }

        while !is_completed(&prediction.status) {
            // TODO: This error has to be accomodated for
            let _ = prediction.reload().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }

        anyhow::Ok(Box::new(ReplicateCompletionResult {
            prediction,
            model_config: self.model_config.clone(),
        }))
    }

    async fn start_streaming(
        &self,
        messages: Vec<Message>,
    ) -> anyhow::Result<Box<dyn CompletionResult>> {
        let inputs = self.get_inputs(&messages);

        let model_details = {
            let splits = self.model_config.model_id.split("/");
            let model_details = splits.map(|x| x.to_string()).collect::<Vec<String>>();
            model_details
        };

        let config = ReplicateConfig::new()?;
        let client = PredictionClient::from(config);

        let prediction = client
            .create(
                model_details
                    .get(0)
                    .ok_or(anyhow!("model_id not in correct format"))?
                    .as_str(),
                model_details
                    .get(1)
                    .ok_or(anyhow!("model_id not in correct format"))?
                    .as_str(),
                inputs,
                true,
            )
            .await?;

        anyhow::Ok(Box::new(ReplicateCompletionResult::new(
            prediction,
            self.model_config.clone(),
        )))
    }
}
