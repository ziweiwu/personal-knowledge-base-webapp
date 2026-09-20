import { expect, test, type Page } from '@playwright/test';
import { openDoc } from './helpers';

// A phone: the pane's scrollbar is invisible there, which is what the line is for.
test.use({ viewport: { width: 390, height: 844 } });

const progressBar = (page: Page) => page.getByRole('progressbar', { name: 'Reading progress' });

test('the line under the topbar follows how far the note has been read', async ({ page }) => {
  await openDoc(page, 'shapes', 'long-document.md');
  await expect(progressBar(page)).toHaveAttribute('aria-valuenow', '0');

  const pane = page.locator('.main-pane');
  await pane.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(progressBar(page)).toHaveAttribute('aria-valuenow', '100');

  await pane.evaluate((element) => {
    element.scrollTop = 0;
  });
  await expect(progressBar(page)).toHaveAttribute('aria-valuenow', '0');
});

test('the topbar keeps its height with the line inside it', async ({ page }) => {
  await openDoc(page, 'shapes', 'long-document.md');
  const heights = await page.evaluate(() => {
    const topbar = document.querySelector('.topbar') as HTMLElement;
    const expected = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--topbar-h'));
    return { actual: topbar.getBoundingClientRect().height, expected };
  });
  expect(heights.actual).toBe(heights.expected);
});

test('a folder listing has no reading progress', async ({ page }) => {
  await page.goto('/f/shapes');
  await expect(page.locator('.folder__toolbar')).toBeVisible();
  await expect(progressBar(page)).toHaveCount(0);
});

test('the line goes away while the editor is open', async ({ page }) => {
  await openDoc(page, 'shapes', 'long-document.md');
  await expect(progressBar(page)).toBeVisible();
  await page.getByRole('button', { name: 'Edit' }).click();
  await expect(page.locator('.editor')).toBeVisible();
  await expect(progressBar(page)).toBeHidden();
});
