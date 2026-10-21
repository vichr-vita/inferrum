# Synthetic bonus requirements

Fictional policy. Amounts are integer cents. Rates are basis points. Each multiplication rounds down to a cent. Budget is per employee, with no shared pool or payment side effects.

| Function | Inputs | New fact | Requirement |
|---|---|---|---|
| `good_performance` | performance_score | good_performance | BR-01: Scores must be 0..100. A score above 50 passes. |
| `exceptional_performance` | performance_score | exceptional_performance | BR-02: A score of at least 90 is exceptional. |
| `tenure_eligible` | tenure_months | tenure_eligible | BR-03: At least 12 months of service are required. |
| `conduct_clear` | disciplinary_hold | conduct_clear | BR-04: An active disciplinary hold blocks eligibility. |
| `employment_eligible` | active, tenure_eligible | employment_eligible | BR-05: Employment must be active and tenure eligible. |
| `bonus_eligible` | employment_eligible, good_performance, conduct_clear, training_complete | bonus_eligible | BR-06: Require eligible employment, good performance, clear conduct, and completed training. |
| `role_rate` | role | role_rate_bps | BR-07: Base rates are 5% for clerks, 7% for specialists, and 8% for managers. |
| `department_factor` | department | department_factor_bps | BR-08: Department factors are 110% engineering, 120% sales, and 100% operations. |
| `company_factor` | company_target_met | company_factor_bps | BR-09: Company factor is 100% when its target is met, otherwise 50%. |
| `performance_factor` | exceptional_performance | performance_factor_bps | BR-10: Exceptional performance multiplies the base by 150%, otherwise 100%. |
| `base_bonus` | salary_cents, role_rate_bps | base_bonus_cents | BR-11: Multiply annual salary by the role rate. |
| `performance_bonus` | base_bonus_cents, performance_factor_bps | performance_bonus_cents | BR-12: Apply the performance factor to the base bonus. |
| `department_bonus` | performance_bonus_cents, department_factor_bps | department_bonus_cents | BR-13: Apply the department factor after performance. |
| `company_bonus` | department_bonus_cents, company_factor_bps | company_bonus_cents | BR-14: Apply the company factor after the department factor. |
| `tenure_award` | tenure_months | tenure_award_cents | BR-15: Add 50,000 cents for at least 36 months of service. |
| `retention_award` | critical_skill, good_performance | retention_award_cents | BR-16: Add 75,000 cents for critical skills with good performance. |
| `proposed_bonus` | company_bonus_cents, tenure_award_cents, retention_award_cents | proposed_bonus_cents | BR-17: Sum the company-adjusted bonus and both awards. |
| `eligible_bonus` | bonus_eligible, proposed_bonus_cents | eligible_bonus_cents | BR-18: Ineligible employees receive a zero proposed payment. |
| `capped_bonus` | eligible_bonus_cents, employee_budget_cents | capped_bonus_cents | BR-19: Cap the bonus at the employee-specific budget allocation. |
| `finance_required` | capped_bonus_cents | finance_required | BR-20: Finance approval is required above 200,000 cents, strictly. |
| `approvals_satisfied` | manager_approved, finance_required, finance_approved | approvals_satisfied | BR-21: Require manager approval and finance approval when required. |
| `final_bonus` | approvals_satisfied, capped_bonus_cents | final_bonus_cents | BR-22: Emit the capped bonus if approvals pass, otherwise zero. |
| `payment_ready` | final_bonus_cents | payment_ready_cents | BR-23: Emit a payment-ready fact only for a positive final bonus. |
| `review_required` | capped_bonus_cents, approvals_satisfied | review_required | BR-24: Emit approval_pending only for a positive bonus awaiting approval. |
| `total_compensation` | salary_cents, final_bonus_cents | total_compensation_cents | BR-25: Total compensation is annual salary plus the final bonus. |
| `payment_reference` | employee_id, payment_ready_cents | payment_reference | BR-26: Derive a reference from employee ID and payment cents only when payment is ready. |
