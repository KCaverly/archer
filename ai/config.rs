use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::prompt::PromptTemplateVariant;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub default_completion_model: ModelConfig,
    pub default_title_model: ModelConfig,
    pub models: Vec<ModelConfig>,
}

#[derive(Eq, Serialize, PartialEq, Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub provider_id: String,
    pub model_id: String,
    pub extra_args: Option<HashMap<String, Value>>,
    pub template: PromptTemplateVariant,
}

const DEFAULT_CONFIG_STR: &str = include_str!("default.json");
lazy_static! {
    pub static ref ARCHER_CONFIG: Config = serde_json::from_str(DEFAULT_CONFIG_STR).unwrap();
}

pub fn merge(v: &Value, fields: &HashMap<String, Value>) -> Value {
    match v {
        Value::Object(m) => {
            let mut m = m.clone();
            for (k, v) in fields {
                m.insert(k.clone(), v.clone());
            }
            Value::Object(m)
        }
        v => v.clone(),
    }
}
