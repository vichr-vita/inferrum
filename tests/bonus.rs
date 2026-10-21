//! Verify fictional policy outcomes and boundary conditions.

#[path = "../examples/support/bonus.rs"]
mod bonus;

use inferrum::{Error, RuleStatus};

#[test]
fn all_synthetic_scenarios_match_expected_outcomes_and_branches() -> Result<(), Error> {
    let engine = bonus::ruleset()?;
    assert_eq!(engine.rules().count(), 26);
    for scenario in bonus::scenarios() {
        let result = engine.evaluate(&scenario.facts()?)?;
        assert_eq!(
            *result.facts.require(bonus::FINAL)?,
            scenario.expected_bonus,
            "{}: {}",
            scenario.id,
            scenario.purpose
        );
        assert_eq!(
            result.facts.get(bonus::REVIEW)?.is_some(),
            scenario.expected_review,
            "{}",
            scenario.id
        );
        assert_eq!(
            result.facts.get(bonus::PAYMENT)?.is_some(),
            scenario.expected_bonus > 0,
            "{}",
            scenario.id
        );
        assert_eq!(
            *result.facts.require(bonus::TOTAL)?,
            scenario.salary + scenario.expected_bonus
        );
        let reference = result
            .trace
            .iter()
            .find(|step| step.rule == "payment_reference")
            .unwrap();
        assert_eq!(
            matches!(reference.status, RuleStatus::Skipped(_)),
            scenario.expected_bonus == 0
        );
    }
    Ok(())
}

#[test]
fn policy_rejects_invalid_scores_and_overflow() -> Result<(), Error> {
    let engine = bonus::ruleset()?;
    let mut scenario = bonus::scenarios().remove(0);
    scenario.score = 101;
    assert!(matches!(
        engine.run(&scenario.facts()?),
        Err(Error::RuleFailed {
            rule: "good_performance",
            ..
        })
    ));
    scenario.score = 94;
    scenario.salary = u64::MAX;
    assert!(matches!(
        engine.run(&scenario.facts()?),
        Err(Error::RuleFailed {
            rule: "total_compensation",
            ..
        })
    ));
    Ok(())
}

#[test]
fn award_and_multiplier_boundaries_are_exact() -> Result<(), Error> {
    let engine = bonus::ruleset()?;
    let mut scenario = bonus::scenarios().remove(0);
    for (score, factor) in [(89, 10_000), (90, 15_000)] {
        scenario.score = score;
        assert_eq!(
            *engine
                .run(&scenario.facts()?)?
                .require(bonus::PERFORMANCE_FACTOR)?,
            factor
        );
    }
    for (months, award) in [(35, 0), (36, 50_000)] {
        scenario.tenure = months;
        assert_eq!(
            *engine
                .run(&scenario.facts()?)?
                .require(bonus::TENURE_AWARD)?,
            award
        );
    }
    scenario.salary = 101;
    scenario.role = bonus::Role::Clerk;
    assert_eq!(*engine.run(&scenario.facts()?)?.require(bonus::BASE)?, 5);
    Ok(())
}
