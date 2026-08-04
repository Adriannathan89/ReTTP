use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Suite {
    pub name: Option<String>,
    pub blocks: Vec<SuiteBlock>,
}