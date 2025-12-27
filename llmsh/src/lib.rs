// ABOUTME: provides llmsh helpers for parsing and validating action plans before sending them.
// ABOUTME: keeps client behavior deterministic by enforcing local validation and mode checks.

use llm_os_common::{parse_action_plan, validate_action_plan, ActionPlan, Mode};

pub fn parse_and_validate_for_send(input: &str) -> anyhow::Result<ActionPlan> {
    let plan = parse_action_plan(input)?;
    validate_action_plan(&plan).map_err(|e| anyhow::anyhow!(e.message))?;

    if plan.mode != Mode::Execute {
        return Err(anyhow::anyhow!("client refuses non-execute mode"));
    }

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_plan_only_mode() {
        let input = r#"{
          "request_id":"req-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[]
        }"#;

        let err = parse_and_validate_for_send(input).unwrap_err();
        assert!(err.to_string().contains("client refuses non-execute mode"));
    }
}


