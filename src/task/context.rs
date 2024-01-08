use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use database_reflection::reflection::{Constraint, ConstraintSide, Database, Table};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::CtGenError;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct CtGenTaskContext {
    database: Database,
    table_name: String,
    table: Arc<Table>,
    constraints_local: Vec<Arc<Constraint>>,
    constraints_foreign: Vec<Arc<Constraint>>,
    prompts: HashMap<String, Value>,
}

impl CtGenTaskContext {
    pub fn new(database: Database, table_name: &str) -> Result<Self> {
        let table = database.table(table_name).ok_or(CtGenError::ValidationError(format!("Table not found: {}", table_name)))?;

        let constraints_local = database.constraints_by_table(table.clone(), Some(ConstraintSide::Local));
        let constraints_foreign = database.constraints_by_table(table.clone(), Some(ConstraintSide::Foreign));

        Ok(Self {
            database,
            table_name: table_name.to_string(),
            table,
            constraints_local,
            constraints_foreign,
            ..Default::default()
        })
    }
}