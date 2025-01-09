use crate::error::CtGenError;
use anyhow::Result;
use chrono::Utc;
use database_reflection::reflection::{Constraint, ConstraintSide, Database, Table};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTaskContext {
    database: Database,
    table_name: String,
    table: Arc<Table>,
    constraints_local: Vec<Arc<Constraint>>,
    constraints_foreign: Vec<Arc<Constraint>>,
    prompts: HashMap<String, Value>,
    timestamp: String,
    ctgen_ver: String,
}

impl CtGenTaskContext {
    /// Init new task context
    pub fn new(database: Database, table_name: &str) -> Result<Self> {
        let table = database
            .table(table_name)
            .ok_or_else(|| CtGenError::ValidationError(format!("Table not found: {}", table_name)))?;

        let constraints_local = database.constraints_by_table(table.clone(), Some(ConstraintSide::Local));
        let constraints_foreign = database.constraints_by_table(table.clone(), Some(ConstraintSide::Foreign));

        Ok(Self {
            database,
            table_name: table_name.to_string(),
            table,
            constraints_local,
            constraints_foreign,
            timestamp: Utc::now().to_rfc3339(),
            ctgen_ver: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        })
    }

    /// Set prompt answer in task context
    pub fn set_prompt_answer(&mut self, prompt_id: &str, prompt_answer: &Value) {
        self.prompts.insert(prompt_id.to_string(), prompt_answer.clone());
    }
}
