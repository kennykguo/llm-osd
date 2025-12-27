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
    pub request_id: String,
    pub session_id: Option<String>,
    pub version: String,
    pub mode: Mode,
    pub actions: Vec<Action>,
    pub confirmation: Option<Confirmation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Confirmation {
    pub token: String,
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
    pub cwd: Option<String>,
    pub env: Option<std::collections::BTreeMap<String, String>>,
    pub timeout_sec: u64,
    pub as_root: bool,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadFileAction {
    pub path: String,
    pub max_bytes: u64,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriteFileAction {
    pub path: String,
    pub content: String,
    pub mode: String,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

pub fn parse_action_plan(input: &str) -> Result<ActionPlan, serde_json::Error> {
    serde_json::from_str(input)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionPlanResult {
    pub request_id: String,
    pub results: Vec<ActionResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionResult {
    Exec(ExecResult),
    ReadFile(ReadFileResult),
    WriteFile(WriteFileResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stdout_truncated: bool,
    pub stderr: String,
    pub stderr_truncated: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadFileResult {
    pub ok: bool,
    pub content_base64: Option<String>,
    pub truncated: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriteFileResult {
    pub ok: bool,
    pub artifacts: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
}

pub fn validate_action_plan(plan: &ActionPlan) -> Result<(), ValidationError> {
    if plan.request_id.trim().is_empty() {
        return Err(ValidationError {
            message: "request_id must be non-empty".to_string(),
        });
    }

    if let Some(session_id) = &plan.session_id {
        if session_id.trim().is_empty() {
            return Err(ValidationError {
                message: "session_id must be non-empty when provided".to_string(),
            });
        }
    }

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
                if let Some(cwd) = &exec.cwd {
                    if cwd.trim().is_empty() {
                        return Err(ValidationError {
                            message: "exec.cwd must be non-empty when provided".to_string(),
                        });
                    }
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

                if exec.danger.is_some() {
                    require_confirmation(plan, "exec requires confirmation when danger is set")?;
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

                if read.danger.is_some() {
                    require_confirmation(plan, "read_file requires confirmation when danger is set")?;
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

                if write.danger.is_some() {
                    require_confirmation(plan, "write_file requires confirmation when danger is set")?;
                }
            }
        }
    }

    Ok(())
}

fn require_confirmation(plan: &ActionPlan, message: &str) -> Result<(), ValidationError> {
    match &plan.confirmation {
        Some(c) if !c.token.trim().is_empty() => Ok(()),
        _ => Err(ValidationError {
            message: message.to_string(),
        }),
    }
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
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec![],
                cwd: None,
                env: None,
                timeout_sec: 5,
                as_root: false,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.argv must be non-empty");
    }

    #[test]
    fn validate_requires_confirmation_when_danger_is_set() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec!["/bin/echo".to_string(), "hi".to_string()],
                cwd: None,
                env: None,
                timeout_sec: 5,
                as_root: false,
                reason: "test".to_string(),
                danger: Some("danger".to_string()),
                recovery: Some("recovery".to_string()),
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(
            err.message,
            "exec requires confirmation when danger is set"
        );
    }
}


