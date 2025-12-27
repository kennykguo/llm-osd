// ABOUTME: writes append-only audit records for each received action plan and its results.
// ABOUTME: keeps auditing deterministic by logging structured json lines.

use anyhow::Context;
use llm_os_common::{ActionPlan, ActionPlanResult};

#[derive(Debug, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct AuditRecord<'a> {
    ts_unix_ms: u64,
    plan: &'a ActionPlan,
    result: &'a ActionPlanResult,
}

pub async fn append_record(
    audit_path: &str,
    ts_unix_ms: u64,
    plan: &ActionPlan,
    result: &ActionPlanResult,
) -> anyhow::Result<()> {
    let record = AuditRecord {
        ts_unix_ms,
        plan,
        result,
    };

    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_path)
        .await
        .with_context(|| format!("open audit log at {audit_path}"))?;

    use tokio::io::AsyncWriteExt;
    file.write_all(&line).await?;
    file.flush().await?;
    Ok(())
}


