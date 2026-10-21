"""Regenerate static visualization choices from the executed synthetic fixtures."""

import csv
import html
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "fixtures" / "bonus"


def read_tsv(name):
    with (DATA / name).open() as file:
        return list(csv.DictReader(file, delimiter="\t"))


rules = read_tsv("functions.tsv")
scenarios = read_tsv("scenarios.tsv")
values = {(r["employee"], r["fact"]): r["value"] for r in read_tsv("inputs.tsv") + read_tsv("derived-facts.tsv")}
source = (ROOT / "examples/support/bonus.rs").read_text()
esc = html.escape


def fields(rule, name):
    return rule[name].split(", ") if rule[name] else []


def value(fact, employee="E01"):
    raw = values.get((employee, fact), "absent")
    if raw.isdigit():
        raw = f"{int(raw):,}" + (" c" if fact.endswith("_cents") else "")
    return raw


def rust_source(name):
    start = source.index(f"rule! {{ {name}(")
    brace = source.index("{", start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end]


def q(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def svg(dot, prefix, title):
    result = subprocess.run(["dot", "-Tsvg"], input=dot, text=True, capture_output=True, check=True).stdout
    result = result[result.index("<svg"):]
    result = re.sub(r'\s(?:width|height)="[^"]*"', "", result, count=2)
    result = re.sub(r'id="([^"]+)"', lambda m: f'id="{prefix}-{m[1]}"', result)
    result = result.replace("<svg ", f'<svg role="img" aria-label="{esc(title)}" ')
    result = re.sub(r"<!--.*?-->", "", result, flags=re.S)
    return result


dot_style = '''graph [bgcolor="#000000", rankdir=TB, nodesep=0.22, ranksep=0.45, pad=0.1];
node [shape=box, fontname="Helvetica", fontsize=14, margin="0.13,0.10", color="#666666", fontcolor="#ffffff", style=filled, fillcolor="#000000"];
edge [color="#777777", arrowsize=0.65];'''

producers = {output: rule["function"] for rule in rules for output in fields(rule, "outputs")}
lines = ["digraph rules {", dot_style]
for rule in rules:
    name = rule["function"]
    result = value(fields(rule, "outputs")[0])
    color = "#74dfd0" if name == "final_bonus" else "#777777"
    lines.append(f'{q(name)} [label={q(name + chr(10) + result)}, color="{color}", penwidth=1.5, tooltip={q(rule["requirement"])}, URL="#fn-{name}"];')
    for fact in fields(rule, "inputs"):
        if fact in producers:
            lines.append(f'{q(producers[fact])} -> {q(name)} [tooltip={q(fact)}];')
lines.append("}")
full_svg = svg("\n".join(lines), "all", "All 26 bonus rule functions and their declared dependencies, with E01 output values")

focus_rules = [r for r in rules if r["function"] in {"finance_required", "approvals_satisfied", "final_bonus", "review_required"}]
focus_facts = sorted({f for r in focus_rules for field in ("inputs", "outputs") for f in fields(r, field)})
lines = ["digraph trace {", dot_style]
for fact in focus_facts:
    color = "#f0b36c" if fact in {"finance_approved", "approvals_satisfied", "review_required"} else "#777777"
    lines.append(f'{q("f_" + fact)} [shape=ellipse, label={q(fact + chr(10) + value(fact, "E03"))}, color="{color}"];')
for rule in focus_rules:
    name = rule["function"]
    lines.append(f'{q(name)} [label={q(name + "()")}, URL="#fn-{name}"];')
    for fact in fields(rule, "inputs"):
        lines.append(f'{q("f_" + fact)} -> {q(name)};')
    for fact in fields(rule, "outputs"):
        lines.append(f'{q(name)} -> {q("f_" + fact)};')
lines.append("}")
focus_svg = svg("\n".join(lines), "focus", "E03 approval branch: missing finance approval produces a review fact and a zero final bonus")


def dependencies(rule, employee):
    return "".join(f'<li><code>{esc(f)}</code><span>{esc(value(f, employee))}</span></li>' for f in fields(rule, "inputs"))


def mobile_rule(rule, employee):
    outputs = "".join(f'<li><code>{esc(f)}</code><strong>{esc(value(f, employee))}</strong></li>' for f in fields(rule, "outputs"))
    return f'<li class="mobile-rule"><a href="#fn-{rule["function"]}">{rule["function"]}()</a><ul class="fact-list">{dependencies(rule, employee)}</ul><p class="arrow">↓ derives</p><ul class="fact-list result">{outputs}</ul></li>'


mobile_all = "".join(mobile_rule(r, "E01") for r in rules)
mobile_focus = "".join(mobile_rule(r, "E03") for r in focus_rules)
catalog = []
for number, rule in enumerate(rules, 1):
    name = rule["function"]
    output = fields(rule, "outputs")[0]
    opened = " open" if name == "final_bonus" else ""
    catalog.append(f'''<details id="fn-{name}"{opened}>
    <summary><span class="rule-number">{number:02}</span><span><b>{name}()</b><small>→ {esc(output)}</small></span><span class="catalog-value">{esc(value(output))}</span></summary>
    <div class="function-body"><div><p>{esc(rule["requirement"])}</p><h4>Reads</h4><ul class="fact-list">{dependencies(rule, "E01")}</ul></div><pre><code>{esc(rust_source(name))}</code></pre></div>
    </details>''')
scenario_rows = "".join(f'<tr><th scope="row">{s["employee"]}</th><td>{esc(s["purpose"])}</td><td>{int(s["actual_bonus_cents"]):,} c</td><td>{"Review" if s["review_required"] == "true" else "Payment" if int(s["actual_bonus_cents"]) else "No payment"}</td></tr>' for s in scenarios)
final_rule = next(r for r in rules if r["function"] == "final_bonus")

page = f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Inferrum | Visualization options</title>
<style>
:root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif;background:#000;color:#fff;line-height:1.5}}
*{{box-sizing:border-box}}body{{margin:0}}main{{max-width:1500px;margin:auto;padding:28px clamp(16px,3vw,48px) 64px}}a{{color:#74dfd0;text-decoration-thickness:1px;text-underline-offset:4px;overflow-wrap:anywhere}}a:focus-visible,summary:focus-visible{{outline:2px solid #74dfd0;outline-offset:3px}}h1{{font-size:26px;margin:0;font-weight:600;letter-spacing:-.7px}}h2{{font-size:25px;margin:0 0 6px}}h3{{font-size:19px;margin:0 0 12px}}h4{{font-size:14px;margin:24px 0 8px}}p{{margin:8px 0 16px}}header{{display:flex;justify-content:space-between;gap:24px;align-items:center;padding-bottom:22px;border-bottom:1px solid #555}}nav{{display:flex;gap:24px;flex-wrap:wrap}}nav a{{display:inline-flex;align-items:center;min-height:44px}}.intro{{margin:22px 0 40px;max-width:800px}}section{{padding:30px 0 48px;border-top:1px solid #555;scroll-margin-top:16px}}.option-title{{display:flex;gap:16px;align-items:baseline}}.option-letter{{color:#74dfd0;font-family:ui-monospace,monospace;font-size:24px}}.option-note{{max-width:830px;margin-bottom:24px}}.toolbar{{display:flex;gap:12px 30px;flex-wrap:wrap;padding:12px 0;border-block:1px solid #333;margin-bottom:20px;font-size:14px}}.workspace{{display:grid;grid-template-columns:minmax(0,1fr) minmax(250px,320px);gap:28px}}.workspace>*{{min-width:0}}.diagram{{width:100%;align-self:start}}svg{{display:block;width:100%;height:auto}}aside{{border-left:1px solid #555;padding-left:22px;align-self:start}}.selected{{color:#74dfd0}}.amount{{font-size:30px;letter-spacing:-1px;margin:8px 0 18px;font-variant-numeric:tabular-nums}}.fact-list{{padding:0;list-style:none;margin:0}}.fact-list li{{padding:8px 0;border-bottom:1px solid #292929;display:flex;justify-content:space-between;gap:12px;align-items:baseline}}.fact-list span,.fact-list strong{{flex:0 1 auto;text-align:right;overflow-wrap:anywhere}}code,pre,.rule-number{{font-family:ui-monospace,SFMono-Regular,Consolas,monospace}}code{{font-size:12px;overflow-wrap:anywhere}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;word-break:normal;margin:16px 0;padding:16px 0;border-block:1px solid #333;font-size:12px}}pre code{{font-size:inherit}}.legend{{display:flex;flex-wrap:wrap;gap:8px 24px;font-size:13px;margin:14px 0}}.legend span{{display:inline-flex;align-items:center;gap:6px}}.line{{width:25px;height:1px;background:#aaa}}.square{{width:12px;height:12px;border:1px solid #fff}}.circle{{width:15px;height:11px;border:1px solid #fff;border-radius:50%}}.focus .workspace{{grid-template-columns:minmax(0,1.3fr) minmax(250px,1fr)}}.warning{{color:#f0b36c}}.trace-line{{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:14px;border-bottom:1px solid #333;padding:14px 0}}.trace-line b{{overflow-wrap:anywhere}}.fact-result{{margin-top:24px;border-top:2px solid #74dfd0;padding-top:14px}}.catalog-layout{{display:grid;grid-template-columns:180px minmax(0,1fr);gap:28px}}.index{{align-self:start}}.index p{{font-size:13px}}.catalog{{min-width:0}}details{{border-bottom:1px solid #444}}summary{{cursor:pointer;list-style:none;display:grid;grid-template-columns:30px minmax(0,1fr) minmax(85px,160px);gap:12px;padding:16px 0;min-height:48px;align-items:baseline}}summary::-webkit-details-marker{{display:none}}summary b{{font:500 14px ui-monospace,SFMono-Regular,monospace;overflow-wrap:anywhere}}summary small{{display:block;font:12px ui-monospace,monospace;margin-top:4px;overflow-wrap:anywhere}}.rule-number{{font-size:12px}}.catalog-value{{text-align:right;overflow-wrap:anywhere;font-variant-numeric:tabular-nums;font-size:14px}}details[open] summary{{color:#74dfd0}}.function-body{{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1.25fr);gap:24px;padding-bottom:24px}}.function-body p{{font-size:14px}}.function-body pre{{margin:0}}.mobile-graph{{display:none}}.mobile-rule{{padding:16px 0;border-bottom:1px solid #555}}.mobile-rule>a{{font:14px ui-monospace,monospace;display:inline-flex;align-items:center;min-height:44px}}.arrow{{font-size:12px;margin:8px 0}}table{{border-collapse:collapse;width:100%;font-size:14px}}th,td{{text-align:left;padding:12px;border-bottom:1px solid #333;overflow-wrap:anywhere}}thead th{{font-weight:500}}td:nth-child(3){{font-variant-numeric:tabular-nums}}.source-note{{font-size:13px;max-width:900px}}.compare{{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:24px;margin:28px 0 40px}}.compare a{{font-size:18px}}.compare p{{font-size:14px;margin:6px 0}}footer{{border-top:1px solid #555;padding-top:20px}}@media(max-width:1000px){{.workspace,.focus .workspace{{grid-template-columns:minmax(0,1fr)}}aside{{border-left:0;border-top:1px solid #555;padding:20px 0 0}}.catalog-layout{{grid-template-columns:minmax(0,1fr)}}.index{{display:none}}}}@media(max-width:700px){{header{{align-items:start;flex-direction:column;gap:8px}}nav{{gap:20px}}main{{padding-top:18px}}.compare{{grid-template-columns:1fr;gap:16px}}.desktop-graph{{display:none}}.mobile-graph{{display:block;padding-left:22px;margin:0}}.function-body{{grid-template-columns:minmax(0,1fr);gap:8px}}summary{{grid-template-columns:22px minmax(0,1fr);gap:8px}}.catalog-value{{grid-column:2;text-align:left}}.toolbar{{gap:6px 16px}}.amount{{font-size:26px}}h2{{font-size:22px}}table thead{{display:none}}table,tbody,tr,th,td{{display:block;width:100%}}tr{{display:grid;grid-template-columns:48px minmax(0,1fr);border-bottom:1px solid #555;padding:10px 0}}th,td{{border:0;padding:4px 0}}td:nth-child(n+3){{grid-column:2}}.fact-list li{{flex-wrap:wrap;gap:4px 12px}}.fact-list li code{{flex:1 1 160px}}.fact-list li span,.fact-list li strong{{text-align:left}}}}
</style></head><body><main>
<header><h1>Inferrum</h1><nav aria-label="Design options"><a href="#option-a">A / Map</a><a href="#option-b">B / Trace</a><a href="#option-c">C / Functions</a></nav></header>
<div class="intro"><p>Three static viewer options. All use the same executed bonus ruleset.</p><p>26 functions · 14 input facts · 26 possible new facts · 14 synthetic cases</p></div>
<div class="compare"><div><a href="#option-a">A / Ruleset map</a><p>See the whole dependency graph beside one function.</p></div><div><a href="#option-b">B / Fact trace</a><p>Explain one outcome through its inputs and rules.</p></div><div><a href="#option-c">C / Functions first</a><p>Read requirements, signatures, source, and results together.</p></div></div>
<section id="option-a"><div class="option-title"><span class="option-letter">A</span><h2>Ruleset map</h2></div><p class="option-note">The full function DAG. Follow a node to its source in option C. On a phone, the same dependencies appear as ordered function entries.</p>
<div class="toolbar"><span>Annual bonus</span><span>E01 / Exceptional specialist</span><span>26 functions</span><span>Selected: final_bonus</span></div>
<div class="workspace"><div><div class="diagram desktop-graph">{full_svg}</div><ol class="mobile-graph" aria-label="All function dependencies">{mobile_all}</ol><div class="legend"><span><i class="square"></i>Function and output value</span><span><i class="line"></i>→ Dependency</span></div></div>
<aside><h3 class="selected">final_bonus()</h3><p>BR-22</p><p>Emit the capped bonus if approvals pass, otherwise zero.</p><h4>New fact</h4><code>final_bonus_cents</code><p class="amount">1,049,000 c</p><h4>Reads</h4><ul class="fact-list">{dependencies(final_rule, "E01")}</ul><pre><code>{esc(rust_source("final_bonus"))}</code></pre><p><a href="#fn-final_bonus">Inspect function</a></p><p class="source-note">All edges show declared dependencies. Values are from E01. An absent value does not remove a dependency.</p></aside></div></section>
<section id="option-b" class="focus"><div class="option-title"><span class="option-letter">B</span><h2>Trace a fact</h2></div><p class="option-note">E03 has an eligible bonus, but finance approval is missing. The branch produces a review fact and a zero final bonus.</p><div class="toolbar"><span>E03 / Finance approval pending</span><span>Approval branch</span><span>{len(focus_rules)} functions / {len(focus_facts)} facts</span></div>
<div class="workspace"><div><div class="diagram desktop-graph">{focus_svg}</div><ol class="mobile-graph" aria-label="Approval branch dependencies">{mobile_focus}</ol><div class="legend"><span><i class="square"></i>Function</span><span><i class="circle"></i>Fact and value</span><span class="warning">Amber marks the pending approval path</span></div></div><aside><h3>Why the bonus is zero</h3><div class="trace-line"><span>Eligible, capped amount</span><b>1,049,000 c</b></div><div class="trace-line"><span>Finance threshold</span><b>&gt; 200,000 c</b></div><div class="trace-line"><span>Manager approved</span><b>true</b></div><div class="trace-line warning"><span>Finance approved</span><b>false</b></div><div class="trace-line"><span>Approvals satisfied</span><b>false</b></div><div class="fact-result"><code>review_required</code><p class="warning">"approval_pending"</p><code>final_bonus_cents</code><p class="amount">0 c</p><p><code>payment_ready_cents</code> is absent. The downstream payment reference function is skipped.</p></div><pre><code>{esc(rust_source("review_required"))}</code></pre><p class="source-note">This is a focused subgraph. The capped bonus is its boundary input; option A shows the preceding calculations.</p></aside></div></section>
<section id="option-c"><div class="option-title"><span class="option-letter">C</span><h2>Functions first</h2></div><p class="option-note">Every requirement next to its function, inputs, and new fact. Expand a row to inspect the Rust source.</p><div class="toolbar"><span>E01 / Exceptional specialist</span><span>Execution order</span><span>26 functions</span></div><div class="catalog-layout"><div class="index"><h3>Evaluation</h3><p>Qualify employment</p><p>↓</p><p>Calculate amounts</p><p>↓</p><p>Apply budget</p><p>↓</p><p>Check approvals</p><p>↓</p><p>Derive new facts</p><p class="source-note">Stage summary. Each row lists its exact dependencies.</p></div><div class="catalog">{"".join(catalog)}</div></div></section>
<section id="scenarios"><h2>Synthetic cases</h2><p>Every result matches the fixture's expected amount and review status. Amounts are cents.</p><table><thead><tr><th>Case</th><th>Requirement exercised</th><th>New bonus fact</th><th>Outcome</th></tr></thead><tbody>{scenario_rows}</tbody></table></section>
<footer><p>Choose A, B, or C as the viewer's starting point.</p><p class="source-note">Static mocks generated from the local Rust example's function catalog, input facts, and evaluated results. The policy and employees are fictional. The crate derives facts; it does not execute payments. No custom viewer is implemented yet.</p></footer>
</main></body></html>'''

output = ROOT / "design/visualization-options.html"
output.write_text(page)
print(f"Wrote {output.name}: {output.stat().st_size:,} bytes")
