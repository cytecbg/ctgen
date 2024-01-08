use serde::{Deserialize, Serialize};
use crate::profile::CtGenPrompt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CtGenTaskPrompt {
    PromptDatabase,
    PromptTable,
    PromptGeneric(CtGenPrompt)
}