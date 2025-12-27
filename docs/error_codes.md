# llm-os error codes

this document enumerates error codes so clients can branch deterministically without parsing english strings.

## request-level error codes (`ActionPlanResult.error.code`)

- `parse_failed`: request body was not valid json or did not match the strict schema (unknown fields, wrong types, etc.)
- `validation_failed`: request parsed but failed semantic validation (empty request_id, exec.as_root, empty argv, etc.)
- `invalid_mode`: daemon received a non-execute mode
- `request_too_large`: daemon rejected the request for exceeding the request size limit

## per-action error codes (`results[].*.error.code`)

these appear inside the per-action result variant:

- `policy_denied`: daemon policy denies this action/program outright
- `confirmation_required`: daemon requires a valid confirmation token for this action/program
- note: the confirmation token is not echoed back in error messages; use the configured token out-of-band.
- `exec_failed`: exec could not be started or exited abnormally before producing a normal result
- `exec_timed_out`: exec exceeded `timeout_sec`
- `read_failed`: read_file failed
- `write_failed`: write_file failed (includes chmod failures)
- `invalid_mode_string`: write_file had an invalid mode string


