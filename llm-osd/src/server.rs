// ABOUTME: hosts the unix socket server for receiving action plans and returning structured results.
// ABOUTME: enforces strict parsing, validation, policy checks, and audit logging.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use llm_os_common::{
    parse_action_plan, validate_action_plan, Action, ActionPlanResult, ActionResult, ErrorCode,
    Mode, RequestError,
};
use std::os::unix::io::AsRawFd;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::actions;
use crate::audit;
use crate::policy;

const MAX_REQUEST_BYTES: usize = 256 * 1024;
#[cfg(test)]
const READ_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
#[cfg(not(test))]
const READ_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

pub async fn run(socket_path: &str, audit_path: &str, confirm_token: &str) -> anyhow::Result<()> {
    if Path::new(socket_path).exists() {
        tokio::fs::remove_file(socket_path)
            .await
            .with_context(|| format!("remove existing socket at {socket_path}"))?;
    }

    let listener = UnixListener::bind(socket_path).with_context(|| format!("bind {socket_path}"))?;

    loop {
        let (stream, _addr) = listener.accept().await?;
        let audit_path = audit_path.to_string();
        let confirm_token = confirm_token.to_string();
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, &audit_path, &confirm_token).await {
                let _ = err;
            }
        });
    }
}

async fn handle_client(mut stream: UnixStream, audit_path: &str, confirm_token: &str) -> anyhow::Result<()> {
    let peer = peer_credentials(&stream);

    let mut input = Vec::new();
    let mut buf = [0u8; 4096];
    let mut exceeded = false;
    let mut idle = false;
    loop {
        let n = match tokio::time::timeout(READ_IDLE_TIMEOUT, stream.read(&mut buf)).await {
            Ok(res) => res?,
            Err(_) => {
                idle = true;
                break;
            }
        };
        if n == 0 {
            break;
        }
        if exceeded {
            continue;
        }
        if input.len() + n > MAX_REQUEST_BYTES {
            exceeded = true;
            continue;
        }
        input.extend_from_slice(&buf[..n]);
    }

    if exceeded {
        let _ = write_request_error(
            &mut stream,
            "unknown",
            ErrorCode::RequestTooLarge,
            "request exceeds max bytes",
        )
        .await;
        return Ok(());
    }

    if idle && input.is_empty() {
        let _ = write_request_error(
            &mut stream,
            "unknown",
            ErrorCode::ParseFailed,
            "read timed out",
        )
        .await;
        return Ok(());
    }

    let input_str = String::from_utf8_lossy(&input);
    let plan = match parse_action_plan(&input_str) {
        Ok(p) => p,
        Err(err) => {
            let _ = write_request_error(
                &mut stream,
                "unknown",
                ErrorCode::ParseFailed,
                &format!("parse failed: {err}"),
            )
            .await;
            return Ok(());
        }
    };

    if let Err(err) = validate_action_plan(&plan) {
        let _ = write_request_error(
            &mut stream,
            &plan.request_id,
            ErrorCode::ValidationFailed,
            &format!("validation failed: {}", err.message),
        )
        .await;
        return Ok(());
    }

    let confirmation_token = plan.confirmation.as_ref().map(|c| c.token.as_str());

    let mut results = Vec::with_capacity(plan.actions.len());
    for action in &plan.actions {
        let result = match plan.mode {
            Mode::Execute => execute_action(action, confirmation_token, confirm_token).await,
            Mode::PlanOnly => plan_action(action, confirmation_token, confirm_token).await,
        };
        results.push(result);
    }

    let response = ActionPlanResult {
        request_id: plan.request_id.clone(),
        executed: plan.mode == Mode::Execute,
        results,
        error: None,
    };
    let response_json = serde_json::to_vec(&response)?;
    stream.write_all(&response_json).await?;
    stream.shutdown().await?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    audit::append_record(audit_path, now_ms, peer, &plan, &response).await?;

    Ok(())
}

fn peer_credentials(stream: &UnixStream) -> Option<audit::PeerCredentials> {
    let fd = stream.as_raw_fd();

    let mut ucred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut ucred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return None;
    }
    if len as usize != std::mem::size_of::<libc::ucred>() {
        return None;
    }

    Some(audit::PeerCredentials {
        pid: ucred.pid,
        uid: ucred.uid,
        gid: ucred.gid,
    })
}

async fn write_request_error(
    stream: &mut UnixStream,
    request_id: &str,
    code: ErrorCode,
    message: &str,
) -> anyhow::Result<()> {
    let response = ActionPlanResult {
        request_id: request_id.to_string(),
        executed: false,
        results: vec![],
        error: Some(RequestError {
            code,
            message: message.to_string(),
        }),
    };
    let response_json = serde_json::to_vec(&response)?;
    stream.write_all(&response_json).await?;
    let _ = stream.shutdown().await;
    Ok(())
}

async fn execute_action(
    action: &Action,
    confirmation_token: Option<&str>,
    confirm_token: &str,
) -> ActionResult {
    match action {
        Action::Exec(exec) => {
            if policy::is_exec_denied(exec) {
                return ActionResult::Exec(llm_os_common::ExecResult {
                    ok: false,
                    exit_code: None,
                    stdout: "".to_string(),
                    stdout_truncated: false,
                    stderr: "".to_string(),
                    stderr_truncated: false,
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::PolicyDenied,
                        message: "exec denied by policy".to_string(),
                    }),
                });
            }
            if policy::exec_requires_confirmation(exec)
                && !policy::confirmation_is_valid(confirmation_token, confirm_token)
            {
                return ActionResult::Exec(llm_os_common::ExecResult {
                    ok: false,
                    exit_code: None,
                    stdout: "".to_string(),
                    stdout_truncated: false,
                    stderr: "".to_string(),
                    stderr_truncated: false,
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::ConfirmationRequired,
                        message: "confirmation required".to_string(),
                    }),
                });
            }
            actions::exec::run(exec).await
        }
        Action::ReadFile(read) => {
            if policy::path_requires_confirmation(&read.path)
                && !policy::confirmation_is_valid(confirmation_token, confirm_token)
            {
                return ActionResult::ReadFile(llm_os_common::ReadFileResult {
                    ok: false,
                    content_base64: None,
                    truncated: false,
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::ConfirmationRequired,
                        message: "confirmation required".to_string(),
                    }),
                });
            }
            actions::files::read(read).await
        }
        Action::WriteFile(write) => {
            if policy::path_requires_confirmation(&write.path)
                && !policy::confirmation_is_valid(confirmation_token, confirm_token)
            {
                return ActionResult::WriteFile(llm_os_common::WriteFileResult {
                    ok: false,
                    artifacts: vec![],
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::ConfirmationRequired,
                        message: "confirmation required".to_string(),
                    }),
                });
            }
            actions::files::write(write).await
        }
        Action::ServiceControl(_svc) => ActionResult::ServiceControl(llm_os_common::ServiceControlResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "service_control is not supported in execute mode".to_string(),
            }),
        }),
        Action::InstallPackages(_pkgs) => ActionResult::InstallPackages(llm_os_common::InstallPackagesResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "install_packages is not supported in execute mode".to_string(),
            }),
        }),
        Action::RemovePackages(_pkgs) => ActionResult::RemovePackages(llm_os_common::RemovePackagesResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "remove_packages is not supported in execute mode".to_string(),
            }),
        }),
        Action::UpdateSystem(_upd) => ActionResult::UpdateSystem(llm_os_common::UpdateSystemResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "update_system is not supported in execute mode".to_string(),
            }),
        }),
        Action::Observe(_obs) => ActionResult::Observe(llm_os_common::ObserveResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "observe is not supported in execute mode".to_string(),
            }),
        }),
        Action::CgroupApply(_cg) => ActionResult::CgroupApply(llm_os_common::CgroupApplyResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "cgroup_apply is not supported in execute mode".to_string(),
            }),
        }),
        Action::FirmwareOp(_fw) => ActionResult::FirmwareOp(llm_os_common::FirmwareOpResult {
            ok: false,
            argv: vec![],
            error: Some(llm_os_common::ActionError {
                code: llm_os_common::ActionErrorCode::PolicyDenied,
                message: "firmware_op is not supported in execute mode".to_string(),
            }),
        }),
        Action::Ping => ActionResult::Pong(llm_os_common::PongResult { ok: true }),
    }
}

async fn plan_action(action: &Action, confirmation_token: Option<&str>, confirm_token: &str) -> ActionResult {
    match action {
        Action::Exec(exec) => {
            if policy::is_exec_denied(exec) {
                return ActionResult::Exec(llm_os_common::ExecResult {
                    ok: false,
                    exit_code: None,
                    stdout: "".to_string(),
                    stdout_truncated: false,
                    stderr: "".to_string(),
                    stderr_truncated: false,
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::PolicyDenied,
                        message: "exec denied by policy".to_string(),
                    }),
                });
            }
            if policy::exec_requires_confirmation(exec)
                && !policy::confirmation_is_valid(confirmation_token, confirm_token)
            {
                return ActionResult::Exec(llm_os_common::ExecResult {
                    ok: false,
                    exit_code: None,
                    stdout: "".to_string(),
                    stdout_truncated: false,
                    stderr: "".to_string(),
                    stderr_truncated: false,
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::ConfirmationRequired,
                        message: "confirmation required".to_string(),
                    }),
                });
            }
            ActionResult::Exec(llm_os_common::ExecResult {
                ok: true,
                exit_code: None,
                stdout: "".to_string(),
                stdout_truncated: false,
                stderr: "".to_string(),
                stderr_truncated: false,
                error: None,
            })
        }
        Action::ReadFile(read) => {
            if policy::path_requires_confirmation(&read.path)
                && !policy::confirmation_is_valid(confirmation_token, confirm_token)
            {
                return ActionResult::ReadFile(llm_os_common::ReadFileResult {
                    ok: false,
                    content_base64: None,
                    truncated: false,
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::ConfirmationRequired,
                        message: "confirmation required".to_string(),
                    }),
                });
            }
            ActionResult::ReadFile(llm_os_common::ReadFileResult {
                ok: true,
                content_base64: None,
                truncated: false,
                error: None,
            })
        }
        Action::WriteFile(write) => {
            if policy::path_requires_confirmation(&write.path)
                && !policy::confirmation_is_valid(confirmation_token, confirm_token)
            {
                return ActionResult::WriteFile(llm_os_common::WriteFileResult {
                    ok: false,
                    artifacts: vec![],
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::ConfirmationRequired,
                        message: "confirmation required".to_string(),
                    }),
                });
            }
            ActionResult::WriteFile(llm_os_common::WriteFileResult {
                ok: true,
                artifacts: vec![],
                error: None,
            })
        }
        Action::ServiceControl(svc) => {
            let verb = match svc.action {
                llm_os_common::ServiceControlVerb::Start => "start",
                llm_os_common::ServiceControlVerb::Stop => "stop",
                llm_os_common::ServiceControlVerb::Restart => "restart",
                llm_os_common::ServiceControlVerb::Enable => "enable",
                llm_os_common::ServiceControlVerb::Disable => "disable",
                llm_os_common::ServiceControlVerb::Status => "status",
            };
            ActionResult::ServiceControl(llm_os_common::ServiceControlResult {
                ok: true,
                argv: vec!["systemctl".to_string(), verb.to_string(), svc.unit.clone()],
                error: None,
            })
        }
        Action::InstallPackages(pkgs) => {
            let mut argv = Vec::new();
            match pkgs.manager {
                llm_os_common::PackageManager::Apt => {
                    argv.push("apt-get".to_string());
                    argv.push("install".to_string());
                    argv.push("-y".to_string());
                }
                llm_os_common::PackageManager::Dnf => {
                    argv.push("dnf".to_string());
                    argv.push("install".to_string());
                    argv.push("-y".to_string());
                }
                llm_os_common::PackageManager::Pacman => {
                    argv.push("pacman".to_string());
                    argv.push("-S".to_string());
                    argv.push("--noconfirm".to_string());
                }
                llm_os_common::PackageManager::Zypper => {
                    argv.push("zypper".to_string());
                    argv.push("install".to_string());
                    argv.push("-y".to_string());
                }
                llm_os_common::PackageManager::Brew => {
                    argv.push("brew".to_string());
                    argv.push("install".to_string());
                }
                llm_os_common::PackageManager::Other => {
                    return ActionResult::InstallPackages(llm_os_common::InstallPackagesResult {
                        ok: false,
                        argv: vec![],
                        error: Some(llm_os_common::ActionError {
                            code: llm_os_common::ActionErrorCode::PolicyDenied,
                            message: "install_packages manager not supported".to_string(),
                        }),
                    });
                }
            }
            argv.extend(pkgs.packages.iter().cloned());

            ActionResult::InstallPackages(llm_os_common::InstallPackagesResult {
                ok: true,
                argv,
                error: None,
            })
        }
        Action::RemovePackages(pkgs) => {
            let mut argv = Vec::new();
            match pkgs.manager {
                llm_os_common::PackageManager::Apt => {
                    argv.push("apt-get".to_string());
                    argv.push("remove".to_string());
                    argv.push("-y".to_string());
                }
                llm_os_common::PackageManager::Dnf => {
                    argv.push("dnf".to_string());
                    argv.push("remove".to_string());
                    argv.push("-y".to_string());
                }
                llm_os_common::PackageManager::Pacman => {
                    argv.push("pacman".to_string());
                    argv.push("-R".to_string());
                    argv.push("--noconfirm".to_string());
                }
                llm_os_common::PackageManager::Zypper => {
                    argv.push("zypper".to_string());
                    argv.push("remove".to_string());
                    argv.push("-y".to_string());
                }
                llm_os_common::PackageManager::Brew => {
                    argv.push("brew".to_string());
                    argv.push("uninstall".to_string());
                }
                llm_os_common::PackageManager::Other => {
                    return ActionResult::RemovePackages(llm_os_common::RemovePackagesResult {
                        ok: false,
                        argv: vec![],
                        error: Some(llm_os_common::ActionError {
                            code: llm_os_common::ActionErrorCode::PolicyDenied,
                            message: "remove_packages manager not supported".to_string(),
                        }),
                    });
                }
            }
            argv.extend(pkgs.packages.iter().cloned());

            ActionResult::RemovePackages(llm_os_common::RemovePackagesResult {
                ok: true,
                argv,
                error: None,
            })
        }
        Action::UpdateSystem(upd) => {
            match upd.manager {
                llm_os_common::PackageManager::Apt => ActionResult::UpdateSystem(llm_os_common::UpdateSystemResult {
                    ok: true,
                    argv: vec![
                        "apt-get".to_string(),
                        "update".to_string(),
                        "&&".to_string(),
                        "apt-get".to_string(),
                        "upgrade".to_string(),
                        "-y".to_string(),
                    ],
                    error: None,
                }),
                _ => ActionResult::UpdateSystem(llm_os_common::UpdateSystemResult {
                    ok: false,
                    argv: vec![],
                    error: Some(llm_os_common::ActionError {
                        code: llm_os_common::ActionErrorCode::PolicyDenied,
                        message: "update_system manager not supported".to_string(),
                    }),
                }),
            }
        }
        Action::Observe(obs) => {
            let base = match obs.tool {
                llm_os_common::ObserveTool::Ps => "ps",
                llm_os_common::ObserveTool::Top => "top",
                llm_os_common::ObserveTool::Journalctl => "journalctl",
                llm_os_common::ObserveTool::Perf => "perf",
                llm_os_common::ObserveTool::Bpftrace => "bpftrace",
                llm_os_common::ObserveTool::Other => {
                    return ActionResult::Observe(llm_os_common::ObserveResult {
                        ok: false,
                        argv: vec![],
                        error: Some(llm_os_common::ActionError {
                            code: llm_os_common::ActionErrorCode::PolicyDenied,
                            message: "observe tool not supported".to_string(),
                        }),
                    });
                }
            };

            let mut argv = Vec::new();
            argv.push(base.to_string());
            argv.extend(obs.args.iter().cloned());

            ActionResult::Observe(llm_os_common::ObserveResult {
                ok: true,
                argv,
                error: None,
            })
        }
        Action::CgroupApply(cg) => {
            let mut argv = Vec::new();
            argv.push("systemd-run".to_string());
            argv.push("--scope".to_string());
            if let Some(w) = cg.cpu_weight {
                argv.push("-p".to_string());
                argv.push(format!("CPUWeight={w}"));
            }
            if let Some(m) = cg.mem_max_bytes {
                argv.push("-p".to_string());
                argv.push(format!("MemoryMax={m}"));
            }
            if let Some(pid) = cg.pid {
                argv.push(format!("--pid={pid}"));
                return ActionResult::CgroupApply(llm_os_common::CgroupApplyResult {
                    ok: true,
                    argv,
                    error: None,
                });
            }
            if let Some(unit) = &cg.unit {
                argv.push(format!("--unit={unit}"));
                return ActionResult::CgroupApply(llm_os_common::CgroupApplyResult {
                    ok: true,
                    argv,
                    error: None,
                });
            }
            ActionResult::CgroupApply(llm_os_common::CgroupApplyResult {
                ok: false,
                argv: vec![],
                error: Some(llm_os_common::ActionError {
                    code: llm_os_common::ActionErrorCode::PolicyDenied,
                    message: "cgroup_apply target is invalid".to_string(),
                }),
            })
        }
        Action::FirmwareOp(fw) => {
            let argv = match fw.op {
                llm_os_common::FirmwareOp::Inventory => vec!["dmidecode".to_string()],
                llm_os_common::FirmwareOp::FwupdUpdate => vec!["fwupdmgr".to_string(), "update".to_string()],
                llm_os_common::FirmwareOp::UefiVarRead => {
                    let name = fw.uefi_var_name.as_deref().unwrap_or("");
                    if name.trim().is_empty() {
                        return ActionResult::FirmwareOp(llm_os_common::FirmwareOpResult {
                            ok: false,
                            argv: vec![],
                            error: Some(llm_os_common::ActionError {
                                code: llm_os_common::ActionErrorCode::PolicyDenied,
                                message: "firmware_op target is invalid".to_string(),
                            }),
                        });
                    }
                    vec![
                        "cat".to_string(),
                        format!("/sys/firmware/efi/efivars/{name}"),
                    ]
                }
            };

            ActionResult::FirmwareOp(llm_os_common::FirmwareOpResult {
                ok: true,
                argv,
                error: None,
            })
        }
        Action::Ping => ActionResult::Pong(llm_os_common::PongResult { ok: true }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn server_exec_echo_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let plan = r#"{
          "request_id":"req-echo-1",
          "session_id":"sess-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"exec","argv":["/bin/echo","hi"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]
        }"#;

        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.request_id, "req-echo-1");

        match &response.results[0] {
            ActionResult::Exec(exec) => {
                assert!(exec.ok);
                assert!(exec.stdout.contains("hi"));
            }
            _ => panic!("unexpected action result type"),
        }

        for _ in 0..50u32 {
            if let Ok(meta) = tokio::fs::metadata(&audit_path).await {
                if meta.len() > 0 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let audit_bytes = tokio::fs::read(&audit_path).await.unwrap();
        let audit_text = std::str::from_utf8(&audit_bytes).unwrap();
        let first_line = audit_text.lines().find(|l| !l.trim().is_empty()).unwrap();
        let v: serde_json::Value = serde_json::from_str(first_line).unwrap();
        assert_eq!(v["request_id"], "req-echo-1");
        assert_eq!(v["session_id"], "sess-1");

        server.abort();
    }

    #[tokio::test]
    async fn audit_includes_peer_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-peer-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"ping"}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert!(response.error.is_none());

        for _ in 0..50u32 {
            if let Ok(meta) = tokio::fs::metadata(&audit_path).await {
                if meta.len() > 0 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let audit_text = tokio::fs::read_to_string(&audit_path).await.unwrap();
        let first_line = audit_text.lines().find(|l| !l.trim().is_empty()).unwrap();
        let v: serde_json::Value = serde_json::from_str(first_line).unwrap();

        assert!(v["peer"]["pid"].is_number());
        assert!(v["peer"]["uid"].is_number());
        assert!(v["peer"]["gid"].is_number());
        assert_eq!(v["peer"]["pid"].as_u64().unwrap(), std::process::id() as u64);

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_write_file_has_no_side_effects() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");
        let out_path = dir.path().join("plan-only.txt");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = format!(
            r#"{{
              "request_id":"req-plan-only-1",
              "version":"0.1",
              "mode":"plan_only",
              "actions":[{{"type":"write_file","path":"{}","content":"hi","mode":"0644","reason":"test","danger":null,"recovery":null}}]
            }}"#,
            out_path.to_string_lossy()
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        match &response.results[0] {
            ActionResult::WriteFile(w) => assert!(w.ok),
            _ => panic!("unexpected action result type"),
        }

        assert!(!out_path.exists());

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_service_control_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-svc-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"service_control","action":"status","unit":"ssh.service","reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-svc-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::ServiceControl(r) => {
                assert!(r.ok);
                assert_eq!(r.argv, vec!["systemctl", "status", "ssh.service"]);
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_install_packages_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-pkg-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"install_packages","manager":"apt","packages":["curl","git"],"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-pkg-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::InstallPackages(r) => {
                assert!(r.ok);
                assert_eq!(
                    r.argv,
                    vec![
                        "apt-get",
                        "install",
                        "-y",
                        "curl",
                        "git"
                    ]
                );
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_remove_packages_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-rmpkg-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"remove_packages","manager":"apt","packages":["curl","git"],"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-rmpkg-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::RemovePackages(r) => {
                assert!(r.ok);
                assert_eq!(r.argv, vec!["apt-get", "remove", "-y", "curl", "git"]);
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_update_system_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-upd-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"update_system","manager":"apt","reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-upd-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::UpdateSystem(r) => {
                assert!(r.ok);
                assert_eq!(
                    r.argv,
                    vec!["apt-get", "update", "&&", "apt-get", "upgrade", "-y"]
                );
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_observe_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-obs-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"observe","tool":"ps","args":["aux"],"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-obs-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::Observe(r) => {
                assert!(r.ok);
                assert_eq!(r.argv, vec!["ps", "aux"]);
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_cgroup_apply_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-cg-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"cgroup_apply","pid":1234,"unit":null,"cpu_weight":100,"mem_max_bytes":1048576,"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-cg-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::CgroupApply(r) => {
                assert!(r.ok);
                assert_eq!(
                    r.argv,
                    vec![
                        "systemd-run",
                        "--scope",
                        "-p",
                        "CPUWeight=100",
                        "-p",
                        "MemoryMax=1048576",
                        "--pid=1234"
                    ]
                );
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_plan_only_firmware_op_returns_structured_result() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-plan-only-fw-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[{"type":"firmware_op","op":"inventory","uefi_var_name":null,"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-plan-only-fw-1");
        assert!(response.error.is_none());
        assert!(!response.executed);
        assert_eq!(response.results.len(), 1);
        match &response.results[0] {
            ActionResult::FirmwareOp(r) => {
                assert!(r.ok);
                assert_eq!(r.argv, vec!["dmidecode"]);
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_ping_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-ping-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"ping"}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-ping-1");
        assert!(response.error.is_none());
        assert_eq!(response.results.len(), 1);

        match &response.results[0] {
            ActionResult::Pong(p) => assert!(p.ok),
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_returns_parse_failed_for_incomplete_json_without_close() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream
            .write_all(b"{\"request_id\":\"req-timeout-1\"")
            .await
            .unwrap();

        let mut out = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(2), stream.read_to_end(&mut out))
            .await
            .unwrap()
            .unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            response.error.as_ref().unwrap().code,
            llm_os_common::ErrorCode::ParseFailed
        );

        server.abort();
    }

    #[tokio::test]
    async fn server_allows_complete_json_without_close() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"req-idle-ping-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"ping"}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();

        let mut out = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(2), stream.read_to_end(&mut out))
            .await
            .unwrap()
            .unwrap();

        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(response.request_id, "req-idle-ping-1");
        assert!(response.error.is_none());
        match &response.results[0] {
            ActionResult::Pong(p) => assert!(p.ok),
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_exec_non_allowlisted_requires_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan_without = r#"{
          "request_id":"req-true-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"exec","argv":["/usr/bin/true"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_without.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();

        match &response.results[0] {
            ActionResult::Exec(exec) => {
                assert!(!exec.ok);
                assert_eq!(
                    exec.error.as_ref().unwrap().code,
                    llm_os_common::ActionErrorCode::ConfirmationRequired
                );
            }
            _ => panic!("unexpected action result type"),
        }

        let plan_with = format!(
            r#"{{
              "request_id":"req-true-2",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"exec","argv":["/usr/bin/true"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}}],
              "confirmation":{{"token":"{}"}}
            }}"#,
            policy::confirmation_token_hint("i-understand")
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_with.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();

        match &response.results[0] {
            ActionResult::Exec(exec) => assert!(exec.ok),
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_rejects_oversized_request_with_json_error() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let big = "a".repeat(300 * 1024);
        let plan = format!(
            r#"{{
              "request_id":"req-big-1",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"exec","argv":["/bin/echo","{}"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}}]
            }}"#,
            big
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            response.error.as_ref().unwrap().code,
            llm_os_common::ErrorCode::RequestTooLarge
        );

        server.abort();
    }

    #[tokio::test]
    async fn server_allows_write_file_near_max_content_under_request_cap() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");
        let out_path = dir.path().join("out.txt");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let content = "a".repeat(64 * 1024);
        let plan = format!(
            r#"{{
              "request_id":"req-write-big-1",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"write_file","path":"{}","content":"{}","mode":"0644","reason":"test","danger":null,"recovery":null}}]
            }}"#,
            out_path.to_string_lossy(),
            content
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert!(response.error.is_none());
        match &response.results[0] {
            ActionResult::WriteFile(w) => assert!(w.ok),
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_returns_json_error_for_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(b"{ not json").await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            response.error.as_ref().unwrap().code,
            llm_os_common::ErrorCode::ParseFailed
        );

        server.abort();
    }

    #[tokio::test]
    async fn server_returns_json_error_for_validation_failure() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan = r#"{
          "request_id":"   ",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"exec","argv":["/bin/echo","hi"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            response.error.as_ref().unwrap().code,
            llm_os_common::ErrorCode::ValidationFailed
        );

        server.abort();
    }

    #[tokio::test]
    async fn server_exec_rm_requires_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");
        let file_path = dir.path().join("deleteme.txt");
        tokio::fs::write(&file_path, "x").await.unwrap();

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan_without = format!(
            r#"{{
              "request_id":"req-rm-1",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"exec","argv":["/bin/rm","{}"],"cwd":"{}","env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}}]
            }}"#,
            file_path.file_name().unwrap().to_string_lossy(),
            dir.path().to_string_lossy()
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_without.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::Exec(exec) => {
                assert!(!exec.ok);
                assert_eq!(
                    exec.error.as_ref().unwrap().code,
                    llm_os_common::ActionErrorCode::ConfirmationRequired
                );
            }
            _ => panic!("unexpected action result type"),
        }

        let plan_with = format!(
            r#"{{
              "request_id":"req-rm-2",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"exec","argv":["/bin/rm","{}"],"cwd":"{}","env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}}],
              "confirmation":{{"token":"{}"}}
            }}"#,
            file_path.file_name().unwrap().to_string_lossy(),
            dir.path().to_string_lossy(),
            policy::confirmation_token_hint("i-understand")
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_with.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::Exec(exec) => assert!(exec.ok),
            _ => panic!("unexpected action result type"),
        }

        for _ in 0..50u32 {
            if let Ok(meta) = tokio::fs::metadata(&audit_path).await {
                if meta.len() > 0 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let audit_text = tokio::fs::read_to_string(&audit_path).await.unwrap();
        assert!(!audit_text.contains("i-understand"));

        assert!(!file_path.exists());

        server.abort();
    }

    #[tokio::test]
    async fn server_exec_non_default_confirmation_token() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "custom-token").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan_bad = r#"{
          "request_id":"req-ct-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"exec","argv":["/usr/bin/true"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}],
          "confirmation":{"token":"i-understand"}
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_bad.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::Exec(exec) => {
                assert!(!exec.ok);
                assert_eq!(
                    exec.error.as_ref().unwrap().code,
                    llm_os_common::ActionErrorCode::ConfirmationRequired
                );
            }
            _ => panic!("unexpected action result type"),
        }

        let plan_good = r#"{
          "request_id":"req-ct-2",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"exec","argv":["/usr/bin/true"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}],
          "confirmation":{"token":"custom-token"}
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_good.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::Exec(exec) => assert!(exec.ok),
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn audit_redacts_write_file_content() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");
        let out_path = dir.path().join("secret.txt");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let secret = "super-secret-token";
        let plan = format!(
            r#"{{
              "request_id":"req-write-secret-1",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"write_file","path":"{}","content":"{}","mode":"0644","reason":"test","danger":null,"recovery":null}}]
            }}"#,
            out_path.to_string_lossy(),
            secret
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        assert!(response.error.is_none());
        assert_eq!(response.results.len(), 1);

        for _ in 0..50u32 {
            if let Ok(meta) = tokio::fs::metadata(&audit_path).await {
                if meta.len() > 0 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let audit_text = tokio::fs::read_to_string(&audit_path).await.unwrap();
        assert!(!audit_text.contains(secret));

        server.abort();
    }

    #[tokio::test]
    async fn server_read_file_absolute_path_requires_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan_without = r#"{
          "request_id":"req-abs-read-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"read_file","path":"/etc/passwd","max_bytes":256,"reason":"test","danger":null,"recovery":null}]
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_without.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::ReadFile(r) => {
                assert!(!r.ok);
                assert_eq!(
                    r.error.as_ref().unwrap().code,
                    llm_os_common::ActionErrorCode::ConfirmationRequired
                );
            }
            _ => panic!("unexpected action result type"),
        }

        let plan_with = r#"{
          "request_id":"req-abs-read-2",
          "version":"0.1",
          "mode":"execute",
          "actions":[{"type":"read_file","path":"/etc/passwd","max_bytes":256,"reason":"test","danger":null,"recovery":null}],
          "confirmation":{"token":"i-understand"}
        }"#;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_with.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::ReadFile(r) => {
                assert!(r.ok);
                assert!(r.content_base64.is_some());
            }
            _ => panic!("unexpected action result type"),
        }

        server.abort();
    }

    #[tokio::test]
    async fn server_write_file_parent_dir_requires_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("llm-osd.sock");
        let audit_path = dir.path().join("audit.jsonl");

        let base = dir.path().join("base");
        tokio::fs::create_dir_all(&base).await.unwrap();
        tokio::fs::create_dir_all(base.join("sub")).await.unwrap();
        let out_path = base.join("sub").join("..").join("secret.txt");

        let socket_path_str = socket_path.to_string_lossy().to_string();
        let audit_path_str = audit_path.to_string_lossy().to_string();

        let server =
            tokio::spawn(async move { run(&socket_path_str, &audit_path_str, "i-understand").await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan_without = format!(
            r#"{{
              "request_id":"req-parentdir-write-1",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"write_file","path":"{}","content":"x","mode":"0644","reason":"test","danger":null,"recovery":null}}]
            }}"#,
            out_path.to_string_lossy()
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_without.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::WriteFile(w) => {
                assert!(!w.ok);
                assert_eq!(
                    w.error.as_ref().unwrap().code,
                    llm_os_common::ActionErrorCode::ConfirmationRequired
                );
            }
            _ => panic!("unexpected action result type"),
        }

        let plan_with = format!(
            r#"{{
              "request_id":"req-parentdir-write-2",
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"write_file","path":"{}","content":"x","mode":"0644","reason":"test","danger":null,"recovery":null}}],
              "confirmation":{{"token":"i-understand"}}
            }}"#,
            out_path.to_string_lossy()
        );

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream.write_all(plan_with.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.unwrap();
        let response: ActionPlanResult = serde_json::from_slice(&out).unwrap();
        match &response.results[0] {
            ActionResult::WriteFile(w) => assert!(w.ok),
            _ => panic!("unexpected action result type"),
        }

        assert!(tokio::fs::try_exists(&out_path).await.unwrap());

        server.abort();
    }
}


