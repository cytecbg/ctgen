use crate::profile::CtGenPrompt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CtGenTaskPrompt {
    PromptDatabase,
    PromptTable,
    PromptGeneric { prompt_id: String, prompt_data: CtGenPrompt },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CtGenRenderedPrompt {
    should_ask: bool,
    enumerate: Option<Vec<String>>,
    prompt: String,
    options: serde_json::Value,
    multiple: bool,
    ordered: bool
}

impl CtGenRenderedPrompt {
    pub fn new(should_ask: bool, enumerate: Option<Vec<String>>, prompt: String, options: serde_json::Value, multiple: bool, ordered: bool) -> CtGenRenderedPrompt {
        CtGenRenderedPrompt {
            should_ask,
            enumerate,
            prompt,
            options,
            multiple,
            ordered
        }
    }

    pub fn should_ask(&self) -> bool {
        self.should_ask
    }
    pub fn enumerate(&self) -> Option<&Vec<String>> {
        self.enumerate.as_ref()
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn options(&self) -> &serde_json::Value {
        &self.options
    }
    pub fn multiple(&self) -> bool {
        self.multiple
    }
    pub fn ordered(&self) -> bool { self.ordered }
}
