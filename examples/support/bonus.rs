//! Fictional annual bonus policy. Amounts use integer cents; rates use basis points.
#![allow(missing_docs)]

use inferrum::{Engine, Error, Fact, Facts, Inputs, Rule};

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

pub const EMPLOYEE_ID: Fact<&str> = Fact::new("employee_id");
pub const ACTIVE: Fact<bool> = Fact::new("active");
pub const TENURE: Fact<u16> = Fact::new("tenure_months");
pub const PERFORMANCE: Fact<u8> = Fact::new("performance_score");
pub const SALARY: Fact<u64> = Fact::new("salary_cents");
pub const ROLE: Fact<Role> = Fact::new("role");
pub const DEPARTMENT: Fact<Department> = Fact::new("department");
pub const TRAINING: Fact<bool> = Fact::new("training_complete");
pub const DISCIPLINARY: Fact<bool> = Fact::new("disciplinary_hold");
pub const TARGET_MET: Fact<bool> = Fact::new("company_target_met");
pub const BUDGET: Fact<u64> = Fact::new("employee_budget_cents");
pub const MANAGER_APPROVED: Fact<bool> = Fact::new("manager_approved");
pub const FINANCE_APPROVED: Fact<bool> = Fact::new("finance_approved");
pub const CRITICAL_SKILL: Fact<bool> = Fact::new("critical_skill");

pub const GOOD: Fact<bool> = Fact::new("good_performance");
pub const EXCEPTIONAL: Fact<bool> = Fact::new("exceptional_performance");
pub const TENURE_OK: Fact<bool> = Fact::new("tenure_eligible");
pub const CONDUCT_OK: Fact<bool> = Fact::new("conduct_clear");
pub const EMPLOYMENT_OK: Fact<bool> = Fact::new("employment_eligible");
pub const ELIGIBLE: Fact<bool> = Fact::new("bonus_eligible");
pub const ROLE_RATE: Fact<u64> = Fact::new("role_rate_bps");
pub const DEPARTMENT_FACTOR: Fact<u64> = Fact::new("department_factor_bps");
pub const COMPANY_FACTOR: Fact<u64> = Fact::new("company_factor_bps");
pub const PERFORMANCE_FACTOR: Fact<u64> = Fact::new("performance_factor_bps");
pub const BASE: Fact<u64> = Fact::new("base_bonus_cents");
pub const PERFORMANCE_BONUS: Fact<u64> = Fact::new("performance_bonus_cents");
pub const DEPARTMENT_BONUS: Fact<u64> = Fact::new("department_bonus_cents");
pub const COMPANY_BONUS: Fact<u64> = Fact::new("company_bonus_cents");
pub const TENURE_AWARD: Fact<u64> = Fact::new("tenure_award_cents");
pub const RETENTION_AWARD: Fact<u64> = Fact::new("retention_award_cents");
pub const PROPOSED: Fact<u64> = Fact::new("proposed_bonus_cents");
pub const ELIGIBLE_AMOUNT: Fact<u64> = Fact::new("eligible_bonus_cents");
pub const CAPPED: Fact<u64> = Fact::new("capped_bonus_cents");
pub const FINANCE_REQUIRED: Fact<bool> = Fact::new("finance_required");
pub const APPROVALS: Fact<bool> = Fact::new("approvals_satisfied");
pub const FINAL: Fact<u64> = Fact::new("final_bonus_cents");
pub const PAYMENT: Fact<u64> = Fact::new("payment_ready_cents");
pub const REVIEW: Fact<&str> = Fact::new("review_required");
pub const TOTAL: Fact<u64> = Fact::new("total_compensation_cents");
pub const REFERENCE: Fact<String> = Fact::new("payment_reference");

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

pub fn good_performance(i: &Inputs<'_>) -> Result<Facts, Error> {
    let score = *i.get(PERFORMANCE)?;
    if score > 100 {
        return Err(Error::Custom(
            "performance_score must be between 0 and 100".into(),
        ));
    }
    Facts::one(GOOD, score > 50)
}
pub fn exceptional_performance(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(EXCEPTIONAL, *i.get(PERFORMANCE)? >= 90)
}
pub fn tenure_eligible(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(TENURE_OK, *i.get(TENURE)? >= 12)
}
pub fn conduct_clear(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(CONDUCT_OK, !*i.get(DISCIPLINARY)?)
}
pub fn employment_eligible(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(EMPLOYMENT_OK, *i.get(ACTIVE)? && *i.get(TENURE_OK)?)
}
pub fn bonus_eligible(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        ELIGIBLE,
        *i.get(EMPLOYMENT_OK)? && *i.get(GOOD)? && *i.get(CONDUCT_OK)? && *i.get(TRAINING)?,
    )
}
pub fn role_rate(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        ROLE_RATE,
        match i.get(ROLE)? {
            Role::Clerk => 500,
            Role::Specialist => 700,
            Role::Manager => 800,
        },
    )
}
pub fn department_factor(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        DEPARTMENT_FACTOR,
        match i.get(DEPARTMENT)? {
            Department::Engineering => 11_000,
            Department::Sales => 12_000,
            Department::Operations => 10_000,
        },
    )
}
pub fn company_factor(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        COMPANY_FACTOR,
        if *i.get(TARGET_MET)? { 10_000 } else { 5_000 },
    )
}
pub fn performance_factor(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        PERFORMANCE_FACTOR,
        if *i.get(EXCEPTIONAL)? { 15_000 } else { 10_000 },
    )
}
pub fn base_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(BASE, scaled(*i.get(SALARY)?, *i.get(ROLE_RATE)?)?)
}
pub fn performance_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        PERFORMANCE_BONUS,
        scaled(*i.get(BASE)?, *i.get(PERFORMANCE_FACTOR)?)?,
    )
}
pub fn department_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        DEPARTMENT_BONUS,
        scaled(*i.get(PERFORMANCE_BONUS)?, *i.get(DEPARTMENT_FACTOR)?)?,
    )
}
pub fn company_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        COMPANY_BONUS,
        scaled(*i.get(DEPARTMENT_BONUS)?, *i.get(COMPANY_FACTOR)?)?,
    )
}
pub fn tenure_award(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(TENURE_AWARD, if *i.get(TENURE)? >= 36 { 50_000 } else { 0 })
}
pub fn retention_award(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        RETENTION_AWARD,
        if *i.get(CRITICAL_SKILL)? && *i.get(GOOD)? {
            75_000
        } else {
            0
        },
    )
}
pub fn proposed_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        PROPOSED,
        sum(&[
            *i.get(COMPANY_BONUS)?,
            *i.get(TENURE_AWARD)?,
            *i.get(RETENTION_AWARD)?,
        ])?,
    )
}
pub fn eligible_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        ELIGIBLE_AMOUNT,
        if *i.get(ELIGIBLE)? {
            *i.get(PROPOSED)?
        } else {
            0
        },
    )
}
pub fn capped_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(CAPPED, (*i.get(ELIGIBLE_AMOUNT)?).min(*i.get(BUDGET)?))
}
pub fn finance_required(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(FINANCE_REQUIRED, *i.get(CAPPED)? > 200_000)
}
pub fn approvals_satisfied(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        APPROVALS,
        *i.get(MANAGER_APPROVED)? && (!*i.get(FINANCE_REQUIRED)? || *i.get(FINANCE_APPROVED)?),
    )
}
pub fn final_bonus(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        FINAL,
        if *i.get(APPROVALS)? {
            *i.get(CAPPED)?
        } else {
            0
        },
    )
}
pub fn payment_ready(i: &Inputs<'_>) -> Result<Facts, Error> {
    let amount = *i.get(FINAL)?;
    if amount > 0 {
        Facts::one(PAYMENT, amount)
    } else {
        Ok(Facts::new())
    }
}
pub fn review_required(i: &Inputs<'_>) -> Result<Facts, Error> {
    if *i.get(CAPPED)? > 0 && !*i.get(APPROVALS)? {
        Facts::one(REVIEW, "approval_pending")
    } else {
        Ok(Facts::new())
    }
}
pub fn total_compensation(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(TOTAL, sum(&[*i.get(SALARY)?, *i.get(FINAL)?])?)
}
pub fn payment_reference(i: &Inputs<'_>) -> Result<Facts, Error> {
    Facts::one(
        REFERENCE,
        format!("BONUS-{}-{}", i.get(EMPLOYEE_ID)?, i.get(PAYMENT)?),
    )
}

pub fn ruleset() -> Result<Engine, Error> {
    Engine::new([
        EMPLOYEE_ID.spec(), ACTIVE.spec(), TENURE.spec(), PERFORMANCE.spec(), SALARY.spec(), ROLE.spec(),
        DEPARTMENT.spec(), TRAINING.spec(), DISCIPLINARY.spec(), TARGET_MET.spec(), BUDGET.spec(),
        MANAGER_APPROVED.spec(), FINANCE_APPROVED.spec(), CRITICAL_SKILL.spec(),
    ], vec![
        Rule::new("good_performance", [PERFORMANCE.spec()], [GOOD.spec()], good_performance).description("BR-01: Scores must be 0..100. A score above 50 passes."),
        Rule::new("exceptional_performance", [PERFORMANCE.spec()], [EXCEPTIONAL.spec()], exceptional_performance).description("BR-02: A score of at least 90 is exceptional."),
        Rule::new("tenure_eligible", [TENURE.spec()], [TENURE_OK.spec()], tenure_eligible).description("BR-03: At least 12 months of service are required."),
        Rule::new("conduct_clear", [DISCIPLINARY.spec()], [CONDUCT_OK.spec()], conduct_clear).description("BR-04: An active disciplinary hold blocks eligibility."),
        Rule::new("employment_eligible", [ACTIVE.spec(), TENURE_OK.spec()], [EMPLOYMENT_OK.spec()], employment_eligible).description("BR-05: Employment must be active and tenure eligible."),
        Rule::new("bonus_eligible", [EMPLOYMENT_OK.spec(), GOOD.spec(), CONDUCT_OK.spec(), TRAINING.spec()], [ELIGIBLE.spec()], bonus_eligible).description("BR-06: Require eligible employment, good performance, clear conduct, and completed training."),
        Rule::new("role_rate", [ROLE.spec()], [ROLE_RATE.spec()], role_rate).description("BR-07: Base rates are 5% for clerks, 7% for specialists, and 8% for managers."),
        Rule::new("department_factor", [DEPARTMENT.spec()], [DEPARTMENT_FACTOR.spec()], department_factor).description("BR-08: Department factors are 110% engineering, 120% sales, and 100% operations."),
        Rule::new("company_factor", [TARGET_MET.spec()], [COMPANY_FACTOR.spec()], company_factor).description("BR-09: Company factor is 100% when its target is met, otherwise 50%."),
        Rule::new("performance_factor", [EXCEPTIONAL.spec()], [PERFORMANCE_FACTOR.spec()], performance_factor).description("BR-10: Exceptional performance multiplies the base by 150%, otherwise 100%."),
        Rule::new("base_bonus", [SALARY.spec(), ROLE_RATE.spec()], [BASE.spec()], base_bonus).description("BR-11: Multiply annual salary by the role rate."),
        Rule::new("performance_bonus", [BASE.spec(), PERFORMANCE_FACTOR.spec()], [PERFORMANCE_BONUS.spec()], performance_bonus).description("BR-12: Apply the performance factor to the base bonus."),
        Rule::new("department_bonus", [PERFORMANCE_BONUS.spec(), DEPARTMENT_FACTOR.spec()], [DEPARTMENT_BONUS.spec()], department_bonus).description("BR-13: Apply the department factor after performance."),
        Rule::new("company_bonus", [DEPARTMENT_BONUS.spec(), COMPANY_FACTOR.spec()], [COMPANY_BONUS.spec()], company_bonus).description("BR-14: Apply the company factor after the department factor."),
        Rule::new("tenure_award", [TENURE.spec()], [TENURE_AWARD.spec()], tenure_award).description("BR-15: Add 50,000 cents for at least 36 months of service."),
        Rule::new("retention_award", [CRITICAL_SKILL.spec(), GOOD.spec()], [RETENTION_AWARD.spec()], retention_award).description("BR-16: Add 75,000 cents for critical skills with good performance."),
        Rule::new("proposed_bonus", [COMPANY_BONUS.spec(), TENURE_AWARD.spec(), RETENTION_AWARD.spec()], [PROPOSED.spec()], proposed_bonus).description("BR-17: Sum the company-adjusted bonus and both awards."),
        Rule::new("eligible_bonus", [ELIGIBLE.spec(), PROPOSED.spec()], [ELIGIBLE_AMOUNT.spec()], eligible_bonus).description("BR-18: Ineligible employees receive a zero proposed payment."),
        Rule::new("capped_bonus", [ELIGIBLE_AMOUNT.spec(), BUDGET.spec()], [CAPPED.spec()], capped_bonus).description("BR-19: Cap the bonus at the employee-specific budget allocation."),
        Rule::new("finance_required", [CAPPED.spec()], [FINANCE_REQUIRED.spec()], finance_required).description("BR-20: Finance approval is required above 200,000 cents, strictly."),
        Rule::new("approvals_satisfied", [MANAGER_APPROVED.spec(), FINANCE_REQUIRED.spec(), FINANCE_APPROVED.spec()], [APPROVALS.spec()], approvals_satisfied).description("BR-21: Require manager approval and finance approval when required."),
        Rule::new("final_bonus", [APPROVALS.spec(), CAPPED.spec()], [FINAL.spec()], final_bonus).description("BR-22: Emit the capped bonus if approvals pass, otherwise zero."),
        Rule::new("payment_ready", [FINAL.spec()], [PAYMENT.spec()], payment_ready).description("BR-23: Emit a payment-ready fact only for a positive final bonus."),
        Rule::new("review_required", [CAPPED.spec(), APPROVALS.spec()], [REVIEW.spec()], review_required).description("BR-24: Emit approval_pending only for a positive bonus awaiting approval."),
        Rule::new("total_compensation", [SALARY.spec(), FINAL.spec()], [TOTAL.spec()], total_compensation).description("BR-25: Total compensation is annual salary plus the final bonus."),
        Rule::new("payment_reference", [EMPLOYEE_ID.spec(), PAYMENT.spec()], [REFERENCE.spec()], payment_reference).description("BR-26: Derive a reference from employee ID and payment cents only when payment is ready."),
    ].into_iter().map(|rule| {
        // This fixture's functions all end with an unindented closing brace.
        // Read the compiled-in source so the inspector follows the actual functions.
        let source = include_str!("bonus.rs");
        let marker = format!("pub fn {}(", rule.id());
        let start = source.find(&marker).expect("fixture rule function exists");
        let end = source[start..].find("\n}").expect("fixture function has a closing brace") + start + 2;
        rule.source(&source[start..end])
    }).collect())
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
        let mut f = Facts::new();
        f.insert(EMPLOYEE_ID, self.id)?;
        f.insert(ACTIVE, self.active)?;
        f.insert(TENURE, self.tenure)?;
        f.insert(PERFORMANCE, self.score)?;
        f.insert(SALARY, self.salary)?;
        f.insert(ROLE, self.role)?;
        f.insert(DEPARTMENT, self.department)?;
        f.insert(TRAINING, self.training)?;
        f.insert(DISCIPLINARY, self.disciplinary)?;
        f.insert(TARGET_MET, self.target_met)?;
        f.insert(BUDGET, self.budget)?;
        f.insert(MANAGER_APPROVED, self.manager_approved)?;
        f.insert(FINANCE_APPROVED, self.finance_approved)?;
        f.insert(CRITICAL_SKILL, self.critical_skill)?;
        Ok(f)
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
