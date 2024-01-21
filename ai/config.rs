use serde::Deserialize;
use serde_json::Value;

use super::prompt::PromptTemplateVariant;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub default_completion_model: ModelConfig,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    pub provider_id: String,
    pub model_id: String,
    pub extra_args: Option<Value>,
    pub template: PromptTemplateVariant,
}

const DEFAULT_CONFIG_STR: &str = include_str!("default.json");
lazy_static! {
    pub static ref ARCHER_CONFIG: Config = serde_json::from_str(DEFAULT_CONFIG_STR).unwrap();
}
