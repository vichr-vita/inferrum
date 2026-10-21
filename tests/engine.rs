//! Public API behavior and graph validation.
#![allow(clippy::unwrap_used)]

use inferrum::{Engine, Error, Fact, Facts, Rule, RuleStatus};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SEED: Fact<i32> = Fact::new("seed");
const LEFT: Fact<i32> = Fact::new("left");
const RIGHT: Fact<i32> = Fact::new("right");
const SUM: Fact<i32> = Fact::new("sum");

fn derive(id: &'static str, from: Fact<i32>, to: Fact<i32>) -> Rule {
    Rule::new(id, [from.spec()], [to.spec()], move |i| {
        Facts::one(to, *i.get(from)? + 1)
    })
}

#[test]
fn unordered_diamond_runs_once_and_returns_only_new_facts() -> Result<(), Error> {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let engine = Engine::new(
        [SEED.spec()],
        vec![
            Rule::new(
                "join",
                [LEFT.spec(), RIGHT.spec()],
                [SUM.spec()],
                move |i| {
                    count.fetch_add(1, Ordering::SeqCst);
                    Facts::one(SUM, i.get(LEFT)? + i.get(RIGHT)?)
                },
            ),
            derive("right", SEED, RIGHT),
            derive("left", SEED, LEFT),
        ],
    )?;
    let input = Facts::one(SEED, 10)?;
    let result = engine.evaluate(&input)?;
    assert_eq!(*result.facts.require(SUM)?, 22);
    assert_eq!(result.facts.len(), 3);
    assert_eq!(result.facts.get(SEED)?, None);
    assert_eq!(input.len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        result.trace.iter().map(|s| s.rule).collect::<Vec<_>>(),
        ["right", "left", "join"]
    );
    assert_eq!(*engine.run(&Facts::one(SEED, 20)?)?.require(SUM)?, 42);
    Ok(())
}

#[test]
fn nonmatching_branch_skips_dependents_but_keeps_independent_results() -> Result<(), Error> {
    let engine = Engine::new(
        [SEED.spec()],
        vec![
            Rule::new("condition", [SEED.spec()], [LEFT.spec()], |_| {
                Ok(Facts::new())
            }),
            derive("dependent", LEFT, SUM),
            derive("independent", SEED, RIGHT),
        ],
    )?;
    let result = engine.evaluate(&Facts::one(SEED, 1)?)?;
    assert_eq!(result.trace[0].status, RuleStatus::NoMatch);
    assert_eq!(result.trace[1].status, RuleStatus::Skipped(vec!["left"]));
    assert_eq!(result.facts.get(SUM)?, None);
    assert_eq!(result.facts.get(RIGHT)?, Some(&2));
    Ok(())
}

#[test]
fn missing_initial_fact_is_a_skip_and_false_is_a_present_value() -> Result<(), Error> {
    const FLAG: Fact<bool> = Fact::new("flag");
    let engine = Engine::new(
        [FLAG.spec()],
        vec![Rule::new("flag_value", [FLAG.spec()], [SUM.spec()], |i| {
            Facts::one(SUM, i32::from(*i.get(FLAG)?))
        })],
    )?;
    assert_eq!(
        engine.evaluate(&Facts::new())?.trace[0].status,
        RuleStatus::Skipped(vec!["flag"])
    );
    assert_eq!(engine.run(&Facts::one(FLAG, false)?)?.get(SUM)?, Some(&0));
    Ok(())
}

#[test]
fn cycles_and_self_dependencies_are_rejected() {
    for rules in [
        vec![derive("self", LEFT, LEFT)],
        vec![derive("a", LEFT, RIGHT), derive("b", RIGHT, LEFT)],
    ] {
        assert!(matches!(Engine::new([], rules), Err(Error::Cycle { .. })));
    }
}

#[test]
fn undeclared_dependencies_are_not_silently_treated_as_inputs() {
    assert!(matches!(
        Engine::new([], vec![derive("a", SEED, LEFT)]),
        Err(Error::UnknownDependency {
            rule: "a",
            fact: "seed"
        })
    ));
}

#[test]
fn multiple_sources_are_rejected_including_initial_inputs() {
    assert!(matches!(
        Engine::new(
            [SEED.spec()],
            vec![derive("a", SEED, LEFT), derive("b", SEED, LEFT)]
        ),
        Err(Error::MultipleProducers("left"))
    ));
    assert!(matches!(
        Engine::new([SEED.spec(), LEFT.spec()], vec![derive("a", SEED, LEFT)]),
        Err(Error::MultipleProducers("left"))
    ));
}

#[test]
fn duplicate_names_and_declarations_are_rejected() {
    assert!(matches!(
        Engine::new([SEED.spec(), SEED.spec()], vec![]),
        Err(Error::DuplicateFact("seed"))
    ));
    assert!(matches!(
        Engine::new(
            [SEED.spec()],
            vec![derive("same", SEED, LEFT), derive("same", SEED, RIGHT)]
        ),
        Err(Error::DuplicateRule("same"))
    ));
    for (inputs, outputs) in [
        (vec![SEED.spec(), SEED.spec()], vec![LEFT.spec()]),
        (vec![SEED.spec()], vec![LEFT.spec(), LEFT.spec()]),
    ] {
        assert!(matches!(
            Engine::new(
                [SEED.spec()],
                vec![Rule::new(
                    "duplicate",
                    inputs,
                    outputs,
                    |_| Ok(Facts::new())
                )]
            ),
            Err(Error::DuplicateFact(_))
        ));
    }
    assert_eq!(
        Facts::one(Fact::new("  "), 1).unwrap_err(),
        Error::EmptyName
    );
}

#[test]
fn types_are_checked_at_graph_and_input_boundaries() -> Result<(), Error> {
    const WRONG: Fact<bool> = Fact::new("seed");
    assert!(matches!(
        Engine::new(
            [SEED.spec()],
            vec![Rule::new("wrong", [WRONG.spec()], [], |_| Ok(Facts::new()))]
        ),
        Err(Error::TypeMismatch { .. })
    ));
    let engine = Engine::new([SEED.spec()], vec![])?;
    assert!(matches!(
        engine.run(&Facts::one(WRONG, true)?),
        Err(Error::TypeMismatch { .. })
    ));
    assert!(matches!(
        engine.run(&Facts::one(LEFT, 1)?),
        Err(Error::UnexpectedInput("left"))
    ));
    assert!(matches!(
        Facts::one(SEED, 1)?.get(WRONG),
        Err(Error::TypeMismatch { .. })
    ));
    Ok(())
}

#[test]
fn duplicate_insertion_preserves_the_original_value() -> Result<(), Error> {
    let mut facts = Facts::one(SEED, 7)?;
    assert_eq!(facts.insert(SEED, 9), Err(Error::DuplicateFact("seed")));
    assert_eq!(facts.get(SEED)?, Some(&7));
    Ok(())
}

#[test]
fn callbacks_cannot_read_undeclared_facts() -> Result<(), Error> {
    let engine = Engine::new(
        [SEED.spec()],
        vec![Rule::new("sneaky", [], [LEFT.spec()], |i| {
            Facts::one(LEFT, *i.get(SEED)?)
        })],
    )?;
    assert_eq!(
        engine.run(&Facts::one(SEED, 1)?).unwrap_err(),
        Error::RuleFailed {
            rule: "sneaky",
            source: Box::new(Error::UndeclaredRead("seed"))
        }
    );
    Ok(())
}

#[test]
fn output_contract_rejects_unknown_and_mistyped_facts() -> Result<(), Error> {
    for rule in [
        Rule::new("bad", [], [LEFT.spec()], |_| Facts::one(RIGHT, 1)),
        Rule::new("bad", [], [LEFT.spec()], |_| {
            Facts::one(Fact::<bool>::new("left"), true)
        }),
    ] {
        let engine = Engine::new([], vec![rule])?;
        assert!(matches!(
            engine.run(&Facts::new()),
            Err(Error::RuleFailed { rule: "bad", .. })
        ));
    }
    Ok(())
}

#[test]
fn errors_stop_downstream_rules_and_preserve_input_facts() -> Result<(), Error> {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let engine = Engine::new(
        [SEED.spec()],
        vec![
            derive("before", SEED, LEFT),
            Rule::new("failure", [LEFT.spec()], [RIGHT.spec()], |_| {
                Err(Error::Custom("invalid business data".into()))
            }),
            Rule::new("after", [RIGHT.spec()], [SUM.spec()], move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                Facts::one(SUM, 99)
            }),
        ],
    )?;
    let input = Facts::one(SEED, 5)?;
    let error = engine.run(&input).unwrap_err();
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(input.len(), 1);
    assert_eq!(input.get(SEED)?, Some(&5));
    Ok(())
}

#[test]
fn partial_multi_output_rules_preserve_available_branches() -> Result<(), Error> {
    let engine = Engine::new(
        [],
        vec![
            Rule::new("pair", [], [LEFT.spec(), RIGHT.spec()], |_| {
                Facts::one(LEFT, 5)
            }),
            derive("available", LEFT, SUM),
        ],
    )?;
    assert_eq!(engine.run(&Facts::new())?.get(SUM)?, Some(&6));
    let engine = Engine::new(
        [],
        vec![
            Rule::new("pair", [], [LEFT.spec(), RIGHT.spec()], |_| {
                let mut f = Facts::one(LEFT, 5)?;
                f.insert(RIGHT, 7)?;
                Ok(f)
            }),
            Rule::new("join", [LEFT.spec(), RIGHT.spec()], [SUM.spec()], |i| {
                Facts::one(SUM, i.get(LEFT)? + i.get(RIGHT)?)
            }),
        ],
    )?;
    assert_eq!(engine.run(&Facts::new())?.get(SUM)?, Some(&12));
    Ok(())
}

#[test]
fn empty_engine_and_non_clone_fact_values_work() -> Result<(), Error> {
    #[derive(Debug)]
    struct Payload {
        value: i32,
    }
    const PAYLOAD: Fact<Payload> = Fact::new("payload");
    let engine = Engine::new(
        [PAYLOAD.spec()],
        vec![Rule::new(
            "payload_value",
            [PAYLOAD.spec()],
            [SUM.spec()],
            |i| Facts::one(SUM, i.get(PAYLOAD)?.value),
        )],
    )?;
    assert_eq!(
        engine
            .run(&Facts::one(PAYLOAD, Payload { value: 17 })?)?
            .get(SUM)?,
        Some(&17)
    );
    assert!(Engine::new([], vec![])?.run(&Facts::new())?.is_empty());
    Ok(())
}
