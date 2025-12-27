// ABOUTME: implements bounded read_file and write_file actions for the daemon.
// ABOUTME: uses base64 for file content transport to keep responses deterministic for binary data.

use base64::Engine;
use llm_os_common::{
    ActionError, ActionErrorCode, ActionResult, ReadFileAction, ReadFileResult, WriteFileAction,
    WriteFileResult,
};

pub async fn read(read: &ReadFileAction) -> ActionResult {
    let max = read.max_bytes as usize;
    let mut file = match tokio::fs::File::open(&read.path).await {
        Ok(f) => f,
        Err(err) => {
            return ActionResult::ReadFile(ReadFileResult {
                ok: false,
                content_base64: None,
                truncated: false,
                error: Some(ActionError {
                    code: ActionErrorCode::ReadFailed,
                    message: format!("read failed: {err}"),
                }),
            })
        }
    };

    let mut data = Vec::new();
    data.reserve(max.saturating_add(1));

    use tokio::io::AsyncReadExt;
    let mut buf = [0u8; 4096];
    while data.len() < max.saturating_add(1) {
        let n = match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => {
                return ActionResult::ReadFile(ReadFileResult {
                    ok: false,
                    content_base64: None,
                    truncated: false,
                    error: Some(ActionError {
                        code: ActionErrorCode::ReadFailed,
                        message: format!("read failed: {err}"),
                    }),
                })
            }
        };

        let remaining = max.saturating_add(1).saturating_sub(data.len());
        data.extend_from_slice(&buf[..n.min(remaining)]);
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn proc_kb_field(field: &str) -> u64 {
        let status = std::fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix(field) {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 2 {
                    return parts[0].parse::<u64>().expect("parse vmrss");
                }
            }
        }
        0
    }

    #[tokio::test]
    async fn read_file_respects_max_bytes_without_rss_spike() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");

        let f = std::fs::File::create(&path).unwrap();
        f.set_len(256 * 1024 * 1024).unwrap();

        let before_hwm = proc_kb_field("VmHWM:");
        let action = ReadFileAction {
            path: path.to_string_lossy().to_string(),
            max_bytes: 16 * 1024,
            reason: "test".to_string(),
            danger: None,
            recovery: None,
        };

        let result = read(&action).await;
        let after_hwm = proc_kb_field("VmHWM:");

        match result {
            ActionResult::ReadFile(r) => {
                assert!(r.ok);
                assert!(r.truncated);
                assert!(r.content_base64.is_some());
            }
            _ => panic!("unexpected result type"),
        }

        let delta = after_hwm.saturating_sub(before_hwm);
        assert!(delta < 100 * 1024, "vmhwm increased too much: {delta} kb");
    }
}

pub async fn write(write: &WriteFileAction) -> ActionResult {
    let mode = match parse_mode(&write.mode) {
        Ok(m) => m,
        Err(err) => {
            return ActionResult::WriteFile(WriteFileResult {
                ok: false,
                artifacts: vec![],
                error: Some(ActionError {
                    code: ActionErrorCode::InvalidModeString,
                    message: err,
                }),
            })
        }
    };

    if let Err(err) = tokio::fs::write(&write.path, write.content.as_bytes()).await {
        return ActionResult::WriteFile(WriteFileResult {
            ok: false,
            artifacts: vec![],
            error: Some(ActionError {
                code: ActionErrorCode::WriteFailed,
                message: format!("write failed: {err}"),
            }),
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
                error: Some(ActionError {
                    code: ActionErrorCode::WriteFailed,
                    message: format!("chmod failed: {err}"),
                }),
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


