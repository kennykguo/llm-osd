// ABOUTME: executes the exec action by spawning a subprocess with bounded runtime and output.
// ABOUTME: returns structured results suitable for deterministic consumption by llmsh.

use llm_os_common::{ActionResult, ExecAction, ExecResult};
use tokio::process::Command;

const MAX_STDIO_BYTES: usize = 8192;

pub async fn run(exec: &ExecAction) -> ActionResult {
    let mut cmd = match exec.argv.first() {
        Some(program) => Command::new(program),
        None => {
            return ActionResult::Exec(ExecResult {
                ok: false,
                exit_code: None,
                stdout: "".to_string(),
                stdout_truncated: false,
                stderr: "".to_string(),
                stderr_truncated: false,
                error: Some("missing argv[0]".to_string()),
            })
        }
    };

    if exec.argv.len() > 1 {
        cmd.args(&exec.argv[1..]);
    }

    if let Some(cwd) = &exec.cwd {
        cmd.current_dir(cwd);
    }

    if let Some(env) = &exec.env {
        cmd.envs(env);
    }

    let output = match tokio::time::timeout(std::time::Duration::from_secs(exec.timeout_sec), cmd.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            return ActionResult::Exec(ExecResult {
                ok: false,
                exit_code: None,
                stdout: "".to_string(),
                stdout_truncated: false,
                stderr: "".to_string(),
                stderr_truncated: false,
                error: Some(format!("exec failed: {err}")),
            })
        }
        Err(_) => {
            return ActionResult::Exec(ExecResult {
                ok: false,
                exit_code: None,
                stdout: "".to_string(),
                stdout_truncated: false,
                stderr: "".to_string(),
                stderr_truncated: false,
                error: Some("exec timed out".to_string()),
            })
        }
    };

    let (stdout, stdout_truncated) = truncate_bytes(&output.stdout);
    let (stderr, stderr_truncated) = truncate_bytes(&output.stderr);

    ActionResult::Exec(ExecResult {
        ok: output.status.success(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        error: None,
    })
}

fn truncate_bytes(bytes: &[u8]) -> (String, bool) {
    if bytes.len() <= MAX_STDIO_BYTES {
        return (String::from_utf8_lossy(bytes).to_string(), false);
    }

    let mut out = String::from_utf8_lossy(&bytes[..MAX_STDIO_BYTES]).to_string();
    out.push_str("\n[truncated]\n");
    (out, true)
}


