use crate::{Error, FactSpec, Facts, Inputs, Rule};
use std::collections::{BTreeMap, BTreeSet};

/// A validated DAG, reusable across independent evaluations.
#[derive(Debug)]
pub struct Engine {
    pub(crate) inputs: BTreeMap<&'static str, FactSpec>,
    pub(crate) rules: Vec<Rule>,
    pub(crate) order: Vec<usize>,
}

/// How a rule ended during one evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleStatus {
    /// The callback produced the named facts.
    Produced(Vec<&'static str>),
    /// The callback ran and returned no facts.
    NoMatch,
    /// The callback did not run because these inputs were absent.
    Skipped(Vec<&'static str>),
}

/// One rule's execution outcome, in topological execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleTrace {
    /// The rule identifier.
    pub rule: &'static str,
    /// Its execution outcome.
    pub status: RuleStatus,
}

/// Derived facts and diagnostic provenance. Initial facts are never returned here.
#[derive(Debug, Clone)]
pub struct Evaluation {
    /// Only the new facts created during this run.
    pub facts: Facts,
    /// One entry per rule, including skipped rules.
    pub trace: Vec<RuleTrace>,
}

fn declarations(
    specs: impl IntoIterator<Item = FactSpec>,
) -> Result<BTreeMap<&'static str, FactSpec>, Error> {
    let mut result = BTreeMap::new();
    for spec in specs {
        if spec.name.trim().is_empty() {
            return Err(Error::EmptyName);
        }
        if result.insert(spec.name, spec).is_some() {
            return Err(Error::DuplicateFact(spec.name));
        }
    }
    Ok(result)
}

impl Engine {
    /// Validates a ruleset and computes a stable topological execution order.
    /// Ties use registration order. Each fact must have exactly one declared source.
    ///
    /// # Errors
    /// Rejects cycles, duplicate names or producers, unknown dependencies, and type conflicts.
    pub fn new(
        inputs: impl IntoIterator<Item = FactSpec>,
        rules: Vec<Rule>,
    ) -> Result<Self, Error> {
        let inputs = declarations(inputs)?;
        let mut sources: BTreeMap<_, _> = inputs
            .iter()
            .map(|(&name, &spec)| (name, (spec, None)))
            .collect();
        let mut names = BTreeSet::new();
        for (index, rule) in rules.iter().enumerate() {
            if rule.id.trim().is_empty() {
                return Err(Error::EmptyName);
            }
            if !names.insert(rule.id) {
                return Err(Error::DuplicateRule(rule.id));
            }
            declarations(rule.inputs.iter().copied())?;
            for (name, spec) in declarations(rule.outputs.iter().copied())? {
                if sources.insert(name, (spec, Some(index))).is_some() {
                    return Err(Error::MultipleProducers(name));
                }
            }
        }
        let mut children = vec![BTreeSet::new(); rules.len()];
        let mut indegrees = vec![0; rules.len()];
        for (index, rule) in rules.iter().enumerate() {
            for input in &rule.inputs {
                let (spec, producer) = sources.get(input.name).ok_or(Error::UnknownDependency {
                    rule: rule.id,
                    fact: input.name,
                })?;
                input.check(spec)?;
                if let Some(parent) = producer {
                    if children[*parent].insert(index) {
                        indegrees[index] += 1;
                    }
                }
            }
        }
        let mut ready: BTreeSet<_> = indegrees
            .iter()
            .enumerate()
            .filter_map(|(i, &count)| (count == 0).then_some(i))
            .collect();
        let mut order = Vec::with_capacity(rules.len());
        while let Some(index) = ready.pop_first() {
            order.push(index);
            for &child in &children[index] {
                indegrees[child] -= 1;
                if indegrees[child] == 0 {
                    ready.insert(child);
                }
            }
        }
        if order.len() != rules.len() {
            return Err(Error::Cycle {
                blocked_rules: rules
                    .iter()
                    .enumerate()
                    .filter_map(|(i, rule)| (indegrees[i] > 0).then_some(rule.id))
                    .collect(),
            });
        }
        Ok(Self {
            inputs,
            rules,
            order,
        })
    }

    /// Derives new facts. Rules run once after their dependencies, without modifying the input collection.
    /// Missing inputs skip a rule; an empty result means the rule did not match.
    ///
    /// # Errors
    /// Rejects undeclared or incorrectly typed initial facts and invalid callback outputs.
    /// Callback errors include the rule name. No partial result is returned on failure.
    pub fn run(&self, inputs: &Facts) -> Result<Facts, Error> {
        Ok(self.evaluate(inputs)?.facts)
    }

    /// Derives facts with an execution trace. See [`Self::run`] for evaluation semantics.
    ///
    /// # Errors
    /// Returns the same errors as [`Self::run`].
    pub fn evaluate(&self, inputs: &Facts) -> Result<Evaluation, Error> {
        for (&name, stored) in &inputs.values {
            self.inputs
                .get(name)
                .ok_or(Error::UnexpectedInput(name))?
                .check(&stored.spec)?;
        }
        let mut available = inputs.clone();
        let mut facts = Facts::new();
        let mut trace = Vec::with_capacity(self.rules.len());
        for &index in &self.order {
            let rule = &self.rules[index];
            let missing: Vec<_> = rule
                .inputs
                .iter()
                .filter(|spec| !available.values.contains_key(spec.name))
                .map(|spec| spec.name)
                .collect();
            let status = if !missing.is_empty() {
                RuleStatus::Skipped(missing)
            } else {
                let result = (|| {
                    let output = (rule.evaluate)(&Inputs {
                        facts: &available,
                        declared: &rule.inputs,
                    })?;
                    // Validate the whole batch before publishing any of its facts to downstream rules.
                    for (&name, stored) in &output.values {
                        rule.outputs
                            .iter()
                            .find(|spec| spec.name == name)
                            .ok_or(Error::UndeclaredOutput(name))?
                            .check(&stored.spec)?;
                        if available.values.contains_key(name) {
                            return Err(Error::DuplicateFact(name));
                        }
                    }
                    Ok(output)
                })()
                .map_err(|source| Error::RuleFailed {
                    rule: rule.id,
                    source: Box::new(source),
                })?;
                if result.is_empty() {
                    RuleStatus::NoMatch
                } else {
                    let names = result.values.keys().copied().collect();
                    available.values.extend(result.values.clone());
                    facts.values.extend(result.values);
                    RuleStatus::Produced(names)
                }
            };
            trace.push(RuleTrace {
                rule: rule.id,
                status,
            });
        }
        Ok(Evaluation { facts, trace })
    }

    /// Iterates over rules in execution order, including requirements and fact signatures.
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.order.iter().map(|&index| &self.rules[index])
    }
    /// Iterates over declared initial inputs in name order.
    pub fn inputs(&self) -> impl Iterator<Item = &FactSpec> {
        self.inputs.values()
    }
}
