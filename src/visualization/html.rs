use crate::{Engine, Error, Facts, RuleStatus};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// One named set of input facts to evaluate and include in an HTML viewer.
/// Values and attached rule source are embedded in the exported file.
#[derive(Debug, Clone)]
pub struct ViewerCase {
    /// Stable identifier used in the viewer URL.
    pub id: String,
    /// Human-readable scenario name.
    pub label: String,
    /// Initial facts for this scenario.
    pub inputs: Facts,
}

impl ViewerCase {
    /// Creates a scenario. The exporter validates nonempty, unique identifiers.
    pub fn new(id: impl Into<String>, label: impl Into<String>, inputs: Facts) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            inputs,
        }
    }
}

// These private types define the browser payload without imposing Serialize on
// user fact values or exposing engine internals as a persistence format.
#[derive(Serialize)]
struct ViewerData<'a> {
    title: &'a str,
    #[serde(rename = "inputCount")]
    input_count: usize,
    rules: Vec<RuleData>,
    edges: Vec<EdgeData>,
    cases: Vec<CaseData<'a>>,
}

#[derive(Serialize)]
struct RuleData {
    id: &'static str,
    requirement: &'static str,
    source: &'static str,
    inputs: Vec<FactData>,
    outputs: Vec<FactData>,
}

#[derive(Serialize)]
struct FactData {
    name: &'static str,
    #[serde(rename = "type")]
    type_name: &'static str,
}

#[derive(Serialize)]
struct EdgeData {
    from: usize,
    to: usize,
    fact: &'static str,
}

#[derive(Serialize)]
struct CaseData<'a> {
    id: &'a str,
    label: &'a str,
    inputs: BTreeMap<&'static str, String>,
    facts: BTreeMap<&'static str, String>,
    trace: BTreeMap<&'static str, TraceData>,
}

#[derive(Serialize)]
struct TraceData {
    kind: &'static str,
    facts: Vec<&'static str>,
}

fn specs(items: &[crate::FactSpec]) -> Vec<FactData> {
    items
        .iter()
        .map(|spec| FactData {
            name: spec.name(),
            type_name: spec.type_name(),
        })
        .collect()
}

fn fact_values(facts: &Facts) -> BTreeMap<&'static str, String> {
    facts
        .iter()
        .map(|(name, value)| (name, format!("{value:?}")))
        .collect()
}

// JSON escaping alone cannot keep </script> inside a script element. Escape
// HTML-sensitive characters after serialization so the payload stays inert.
fn script_json(data: &impl Serialize) -> Result<String, serde_json::Error> {
    Ok(serde_json::to_string(data)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029"))
}

impl Engine {
    /// Evaluates scenarios and exports a self-contained, interactive HTML ruleset viewer.
    ///
    /// The file embeds the graph, requirements, optional [`crate::Rule::source`] text,
    /// scenario inputs, newly derived facts, and execution traces. Fact values use
    /// their `Debug` representation. The browser displays these snapshots without
    /// executing Rust callbacks. No server, Graphviz installation, or network is needed.
    /// Pass an empty case list to inspect the ruleset without evaluating it.
    ///
    /// # Errors
    /// Returns an error for empty or duplicate case identifiers, or if a case fails
    /// evaluation. Callbacks execute once per case, so they should be pure.
    pub fn to_html(&self, title: &str, cases: &[ViewerCase]) -> Result<String, Error> {
        let mut ids = BTreeSet::new();
        for case in cases {
            if case.id.trim().is_empty() || !ids.insert(&case.id) {
                return Err(Error::Custom(
                    "viewer case IDs must be nonempty and unique".into(),
                ));
            }
        }
        let rules = self
            .rules()
            .map(|rule| RuleData {
                id: rule.id(),
                requirement: rule.requirement(),
                source: rule.source_code(),
                inputs: specs(rule.inputs()),
                outputs: specs(rule.outputs()),
            })
            .collect();
        let edges = self
            .graph(super::GraphView::Rules)
            .edges
            .iter()
            // Rule-view node indices follow the same execution order as `rules`.
            .map(|edge| EdgeData {
                from: edge.from,
                to: edge.to,
                fact: edge.fact,
            })
            .collect();
        let mut snapshots = Vec::with_capacity(cases.len());
        for case in cases {
            let result = self
                .evaluate(&case.inputs)
                .map_err(|error| Error::Custom(format!("viewer case {:?}: {error}", case.id)))?;
            let trace = result
                .trace
                .into_iter()
                .map(|step| {
                    let (kind, facts) = match step.status {
                        RuleStatus::Produced(names) => ("produced", names),
                        RuleStatus::NoMatch => ("no_match", Vec::new()),
                        RuleStatus::Skipped(names) => ("skipped", names),
                    };
                    (step.rule, TraceData { kind, facts })
                })
                .collect();
            snapshots.push(CaseData {
                id: &case.id,
                label: &case.label,
                inputs: fact_values(&case.inputs),
                facts: fact_values(&result.facts),
                trace,
            });
        }
        let data = script_json(&ViewerData {
            title,
            input_count: self.inputs.len(),
            rules,
            edges,
            cases: snapshots,
        })
        .map_err(|error| Error::Custom(format!("cannot serialize viewer data: {error}")))?;
        // Substitute each slot once, before adding user-controlled data. A value
        // containing a template token must never be interpreted as template syntax.
        let (before, after) = include_str!("viewer.html")
            .split_once("<!-- INFERRUM_DATA -->")
            .expect("viewer template contains its data slot");
        Ok(format!(
            "{}<script id=\"inferrum-data\" type=\"application/json\">{data}</script>{}<script>{}</script></body></html>",
            before,
            after,
            include_str!("viewer.js")
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips_without_html_or_literal_control_characters() {
        let text = "</script><!-- INFERRUM_DATA -->\"\\\n\t&\u{2028}\u{2029}\0 café 🦀";
        let data = script_json(&text).unwrap();
        assert!(!data.contains(['<', '>', '&', '\u{2028}', '\u{2029}']));
        assert!(!data.chars().any(char::is_control));
        assert_eq!(serde_json::from_str::<String>(&data).unwrap(), text);
    }
}
