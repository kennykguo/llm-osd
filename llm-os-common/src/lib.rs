// ABOUTME: defines the shared action protocol types used by llmsh and llm-osd.
// ABOUTME: provides parsing and validation helpers to keep execution deterministic.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    PlanOnly,
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionPlan {
    pub version: String,
    pub mode: Mode,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Exec(ExecAction),
    ReadFile(ReadFileAction),
    WriteFile(WriteFileAction),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecAction {
    pub argv: Vec<String>,
    pub timeout_sec: u64,
    pub as_root: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadFileAction {
    pub path: String,
    pub max_bytes: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriteFileAction {
    pub path: String,
    pub content: String,
    pub mode: String,
    pub reason: String,
}

pub fn parse_action_plan(input: &str) -> Result<ActionPlan, serde_json::Error> {
    serde_json::from_str(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_unknown_fields_in_exec_action() {
        let input = r#"
        {
          "version": "0.1",
          "mode": "execute",
          "actions": [
            {
              "type": "exec",
              "argv": ["echo", "hi"],
              "timeout_sec": 5,
              "as_root": false,
              "reason": "test",
              "unexpected": "hallucination"
            }
          ]
        }
        "#;

        let parsed = parse_action_plan(input);
        assert!(parsed.is_err());
    }
}


