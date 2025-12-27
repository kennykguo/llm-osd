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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
}

pub fn validate_action_plan(plan: &ActionPlan) -> Result<(), ValidationError> {
    if plan.version.trim().is_empty() {
        return Err(ValidationError {
            message: "version must be non-empty".to_string(),
        });
    }

    for action in &plan.actions {
        match action {
            Action::Exec(exec) => {
                if exec.argv.is_empty() {
                    return Err(ValidationError {
                        message: "exec.argv must be non-empty".to_string(),
                    });
                }
                if exec.timeout_sec == 0 {
                    return Err(ValidationError {
                        message: "exec.timeout_sec must be >= 1".to_string(),
                    });
                }
                if exec.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "exec.reason must be non-empty".to_string(),
                    });
                }
            }
            Action::ReadFile(read) => {
                if read.path.trim().is_empty() {
                    return Err(ValidationError {
                        message: "read_file.path must be non-empty".to_string(),
                    });
                }
                if read.max_bytes == 0 {
                    return Err(ValidationError {
                        message: "read_file.max_bytes must be >= 1".to_string(),
                    });
                }
                if read.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "read_file.reason must be non-empty".to_string(),
                    });
                }
            }
            Action::WriteFile(write) => {
                if write.path.trim().is_empty() {
                    return Err(ValidationError {
                        message: "write_file.path must be non-empty".to_string(),
                    });
                }
                if write.mode.trim().is_empty() {
                    return Err(ValidationError {
                        message: "write_file.mode must be non-empty".to_string(),
                    });
                }
                if write.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "write_file.reason must be non-empty".to_string(),
                    });
                }
            }
        }
    }

    Ok(())
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

    #[test]
    fn validate_rejects_empty_exec_argv() {
        let plan = ActionPlan {
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec![],
                timeout_sec: 5,
                as_root: false,
                reason: "test".to_string(),
            })],
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.argv must be non-empty");
    }
}


