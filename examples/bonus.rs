//! Run the synthetic bonus policy and export facts, requirements, and DAGs.

#[path = "support/bonus.rs"]
mod bonus;

use inferrum::visualization::{GraphView, ViewerCase};
use std::{error::Error, fmt::Write as _, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/bonus"));
    fs::create_dir_all(&output)?;
    let engine = bonus::ruleset()?;
    fs::write(
        output.join("rules.dot"),
        engine.graph(GraphView::Rules).to_dot(),
    )?;
    fs::write(
        output.join("facts.dot"),
        engine.graph(GraphView::Facts).to_dot(),
    )?;

    let mut requirements = String::from(
        "# Synthetic bonus requirements\n\nFictional policy. Amounts are integer cents. Rates are basis points. Each multiplication rounds down to a cent. Budget is per employee, with no shared pool or payment side effects.\n\n| Function | Inputs | New fact | Requirement |\n|---|---|---|---|\n",
    );
    let mut catalog = String::from("function\tinputs\toutputs\trequirement\n");
    for rule in engine.rules() {
        let inputs = rule
            .inputs()
            .iter()
            .map(|f| f.name())
            .collect::<Vec<_>>()
            .join(", ");
        let outputs = rule
            .outputs()
            .iter()
            .map(|f| f.name())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            requirements,
            "| `{}` | {} | {} | {} |",
            rule.id(),
            inputs,
            outputs,
            rule.requirement()
        )?;
        writeln!(
            catalog,
            "{}\t{}\t{}\t{}",
            rule.id(),
            inputs,
            outputs,
            rule.requirement()
        )?;
    }
    fs::write(output.join("requirements.md"), requirements)?;
    fs::write(output.join("functions.tsv"), catalog)?;

    let mut scenarios = String::from(
        "employee\tpurpose\texpected_bonus_cents\tactual_bonus_cents\treview_required\n",
    );
    let mut inputs = String::from("employee\tfact\tvalue\n");
    let mut facts = String::from("employee\tfact\tvalue\n");
    let mut trace = String::from("employee\tfunction\tstatus\n");
    for scenario in bonus::scenarios() {
        let seeds = scenario.facts()?;
        let result = engine.evaluate(&seeds)?;
        let amount = *result.facts.require(bonus::FINAL)?;
        let review = result.facts.get(bonus::REVIEW)?.is_some();
        if amount != scenario.expected_bonus || review != scenario.expected_review {
            return Err(
                format!("scenario {} differs from its expected outcome", scenario.id).into(),
            );
        }
        println!(
            "{}  {:>10} cents  {}",
            scenario.id, amount, scenario.purpose
        );
        writeln!(
            scenarios,
            "{}\t{}\t{}\t{}\t{}",
            scenario.id, scenario.purpose, scenario.expected_bonus, amount, review
        )?;
        for (name, value) in seeds.iter() {
            writeln!(inputs, "{}\t{name}\t{value:?}", scenario.id)?;
        }
        for (name, value) in result.facts.iter() {
            writeln!(facts, "{}\t{name}\t{value:?}", scenario.id)?;
        }
        for step in result.trace {
            writeln!(trace, "{}\t{}\t{:?}", scenario.id, step.rule, step.status)?;
        }
    }
    fs::write(output.join("scenarios.tsv"), scenarios)?;
    fs::write(output.join("inputs.tsv"), inputs)?;
    fs::write(output.join("derived-facts.tsv"), facts)?;
    fs::write(output.join("trace.tsv"), trace)?;
    let cases = bonus::scenarios()
        .into_iter()
        .map(|scenario| {
            Ok(ViewerCase::new(
                scenario.id,
                scenario.purpose,
                scenario.facts()?,
            ))
        })
        .collect::<Result<Vec<_>, inferrum::Error>>()?;
    fs::write(
        output.join("index.html"),
        engine.to_html("Synthetic annual bonus", &cases)?,
    )?;
    println!(
        "Exported {} rules to {}",
        engine.rules().count(),
        output.display()
    );
    println!("Viewer: {}", output.join("index.html").display());
    Ok(())
}
