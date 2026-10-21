//! DAG export for Graphviz and custom viewers. No renderer is required by the crate.

use crate::Engine;
use std::{collections::BTreeMap, fmt::Write};

mod html;
pub use html::ViewerCase;

/// The level of detail in a DAG export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphView {
    /// Includes initial inputs, rule functions, and derived facts.
    Facts,
    /// Includes rule functions and labels their edges with the facts passed between them.
    Rules,
}

/// A node's role in the ruleset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// An externally supplied fact.
    Input,
    /// A derived fact.
    Fact,
    /// A rule function.
    Rule,
}

/// A graph node. IDs are deterministic for a given ruleset and view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The index used by graph edges.
    pub id: usize,
    /// The fact or rule name.
    pub label: &'static str,
    /// The node's role.
    pub kind: NodeKind,
    /// Rust type name for a fact, or business requirement for a rule.
    pub detail: &'static str,
}

/// A directed dependency edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Producer node index.
    pub from: usize,
    /// Consumer node index.
    pub to: usize,
    /// The fact carried by this edge.
    pub fact: &'static str,
}

/// An inspectable, renderer-independent DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    /// All nodes in ID order.
    pub nodes: Vec<Node>,
    /// All declared dependencies.
    pub edges: Vec<Edge>,
}

impl Engine {
    /// Builds the declared dependency graph, including branches that may not produce facts at runtime.
    pub fn graph(&self, view: GraphView) -> Graph {
        let mut graph = Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let mut fact_nodes = BTreeMap::new();
        if view == GraphView::Facts {
            let all_facts = self.inputs().map(|spec| (spec, NodeKind::Input)).chain(
                self.rules()
                    .flat_map(|rule| rule.outputs().iter().map(|spec| (spec, NodeKind::Fact))),
            );
            for (spec, kind) in all_facts {
                let id = graph.nodes.len();
                fact_nodes.insert(spec.name(), id);
                graph.nodes.push(Node {
                    id,
                    label: spec.name(),
                    kind,
                    detail: spec.type_name(),
                });
            }
        }
        let mut producers = BTreeMap::new();
        let mut rule_nodes = BTreeMap::new();
        for rule in self.rules() {
            let id = graph.nodes.len();
            rule_nodes.insert(rule.id(), id);
            graph.nodes.push(Node {
                id,
                label: rule.id(),
                kind: NodeKind::Rule,
                detail: rule.requirement(),
            });
            for output in rule.outputs() {
                producers.insert(output.name(), id);
            }
        }
        for rule in self.rules() {
            let id = rule_nodes[rule.id()];
            for input in rule.inputs() {
                let from = match view {
                    GraphView::Facts => fact_nodes.get(input.name()),
                    GraphView::Rules => producers.get(input.name()),
                };
                if let Some(&from) = from {
                    graph.edges.push(Edge {
                        from,
                        to: id,
                        fact: input.name(),
                    });
                }
            }
            if view == GraphView::Facts {
                for output in rule.outputs() {
                    graph.edges.push(Edge {
                        from: id,
                        to: fact_nodes[output.name()],
                        fact: output.name(),
                    });
                }
            }
        }
        graph
    }
}

fn quoted(text: &str) -> String {
    let mut result = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => result.push(' '),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

impl Graph {
    /// Exports escaped Graphviz DOT with a top-to-bottom layout.
    /// Run `dot -Tsvg rules.dot -o rules.svg` to render it.
    pub fn to_dot(&self) -> String {
        let mut dot = String::from(
            "digraph inferrum {\n  graph [rankdir=TB, bgcolor=\"#000000\", pad=0.3];\n  node [fontname=\"Helvetica\", fontcolor=\"#ffffff\", color=\"#aaaaaa\", style=filled, fillcolor=\"#000000\"];\n  edge [color=\"#888888\", fontcolor=\"#ffffff\", fontname=\"Helvetica\", fontsize=10];\n",
        );
        for node in &self.nodes {
            let shape = match node.kind {
                NodeKind::Input => "hexagon",
                NodeKind::Fact => "ellipse",
                NodeKind::Rule => "box",
            };
            writeln!(
                dot,
                "  n{} [label={}, tooltip={}, shape={}];",
                node.id,
                quoted(node.label),
                quoted(node.detail),
                shape
            )
            .expect("writing to a String cannot fail");
        }
        let rules_only = self.nodes.iter().all(|node| node.kind == NodeKind::Rule);
        for edge in &self.edges {
            let label = if rules_only {
                format!(" [label={}]", quoted(edge.fact))
            } else {
                String::new()
            };
            writeln!(dot, "  n{} -> n{}{};", edge.from, edge.to, label)
                .expect("writing to a String cannot fail");
        }
        dot.push_str("}\n");
        dot
    }
}
