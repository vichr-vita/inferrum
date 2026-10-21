//! Exercise the encoded C interface, including ownership and failure paths.
#![allow(clippy::unwrap_used)]

use super::*;
use wire::{BonusInput, BonusRequest, BonusResponse, ErrorCode, bonus_response::Outcome};

struct Handle(*mut InferrumEngine);

impl Handle {
    fn new() -> Self {
        let created = inferrum_bonus_new();
        assert!(!created.engine.is_null());
        assert!(created.error.data.is_null());
        Self(created.engine)
    }

    fn evaluate(&self, request: BonusRequest) -> BonusResponse {
        self.bytes(&request.encode_to_vec())
    }

    fn bytes(&self, bytes: &[u8]) -> BonusResponse {
        // SAFETY: The handle stays live and the byte slice is readable until return.
        let buffer = unsafe { inferrum_bonus_evaluate(self.0, bytes.as_ptr(), bytes.len()) };
        decode(buffer)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: This helper owns the handle and releases it once after all calls.
        unsafe { inferrum_engine_free(self.0) };
    }
}

fn decode(buffer: InferrumBuffer) -> BonusResponse {
    // SAFETY: The library returned this live buffer; decode copies its contents.
    let response =
        unsafe { BonusResponse::decode(std::slice::from_raw_parts(buffer.data, buffer.len)) };
    // SAFETY: Return the unchanged allocation exactly once, even on decode failure.
    unsafe { inferrum_buffer_free(buffer) };
    response.unwrap()
}

fn result(response: BonusResponse) -> wire::BonusResult {
    assert_eq!(response.policy_version, "bonus.v1");
    match response.outcome.unwrap() {
        Outcome::Result(result) => *result,
        Outcome::Error(error) => panic!("unexpected error: {error:?}"),
    }
}

fn failure(response: BonusResponse, code: ErrorCode) -> wire::EvaluationError {
    assert_eq!(response.policy_version, "bonus.v1");
    match response.outcome.unwrap() {
        Outcome::Error(error) => {
            assert_eq!(error.code, i32::from(code));
            error
        }
        Outcome::Result(result) => panic!("expected error, got {result:?}"),
    }
}

#[test]
fn encoded_evaluations_match_all_existing_scenarios() {
    let handle = Handle::new();
    for scenario in policy::scenarios() {
        let input = BonusInput {
            employee_id: Some(scenario.id.into()),
            active: Some(scenario.active),
            tenure_months: Some(scenario.tenure.into()),
            performance_score: Some(scenario.score.into()),
            salary_cents: Some(scenario.salary),
            role: Some(
                match scenario.role {
                    policy::Role::Clerk => wire::Role::Clerk,
                    policy::Role::Specialist => wire::Role::Specialist,
                    policy::Role::Manager => wire::Role::Manager,
                }
                .into(),
            ),
            department: Some(
                match scenario.department {
                    policy::Department::Engineering => wire::Department::Engineering,
                    policy::Department::Sales => wire::Department::Sales,
                    policy::Department::Operations => wire::Department::Operations,
                }
                .into(),
            ),
            training_complete: Some(scenario.training),
            disciplinary_hold: Some(scenario.disciplinary),
            company_target_met: Some(scenario.target_met),
            employee_budget_cents: Some(scenario.budget),
            manager_approved: Some(scenario.manager_approved),
            finance_approved: Some(scenario.finance_approved),
            critical_skill: Some(scenario.critical_skill),
        };
        let result = result(handle.evaluate(BonusRequest {
            inputs: Some(input),
            include_trace: true,
        }));
        let facts = result.facts.unwrap();
        assert_eq!(
            facts.final_bonus_cents,
            Some(scenario.expected_bonus),
            "{}",
            scenario.id
        );
        assert_eq!(
            facts.total_compensation_cents,
            Some(scenario.salary + scenario.expected_bonus)
        );
        assert_eq!(facts.review_required.is_some(), scenario.expected_review);
        assert_eq!(
            facts.payment_ready_cents,
            (scenario.expected_bonus > 0).then_some(scenario.expected_bonus)
        );
        assert_eq!(
            facts.payment_reference,
            (scenario.expected_bonus > 0)
                .then(|| { format!("BONUS-{}-{}", scenario.id, scenario.expected_bonus) })
        );
        assert_eq!(result.trace.len(), 26);
    }
}

#[test]
fn false_zero_and_absence_survive_encoding_without_retaining_previous_facts() {
    let handle = Handle::new();
    let present = result(handle.evaluate(BonusRequest {
        inputs: Some(BonusInput {
            disciplinary_hold: Some(false),
            tenure_months: Some(0),
            performance_score: Some(0),
            ..Default::default()
        }),
        include_trace: true,
    }));
    let facts = present.facts.unwrap();
    assert_eq!(facts.conduct_clear, Some(true));
    assert_eq!(facts.tenure_eligible, Some(false));
    assert_eq!(facts.good_performance, Some(false));
    assert!(
        present
            .trace
            .iter()
            .any(|step| step.status == i32::from(wire::RuleStatus::Produced))
    );

    let absent = result(handle.evaluate(BonusRequest {
        include_trace: true,
        ..Default::default()
    }));
    assert_eq!(absent.facts.unwrap(), wire::BonusFacts::default());
    assert!(
        absent
            .trace
            .iter()
            .all(|step| step.status == i32::from(wire::RuleStatus::Skipped))
    );
    assert_eq!(absent.trace[0].missing_inputs, ["performance_score"]);

    let no_trace = result(handle.evaluate(BonusRequest::default()));
    assert!(no_trace.trace.is_empty());
}

#[test]
fn invalid_values_and_malformed_requests_return_structured_errors() {
    let handle = Handle::new();
    for (input, field) in [
        (
            BonusInput {
                performance_score: Some(101),
                ..Default::default()
            },
            "performance_score",
        ),
        (
            BonusInput {
                performance_score: Some(u32::MAX),
                ..Default::default()
            },
            "performance_score",
        ),
        (
            BonusInput {
                tenure_months: Some(65536),
                ..Default::default()
            },
            "tenure_months",
        ),
        (
            BonusInput {
                role: Some(0),
                ..Default::default()
            },
            "role",
        ),
        (
            BonusInput {
                role: Some(999),
                ..Default::default()
            },
            "role",
        ),
        (
            BonusInput {
                department: Some(0),
                ..Default::default()
            },
            "department",
        ),
        (
            BonusInput {
                department: Some(-1),
                ..Default::default()
            },
            "department",
        ),
    ] {
        let error = failure(
            handle.evaluate(BonusRequest {
                inputs: Some(input),
                ..Default::default()
            }),
            ErrorCode::InvalidInput,
        );
        assert_eq!(error.field.as_deref(), Some(field));
    }
    failure(handle.bytes(&[0xff]), ErrorCode::Decode);
    // SAFETY: Null handles and null zero-length requests are explicitly accepted.
    failure(
        decode(unsafe { inferrum_bonus_evaluate(ptr::null(), ptr::null(), 0) }),
        ErrorCode::InvalidInput,
    );
    // SAFETY: A null request is rejected before dereference when length is nonzero.
    failure(
        decode(unsafe { inferrum_bonus_evaluate(handle.0, ptr::null(), 1) }),
        ErrorCode::InvalidInput,
    );
    // SAFETY: Null with zero length is a valid empty Protobuf request.
    result(decode(unsafe {
        inferrum_bonus_evaluate(handle.0, ptr::null(), 0)
    }));
    // SAFETY: Null releases are documented no-ops.
    unsafe {
        inferrum_buffer_free(InferrumBuffer::empty());
        inferrum_engine_free(ptr::null_mut());
    }
}

#[test]
fn callback_errors_and_panics_do_not_cross_the_c_interface() {
    for panics in [false, true] {
        let engine = inferrum::Engine::new(
            [],
            vec![inferrum::Rule::new("failure", [], [], move |_| {
                if panics {
                    panic!("test panic");
                }
                Err(inferrum::Error::Custom("test failure".into()))
            })],
        )
        .unwrap();
        let handle = Handle(Box::into_raw(Box::new(InferrumEngine {
            policy: bonus::BonusPolicy { engine },
        })));
        let error = failure(
            handle.evaluate(BonusRequest::default()),
            if panics {
                ErrorCode::Internal
            } else {
                ErrorCode::RuleFailed
            },
        );
        assert_eq!(
            error.rule.as_deref(),
            if panics { None } else { Some("failure") }
        );
    }
}
