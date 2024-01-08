use crate::profile::CtGenPrompt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CtGenTaskPrompt {
    PromptDatabase,
    PromptTable,
    PromptGeneric { prompt_id: String, prompt_data: CtGenPrompt },
}
