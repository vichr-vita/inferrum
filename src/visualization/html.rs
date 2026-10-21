use crate::{Engine, Error, Facts, RuleStatus};
use std::{collections::BTreeSet, fmt::Write};

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

// Escape JSON for a non-executable script element as well as JSON syntax itself.
// In particular, a fact value containing </script> must remain plain text.
fn json(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' | '\u{0}'..='\u{1f}' => {
                write!(result, "\\u{:04x}", u32::from(character))
                    .expect("writing to a String cannot fail");
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

fn list<'a>(values: impl Iterator<Item = &'a str>) -> String {
    format!("[{}]", values.map(json).collect::<Vec<_>>().join(","))
}

fn fact_values(facts: &Facts) -> String {
    format!(
        "{{{}}}",
        facts
            .iter()
            .map(|(name, value)| format!("{}:{}", json(name), json(&format!("{value:?}"))))
            .collect::<Vec<_>>()
            .join(",")
    )
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
            .map(|rule| {
                let specs = |items: &[crate::FactSpec]| {
                    format!(
                        "[{}]",
                        items
                            .iter()
                            .map(|spec| format!(
                                r#"{{"name":{},"type":{}}}"#,
                                json(spec.name()),
                                json(spec.type_name())
                            ))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                format!(
                    r#"{{"id":{},"requirement":{},"source":{},"inputs":{},"outputs":{}}}"#,
                    json(rule.id()),
                    json(rule.requirement()),
                    json(rule.source_code()),
                    specs(rule.inputs()),
                    specs(rule.outputs())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let edges = self
            .graph(super::GraphView::Rules)
            .edges
            .iter()
            .map(|edge| {
                // Rule-view node indices follow the same execution order as `rules`.
                format!(
                    r#"{{"from":{},"to":{},"fact":{}}}"#,
                    edge.from,
                    edge.to,
                    json(edge.fact)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut snapshots = Vec::with_capacity(cases.len());
        for case in cases {
            let result = self
                .evaluate(&case.inputs)
                .map_err(|error| Error::Custom(format!("viewer case {:?}: {error}", case.id)))?;
            let trace = result
                .trace
                .iter()
                .map(|step| {
                    let (kind, names) = match &step.status {
                        RuleStatus::Produced(names) => ("produced", names.as_slice()),
                        RuleStatus::NoMatch => ("no_match", &[][..]),
                        RuleStatus::Skipped(names) => ("skipped", names.as_slice()),
                    };
                    format!(
                        r#"{}:{{"kind":{},"facts":{}}}"#,
                        json(step.rule),
                        json(kind),
                        list(names.iter().copied())
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            snapshots.push(format!(
                r#"{{"id":{},"label":{},"inputs":{},"facts":{},"trace":{{{}}}}}"#,
                json(&case.id),
                json(&case.label),
                fact_values(&case.inputs),
                fact_values(&result.facts),
                trace
            ));
        }
        let data = format!(
            r#"{{"title":{},"inputCount":{},"rules":[{}],"edges":[{}],"cases":[{}]}}"#,
            json(title),
            self.inputs.len(),
            rules,
            edges,
            snapshots.join(",")
        );
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
mod tests {
    use super::*;

    #[test]
    fn json_cannot_close_its_script_container() {
        assert_eq!(
            json("</script>\"\\\n\t&\u{2028}"),
            "\"\\u003c/script\\u003e\\\"\\\\\\u000a\\u0009\\u0026\\u2028\""
        );
        assert_eq!(json("café 🦀"), "\"café 🦀\"");
    }
}
