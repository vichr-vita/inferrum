const { test, expect } = require('@playwright/test');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const mock = pathToFileURL(path.resolve(__dirname, '../../design/visualization-options.html')).href;

for (const width of [320, 390, 844, 1440]) {
  test(`populated options fit ${width}px with expanded source and long values`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 844 ? 390 : 900 });
    await page.goto(mock);
    await expect(page.locator('.catalog details')).toHaveCount(26);
    await expect(page.locator('tbody tr')).toHaveCount(14);
    await page.locator('#fn-payment_reference summary').click();
    await page.locator('#fn-payment_reference .catalog-value').evaluate(element => {
      element.textContent = 'BONUS-EMPLOYEE_WITH_A_VERY_LONG_IDENTIFIER_12345678901234567890-1049000';
    });
    await expect(page.locator('#fn-payment_reference pre')).toBeVisible();
    const overflow = await page.evaluate(() => {
      const width = document.documentElement.clientWidth;
      return [...document.querySelectorAll('main *')].filter(element => {
        if (element instanceof SVGElement || !element.getClientRects().length) return false;
        const rect = element.getBoundingClientRect();
        return rect.right > width + 1 || rect.left < -1 || element.scrollWidth > element.clientWidth + 1 && ['PRE', 'CODE', 'SUMMARY', 'TD'].includes(element.tagName);
      }).map(element => `${element.tagName}.${element.className}`);
    });
    expect(overflow).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
    if (width <= 390) {
      await expect(page.locator('#option-a .desktop-graph')).toBeHidden();
      await expect(page.locator('#option-a .mobile-rule')).toHaveCount(26);
    } else {
      await expect(page.locator('#option-a svg')).toBeVisible();
    }
    if (width === 1440 || width === 390) {
      await page.screenshot({ path: test.info().outputPath(`options-${width}.png`), fullPage: true });
    }
  });
}

test('function disclosure supports keyboard navigation', async ({ page }) => {
  await page.goto(mock);
  const summary = page.locator('#fn-review_required summary');
  await summary.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#fn-review_required pre')).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.locator('#fn-review_required pre')).toBeHidden();
});
