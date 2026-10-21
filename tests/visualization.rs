//! Graph exports follow the executable dependencies.
#![cfg(feature = "visualization")]
#![allow(clippy::unwrap_used)]

use inferrum::{
    Engine, Error, Fact, Facts, Rule,
    visualization::{GraphView, NodeKind},
};

#[test]
fn html_embeds_evaluated_facts_source_and_trace_as_inert_data() -> Result<(), Error> {
    use inferrum::visualization::ViewerCase;
    const INPUT: Fact<&str> = Fact::new("input");
    const OUTPUT: Fact<&str> = Fact::new("output");
    let engine = Engine::new(
        [INPUT.spec()],
        vec![
            Rule::new("copy", [INPUT.spec()], [OUTPUT.spec()], |i| {
                Facts::one(OUTPUT, *i.get(INPUT)?)
            })
            .description("A <requirement>")
            .source("fn copy() { /* </script> */ }"),
        ],
    )?;
    let html = engine.to_html(
        "<title> & café 🦀\u{2028}\u{2029}\0 <!-- INFERRUM_DATA -->",
        &[ViewerCase::new(
            "case",
            "A </script> case",
            Facts::one(INPUT, "</script><img src=x onerror=alert(1)>")?,
        )],
    )?;
    let data = html
        .split("type=\"application/json\">")
        .nth(1)
        .unwrap()
        .split("</script>")
        .next()
        .unwrap();
    assert!(!data.contains('<'));
    assert!(data.contains(r#""trace":{"copy":{"kind":"produced","facts":["output"]}}"#));
    assert!(data.contains(r#""source":"fn copy() { /* \u003c/script\u003e */ }"#));
    assert!(data.contains(r#""facts":{"output":"\"\u003c/script\u003e"#));
    let data: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        data["title"],
        "<title> & café 🦀\u{2028}\u{2029}\0 <!-- INFERRUM_DATA -->"
    );
    assert_eq!(data["inputCount"], 1);
    assert_eq!(data["rules"][0]["inputs"][0]["type"], "&str");
    assert_eq!(data["cases"][0]["label"], "A </script> case");
    assert_eq!(
        data["cases"][0]["inputs"]["input"],
        data["cases"][0]["facts"]["output"]
    );
    assert!(html.contains("id=\"diagram\""));
    Ok(())
}

#[test]
fn html_serializes_all_trace_outcomes_and_dependency_edges() -> Result<(), Error> {
    use inferrum::visualization::ViewerCase;
    const A: Fact<bool> = Fact::new("a");
    const B: Fact<bool> = Fact::new("b");
    let engine = Engine::new(
        [],
        vec![
            Rule::new("produced", [], [A.spec()], |_| Facts::one(A, false)),
            Rule::new("no_match", [A.spec()], [B.spec()], |_| Ok(Facts::new())),
            Rule::new("skipped", [B.spec()], [], |_| unreachable!("missing input")),
        ],
    )?;
    let html = engine.to_html("Trace", &[ViewerCase::new("case", "Case", Facts::new())])?;
    let data = html
        .split("type=\"application/json\">")
        .nth(1)
        .unwrap()
        .split("</script>")
        .next()
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(data["cases"][0]["facts"], serde_json::json!({"a": "false"}));
    assert_eq!(
        data["cases"][0]["trace"],
        serde_json::json!({
            "produced": {"kind": "produced", "facts": ["a"]},
            "no_match": {"kind": "no_match", "facts": []},
            "skipped": {"kind": "skipped", "facts": ["b"]},
        })
    );
    assert_eq!(
        data["edges"],
        serde_json::json!([
            {"from": 0, "to": 1, "fact": "a"},
            {"from": 1, "to": 2, "fact": "b"},
        ])
    );
    Ok(())
}

#[test]
fn html_supports_topology_only_and_rejects_invalid_case_ids_or_failed_runs() -> Result<(), Error> {
    use inferrum::visualization::ViewerCase;
    let engine = Engine::new(
        [],
        vec![Rule::new("fail", [], [], |_| {
            Err(Error::Custom("failure".into()))
        })],
    )?;
    assert!(engine.to_html("Topology", &[])?.contains(r#""cases":[]"#));
    assert!(
        engine
            .to_html("Failure", &[ViewerCase::new("one", "One", Facts::new())])
            .is_err()
    );
    let empty = Engine::new([], vec![])?;
    assert!(empty.to_html("Empty", &[])?.contains(r#""rules":[]"#));
    assert!(
        empty
            .to_html("Invalid", &[ViewerCase::new(" ", "Blank ID", Facts::new())])
            .is_err()
    );
    assert!(
        empty
            .to_html(
                "Duplicate",
                &[
                    ViewerCase::new("one", "First", Facts::new()),
                    ViewerCase::new("one", "Second", Facts::new())
                ]
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn html_preserves_case_and_evaluation_error_sources() -> Result<(), Error> {
    use inferrum::visualization::ViewerCase;
    use std::error::Error as _;

    let engine = Engine::new(
        [],
        vec![Rule::new("fail", [], [], |_| {
            Err(Error::Custom("invalid business data".into()))
        })],
    )?;
    let error = engine
        .to_html(
            "Failure",
            &[ViewerCase::new("case-1", "Case", Facts::new())],
        )
        .unwrap_err();
    assert_eq!(
        error,
        Error::ViewerCaseFailed {
            case: "case-1".into(),
            source: Box::new(Error::RuleFailed {
                rule: "fail",
                source: Box::new(Error::Custom("invalid business data".into())),
            }),
        }
    );
    let rule_error = error.source().expect("case evaluation error");
    assert_eq!(
        rule_error.downcast_ref::<Error>(),
        Some(&Error::RuleFailed {
            rule: "fail",
            source: Box::new(Error::Custom("invalid business data".into())),
        })
    );
    assert_eq!(
        rule_error
            .source()
            .expect("rule callback error")
            .downcast_ref::<Error>(),
        Some(&Error::Custom("invalid business data".into()))
    );
    Ok(())
}

#[test]
fn full_and_compact_graphs_preserve_dependencies() -> Result<(), Error> {
    const A: Fact<i32> = Fact::new("a");
    const B: Fact<i32> = Fact::new("b");
    const C: Fact<i32> = Fact::new("c");
    let engine = Engine::new(
        [A.spec()],
        vec![
            Rule::new("second", [B.spec()], [C.spec()], |i| {
                Facts::one(C, *i.get(B)?)
            }),
            Rule::new("first", [A.spec()], [B.spec()], |i| {
                Facts::one(B, *i.get(A)?)
            }),
        ],
    )?;
    let full = engine.graph(GraphView::Facts);
    assert_eq!(full.nodes.len(), 5);
    assert_eq!(full.edges.len(), 4);
    assert_eq!(
        full.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Input)
            .count(),
        1
    );
    let edges: Vec<_> = full
        .edges
        .iter()
        .map(|e| (full.nodes[e.from].label, full.nodes[e.to].label))
        .collect();
    assert_eq!(
        edges,
        [
            ("a", "first"),
            ("first", "b"),
            ("b", "second"),
            ("second", "c")
        ]
    );
    let compact = engine.graph(GraphView::Rules);
    assert_eq!(compact.nodes.len(), 2);
    assert_eq!(compact.edges.len(), 1);
    assert_eq!(compact.edges[0].fact, "b");
    assert_eq!(full, engine.graph(GraphView::Facts));
    Ok(())
}

#[test]
fn dot_quotes_labels_without_interpreting_them_as_markup() -> Result<(), Error> {
    const ODD: Fact<i32> = Fact::new("quote\"\\line\n<node>");
    let engine = Engine::new([ODD.spec()], vec![])?;
    let dot = engine.graph(GraphView::Facts).to_dot();
    assert!(dot.contains("label=\"quote\\\"\\\\line\\n<node>\""));
    assert!(dot.contains("shape=hexagon"));
    assert_eq!(dot, engine.graph(GraphView::Facts).to_dot());
    Ok(())
}
