// ABOUTME: hosts the unix socket server for receiving action plans and returning structured results.
// ABOUTME: enforces strict parsing, validation, policy checks, and audit logging.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use llm_os_common::{
    parse_action_plan, validate_action_plan, Action, ActionPlanResult, ActionResult, ErrorCode,
    Mode, RequestError,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::actions;
use crate::audit;
use crate::policy;

const MAX_REQUEST_BYTES: usize = 64 * 1024;
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

    if idle {
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

    if plan.mode != Mode::Execute {
        let _ = write_request_error(
            &mut stream,
            &plan.request_id,
            ErrorCode::InvalidMode,
            "invalid mode",
        )
        .await;
        return Ok(());
    }

    let confirmation_token = plan.confirmation.as_ref().map(|c| c.token.as_str());

    let mut results = Vec::with_capacity(plan.actions.len());
    for action in &plan.actions {
        let result = execute_action(action, confirmation_token, confirm_token).await;
        results.push(result);
    }

    let response = ActionPlanResult {
        request_id: plan.request_id.clone(),
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
    audit::append_record(audit_path, now_ms, &plan, &response).await?;

    Ok(())
}

async fn write_request_error(
    stream: &mut UnixStream,
    request_id: &str,
    code: ErrorCode,
    message: &str,
) -> anyhow::Result<()> {
    let response = ActionPlanResult {
        request_id: request_id.to_string(),
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
        Action::ReadFile(read) => actions::files::read(read).await,
        Action::WriteFile(write) => actions::files::write(write).await,
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

        let big = "a".repeat(70 * 1024);
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
}


