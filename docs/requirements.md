# Inferrum requirements

## Engine

1. A business case starts with a collection of typed, named facts.
2. A rule declares every fact it reads and every fact it may produce.
3. A ruleset must form a DAG. Reject cycles, unknown dependencies, duplicate
   rule identifiers, duplicate producers, and incompatible fact types before running.
4. Evaluate rules once in a stable topological order, regardless of registration
   order. Registration order breaks ties between available rules.
5. Rule callbacks return only new facts. Never replace or remove input facts.
6. Allow a condition to return no facts. Skip any downstream rule whose required
   facts are absent, while continuing independent branches.
7. Distinguish absent facts from false and zero values.
8. Reject undeclared reads and undeclared or incorrectly typed outputs.
9. Return an error with the failing rule's identifier. Do not return partial results.
10. Offer an execution trace without making actions or commands part of rule output.
11. Reuse a validated ruleset for independent business cases without retaining facts.
12. Export the same declarations used by execution as function-only and full fact DAGs.

## Fictional bonus policy

This fixture is inspired by the employee example in `eg.inferrum`. It is invented
test data. No personal data or real compensation policies are included.

The executable source of the 26 policy requirements is the `description` on each
rule in `examples/support/bonus.rs`. Running the example exports the complete
requirements table and function catalog. This keeps the graph, requirements,
and callbacks connected to the same declarations.

The policy uses these stages:

1. Qualify active employees with at least 12 months of tenure, a performance score
   above 50, completed training, and no disciplinary hold.
2. Calculate a role-based salary percentage. Apply performance, department, and
   company factors in that order. Add tenure and critical-skill awards.
3. Set ineligible bonuses to zero, then cap eligible bonuses at each employee's
   budget allocation.
4. Require manager approval for payment. Also require finance approval when the
   capped amount exceeds 200,000 cents. Exactly 200,000 does not require finance.
5. Emit a final bonus even when it is zero. Emit payment-ready facts only when
   positive. Emit a review fact only when a positive capped bonus awaits approval.
6. Derive total compensation and a payment reference. No payment is executed.

All amounts are nonnegative integer cents. All rates are integer basis points.
Round down after each multiplication. Reject invalid performance scores and
arithmetic overflow. Each budget allocation belongs to one employee; there is no
shared department balance to consume.

The 14 fixed scenarios cover every role and department, a capped bonus, pending
finance and manager approvals, score 50 and 51, tenure 11 and 12, inactive
employment, incomplete training, a disciplinary hold, a missed company target,
zero budget, and the exact finance threshold and one cent above it.

## Ruleset viewer

Option A is the approved design. The viewer shows the function DAG beside a rule
inspector, with a black background and white text. Each node displays the selected
scenario's output or execution status. Edges retain the same geometry across
scenario changes and show the declared dependency direction.

1. Export one self-contained HTML file from any validated engine. Include named
   scenario inputs, evaluated facts, traces, requirements, and optional Rust source.
2. Selecting a function updates the inspector and highlights its incident edges.
   Inputs link to their producer functions. Distinguish a false or zero fact from
   an absent fact, a rule with no match, and a skipped rule.
3. Selecting a scenario updates values and statuses without moving the graph.
   Browser code displays snapshots; it does not reimplement business rules.
4. Support keyboard selection and URL fragments for the scenario and function.
   Restore state on reload and browser history navigation.
5. Reflow to an ordered dependency list on narrow screens. Keep the selected
   inspector accessible, wrap long names and source, and prevent horizontal overflow.
6. Display source and fact values as inert text, even when they contain HTML.
7. Support topology-only exports and empty rulesets.
8. Run focused Playwright CI checks in Chromium and WebKit at mobile and desktop
   widths, including populated screens, long values, and expanded source.
