// ABOUTME: writes append-only audit records for each received action plan and its results.
// ABOUTME: keeps auditing deterministic by logging structured json lines.

use anyhow::Context;
use llm_os_common::{ActionPlan, ActionPlanResult};

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PeerCredentials {
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct AuditRecord<'a> {
    ts_unix_ms: u64,
    peer: Option<PeerCredentials>,
    request_id: &'a str,
    session_id: Option<&'a str>,
    plan: serde_json::Value,
    result: &'a ActionPlanResult,
}

pub async fn append_record(
    audit_path: &str,
    ts_unix_ms: u64,
    peer: Option<PeerCredentials>,
    plan: &ActionPlan,
    result: &ActionPlanResult,
) -> anyhow::Result<()> {
    let redacted_plan = redact_plan(plan)?;

    let record = AuditRecord {
        ts_unix_ms,
        peer,
        request_id: plan.request_id.as_str(),
        session_id: plan.session_id.as_deref(),
        plan: redacted_plan,
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

fn redact_plan(plan: &ActionPlan) -> anyhow::Result<serde_json::Value> {
    let mut v = serde_json::to_value(plan)?;

    if let Some(obj) = v.as_object_mut() {
        if let Some(conf) = obj.get_mut("confirmation") {
            if let Some(conf_obj) = conf.as_object_mut() {
                if conf_obj.contains_key("token") {
                    conf_obj.insert("token".to_string(), serde_json::Value::String("[redacted]".to_string()));
                }
            }
        }
        if let Some(actions) = obj.get_mut("actions") {
            if let Some(arr) = actions.as_array_mut() {
                for action in arr {
                    if let Some(action_obj) = action.as_object_mut() {
                        match action_obj.get("type").and_then(|t| t.as_str()) {
                            Some("write_file") => {
                                if action_obj.contains_key("content") {
                                    action_obj.insert(
                                        "content".to_string(),
                                        serde_json::Value::String("[redacted]".to_string()),
                                    );
                                }
                            }
                            Some("exec") => {
                                if let Some(env) = action_obj.get_mut("env") {
                                    if let Some(env_obj) = env.as_object_mut() {
                                        for (_, v) in env_obj.iter_mut() {
                                            *v = serde_json::Value::String("[redacted]".to_string());
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(v)
}


