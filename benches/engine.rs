//! Benchmarks of the fictional bonus policy, with inputs prepared outside timing.
#![allow(missing_docs)] // Criterion's generated entry points have no rustdoc.

#[path = "../examples/support/bonus.rs"]
mod bonus;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

fn engine(c: &mut Criterion) {
    c.bench_function("bonus/construct", |b| {
        b.iter(|| black_box(bonus::ruleset().expect("valid bonus policy")));
    });

    let engine = bonus::ruleset().expect("valid bonus policy");
    let scenarios = bonus::scenarios();
    let inputs: Vec<_> = scenarios
        .iter()
        .map(|scenario| scenario.facts().expect("valid scenario inputs"))
        .collect();
    // Check the workload once, without including assertions in the measurements.
    for (scenario, input) in scenarios.iter().zip(&inputs) {
        let facts = engine.run(input).expect("valid scenario");
        assert_eq!(
            *facts.require(bonus::FINAL).expect("final bonus"),
            scenario.expected_bonus,
            "{}: {}",
            scenario.id,
            scenario.purpose
        );
        assert_eq!(
            facts.get(bonus::REVIEW).expect("review outcome").is_some(),
            scenario.expected_review,
            "{}: {}",
            scenario.id,
            scenario.purpose
        );
    }

    let mut group = c.benchmark_group("bonus");
    group.throughput(Throughput::Elements(inputs.len() as u64));
    group.bench_function("run_all_cases", |b| {
        b.iter(|| {
            for input in &inputs {
                black_box(engine.run(black_box(input)).expect("valid scenario"));
            }
        });
    });
    group.bench_function("evaluate_all_cases", |b| {
        b.iter(|| {
            for input in &inputs {
                black_box(engine.evaluate(black_box(input)).expect("valid scenario"));
            }
        });
    });
    group.finish();

    #[cfg(feature = "visualization")]
    {
        use inferrum::visualization::ViewerCase;
        let cases: Vec<_> = scenarios
            .iter()
            .zip(inputs)
            .map(|(scenario, inputs)| ViewerCase::new(scenario.id, scenario.purpose, inputs))
            .collect();
        c.bench_function("bonus/html_all_cases", |b| {
            b.iter(|| {
                black_box(
                    engine
                        .to_html("Synthetic annual bonus", black_box(&cases))
                        .expect("valid viewer cases"),
                )
            });
        });
    }
}

criterion_group!(benches, engine);
criterion_main!(benches);
