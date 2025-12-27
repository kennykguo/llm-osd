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
    ServiceControl(ServiceControlAction),
    InstallPackages(InstallPackagesAction),
    RemovePackages(RemovePackagesAction),
    UpdateSystem(UpdateSystemAction),
    Observe(ObserveAction),
    CgroupApply(CgroupApplyAction),
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CgroupApplyAction {
    pub pid: Option<u32>,
    pub unit: Option<String>,
    pub cpu_weight: Option<u64>,
    pub mem_max_bytes: Option<u64>,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObserveTool {
    Ps,
    Top,
    Journalctl,
    Perf,
    Bpftrace,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObserveAction {
    pub tool: ObserveTool,
    pub args: Vec<String>,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UpdateSystemAction {
    pub manager: PackageManager,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RemovePackagesAction {
    pub manager: PackageManager,
    pub packages: Vec<String>,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Brew,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallPackagesAction {
    pub manager: PackageManager,
    pub packages: Vec<String>,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceControlVerb {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceControlAction {
    pub action: ServiceControlVerb,
    pub unit: String,
    pub reason: String,
    pub danger: Option<String>,
    pub recovery: Option<String>,
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
    pub executed: bool,
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
    ServiceControl(ServiceControlResult),
    InstallPackages(InstallPackagesResult),
    RemovePackages(RemovePackagesResult),
    UpdateSystem(UpdateSystemResult),
    Observe(ObserveResult),
    CgroupApply(CgroupApplyResult),
    Pong(PongResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CgroupApplyResult {
    pub ok: bool,
    pub argv: Vec<String>,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObserveResult {
    pub ok: bool,
    pub argv: Vec<String>,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UpdateSystemResult {
    pub ok: bool,
    pub argv: Vec<String>,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RemovePackagesResult {
    pub ok: bool,
    pub argv: Vec<String>,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallPackagesResult {
    pub ok: bool,
    pub argv: Vec<String>,
    pub error: Option<ActionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceControlResult {
    pub ok: bool,
    pub argv: Vec<String>,
    pub error: Option<ActionError>,
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
    const MAX_VERSION_BYTES: usize = 128;
    const MAX_MODE_BYTES: usize = 128;
    const MAX_EXEC_TIMEOUT_SEC: u64 = 60;
    const MAX_SYSTEMD_UNIT_BYTES: usize = 256;
    const MAX_PACKAGE_NAME_BYTES: usize = 128;
    const MAX_PACKAGES: usize = 128;
    const MAX_OBSERVE_ARGS: usize = 64;
    const MAX_OBSERVE_ARG_BYTES: usize = 2048;

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

    if let Some(conf) = &plan.confirmation {
        if conf.token.trim().is_empty() {
            return Err(ValidationError {
                message: "confirmation.token must be non-empty".to_string(),
            });
        }
        if conf.token.as_bytes().len() > 1024 {
            return Err(ValidationError {
                message: "confirmation.token is too long".to_string(),
            });
        }
    }

    if plan.version.trim().is_empty() {
        return Err(ValidationError {
            message: "version must be non-empty".to_string(),
        });
    }
    if plan.version.as_bytes().len() > MAX_VERSION_BYTES {
        return Err(ValidationError {
            message: "version is too long".to_string(),
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
                if exec.timeout_sec > MAX_EXEC_TIMEOUT_SEC {
                    return Err(ValidationError {
                        message: "exec.timeout_sec is too large".to_string(),
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
                if let Some(danger) = &exec.danger {
                    if danger.as_bytes().len() > MAX_REASON_BYTES {
                        return Err(ValidationError {
                            message: "danger is too long".to_string(),
                        });
                    }
                }
                if let Some(recovery) = &exec.recovery {
                    if recovery.as_bytes().len() > MAX_REASON_BYTES {
                        return Err(ValidationError {
                            message: "recovery is too long".to_string(),
                        });
                    }
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
                if let Some(danger) = &read.danger {
                    if danger.as_bytes().len() > MAX_REASON_BYTES {
                        return Err(ValidationError {
                            message: "danger is too long".to_string(),
                        });
                    }
                }
                if let Some(recovery) = &read.recovery {
                    if recovery.as_bytes().len() > MAX_REASON_BYTES {
                        return Err(ValidationError {
                            message: "recovery is too long".to_string(),
                        });
                    }
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
                if write.mode.as_bytes().len() > MAX_MODE_BYTES {
                    return Err(ValidationError {
                        message: "write_file.mode is too long".to_string(),
                    });
                }
                if !is_octal_mode(&write.mode) {
                    return Err(ValidationError {
                        message: "write_file.mode is invalid".to_string(),
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
                if let Some(danger) = &write.danger {
                    if danger.as_bytes().len() > MAX_REASON_BYTES {
                        return Err(ValidationError {
                            message: "danger is too long".to_string(),
                        });
                    }
                }
                if let Some(recovery) = &write.recovery {
                    if recovery.as_bytes().len() > MAX_REASON_BYTES {
                        return Err(ValidationError {
                            message: "recovery is too long".to_string(),
                        });
                    }
                }

                if write.danger.is_some() {
                    require_confirmation(plan, "write_file requires confirmation when danger is set")?;
                }
            }
            Action::ServiceControl(svc) => {
                if svc.unit.trim().is_empty() {
                    return Err(ValidationError {
                        message: "service_control.unit must be non-empty".to_string(),
                    });
                }
                if svc.unit.as_bytes().len() > MAX_SYSTEMD_UNIT_BYTES {
                    return Err(ValidationError {
                        message: "service_control.unit is too long".to_string(),
                    });
                }
                if svc.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "service_control.reason must be non-empty".to_string(),
                    });
                }
                if svc.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "service_control.reason is too long".to_string(),
                    });
                }
            }
            Action::InstallPackages(pkgs) => {
                if pkgs.packages.is_empty() {
                    return Err(ValidationError {
                        message: "install_packages.packages must be non-empty".to_string(),
                    });
                }
                if pkgs.packages.len() > MAX_PACKAGES {
                    return Err(ValidationError {
                        message: "install_packages.packages has too many entries".to_string(),
                    });
                }
                for pkg in &pkgs.packages {
                    if pkg.trim().is_empty() {
                        return Err(ValidationError {
                            message: "install_packages.packages entries must be non-empty".to_string(),
                        });
                    }
                    if pkg.as_bytes().len() > MAX_PACKAGE_NAME_BYTES {
                        return Err(ValidationError {
                            message: "install_packages.packages entry is too long".to_string(),
                        });
                    }
                }
                if pkgs.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "install_packages.reason must be non-empty".to_string(),
                    });
                }
                if pkgs.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "install_packages.reason is too long".to_string(),
                    });
                }
            }
            Action::RemovePackages(pkgs) => {
                if pkgs.packages.is_empty() {
                    return Err(ValidationError {
                        message: "remove_packages.packages must be non-empty".to_string(),
                    });
                }
                if pkgs.packages.len() > MAX_PACKAGES {
                    return Err(ValidationError {
                        message: "remove_packages.packages has too many entries".to_string(),
                    });
                }
                for pkg in &pkgs.packages {
                    if pkg.trim().is_empty() {
                        return Err(ValidationError {
                            message: "remove_packages.packages entries must be non-empty".to_string(),
                        });
                    }
                    if pkg.as_bytes().len() > MAX_PACKAGE_NAME_BYTES {
                        return Err(ValidationError {
                            message: "remove_packages.packages entry is too long".to_string(),
                        });
                    }
                }
                if pkgs.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "remove_packages.reason must be non-empty".to_string(),
                    });
                }
                if pkgs.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "remove_packages.reason is too long".to_string(),
                    });
                }
            }
            Action::UpdateSystem(upd) => {
                if upd.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "update_system.reason must be non-empty".to_string(),
                    });
                }
                if upd.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "update_system.reason is too long".to_string(),
                    });
                }
            }
            Action::Observe(obs) => {
                if obs.args.len() > MAX_OBSERVE_ARGS {
                    return Err(ValidationError {
                        message: "observe.args has too many entries".to_string(),
                    });
                }
                for arg in &obs.args {
                    if arg.trim().is_empty() {
                        return Err(ValidationError {
                            message: "observe.args entries must be non-empty".to_string(),
                        });
                    }
                    if arg.as_bytes().len() > MAX_OBSERVE_ARG_BYTES {
                        return Err(ValidationError {
                            message: "observe.args entry is too long".to_string(),
                        });
                    }
                }
                if obs.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "observe.reason must be non-empty".to_string(),
                    });
                }
                if obs.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "observe.reason is too long".to_string(),
                    });
                }
            }
            Action::CgroupApply(cg) => {
                if cg.pid.is_none() && cg.unit.is_none() {
                    return Err(ValidationError {
                        message: "cgroup_apply requires pid or unit".to_string(),
                    });
                }
                if cg.pid.is_some() && cg.unit.is_some() {
                    return Err(ValidationError {
                        message: "cgroup_apply must not set both pid and unit".to_string(),
                    });
                }
                if let Some(unit) = &cg.unit {
                    if unit.trim().is_empty() {
                        return Err(ValidationError {
                            message: "cgroup_apply.unit must be non-empty when provided".to_string(),
                        });
                    }
                    if unit.as_bytes().len() > MAX_SYSTEMD_UNIT_BYTES {
                        return Err(ValidationError {
                            message: "cgroup_apply.unit is too long".to_string(),
                        });
                    }
                }
                if cg.cpu_weight.is_none() && cg.mem_max_bytes.is_none() {
                    return Err(ValidationError {
                        message: "cgroup_apply requires at least one setting".to_string(),
                    });
                }
                if cg.reason.trim().is_empty() {
                    return Err(ValidationError {
                        message: "cgroup_apply.reason must be non-empty".to_string(),
                    });
                }
                if cg.reason.as_bytes().len() > MAX_REASON_BYTES {
                    return Err(ValidationError {
                        message: "cgroup_apply.reason is too long".to_string(),
                    });
                }
            }
            Action::Ping => {}
        }
    }

    Ok(())
}

fn is_octal_mode(mode: &str) -> bool {
    let mode = mode.trim();
    let mode = mode.strip_prefix("0o").unwrap_or(mode);
    let bytes = mode.as_bytes();
    if bytes.len() != 3 && bytes.len() != 4 {
        return false;
    }
    bytes.iter().all(|b| (*b >= b'0') && (*b <= b'7'))
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

    #[test]
    fn validate_rejects_version_too_long() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "a".repeat(129),
            mode: Mode::Execute,
            actions: vec![Action::Ping],
            confirmation: None,
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "version is too long");
    }

    #[test]
    fn validate_rejects_confirmation_token_too_long() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Ping],
            confirmation: Some(Confirmation {
                token: "a".repeat(1025),
            }),
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "confirmation.token is too long");
    }

    #[test]
    fn validate_rejects_danger_too_long() {
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
                danger: Some("a".repeat(2049)),
                recovery: None,
            })],
            confirmation: Some(Confirmation {
                token: "i-understand".to_string(),
            }),
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "danger is too long");
    }

    #[test]
    fn validate_rejects_recovery_too_long() {
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
                recovery: Some("a".repeat(2049)),
            })],
            confirmation: Some(Confirmation {
                token: "i-understand".to_string(),
            }),
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "recovery is too long");
    }

    #[test]
    fn validate_rejects_write_file_mode_too_long() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::WriteFile(WriteFileAction {
                path: "./out.txt".to_string(),
                content: "x".to_string(),
                mode: "a".repeat(129),
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };
        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "write_file.mode is too long");
    }

    #[test]
    fn validate_rejects_exec_timeout_too_large() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::Exec(ExecAction {
                argv: vec!["/bin/echo".to_string(), "hi".to_string()],
                cwd: None,
                env: None,
                timeout_sec: 61,
                as_root: false,
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "exec.timeout_sec is too large");
    }

    #[test]
    fn validate_rejects_write_file_mode_format() {
        let plan = ActionPlan {
            request_id: "req-1".to_string(),
            session_id: None,
            version: "0.1".to_string(),
            mode: Mode::Execute,
            actions: vec![Action::WriteFile(WriteFileAction {
                path: "./out.txt".to_string(),
                content: "x".to_string(),
                mode: "not-octal".to_string(),
                reason: "test".to_string(),
                danger: None,
                recovery: None,
            })],
            confirmation: None,
        };

        let err = validate_action_plan(&plan).unwrap_err();
        assert_eq!(err.message, "write_file.mode is invalid");
    }
}


