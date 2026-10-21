const { test, expect } = require('@playwright/test');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const output = process.env.INFERRUM_BONUS_OUTPUT || path.resolve(__dirname, '../../target/bonus');
const file = path.resolve(output, 'index.html');
const viewer = pathToFileURL(file).href;
const payloadPattern = /(<script id="inferrum-data" type="application\/json">)([\s\S]*?)(<\/script>)/;

async function assertNoOverflow(page) {
  const overflow = await page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    return [...document.querySelectorAll('main *')].filter(element => {
      if (element instanceof SVGElement || !element.getClientRects().length) return false;
      const rect = element.getBoundingClientRect();
      return rect.right > width + 1 || rect.left < -1 ||
        element.scrollWidth > element.clientWidth + 1 && ['PRE', 'CODE', 'SUMMARY', 'LI', 'P'].includes(element.tagName);
    }).map(element => `${element.tagName}#${element.id}.${element.className}`);
  });
  expect(overflow).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
}

test('scenario changes update facts, trace, and inspector without moving the DAG', async ({ page }) => {
  const errors = [];
  const externalRequests = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => { if (/^https?:/.test(request.url())) externalRequests.push(request.url()); });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${viewer}#case=E01&rule=final_bonus`);
  await expect(page.locator('.rule-node')).toHaveCount(26);
  await expect(page.locator('#outputs [data-fact="final_bonus_cents"] .fact-value')).toHaveText('1,049,000 c');
  await expect(page.locator('#source')).toContainText('final_bonus');
  await expect(page.locator('#source')).toContainText('-> FINAL');
  const positions = await page.locator('.rule-node rect').evaluateAll(nodes => nodes.map(n => [n.getAttribute('x'), n.getAttribute('y')]));
  await page.selectOption('#case-select', 'E03');
  await expect(page.locator('#outputs .fact-value')).toHaveText('0 c');
  await expect(page.locator('#inputs [data-fact="approvals_satisfied"] .fact-value')).toHaveText('false');
  expect(await page.locator('.rule-node rect').evaluateAll(nodes => nodes.map(n => [n.getAttribute('x'), n.getAttribute('y')]))).toEqual(positions);
  await page.locator('.rule-node[data-rule="payment_reference"]').click();
  await expect(page.locator('#rule-status')).toHaveText('Skipped. Missing payment_ready_cents.');
  await expect(page.locator('#outputs .fact-value')).toHaveText('Absent');
  await page.locator('#inputs a').click();
  await expect(page.locator('#rule-title')).toHaveText('payment_ready()');
  await expect(page.locator('#rule-status')).toHaveText('No match. No new facts.');
  expect(errors).toEqual([]);
  expect(externalRequests).toEqual([]);
});

test('all scenarios display their exported bonus amounts', async ({ page }) => {
  const rows = readFileSync(path.resolve(output, 'scenarios.tsv'), 'utf8').trim().split('\n').slice(1);
  await page.goto(`${viewer}#rule=final_bonus`);
  for (const row of rows) {
    const [id, , expected] = row.split('\t');
    await page.selectOption('#case-select', id);
    await expect(page.locator('#outputs .fact-value')).toHaveText(`${BigInt(expected).toLocaleString('en-US')} c`);
  }
});

test('keyboard selection and URL history restore the selected case and rule', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${viewer}#case=E01&rule=final_bonus`);
  const node = page.locator('.rule-node[data-rule="review_required"]');
  await node.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#rule-title')).toHaveText('review_required()');
  await page.selectOption('#case-select', 'E03');
  await expect(page.locator('#outputs .fact-value')).toHaveText('"approval_pending"');
  await page.reload();
  await expect(page.locator('#rule-title')).toHaveText('review_required()');
  await expect(page.locator('#case-select')).toHaveValue('E03');
  await page.goBack();
  await expect(page.locator('#case-select')).toHaveValue('E01');
  await page.goBack();
  await expect(page.locator('#rule-title')).toHaveText('final_bonus()');
  await page.goto(`${viewer}#case=unknown&rule=unknown`);
  await expect(page.locator('#case-select')).toHaveValue('E01');
  await expect(page.locator('#rule-title')).toHaveText('good_performance()');
});

for (const width of [320, 390, 844, 1440]) {
  test(`viewer fits ${width}px with selected facts and expanded source`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 844 ? 390 : 900 });
    await page.goto(`${viewer}#case=E03&rule=payment_reference`);
    await expect(page.locator('#source-panel')).toHaveAttribute('open', '');
    await expect(page.locator('#source')).toBeVisible();
    await assertNoOverflow(page);
    if (width <= 1100) {
      await expect(page.locator('#diagram')).toBeHidden();
      await expect(page.locator('#rule-list li')).toHaveCount(26);
      await page.locator('#rule-list a').filter({ hasText: /^final_bonus\(\)$/ }).click();
      await expect(page.locator('#rule-title')).toHaveText('final_bonus()');
      await expect(page.locator('#inspector')).toBeFocused();
      await assertNoOverflow(page);
    } else {
      await expect(page.locator('#diagram')).toBeVisible();
    }
  });
}

test('long identifiers and HTML-like values stay readable and inert', async ({ page }) => {
  const longName = 'rule_with_a_very_long_identifier_'.repeat(4);
  const malicious = '</script><img src=x onerror="window.injected=true">';
  const html = readFileSync(file, 'utf8').replace(payloadPattern, (_, opening, raw, closing) => {
    const data = JSON.parse(raw);
    const rule = data.rules.find(rule => rule.id === 'final_bonus');
    rule.id = longName;
    rule.source = malicious.repeat(4);
    rule.requirement = malicious;
    for (const scenario of data.cases) {
      scenario.trace[longName] = scenario.trace.final_bonus;
      delete scenario.trace.final_bonus;
      scenario.facts.final_bonus_cents = malicious.repeat(3);
    }
    data.cases[0].label = longName;
    data.cases[1].facts.final_bonus_cents = '18446744073709551615';
    return opening + JSON.stringify(data).replaceAll('<', '\\u003c') + closing;
  });
  await page.route('http://inferrum.test/', route => route.fulfill({ contentType: 'text/html', body: html }));
  for (const width of [320, 390, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(`http://inferrum.test/#case=E01&rule=${longName}`);
    await expect(page.locator('#rule-title')).toHaveText(`${longName}()`);
    await expect(page.locator('#source')).toHaveText(malicious.repeat(4));
    await expect(page.locator('img')).toHaveCount(0);
    expect(await page.evaluate(() => window.injected)).toBeUndefined();
    await assertNoOverflow(page);
    await page.selectOption('#case-select', 'E02');
    await expect(page.locator('#outputs .fact-value')).toHaveText('18,446,744,073,709,551,615 c');
    await assertNoOverflow(page);
  }
});

test('C4 rule boxes and fact labels remain legible without overlapping', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${viewer}#case=E01&rule=final_bonus`);
  const data = JSON.parse(await page.locator('#inferrum-data').textContent());
  await expect(page.locator('.node-type')).toHaveCount(data.rules.length);
  await expect(page.locator('.edge-label')).toHaveCount(data.edges.length);
  const layoutErrors = await page.locator('#diagram').evaluate(diagram => {
    const errors = [];
    const boxes = [...diagram.querySelectorAll('.rule-node rect')].map(rect => rect.getBBox());
    for (const node of diagram.querySelectorAll('.rule-node')) {
      const box = node.querySelector('rect').getBBox();
      for (const text of node.querySelectorAll('text')) {
        const b = text.getBBox();
        if (b.x < box.x + 8 || b.x + b.width > box.x + box.width - 8 || b.y < box.y || b.y + b.height > box.y + box.height) errors.push(text.textContent);
      }
    }
    for (const label of diagram.querySelectorAll('.edge-label')) {
      const b = label.getBBox();
      if (boxes.some(box => b.x < box.x + box.width && b.x + b.width > box.x && b.y < box.y + box.height && b.y + b.height > box.y)) errors.push(label.dataset.fact);
    }
    return errors;
  });
  expect(layoutErrors).toEqual([]);
  const selected = page.locator('.rule-node[data-rule="final_bonus"]');
  expect((await selected.locator('.node-description').allTextContents()).join(' ')).toBe(data.rules.find(rule => rule.id === 'final_bonus').requirement);
  await page.getByRole('button', { name: 'Fit map', exact: true }).click();
  expect(await page.locator('#diagram-viewport').evaluate(viewport => viewport.scrollWidth <= viewport.clientWidth + 1 && viewport.scrollHeight <= viewport.clientHeight + 1)).toBe(true);
  await page.getByRole('button', { name: 'Actual size', exact: true }).click();
  const inView = await selected.evaluate(node => {
    const box = node.getBoundingClientRect(), viewport = document.querySelector('#diagram-viewport').getBoundingClientRect();
    return box.left >= viewport.left && box.right <= viewport.right && box.top >= viewport.top && box.bottom <= viewport.bottom;
  });
  expect(inView).toBe(true);
});

test('highlighted arrows flow toward consumers briefly and respect reduced motion', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto(`${viewer}#case=E01&rule=final_bonus`);
  const motion = await page.locator('.edge.selected').evaluateAll(paths => paths.map(path => {
    const animation = path.getAnimations()[0];
    if (!animation) return null;
    animation.pause();
    animation.currentTime = 0;
    const start = getComputedStyle(path).strokeDashoffset;
    animation.currentTime = 400;
    const moving = getComputedStyle(path).strokeDashoffset;
    const timing = animation.effect.getTiming();
    animation.finish();
    return { start, moving, duration: timing.duration, iterations: timing.iterations, state: animation.playState };
  }));
  expect(motion).toHaveLength(4);
  for (const arrow of motion) {
    expect(arrow).not.toBeNull();
    expect(parseFloat(arrow.moving)).toBeLessThan(parseFloat(arrow.start));
    expect(arrow.duration * arrow.iterations).toBeGreaterThan(0);
    expect(arrow.duration * arrow.iterations).toBeLessThanOrEqual(3000);
    expect(arrow.state).toBe('finished');
  }
  expect(await page.locator('.edge:not(.selected)').evaluateAll(paths => paths.every(path => path.getAnimations().length === 0))).toBe(true);
  await page.selectOption('#rule-select', 'capped_bonus');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect.poll(() => page.evaluate(() => document.getAnimations().length)).toBe(0);
  await page.selectOption('#rule-select', 'payment_ready');
  await expect(page.locator('.edge.selected')).toHaveCount(2);
  expect(await page.evaluate(() => document.getAnimations().length)).toBe(0);
});

test('topology-only and empty rulesets have usable states', async ({ page }) => {
  let empty = false;
  await page.route('http://inferrum.test/', route => {
    const html = readFileSync(file, 'utf8').replace(payloadPattern, (_, opening, raw, closing) => {
      const data = JSON.parse(raw);
      data.cases = [];
      if (empty) { data.rules = []; data.edges = []; }
      return opening + JSON.stringify(data).replaceAll('<', '\\u003c') + closing;
    });
    return route.fulfill({ contentType: 'text/html', body: html });
  });
  await page.goto('http://inferrum.test/');
  await expect(page.locator('#case-select')).toBeDisabled();
  await expect(page.locator('#rule-status')).toHaveText('Not evaluated');
  await expect(page.locator('#outputs .fact-value')).toHaveText('Not evaluated');
  empty = true;
  await page.reload();
  await expect(page.locator('#empty')).toHaveText('This ruleset has no functions.');
  await expect(page.locator('#inspector')).toBeHidden();
  await expect(page.locator('#rule-select')).toBeDisabled();
});
