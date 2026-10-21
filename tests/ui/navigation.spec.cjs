const { test, expect } = require('@playwright/test');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const output = process.env.INFERRUM_BONUS_OUTPUT || path.resolve(__dirname, '../../target/bonus');
const viewer = pathToFileURL(path.resolve(output, 'index.html')).href;

test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${viewer}#case=E01&rule=final_bonus`);
  await expect(page.getByRole('button', { name: 'Fit map', exact: true })).toHaveAttribute('aria-pressed', 'true');
  expect(await page.locator('#diagram-viewport').evaluate(el => el.scrollWidth <= el.clientWidth + 1 && el.scrollHeight <= el.clientHeight + 1)).toBe(true);
  await page.getByRole('button', { name: 'Actual size', exact: true }).click();
});

async function cameraAt(page, point) {
  return page.locator('#diagram').evaluate((diagram, point) => {
    const bounds = diagram.getBoundingClientRect();
    const zoom = bounds.width / diagram.viewBox.baseVal.width;
    return { zoom, x: (point.x - bounds.left) / zoom, y: (point.y - bounds.top) / zoom };
  }, point);
}

test('dragging a rule pans without selecting it and releases outside the canvas', async ({ page }) => {
  const viewport = page.locator('#diagram-viewport');
  const node = page.locator('.rule-node[data-rule="approvals_satisfied"]');
  const box = await node.boundingBox();
  const start = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const before = await viewport.evaluate(el => ({ x: el.scrollLeft, y: el.scrollTop }));
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(start.x - 90, start.y - 60, { steps: 8 });
  await expect(viewport).toHaveClass('dragging');
  const during = await viewport.evaluate(el => ({ x: el.scrollLeft, y: el.scrollTop }));
  expect(during.x - before.x).toBeCloseTo(90, 0);
  expect(during.y - before.y).toBeCloseTo(60, 0);
  await page.mouse.move(20, start.y - 60, { steps: 5 });
  await page.mouse.up();
  await expect(viewport).not.toHaveClass('dragging');
  await expect(page.locator('#rule-title')).toHaveText('final_bonus()');
  const released = await viewport.evaluate(el => el.scrollLeft);
  await page.mouse.move(start.x, start.y);
  expect(await viewport.evaluate(el => el.scrollLeft)).toBe(released);
  await page.getByRole('button', { name: 'Actual size', exact: true }).click();
  await page.locator('.rule-node[data-rule="approvals_satisfied"]').click();
  await expect(page.locator('#rule-title')).toHaveText('approvals_satisfied()');
});

test('trackpad pinch anchors zoom, preserves regular scrolling, and supports keyboard reset', async ({ page }) => {
  const viewport = page.locator('#diagram-viewport');
  const box = await viewport.boundingBox();
  const point = { x: box.x + box.width * 0.55, y: box.y + box.height * 0.45 };
  const before = await cameraAt(page, point);
  await page.mouse.move(point.x, point.y);
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, -35);
  await page.keyboard.up('Control');
  await expect.poll(async () => (await cameraAt(page, point)).zoom).toBeGreaterThan(before.zoom);
  const after = await cameraAt(page, point);
  expect(Math.abs(after.x - before.x)).toBeLessThan(2);
  expect(Math.abs(after.y - before.y)).toBeLessThan(2);
  const scroll = await viewport.evaluate(el => el.scrollTop);
  await page.mouse.wheel(0, 100);
  await expect.poll(() => viewport.evaluate(el => el.scrollTop)).toBeGreaterThan(scroll);
  const panned = await viewport.evaluate(el => ({ x: el.scrollLeft, y: el.scrollTop }));
  await page.selectOption('#case-select', 'E03');
  expect(await viewport.evaluate(el => ({ x: el.scrollLeft, y: el.scrollTop }))).toEqual(panned);
  expect((await cameraAt(page, point)).zoom).toBeCloseTo(after.zoom, 4);
  await viewport.focus();
  await page.keyboard.press('+');
  expect((await cameraAt(page, point)).zoom).toBeGreaterThan(after.zoom);
  await page.keyboard.press('0');
  expect(await viewport.evaluate(el => el.scrollWidth <= el.clientWidth + 1 && el.scrollHeight <= el.clientHeight + 1)).toBe(true);
  await page.keyboard.press('1');
  expect((await cameraAt(page, point)).zoom).toBeCloseTo(1, 4);
});

test('Safari pinch events use cumulative gesture scale without zooming the page', async ({ page }) => {
  const viewport = page.locator('#diagram-viewport');
  const box = await viewport.boundingBox();
  const point = { x: box.x + box.width * 0.55, y: box.y + box.height * 0.45 };
  for (const reportedPoint of [point, { x: 0, y: 0 }]) {
    await page.getByRole('button', { name: 'Actual size', exact: true }).click();
    await page.mouse.move(point.x, point.y);
    const before = await cameraAt(page, point);
    for (const [type, scale] of [['gesturestart', 1], ['gesturechange', 1.2], ['gesturechange', 1.5], ['gestureend', 1.5]]) {
      const canceled = await viewport.evaluate((el, data) => {
        const event = new Event(data.type, { bubbles: true, cancelable: true });
        Object.assign(event, { scale: data.scale, clientX: data.point.x, clientY: data.point.y });
        return !el.dispatchEvent(event);
      }, { type, scale, point: reportedPoint });
      expect(canceled).toBe(true);
    }
    const after = await cameraAt(page, point);
    expect(after.zoom).toBeCloseTo(before.zoom * 1.5, 3);
    expect(Math.abs(after.x - before.x)).toBeLessThan(2);
    expect(Math.abs(after.y - before.y)).toBeLessThan(2);
  }
  await page.getByRole('button', { name: 'Fit map', exact: true }).click();
  await page.setViewportSize({ width: 1280, height: 900 });
  await expect.poll(() => viewport.evaluate(el => el.scrollWidth <= el.clientWidth + 1 && el.scrollHeight <= el.clientHeight + 1)).toBe(true);
});

test('two-finger touch combines panning and pinch zoom', async ({ page, browserName }) => {
  test.skip(browserName !== 'chromium', 'CDP supplies real multi-touch input; Safari gestures are covered separately.');
  const viewport = page.locator('#diagram-viewport');
  const box = await viewport.boundingBox();
  const point = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const before = await cameraAt(page, point);
  const session = await page.context().newCDPSession(page);
  const touch = (id, x, y) => ({ id, x, y });
  await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [touch(1, point.x - 60, point.y), touch(2, point.x + 60, point.y)] });
  await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [touch(1, point.x - 60, point.y - 20), touch(2, point.x + 120, point.y - 20)] });
  await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  const after = await cameraAt(page, { x: point.x + 30, y: point.y - 20 });
  expect(after.zoom).toBeCloseTo(before.zoom * 1.5, 2);
  expect(Math.abs(after.x - before.x)).toBeLessThan(2);
  expect(Math.abs(after.y - before.y)).toBeLessThan(2);
  await expect(viewport).not.toHaveClass('dragging');
  await expect(page.locator('#rule-title')).toHaveText('final_bonus()');
  await session.detach();
});
