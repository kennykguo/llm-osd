// ABOUTME: provides llmsh helpers for parsing and validating action plans before sending them.
// ABOUTME: keeps client behavior deterministic by enforcing local validation and mode checks.

use llm_os_common::{parse_action_plan, validate_action_plan, ActionPlan, ErrorCode, Mode, RequestError};

pub fn parse_and_validate(input: &str) -> anyhow::Result<ActionPlan> {
    let plan = parse_action_plan(input)?;
    validate_action_plan(&plan).map_err(|e| anyhow::anyhow!(e.message))?;
    Ok(plan)
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidateVerdict {
    pub ok: bool,
    pub error: Option<RequestError>,
}

pub fn validate_verdict(input: &str) -> ValidateVerdict {
    match parse_action_plan(input) {
        Ok(plan) => match validate_action_plan(&plan) {
            Ok(()) => ValidateVerdict { ok: true, error: None },
            Err(err) => ValidateVerdict {
                ok: false,
                error: Some(RequestError {
                    code: ErrorCode::ValidationFailed,
                    message: err.message,
                }),
            },
        },
        Err(err) => ValidateVerdict {
            ok: false,
            error: Some(RequestError {
                code: ErrorCode::ParseFailed,
                message: err.to_string(),
            }),
        },
    }
}

pub fn parse_and_validate_for_send(input: &str) -> anyhow::Result<ActionPlan> {
    let plan = parse_and_validate(input)?;

    if plan.mode != Mode::Execute {
        return Err(anyhow::anyhow!("client refuses non-execute mode"));
    }

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_allows_plan_only_mode() {
        let input = r#"{
          "request_id":"req-1",
          "version":"0.1",
          "mode":"plan_only",
          "actions":[]
        }"#;

        parse_and_validate(input).unwrap();
    }

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

    #[test]
    fn verdict_reports_parse_failed_for_unknown_fields() {
        let input = r#"{
          "request_id":"req-1",
          "version":"0.1",
          "mode":"execute",
          "actions":[],
          "unexpected":"x"
        }"#;

        let v = validate_verdict(input);
        assert!(!v.ok);
        assert_eq!(v.error.as_ref().unwrap().code, ErrorCode::ParseFailed);
    }

    #[test]
    fn verdict_reports_validation_failed_for_missing_request_id() {
        let input = r#"{
          "request_id":"   ",
          "version":"0.1",
          "mode":"execute",
          "actions":[]
        }"#;

        let v = validate_verdict(input);
        assert!(!v.ok);
        assert_eq!(
            v.error.as_ref().unwrap().code,
            ErrorCode::ValidationFailed
        );
    }
}


