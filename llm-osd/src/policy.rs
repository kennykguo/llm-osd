// ABOUTME: enforces allow/deny policies over requested actions before execution.
// ABOUTME: keeps the daemon behavior deterministic and auditable under llm hallucinations.

use llm_os_common::ExecAction;

pub fn is_exec_denied(exec: &ExecAction) -> bool {
    let program = match exec.argv.first() {
        Some(p) => p.as_str(),
        None => return true,
    };

    match program {
        "/bin/rm" | "rm" => true,
        "/bin/dd" | "dd" => true,
        "/sbin/mkfs" | "/sbin/mkfs.ext4" | "mkfs" | "mkfs.ext4" => true,
        "/sbin/shutdown" | "shutdown" => true,
        "/sbin/reboot" | "reboot" => true,
        _ => false,
    }
}


