// ABOUTME: enforces allow/deny policies over requested actions before execution.
// ABOUTME: keeps the daemon behavior deterministic and auditable under llm hallucinations.

use llm_os_common::ExecAction;

const CONFIRM_TOKEN: &str = "i-understand";

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
        _ => false,
    }
}

pub fn confirmation_is_valid(token: Option<&str>) -> bool {
    match token {
        Some(t) => t.trim() == CONFIRM_TOKEN,
        None => false,
    }
}

pub fn confirmation_token_hint() -> &'static str {
    CONFIRM_TOKEN
}
