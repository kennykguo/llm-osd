// ABOUTME: enforces allow/deny policies over requested actions before execution.
// ABOUTME: keeps the daemon behavior deterministic and auditable under llm hallucinations.

use llm_os_common::ExecAction;

fn exec_allowed_without_confirmation(program: &str) -> bool {
    match program {
        "/bin/echo" | "echo" => true,
        _ => false,
    }
}

pub fn is_exec_denied(exec: &ExecAction) -> bool {
    let program = match exec.argv.first() {
        Some(p) => p.as_str(),
        None => return true,
    };

    match program {
        "/bin/dd" | "dd" => true,
        "/sbin/mkfs" | "/sbin/mkfs.ext4" | "mkfs" | "mkfs.ext4" => true,
        "/sbin/shutdown" | "shutdown" => true,
        "/sbin/reboot" | "reboot" => true,
        _ => false,
    }
}

pub fn exec_requires_confirmation(exec: &ExecAction) -> bool {
    let program = match exec.argv.first() {
        Some(p) => p.as_str(),
        None => return true,
    };

    match program {
        "/bin/rm" | "rm" => true,
        _ => !exec_allowed_without_confirmation(program),
    }
}

pub fn confirmation_is_valid(token: Option<&str>, expected_token: &str) -> bool {
    match token {
        Some(t) => t.trim() == expected_token,
        None => false,
    }
}

pub fn confirmation_token_hint(expected_token: &str) -> &str {
    expected_token
}
