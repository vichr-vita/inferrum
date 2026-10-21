//! Declarative authoring through the exported macros from a downstream crate.
#![allow(clippy::unwrap_used)]

use inferrum::{Error, RuleStatus, declare_facts, facts, rule, ruleset};

mod keys {
    inferrum::declare_facts! {
        /// Input owned by the caller, borrowed by rules without cloning.
        pub(super) PAYLOAD: super::Payload = "payload";
    }
}

#[derive(Debug)]
struct Payload {
    text: String,
}

declare_facts! {
    SIZE: usize = "size";
    LABEL: String = "label";
    FLAG: bool = "flag";
    SUMMARY: String = "summary";
}

#[test]
fn typed_bindings_borrow_non_clone_values_and_register_dependencies() -> Result<(), Error> {
    let suffix = String::from(" bytes");
    let engine = ruleset! {
        inputs [keys::PAYLOAD, FLAG,];
        rules [
            rule! { summarize(size: SIZE, flag: FLAG,) -> SUMMARY {
                format!("{size}{suffix}: {flag}")
            }}.description("Describe the payload."),
            rule! { measure(payload: keys::PAYLOAD) -> SIZE {
                payload.text.len()
            }},
        ];
    }?;
    let rules: Vec<_> = engine.rules().collect();
    assert_eq!(rules[0].id(), "measure");
    assert_eq!(rules[1].inputs(), [SIZE.spec(), FLAG.spec()]);
    assert_eq!(rules[1].outputs(), [SUMMARY.spec()]);
    assert_eq!(rules[1].requirement(), "Describe the payload.");
    assert!(rules[1].source_code().contains("summarize"));
    assert!(rules[1].source_code().contains("format!"));

    let input = facts! {
        keys::PAYLOAD => Payload { text: "hello".into() },
        FLAG => false,
    }?;
    let result = engine.run(&input)?;
    assert_eq!(result.require(SUMMARY)?, "5 bytes: false");
    assert_eq!(input.require(keys::PAYLOAD)?.text, "hello");
    assert!(result.get(keys::PAYLOAD)?.is_none());
    Ok(())
}

#[test]
fn caller_stringify_macro_cannot_change_rule_ids_or_source() {
    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($token:tt)*) => {
            "shadowed"
        };
    }

    let required = rule! { required() -> SIZE { 1 } };
    let optional = rule! { optional() -> ? SIZE { Some(2) } };
    let multiple = rule! { multiple() -> [SIZE] { facts! { SIZE => 3 } } };
    for (rule, name) in [
        (required, "required"),
        (optional, "optional"),
        (multiple, "multiple"),
    ] {
        assert_eq!(rule.id(), name);
        assert!(rule.source_code().contains(name));
        assert!(!rule.source_code().contains("shadowed"));
    }
}

#[test]
fn optional_outputs_distinguish_no_match_from_missing_input() -> Result<(), Error> {
    let engine = ruleset! {
        inputs [FLAG];
        rules [
            rule! { label(flag: FLAG) -> ? LABEL {
                flag.then(|| "approved".to_owned())
            }},
            rule! { measure(label: LABEL) -> SIZE { label.len() }},
        ];
    }?;
    let result = engine.evaluate(&facts! { FLAG => false }?)?;
    assert!(result.facts.is_empty());
    assert_eq!(result.trace[0].status, RuleStatus::NoMatch);
    assert_eq!(result.trace[1].status, RuleStatus::Skipped(vec!["label"]));
    assert_eq!(engine.run(&facts! { FLAG => true }?)?.get(SIZE)?, Some(&8));
    assert_eq!(
        engine.evaluate(&facts! {}?)?.trace[0].status,
        RuleStatus::Skipped(vec!["flag"])
    );
    Ok(())
}

#[test]
fn multi_output_rules_can_emit_a_subset() -> Result<(), Error> {
    let engine = ruleset! {
        inputs [FLAG];
        rules [rule! { describe(flag: FLAG) -> [SIZE, LABEL,] {
            if *flag {
                facts! { SIZE => 4, LABEL => "full".to_owned() }
            } else {
                facts! { SIZE => 0 }
            }
        }}];
    }?;
    let full = engine.run(&facts! { FLAG => true }?)?;
    assert_eq!(full.get(SIZE)?, Some(&4));
    assert_eq!(full.require(LABEL)?, "full");
    let partial = engine.run(&facts! { FLAG => false }?)?;
    assert_eq!(partial.get(SIZE)?, Some(&0));
    assert_eq!(partial.get(LABEL)?, None);
    Ok(())
}

#[test]
fn rule_bodies_propagate_question_mark_and_early_errors() -> Result<(), Error> {
    let engine = ruleset! {
        inputs [FLAG];
        rules [rule! { reject(flag: FLAG) -> SIZE {
            if *flag {
                return Err(Error::Custom("early error".into()));
            }
            Err::<usize, _>(Error::Custom("question mark".into()))?
        }}];
    }?;
    for (flag, message) in [(true, "early error"), (false, "question mark")] {
        assert_eq!(
            engine.run(&facts! { FLAG => flag }?).unwrap_err(),
            Error::RuleFailed {
                rule: "reject",
                source: Box::new(Error::Custom(message.into())),
            }
        );
    }
    Ok(())
}

#[test]
fn fact_batches_evaluate_once_in_order_and_stop_on_insertion_error() {
    let mut calls = Vec::new();
    let result = facts! {
        { calls.push("key"); SIZE } => { calls.push("value"); 1 },
        SIZE => { calls.push("duplicate"); 2 },
        FLAG => { calls.push("unreachable"); true },
    };
    assert_eq!(result.unwrap_err(), Error::DuplicateFact("size"));
    assert_eq!(calls, ["key", "value", "duplicate"]);
}

#[test]
fn empty_and_constant_rulesets_preserve_engine_validation() -> Result<(), Error> {
    let empty = ruleset! { inputs []; rules []; }?;
    assert!(empty.run(&facts! {}?)?.is_empty());
    let constant = ruleset! {
        inputs [];
        rules [rule! { constant() -> SIZE { 3 } }];
    }?;
    assert_eq!(constant.run(&facts! {}?)?.get(SIZE)?, Some(&3));
    assert!(matches!(
        ruleset! {
            inputs [];
            rules [rule! { missing(size: SIZE) -> SUMMARY { size.to_string() } }];
        },
        Err(Error::UnknownDependency {
            rule: "missing",
            fact: "size"
        })
    ));
    Ok(())
}
