// ABOUTME: hosts the unix socket server for receiving action plans and returning structured results.
// ABOUTME: enforces strict parsing, validation, policy checks, and audit logging.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use llm_os_common::{parse_action_plan, validate_action_plan, Action, ActionPlanResult, ActionResult, Mode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::actions;
use crate::audit;
use crate::policy;

pub async fn run(socket_path: &str, audit_path: &str) -> anyhow::Result<()> {
    if Path::new(socket_path).exists() {
        tokio::fs::remove_file(socket_path)
            .await
            .with_context(|| format!("remove existing socket at {socket_path}"))?;
    }

    let listener = UnixListener::bind(socket_path).with_context(|| format!("bind {socket_path}"))?;

    loop {
        let (stream, _addr) = listener.accept().await?;
        let audit_path = audit_path.to_string();
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, &audit_path).await {
                let _ = err;
            }
        });
    }
}

async fn handle_client(mut stream: UnixStream, audit_path: &str) -> anyhow::Result<()> {
    let mut input = Vec::new();
    stream.read_to_end(&mut input).await?;

    let input_str = String::from_utf8_lossy(&input);
    let plan = parse_action_plan(&input_str)?;
    validate_action_plan(&plan).map_err(|e| anyhow::anyhow!(e.message))?;

    if plan.mode != Mode::Execute {
        return Err(anyhow::anyhow!("only mode=execute is accepted by the daemon"));
    }

    let confirmation_token = plan.confirmation.as_ref().map(|c| c.token.as_str());

    let mut results = Vec::with_capacity(plan.actions.len());
    for action in &plan.actions {
        let result = execute_action(action, confirmation_token).await;
        results.push(result);
    }

    let response = ActionPlanResult { results };
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

async fn execute_action(action: &Action, confirmation_token: Option<&str>) -> ActionResult {
    match action {
        Action::Exec(exec) => {
            if policy::is_exec_denied(exec) {
                return ActionResult::Exec(llm_os_common::ExecResult {
                    ok: false,
                    exit_code: None,
                    stdout: "".to_string(),
                    stderr: "".to_string(),
                    error: Some("exec denied by policy".to_string()),
                });
            }
            if policy::exec_requires_confirmation(exec) && !policy::confirmation_is_valid(confirmation_token) {
                return ActionResult::Exec(llm_os_common::ExecResult {
                    ok: false,
                    exit_code: None,
                    stdout: "".to_string(),
                    stderr: "".to_string(),
                    error: Some(format!(
                        "confirmation required (token: {})",
                        policy::confirmation_token_hint()
                    )),
                });
            }
            actions::exec::run(exec).await
        }
        Action::ReadFile(read) => actions::files::read(read).await,
        Action::WriteFile(write) => actions::files::write(write).await,
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

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str).await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let plan = r#"{
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

        match &response.results[0] {
            ActionResult::Exec(exec) => {
                assert!(exec.ok);
                assert!(exec.stdout.contains("hi"));
            }
            _ => panic!("unexpected action result type"),
        }

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

        let server = tokio::spawn(async move { run(&socket_path_str, &audit_path_str).await });

        for _ in 0..50u32 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let plan_without = format!(
            r#"{{
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
                assert!(exec.error.as_deref().unwrap_or("").contains("confirmation required"));
            }
            _ => panic!("unexpected action result type"),
        }

        let plan_with = format!(
            r#"{{
              "version":"0.1",
              "mode":"execute",
              "actions":[{{"type":"exec","argv":["/bin/rm","{}"],"cwd":"{}","env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}}],
              "confirmation":{{"token":"{}"}}
            }}"#,
            file_path.file_name().unwrap().to_string_lossy(),
            dir.path().to_string_lossy(),
            policy::confirmation_token_hint()
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

        assert!(!file_path.exists());

        server.abort();
    }
}


