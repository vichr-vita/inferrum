//! Conversion for the compiled bonus policy. No foreign pointers enter this module.
#![forbid(unsafe_code)]

use inferrum::{Engine, Error, Facts, RuleStatus};

use crate::{policy, wire};

pub(crate) const POLICY_VERSION: &str = "bonus.v1";

pub(crate) struct BonusPolicy {
    pub(crate) engine: Engine,
}

impl BonusPolicy {
    pub(crate) fn new() -> Result<Self, wire::EvaluationError> {
        Ok(Self {
            engine: policy::ruleset().map_err(engine_error)?,
        })
    }

    pub(crate) fn evaluate(&self, request: wire::BonusRequest) -> wire::BonusResponse {
        response((|| {
            let inputs = input_facts(request.inputs.unwrap_or_default())?;
            let evaluation = self.engine.evaluate(&inputs).map_err(engine_error)?;
            Ok(wire::BonusResult {
                facts: Some(output_facts(&evaluation.facts).map_err(engine_error)?),
                trace: if request.include_trace {
                    evaluation.trace.into_iter().map(trace).collect()
                } else {
                    Vec::new()
                },
            })
        })())
    }
}

pub(crate) fn response(
    result: Result<wire::BonusResult, wire::EvaluationError>,
) -> wire::BonusResponse {
    use wire::bonus_response::Outcome;
    wire::BonusResponse {
        policy_version: POLICY_VERSION.into(),
        outcome: Some(match result {
            Ok(result) => Outcome::Result(Box::new(result)),
            Err(error) => Outcome::Error(error),
        }),
    }
}

pub(crate) fn error(code: wire::ErrorCode, message: impl Into<String>) -> wire::EvaluationError {
    wire::EvaluationError {
        code: code.into(),
        message: message.into(),
        field: None,
        rule: None,
    }
}

fn invalid(field: &str, message: &str) -> wire::EvaluationError {
    wire::EvaluationError {
        field: Some(field.into()),
        ..error(wire::ErrorCode::InvalidInput, message)
    }
}

fn engine_error(source: Error) -> wire::EvaluationError {
    let mut result = error(wire::ErrorCode::Internal, source.to_string());
    if let Error::RuleFailed { rule, .. } = source {
        result.code = wire::ErrorCode::RuleFailed.into();
        result.rule = Some(rule.into());
    }
    result
}

fn input_facts(input: wire::BonusInput) -> Result<Facts, wire::EvaluationError> {
    let mut facts = Facts::new();
    // Only insert present fields. Defaulting here would change rule eligibility.
    macro_rules! insert {
        ($key:ident, $value:expr) => {
            if let Some(value) = $value {
                facts.insert(policy::$key, value).map_err(engine_error)?;
            }
        };
    }
    insert!(EMPLOYEE_ID, input.employee_id);
    insert!(ACTIVE, input.active);
    insert!(
        TENURE,
        input
            .tenure_months
            .map(u16::try_from)
            .transpose()
            .map_err(|_| {
                invalid("tenure_months", "tenure_months must be between 0 and 65535")
            })?
    );
    insert!(
        PERFORMANCE,
        input
            .performance_score
            .map(|value| {
                u8::try_from(value)
                    .ok()
                    .filter(|value| *value <= 100)
                    .ok_or_else(|| {
                        invalid(
                            "performance_score",
                            "performance_score must be between 0 and 100",
                        )
                    })
            })
            .transpose()?
    );
    insert!(SALARY, input.salary_cents);
    insert!(
        ROLE,
        input
            .role
            .map(|value| match wire::Role::try_from(value) {
                Ok(wire::Role::Clerk) => Ok(policy::Role::Clerk),
                Ok(wire::Role::Specialist) => Ok(policy::Role::Specialist),
                Ok(wire::Role::Manager) => Ok(policy::Role::Manager),
                _ => Err(invalid(
                    "role",
                    "role must be CLERK, SPECIALIST, or MANAGER"
                )),
            })
            .transpose()?
    );
    insert!(
        DEPARTMENT,
        input
            .department
            .map(|value| match wire::Department::try_from(value) {
                Ok(wire::Department::Engineering) => Ok(policy::Department::Engineering),
                Ok(wire::Department::Sales) => Ok(policy::Department::Sales),
                Ok(wire::Department::Operations) => Ok(policy::Department::Operations),
                _ => Err(invalid(
                    "department",
                    "department must be ENGINEERING, SALES, or OPERATIONS"
                )),
            })
            .transpose()?
    );
    insert!(TRAINING, input.training_complete);
    insert!(DISCIPLINARY, input.disciplinary_hold);
    insert!(TARGET_MET, input.company_target_met);
    insert!(BUDGET, input.employee_budget_cents);
    insert!(MANAGER_APPROVED, input.manager_approved);
    insert!(FINANCE_APPROVED, input.finance_approved);
    insert!(CRITICAL_SKILL, input.critical_skill);
    Ok(facts)
}

fn output_facts(facts: &Facts) -> Result<wire::BonusFacts, Error> {
    macro_rules! outputs {
        ($($field:ident => $key:ident),* $(,)?) => {
            wire::BonusFacts {
                $($field: facts.get(policy::$key)?.copied(),)*
                review_required: facts.get(policy::REVIEW)?.map(|value| (*value).to_owned()),
                payment_reference: facts.get(policy::REFERENCE)?.cloned(),
            }
        };
    }
    Ok(outputs! {
        good_performance => GOOD,
        exceptional_performance => EXCEPTIONAL,
        tenure_eligible => TENURE_OK,
        conduct_clear => CONDUCT_OK,
        employment_eligible => EMPLOYMENT_OK,
        bonus_eligible => ELIGIBLE,
        role_rate_bps => ROLE_RATE,
        department_factor_bps => DEPARTMENT_FACTOR,
        company_factor_bps => COMPANY_FACTOR,
        performance_factor_bps => PERFORMANCE_FACTOR,
        base_bonus_cents => BASE,
        performance_bonus_cents => PERFORMANCE_BONUS,
        department_bonus_cents => DEPARTMENT_BONUS,
        company_bonus_cents => COMPANY_BONUS,
        tenure_award_cents => TENURE_AWARD,
        retention_award_cents => RETENTION_AWARD,
        proposed_bonus_cents => PROPOSED,
        eligible_bonus_cents => ELIGIBLE_AMOUNT,
        capped_bonus_cents => CAPPED,
        finance_required => FINANCE_REQUIRED,
        approvals_satisfied => APPROVALS,
        final_bonus_cents => FINAL,
        payment_ready_cents => PAYMENT,
        total_compensation_cents => TOTAL,
    })
}

fn trace(step: inferrum::RuleTrace) -> wire::RuleTrace {
    let mut result = wire::RuleTrace {
        rule: step.rule.into(),
        ..Default::default()
    };
    match step.status {
        RuleStatus::Produced(names) => {
            result.status = wire::RuleStatus::Produced.into();
            result.produced_facts = names.into_iter().map(String::from).collect();
        }
        RuleStatus::NoMatch => result.status = wire::RuleStatus::NoMatch.into(),
        RuleStatus::Skipped(names) => {
            result.status = wire::RuleStatus::Skipped.into();
            result.missing_inputs = names.into_iter().map(String::from).collect();
        }
    }
    result
}
