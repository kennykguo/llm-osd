// ABOUTME: implements bounded read_file and write_file actions for the daemon.
// ABOUTME: uses base64 for file content transport to keep responses deterministic for binary data.

use base64::Engine;
use llm_os_common::{ActionResult, ReadFileAction, ReadFileResult, WriteFileAction, WriteFileResult};

pub async fn read(read: &ReadFileAction) -> ActionResult {
    let data = match tokio::fs::read(&read.path).await {
        Ok(d) => d,
        Err(err) => {
            return ActionResult::ReadFile(ReadFileResult {
                ok: false,
                content_base64: None,
                truncated: false,
                error: Some(format!("read failed: {err}")),
            })
        }
    };

    let max = read.max_bytes as usize;
    let truncated = data.len() > max;
    let slice = if truncated { &data[..max] } else { &data[..] };

    let content_base64 = base64::engine::general_purpose::STANDARD.encode(slice);
    ActionResult::ReadFile(ReadFileResult {
        ok: true,
        content_base64: Some(content_base64),
        truncated,
        error: None,
    })
}

pub async fn write(write: &WriteFileAction) -> ActionResult {
    let mode = match parse_mode(&write.mode) {
        Ok(m) => m,
        Err(err) => {
            return ActionResult::WriteFile(WriteFileResult {
                ok: false,
                artifacts: vec![],
                error: Some(err),
            })
        }
    };

    if let Err(err) = tokio::fs::write(&write.path, write.content.as_bytes()).await {
        return ActionResult::WriteFile(WriteFileResult {
            ok: false,
            artifacts: vec![],
            error: Some(format!("write failed: {err}")),
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(mode);
        if let Err(err) = tokio::fs::set_permissions(&write.path, perms).await {
            return ActionResult::WriteFile(WriteFileResult {
                ok: false,
                artifacts: vec![],
                error: Some(format!("chmod failed: {err}")),
            });
        }
    }

    ActionResult::WriteFile(WriteFileResult {
        ok: true,
        artifacts: vec![write.path.clone()],
        error: None,
    })
}

fn parse_mode(mode: &str) -> Result<u32, String> {
    let mode = mode.trim();
    let mode = mode.strip_prefix("0o").unwrap_or(mode);
    u32::from_str_radix(mode, 8).map_err(|_| "mode must be an octal string like 0644".to_string())
}


