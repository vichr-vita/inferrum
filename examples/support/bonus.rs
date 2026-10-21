//! Fictional annual bonus policy. Amounts use integer cents; rates use basis points.
#![allow(missing_docs)]

use inferrum::{Engine, Error, Facts, declare_facts, facts, rule, ruleset};

#[derive(Debug, Clone, Copy)]
pub enum Role {
    Clerk,
    Specialist,
    Manager,
}
#[derive(Debug, Clone, Copy)]
pub enum Department {
    Engineering,
    Sales,
    Operations,
}

declare_facts! {
    pub EMPLOYEE_ID: &str = "employee_id";
    pub ACTIVE: bool = "active";
    pub TENURE: u16 = "tenure_months";
    pub PERFORMANCE: u8 = "performance_score";
    pub SALARY: u64 = "salary_cents";
    pub ROLE: Role = "role";
    pub DEPARTMENT: Department = "department";
    pub TRAINING: bool = "training_complete";
    pub DISCIPLINARY: bool = "disciplinary_hold";
    pub TARGET_MET: bool = "company_target_met";
    pub BUDGET: u64 = "employee_budget_cents";
    pub MANAGER_APPROVED: bool = "manager_approved";
    pub FINANCE_APPROVED: bool = "finance_approved";
    pub CRITICAL_SKILL: bool = "critical_skill";

    pub GOOD: bool = "good_performance";
    pub EXCEPTIONAL: bool = "exceptional_performance";
    pub TENURE_OK: bool = "tenure_eligible";
    pub CONDUCT_OK: bool = "conduct_clear";
    pub EMPLOYMENT_OK: bool = "employment_eligible";
    pub ELIGIBLE: bool = "bonus_eligible";
    pub ROLE_RATE: u64 = "role_rate_bps";
    pub DEPARTMENT_FACTOR: u64 = "department_factor_bps";
    pub COMPANY_FACTOR: u64 = "company_factor_bps";
    pub PERFORMANCE_FACTOR: u64 = "performance_factor_bps";
    pub BASE: u64 = "base_bonus_cents";
    pub PERFORMANCE_BONUS: u64 = "performance_bonus_cents";
    pub DEPARTMENT_BONUS: u64 = "department_bonus_cents";
    pub COMPANY_BONUS: u64 = "company_bonus_cents";
    pub TENURE_AWARD: u64 = "tenure_award_cents";
    pub RETENTION_AWARD: u64 = "retention_award_cents";
    pub PROPOSED: u64 = "proposed_bonus_cents";
    pub ELIGIBLE_AMOUNT: u64 = "eligible_bonus_cents";
    pub CAPPED: u64 = "capped_bonus_cents";
    pub FINANCE_REQUIRED: bool = "finance_required";
    pub APPROVALS: bool = "approvals_satisfied";
    pub FINAL: u64 = "final_bonus_cents";
    pub PAYMENT: u64 = "payment_ready_cents";
    pub REVIEW: &str = "review_required";
    pub TOTAL: u64 = "total_compensation_cents";
    pub REFERENCE: String = "payment_reference";
}

fn scaled(amount: u64, bps: u64) -> Result<u64, Error> {
    u64::try_from(u128::from(amount) * u128::from(bps) / 10_000)
        .map_err(|_| Error::Custom("bonus amount exceeds u64 cents".into()))
}
fn sum(amounts: &[u64]) -> Result<u64, Error> {
    amounts.iter().try_fold(0_u64, |total, &value| {
        total
            .checked_add(value)
            .ok_or_else(|| Error::Custom("compensation exceeds u64 cents".into()))
    })
}

pub fn ruleset() -> Result<Engine, Error> {
    ruleset! {
        inputs [
            EMPLOYEE_ID, ACTIVE, TENURE, PERFORMANCE, SALARY, ROLE, DEPARTMENT,
            TRAINING, DISCIPLINARY, TARGET_MET, BUDGET, MANAGER_APPROVED,
            FINANCE_APPROVED, CRITICAL_SKILL,
        ];
        rules [
            rule! { good_performance(score: PERFORMANCE) -> GOOD {
                if *score > 100 {
                    return Err(Error::Custom(
                        "performance_score must be between 0 and 100".into(),
                    ));
                }
                *score > 50
            }}.description("BR-01: Scores must be 0..100. A score above 50 passes."),
            rule! { exceptional_performance(score: PERFORMANCE) -> EXCEPTIONAL {
                *score >= 90
            }}.description("BR-02: A score of at least 90 is exceptional."),
            rule! { tenure_eligible(months: TENURE) -> TENURE_OK {
                *months >= 12
            }}.description("BR-03: At least 12 months of service are required."),
            rule! { conduct_clear(hold: DISCIPLINARY) -> CONDUCT_OK {
                !*hold
            }}.description("BR-04: An active disciplinary hold blocks eligibility."),
            rule! { employment_eligible(active: ACTIVE, tenure_ok: TENURE_OK) -> EMPLOYMENT_OK {
                *active && *tenure_ok
            }}.description("BR-05: Employment must be active and tenure eligible."),
            rule! { bonus_eligible(
                employment_ok: EMPLOYMENT_OK,
                good: GOOD,
                conduct_ok: CONDUCT_OK,
                training: TRAINING,
            ) -> ELIGIBLE {
                *employment_ok && *good && *conduct_ok && *training
            }}.description("BR-06: Require eligible employment, good performance, clear conduct, and completed training."),
            rule! { role_rate(role: ROLE) -> ROLE_RATE {
                match role {
                    Role::Clerk => 500,
                    Role::Specialist => 700,
                    Role::Manager => 800,
                }
            }}.description("BR-07: Base rates are 5% for clerks, 7% for specialists, and 8% for managers."),
            rule! { department_factor(department: DEPARTMENT) -> DEPARTMENT_FACTOR {
                match department {
                    Department::Engineering => 11_000,
                    Department::Sales => 12_000,
                    Department::Operations => 10_000,
                }
            }}.description("BR-08: Department factors are 110% engineering, 120% sales, and 100% operations."),
            rule! { company_factor(target_met: TARGET_MET) -> COMPANY_FACTOR {
                if *target_met { 10_000 } else { 5_000 }
            }}.description("BR-09: Company factor is 100% when its target is met, otherwise 50%."),
            rule! { performance_factor(exceptional: EXCEPTIONAL) -> PERFORMANCE_FACTOR {
                if *exceptional { 15_000 } else { 10_000 }
            }}.description("BR-10: Exceptional performance multiplies the base by 150%, otherwise 100%."),
            rule! { base_bonus(salary: SALARY, rate: ROLE_RATE) -> BASE {
                scaled(*salary, *rate)?
            }}.description("BR-11: Multiply annual salary by the role rate."),
            rule! { performance_bonus(base: BASE, factor: PERFORMANCE_FACTOR) -> PERFORMANCE_BONUS {
                scaled(*base, *factor)?
            }}.description("BR-12: Apply the performance factor to the base bonus."),
            rule! { department_bonus(
                bonus: PERFORMANCE_BONUS,
                factor: DEPARTMENT_FACTOR,
            ) -> DEPARTMENT_BONUS {
                scaled(*bonus, *factor)?
            }}.description("BR-13: Apply the department factor after performance."),
            rule! { company_bonus(bonus: DEPARTMENT_BONUS, factor: COMPANY_FACTOR) -> COMPANY_BONUS {
                scaled(*bonus, *factor)?
            }}.description("BR-14: Apply the company factor after the department factor."),
            rule! { tenure_award(months: TENURE) -> TENURE_AWARD {
                if *months >= 36 { 50_000 } else { 0 }
            }}.description("BR-15: Add 50,000 cents for at least 36 months of service."),
            rule! { retention_award(critical_skill: CRITICAL_SKILL, good: GOOD) -> RETENTION_AWARD {
                if *critical_skill && *good { 75_000 } else { 0 }
            }}.description("BR-16: Add 75,000 cents for critical skills with good performance."),
            rule! { proposed_bonus(
                company_bonus: COMPANY_BONUS,
                tenure_award: TENURE_AWARD,
                retention_award: RETENTION_AWARD,
            ) -> PROPOSED {
                sum(&[*company_bonus, *tenure_award, *retention_award])?
            }}.description("BR-17: Sum the company-adjusted bonus and both awards."),
            rule! { eligible_bonus(eligible: ELIGIBLE, proposed: PROPOSED) -> ELIGIBLE_AMOUNT {
                if *eligible { *proposed } else { 0 }
            }}.description("BR-18: Ineligible employees receive a zero proposed payment."),
            rule! { capped_bonus(amount: ELIGIBLE_AMOUNT, budget: BUDGET) -> CAPPED {
                (*amount).min(*budget)
            }}.description("BR-19: Cap the bonus at the employee-specific budget allocation."),
            rule! { finance_required(capped: CAPPED) -> FINANCE_REQUIRED {
                *capped > 200_000
            }}.description("BR-20: Finance approval is required above 200,000 cents, strictly."),
            rule! { approvals_satisfied(
                manager: MANAGER_APPROVED,
                finance_required: FINANCE_REQUIRED,
                finance: FINANCE_APPROVED,
            ) -> APPROVALS {
                *manager && (!*finance_required || *finance)
            }}.description("BR-21: Require manager approval and finance approval when required."),
            rule! { final_bonus(approved: APPROVALS, capped: CAPPED) -> FINAL {
                if *approved { *capped } else { 0 }
            }}.description("BR-22: Emit the capped bonus if approvals pass, otherwise zero."),
            rule! { payment_ready(amount: FINAL) -> ? PAYMENT {
                (*amount > 0).then_some(*amount)
            }}.description("BR-23: Emit a payment-ready fact only for a positive final bonus."),
            rule! { review_required(capped: CAPPED, approved: APPROVALS) -> ? REVIEW {
                (*capped > 0 && !*approved).then_some("approval_pending")
            }}.description("BR-24: Emit approval_pending only for a positive bonus awaiting approval."),
            rule! { total_compensation(salary: SALARY, bonus: FINAL) -> TOTAL {
                sum(&[*salary, *bonus])?
            }}.description("BR-25: Total compensation is annual salary plus the final bonus."),
            rule! { payment_reference(employee: EMPLOYEE_ID, payment: PAYMENT) -> REFERENCE {
                format!("BONUS-{employee}-{payment}")
            }}.description("BR-26: Derive a reference from employee ID and payment cents only when payment is ready."),
        ];
    }
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: &'static str,
    pub purpose: &'static str,
    pub active: bool,
    pub tenure: u16,
    pub score: u8,
    pub salary: u64,
    pub role: Role,
    pub department: Department,
    pub training: bool,
    pub disciplinary: bool,
    pub target_met: bool,
    pub budget: u64,
    pub manager_approved: bool,
    pub finance_approved: bool,
    pub critical_skill: bool,
    pub expected_bonus: u64,
    pub expected_review: bool,
}

impl Scenario {
    pub fn facts(&self) -> Result<Facts, Error> {
        facts! {
            EMPLOYEE_ID => self.id,
            ACTIVE => self.active,
            TENURE => self.tenure,
            PERFORMANCE => self.score,
            SALARY => self.salary,
            ROLE => self.role,
            DEPARTMENT => self.department,
            TRAINING => self.training,
            DISCIPLINARY => self.disciplinary,
            TARGET_MET => self.target_met,
            BUDGET => self.budget,
            MANAGER_APPROVED => self.manager_approved,
            FINANCE_APPROVED => self.finance_approved,
            CRITICAL_SKILL => self.critical_skill,
        }
    }
}

/// Fixed fixtures with independently specified expected outcomes. No random seed is needed.
pub fn scenarios() -> Vec<Scenario> {
    let base = Scenario {
        id: "E01",
        purpose: "Exceptional specialist",
        active: true,
        tenure: 48,
        score: 94,
        salary: 8_000_000,
        role: Role::Specialist,
        department: Department::Engineering,
        training: true,
        disciplinary: false,
        target_met: true,
        budget: 1_200_000,
        manager_approved: true,
        finance_approved: true,
        critical_skill: true,
        expected_bonus: 1_049_000,
        expected_review: false,
    };
    vec![
        base.clone(),
        Scenario {
            id: "E02",
            purpose: "Budget caps manager bonus",
            role: Role::Manager,
            department: Department::Sales,
            budget: 600_000,
            expected_bonus: 600_000,
            ..base.clone()
        },
        Scenario {
            id: "E03",
            purpose: "Finance approval pending",
            finance_approved: false,
            expected_bonus: 0,
            expected_review: true,
            ..base.clone()
        },
        Scenario {
            id: "E04",
            purpose: "Performance exactly 50",
            score: 50,
            expected_bonus: 0,
            ..base.clone()
        },
        Scenario {
            id: "E05",
            purpose: "Performance just above 50",
            score: 51,
            expected_bonus: 741_000,
            ..base.clone()
        },
        Scenario {
            id: "E06",
            purpose: "Tenure below 12 months",
            tenure: 11,
            expected_bonus: 0,
            ..base.clone()
        },
        Scenario {
            id: "E07",
            purpose: "Inactive employee",
            active: false,
            expected_bonus: 0,
            ..base.clone()
        },
        Scenario {
            id: "E08",
            purpose: "Training incomplete",
            training: false,
            expected_bonus: 0,
            ..base.clone()
        },
        Scenario {
            id: "E09",
            purpose: "Disciplinary hold",
            disciplinary: true,
            expected_bonus: 0,
            ..base.clone()
        },
        Scenario {
            id: "E10",
            purpose: "Company target missed",
            target_met: false,
            expected_bonus: 587_000,
            ..base.clone()
        },
        Scenario {
            id: "E11",
            purpose: "Exactly finance threshold",
            role: Role::Clerk,
            department: Department::Operations,
            salary: 4_000_000,
            tenure: 12,
            score: 89,
            critical_skill: false,
            finance_approved: false,
            expected_bonus: 200_000,
            ..base.clone()
        },
        Scenario {
            id: "E12",
            purpose: "One cent above finance threshold",
            role: Role::Clerk,
            department: Department::Operations,
            salary: 4_000_020,
            tenure: 12,
            score: 89,
            critical_skill: false,
            finance_approved: false,
            expected_bonus: 0,
            expected_review: true,
            ..base.clone()
        },
        Scenario {
            id: "E13",
            purpose: "Manager approval pending",
            manager_approved: false,
            expected_bonus: 0,
            expected_review: true,
            ..base.clone()
        },
        Scenario {
            id: "E14",
            purpose: "Zero allocated budget",
            budget: 0,
            expected_bonus: 0,
            ..base
        },
    ]
}
