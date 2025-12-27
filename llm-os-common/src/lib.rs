// ABOUTME: defines the shared action protocol types used by llmsh and llm-osd.
// ABOUTME: provides parsing and validation helpers to keep execution deterministic.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ParseFailed,
    ValidationFailed,
    InvalidMode,
    RequestTooLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionErrorCode {
    PolicyDenied,
    ConfirmationRequired,
    ExecFailed,
    ExecTimedOut,
    ReadFailed,
    WriteFailed,
    InvalidModeString,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    PlanOnly,
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionPlan {
    pub request_id: String,
    pub session_id: Option<String>,
    pub version: String,
    pub mode: Mode,
    pub actions: Vec<Action>,
    pub confirmation: Option<Confirmation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Confirmation {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Exec(ExecAction),
    ReadFile(ReadFileAction),
    WriteFile(WriteFileAction),
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadFileAction {
    pub path: String,
    pub max_bytes: u64,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionPlanResult {
    pub request_id: String,
    pub results: Vec<ActionResult>,
    pub error: Option<RequestError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionError {
    pub code: ActionErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionResult {
    Exec(ExecResult),
    ReadFile(ReadFileResult),
    WriteFile(WriteFileResult),
    Pong(PongResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PongResult {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stdout_truncated: bool,
    pub stderr: String,
    pub stderr_truncated: bool,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadFileResult {
    pub ok: bool,
    pub content_base64: Option<String>,
    pub truncated: bool,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriteFileResult {
    pub ok: bool,
    pub artifacts: Vec<String>,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
}

pub fn validate_action_plan(plan: &ActionPlan) -> Result<(), ValidationError> {
    const MAX_READ_FILE_BYTES: u64 = 64 * 1024;
    const MAX_WRITE_FILE_BYTES: usize = 64 * 1024;
    const MAX_ACTIONS: usize = 64;
    const MAX_EXEC_ARGC: usize = 64;
    const MAX_EXEC_ARG_BYTES: usize = 2048;
    const MAX_EXEC_ENV_ENTRIES: usize = 32;
    const MAX_EXEC_ENV_KEY_BYTES: usize = 128;
    const MAX_EXEC_ENV_VALUE_BYTES: usize = 2048;
    const MAX_REQUEST_ID_BYTES: usize = 128;
    const MAX_SESSION_ID_BYTES: usize = 128;
    const MAX_REASON_BYTES: usize = 2048;
    const MAX_PATH_BYTES: usize = 4096;

    if plan.actions.len() > MAX_ACTIONS {
        return Err(ValidationError {
            message: "too many actions".to_string(),
        });
    }

    if plan.request_id.trim().is_empty() {
        return Err(ValidationError {
            message: "request_id must be non-empty".to_string(),
        });
    }
    if plan.request_id.as_bytes().len() > MAX_REQUEST_ID_BYTES {
        return Err(ValidationError {
            message: "request_id is too long".to_string(),
        });
    }

    if let Some(session_id) = &plan.session_id {
        if session_id.trim().is_empty() {
            return Err(ValidationError {
                message: "session_id must be non-empty when provided".to_string(),
            });
        }
        if session_id.as_bytes().len() > MAX_SESSION_ID_BYTES {
            return Err(ValidationError {
                message: "session_id is too long".to_string(),
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
                if exec.as_root {
                    return Err(ValidationError {
                        message: "exec.as_root is not supported".to_string(),
                    });
                }
                if exec.argv.len() > MAX_EXEC_ARGC {
                    return Err(ValidationError {
                        message: "exec.argv has too many args".to_string(),
                    });
                }
                if exec
                    .argv
                    .iter()
                    .any(|a| a.as_bytes().len() > MAX_EXEC_ARG_BYTES)
                {
                    return Err(ValidationError {
                        message: "exec.argv arg is too long".to_string(),
                    });
                }
                if let Some(cwd) = &exec.cwd {
                    if cwd.trim().is_empty() {
                        return Err(ValidationError {
                            message: "exec.cwd must be non-empty when provided".to_string(),
                        });
                    }
                }
                if let Some(env) = &exec.env {
                    if env.len() > MAX_EXEC_ENV_ENTRIES {
                        return Err(ValidationError {
                            message: "exec.env has too many entries".to_string(),
                        });
                    }
                    for (k, v) in env {
                        if k.as_bytes().len() > MAX_EXEC_ENV_KEY_BYTES {
                            return Err(ValidationError {
                                message: "exec.env key is too long".to_string(),
                            });
                        }
                        if v.as_bytes().len() > MAX_EXEC_ENV_VALUE_BYTES {
                            return Err(ValidationError {
                                message: "exec.env value is too long".to_string(),
                            });
                        }
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
                if exec.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "reason is too long".to_string(),
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
                if read.path.as_bytes().len() > MAX_PATH_BYTES {
                    return Err(ValidationError {
                        message: "path is too long".to_string(),
                    });
                }
                if read.max_bytes == 0 {
                    return Err(ValidationError {
                        message: "read_file.max_bytes must be >= 1".to_string(),
                    });
                }
                if read.max_bytes > MAX_READ_FILE_BYTES {
                    return Err(ValidationError {
                        message: "read_file.max_bytes is too large".to_string(),
                    });
                }
                if read.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "read_file.reason must be non-empty".to_string(),
                    });
                }
                if read.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "reason is too long".to_string(),
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
                if write.path.as_bytes().len() > MAX_PATH_BYTES {
                    return Err(ValidationError {
                        message: "path is too long".to_string(),
                    });
                }
                if write.content.as_bytes().len() > MAX_WRITE_FILE_BYTES {
                    return Err(ValidationError {
                        message: "write_file.content is too large".to_string(),
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
                if write.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "reason is too long".to_string(),
                    });
                }

                if write.danger.is_some() {
                    require_confirmation(plan, "write_file requires confirmation when danger is set")?;
                }
            }
            Action::Ping => {}
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

    #[test]
    fn json_schema_generation_includes_request_id() {
        let schema = schemars::schema_for!(ActionPlan);
        let value = serde_json::to_value(&schema).unwrap();
        assert!(value.to_string().contains("\"request_id\""));
    }

    #[test]
    fn validate_rejects_as_root_true() {
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
                as_root: true,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.as_root is not supported");
    }

    #[test]
    fn validate_rejects_read_file_max_bytes_too_large() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::ReadFile(ReadFileAction {
                path: "./Cargo.toml".to_string(),
                max_bytes: 10 * 1024 * 1024,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "read_file.max_bytes is too large");
    }

    #[test]
    fn validate_rejects_write_file_content_too_large() {
        let big = "a".repeat(128 * 1024);
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::WriteFile(WriteFileAction {
                path: "./out.txt".to_string(),
                content: big,
                mode: "0644".to_string(),
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "write_file.content is too large");
    }

    #[test]
    fn validate_rejects_too_many_actions() {
        let mut actions = Vec::new();
        for _ in 0..65 {
            actions.push(Action::Ping);
        }
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions,
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "too many actions");
    }

    #[test]
    fn validate_rejects_exec_too_many_args() {
        let mut argv = Vec::new();
        argv.push("/bin/echo".to_string());
        for _ in 0..64 {
            argv.push("x".to_string());
        }

        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv,
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
        assert_eq!(err.message, "exec.argv has too many args");
    }

    #[test]
    fn validate_rejects_exec_arg_too_long() {
        let long = "a".repeat(2049);
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec!["/bin/echo".to_string(), long],
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
        assert_eq!(err.message, "exec.argv arg is too long");
    }

    #[test]
    fn validate_rejects_exec_env_too_many_entries() {
        let mut env = std::collections::BTreeMap::new();
        for i in 0..33 {
            env.insert(format!("K{i}"), "V".to_string());
        }
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec!["/bin/echo".to_string(), "hi".to_string()],
                cwd: None,
                env: Some(env),
                timeout_sec: 5,
                as_root: false,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.env has too many entries");
    }

    #[test]
    fn validate_rejects_exec_env_key_too_long() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("K".repeat(129), "V".to_string());
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec!["/bin/echo".to_string(), "hi".to_string()],
                cwd: None,
                env: Some(env),
                timeout_sec: 5,
                as_root: false,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.env key is too long");
    }

    #[test]
    fn validate_rejects_exec_env_value_too_long() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("K".to_string(), "V".repeat(2049));
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec!["/bin/echo".to_string(), "hi".to_string()],
                cwd: None,
                env: Some(env),
                timeout_sec: 5,
                as_root: false,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.env value is too long");
    }

    #[test]
    fn validate_rejects_request_id_too_long() {
        let plan = ActionPlan {
            request_id: "a".repeat(129),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Ping],
            confirmation: None,
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "request_id is too long");
    }

    #[test]
    fn validate_rejects_session_id_too_long() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: Some("a".repeat(129)),
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Ping],
            confirmation: None,
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "session_id is too long");
    }

    #[test]
    fn validate_rejects_reason_too_long() {
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
                reason: "a".repeat(2049),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "reason is too long");
    }

    #[test]
    fn validate_rejects_path_too_long() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::ReadFile(ReadFileAction {
                path: "a".repeat(4097),
                max_bytes: 1,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "path is too long");
    }
}


