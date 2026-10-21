import { Graph, layout, type GraphLabel, type NodeLabel, type EdgeLabel } from '@dagrejs/dagre';

type FactSpec = { name: string; type: string };
type Rule = { id: string; requirement: string; source: string; inputs: FactSpec[]; outputs: FactSpec[] };
type Trace = { kind: 'produced' | 'no_match' | 'skipped'; facts: string[] };
type Snapshot = { id: string; label: string; inputs: Record<string, string>; facts: Record<string, string>; trace: Record<string, Trace> };
type ViewerData = { title: string; inputCount: number; rules: Rule[]; edges: { from: number; to: number; fact: string }[]; cases: Snapshot[] };

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text = '', className = '') {
  const node = document.createElement(tag);
  node.textContent = text;
  node.className = className;
  return node;
}

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attributes: Record<string, string | number> = {}, text = '') {
  const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  node.textContent = text;
  return node;
}

function required<T extends Element>(selector: string, type: { new(...args: never[]): T }): T {
  const node = document.querySelector(selector);
  if (!(node instanceof type)) throw new Error(`Missing viewer element: ${selector}`);
  return node;
}

// The crate owns this JSON schema; browser tests exercise the Rust-to-DOM boundary.
const data: ViewerData = JSON.parse(required('#inferrum-data', HTMLScriptElement).textContent ?? '{}');
const caseSelect = required('#case-select', HTMLSelectElement);
const ruleSelect = required('#rule-select', HTMLSelectElement);
const diagram = required('#diagram', SVGSVGElement);
const inspector = required('#inspector', HTMLElement);
const list = required('#rule-list', HTMLOListElement);
const rules = new Map(data.rules.map(rule => [rule.id, rule]));
const producers = new Map(data.rules.flatMap(rule => rule.outputs.map(fact => [fact.name, rule.id] as const)));
const caseMap = new Map(data.cases.map(snapshot => [snapshot.id, snapshot]));
let currentCase: Snapshot | undefined;
let currentRule: Rule | undefined;
let facts = new Map<string, string>();
let traces = new Map<string, Trace>();

function formatted(name: string, raw: string | undefined): string {
  if (raw === undefined) return currentCase ? 'Absent' : 'Not evaluated';
  // Debug values remain strings. BigInt avoids losing cents above 2^53.
  return /^\d+$/.test(raw) ? `${BigInt(raw).toLocaleString('en-US')}${name.endsWith('_cents') ? ' c' : ''}` : raw;
}

function status(rule: Rule): string {
  const trace = traces.get(rule.id);
  if (!trace) return 'Not evaluated';
  switch (trace.kind) {
    case 'produced': return `Produced ${trace.facts.length} ${trace.facts.length === 1 ? 'fact' : 'facts'}`;
    case 'no_match': return 'No match. No new facts.';
    case 'skipped': return `Skipped. Missing ${trace.facts.join(', ')}.`;
  }
}

function outputLabel(rule: Rule): string {
  const trace = traces.get(rule.id);
  if (trace?.kind === 'no_match') return 'No match';
  if (trace?.kind === 'skipped') return 'Skipped';
  if (!currentCase) return 'Not evaluated';
  return rule.outputs.map(fact => formatted(fact.name, facts.get(fact.name))).join(' · ') || 'No outputs';
}

function route(caseId: string | undefined, ruleId: string | undefined): string {
  const params = new URLSearchParams();
  if (caseId) params.set('case', caseId);
  if (ruleId) params.set('rule', ruleId);
  return `#${params}`;
}

function select(ruleId: string | undefined, caseId = currentCase?.id, focusInspector = false) {
  const hash = route(caseId, ruleId);
  if (location.hash !== hash) history.pushState(null, '', hash);
  render();
  if (focusInspector && matchMedia('(max-width: 1100px)').matches) {
    inspector.scrollIntoView({ block: 'start' });
    inspector.focus({ preventScroll: true });
  }
}

function ruleLink(ruleId: string, label: string) {
  const link = element('a', label);
  link.href = route(currentCase?.id, ruleId);
  link.addEventListener('click', event => {
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    select(ruleId, currentCase?.id, true);
  });
  return link;
}

function renderFacts(target: HTMLUListElement, declarations: FactSpec[], showProducer: boolean) {
  target.replaceChildren();
  if (!declarations.length) { target.append(element('li', 'None')); return; }
  for (const fact of declarations) {
    const row = element('li');
    row.dataset.fact = fact.name;
    row.append(element('code', fact.name, 'fact-name'), element('span', fact.type, 'fact-type'), element('span', formatted(fact.name, facts.get(fact.name)), 'fact-value'));
    if (showProducer) {
      const producer = producers.get(fact.name);
      const origin = producer ? ruleLink(producer, `← ${producer}()`) : element('span', 'Initial input');
      origin.className = 'fact-origin';
      row.append(origin);
    }
    target.append(row);
  }
}

function renderInspector() {
  inspector.hidden = !currentRule;
  if (!currentRule) return;
  required('#rule-title', HTMLElement).textContent = `${currentRule.id}()`;
  inspector.scrollTop = 0;
  const statusNode = required('#rule-status', HTMLElement);
  statusNode.textContent = status(currentRule);
  statusNode.dataset.kind = traces.get(currentRule.id)?.kind ?? 'not_evaluated';
  required('#rule-requirement', HTMLElement).textContent = currentRule.requirement || 'No requirement supplied.';
  required('#source', HTMLElement).textContent = currentRule.source || 'No source excerpt supplied.';
  renderFacts(required('#inputs', HTMLUListElement), currentRule.inputs, true);
  renderFacts(required('#outputs', HTMLUListElement), currentRule.outputs, false);
}

// Lay out once using the largest value summary across cases. Selecting a case
// updates labels and status without moving the graph or resetting keyboard focus.
const graph = new Graph<GraphLabel, NodeLabel, EdgeLabel>({ multigraph: true });
graph.setGraph({ rankdir: 'TB', nodesep: 18, ranksep: 46, marginx: 10, marginy: 10 });
const measure = document.createElement('canvas').getContext('2d');
if (measure) measure.font = '14px Arial';
const widthOf = (text: string) => measure?.measureText(text).width ?? text.length * 8;
const short = (text: string) => Array.from(text).length > 29 ? `${Array.from(text).slice(0, 28).join('')}…` : text;
function linesFor(text: string) {
  const characters = Array.from(text);
  const lines: string[] = [];
  for (let start = 0; start < characters.length; start += 26) lines.push(characters.slice(start, start + 26).join(''));
  return lines;
}
const labels = data.rules.map(rule => linesFor(rule.id));
for (const [index, rule] of data.rules.entries()) {
  const valueLabels = data.cases.map(snapshot => short(rule.outputs.map(fact => formatted(fact.name, Object.hasOwn(snapshot.facts, fact.name) ? snapshot.facts[fact.name] : undefined)).join(' · ')));
  const width = Math.max(100, ...[...(labels[index] ?? []), ...valueLabels, 'Not evaluated'].map(widthOf)) + 24;
  graph.setNode(String(index), { width, height: (labels[index]?.length ?? 1) * 18 + 36 });
}
data.edges.forEach((edge, index) => graph.setEdge(String(edge.from), String(edge.to), {}, String(index)));
if (data.rules.length) layout(graph);

const nodeElements: { rule: Rule; node: SVGAElement; value: SVGTextElement; title: SVGTitleElement }[] = [];
const edgeElements: { from: string; to: string; path: SVGPathElement }[] = [];

function buildDiagram() {
  const bounds = graph.graph();
  diagram.setAttribute('viewBox', `0 0 ${bounds.width ?? 1} ${bounds.height ?? 1}`);
  const defs = svg('defs');
  for (const [id, color] of [['arrow', '#888'], ['arrow-selected', '#74dfd0']]) {
    const marker = svg('marker', { id: id ?? '', viewBox: '0 0 10 10', refX: 9, refY: 5, markerWidth: 6, markerHeight: 6, orient: 'auto-start-reverse' });
    marker.append(svg('path', { d: 'M 0 0 L 10 5 L 0 10 z', fill: color ?? '#888' }));
    defs.append(marker);
  }
  diagram.append(defs);
  data.edges.forEach((edge, index) => {
    const geometry = graph.edge({ v: String(edge.from), w: String(edge.to), name: String(index) });
    const points = geometry?.points ?? [];
    const path = svg('path', { class: 'edge', d: points.map((point, i) => `${i ? 'L' : 'M'}${point.x},${point.y}`).join(' '), 'marker-end': 'url(#arrow)' });
    path.append(svg('title', {}, edge.fact));
    diagram.append(path);
    edgeElements.push({ from: data.rules[edge.from]?.id ?? '', to: data.rules[edge.to]?.id ?? '', path });
  });
  data.rules.forEach((rule, index) => {
    const position = graph.node(String(index));
    if (!position || position.x === undefined || position.y === undefined) return;
    const node = svg('a', { class: 'rule-node', tabindex: 0, 'data-rule': rule.id });
    const title = svg('title');
    const x = position.x, top = position.y - position.height / 2;
    node.append(title, svg('rect', { x: x - position.width / 2, y: top, width: position.width, height: position.height }));
    for (const [lineIndex, line] of (labels[index] ?? []).entries()) node.append(svg('text', { x, y: top + 20 + lineIndex * 18 }, line));
    const value = svg('text', { x, y: top + position.height - 12, class: 'node-value' });
    node.append(value);
    node.addEventListener('click', event => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      select(rule.id, currentCase?.id, true);
    });
    node.addEventListener('keydown', event => {
      if (event.key === ' ') { event.preventDefault(); select(rule.id, currentCase?.id, true); }
    });
    diagram.append(node);
    nodeElements.push({ rule, node, value, title });
  });
}

function render() {
  const params = new URLSearchParams(location.hash.slice(1));
  currentCase = caseMap.get(params.get('case') ?? '') ?? data.cases[0];
  currentRule = rules.get(params.get('rule') ?? '') ?? data.rules[0];
  facts = new Map([...Object.entries(currentCase?.inputs ?? {}), ...Object.entries(currentCase?.facts ?? {})]);
  traces = new Map(Object.entries(currentCase?.trace ?? {}));
  caseSelect.value = currentCase?.id ?? '';
  ruleSelect.value = currentRule?.id ?? '';
  const canonical = route(currentCase?.id, currentRule?.id);
  if (location.hash !== canonical) history.replaceState(null, '', canonical);
  required('#case-caption', HTMLElement).textContent = currentCase ? `${currentCase.id} / ${currentCase.label}` : 'Ruleset only';
  required('#counts', HTMLElement).textContent = `${data.rules.length} functions · ${data.inputCount} input facts`;
  const noMatch = [...traces.values()].filter(trace => trace.kind === 'no_match').length;
  const skipped = [...traces.values()].filter(trace => trace.kind === 'skipped').length;
  required('#run-summary', HTMLElement).textContent = currentCase ? `${Object.keys(currentCase.facts).length} new facts · ${noMatch} no match · ${skipped} skipped` : 'No evaluation';
  renderInspector();
  for (const { rule, node, value, title } of nodeElements) {
    const label = outputLabel(rule);
    node.setAttribute('href', route(currentCase?.id, rule.id));
    node.setAttribute('aria-label', `${rule.id}, ${status(rule)}`);
    node.setAttribute('aria-current', String(rule === currentRule));
    node.dataset.status = traces.get(rule.id)?.kind ?? 'not_evaluated';
    value.textContent = short(label);
    title.textContent = `${rule.id}\n${rule.requirement}\n${label}`;
  }
  for (const { from, to, path } of edgeElements) {
    const selected = from === currentRule?.id || to === currentRule?.id;
    path.classList.toggle('selected', selected);
    path.setAttribute('marker-end', selected ? 'url(#arrow-selected)' : 'url(#arrow)');
  }
  list.replaceChildren();
  for (const rule of data.rules) {
    const row = element('li');
    const link = ruleLink(rule.id, `${rule.id}()`);
    link.setAttribute('aria-current', String(rule === currentRule));
    row.append(link, element('p', status(rule)), element('p', `Reads: ${rule.inputs.map(fact => fact.name).join(', ') || 'none'}`));
    for (const fact of rule.outputs) row.append(element('p', `→ ${fact.name}: ${formatted(fact.name, facts.get(fact.name))}`));
    list.append(row);
  }
}

required('#title', HTMLElement).textContent = data.title;
document.title = `${data.title} | Inferrum`;
for (const snapshot of data.cases) caseSelect.add(new Option(`${snapshot.id} / ${snapshot.label}`, snapshot.id));
if (!data.cases.length) caseSelect.add(new Option('Ruleset only', ''));
caseSelect.disabled = data.cases.length < 2;
for (const rule of data.rules) ruleSelect.add(new Option(`${rule.id}()`, rule.id));
ruleSelect.disabled = !data.rules.length;
required('#empty', HTMLElement).hidden = !!data.rules.length;
diagram.toggleAttribute('hidden', !data.rules.length);
buildDiagram();
caseSelect.addEventListener('change', () => select(currentRule?.id, caseSelect.value));
ruleSelect.addEventListener('change', () => select(ruleSelect.value));
window.addEventListener('popstate', render);
window.addEventListener('hashchange', render);
required('#copy-link', HTMLButtonElement).addEventListener('click', async event => {
  const button = event.currentTarget;
  if (!(button instanceof HTMLButtonElement)) return;
  try {
    if (!navigator.clipboard) throw new Error('Clipboard unavailable');
    await navigator.clipboard.writeText(location.href);
    button.textContent = 'Link copied';
  } catch {
    button.textContent = 'Copy URL from address bar';
  }
});
render();
required('#viewer', HTMLElement).hidden = false;
