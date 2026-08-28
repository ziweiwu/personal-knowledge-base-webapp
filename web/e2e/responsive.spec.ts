import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * Runs in the phone-portrait and phone-landscape projects. Landscape is the orientation
 * that has broken twice: once on a fixed tail padding that ate a quarter of the screen,
 * and once on the notch, which nothing was reserving space for.
 */
test('the page never scrolls sideways, whatever the document holds', async ({ page }) => {
  for (const path of ['tables.md', 'code-and-highlighting.md', 'long-document.md']) {
    await openDoc(page, 'shapes', path);
    const overflows = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(overflows, path).toBe(false);
  }
});

test('the tail below the document is scroll-past room, not a dead band', async ({ page }) => {
  await openDoc(page, 'shapes', 'index.md');
  const { paneHeight, tail } = await page.evaluate(() => {
    const pane = document.querySelector('.main-pane') as HTMLElement;
    const doc = pane.firstElementChild as HTMLElement;
    return {
      paneHeight: pane.getBoundingClientRect().height,
      tail: parseFloat(getComputedStyle(doc).paddingBottom),
    };
  });
  // A fixed 96px was a ninth of a portrait phone and over a quarter of a landscape one.
  expect(tail).toBeLessThan(paneHeight / 4);
  expect(tail).toBeGreaterThanOrEqual(16);
});

test('the drawer opens, takes focus, and neutralises the page behind it', async ({ page }) => {
  await openDoc(page, 'shapes', 'index.md');
  await page.getByRole('button', { name: /open document list/i }).click();

  const drawer = page.locator('#sidebar-drawer');
  await expect(drawer).toBeVisible();
  expect(await page.locator('.main-pane').evaluate((node) => (node as HTMLElement).inert)).toBe(true);

  // The scrim covers the viewport, but the drawer covers most of the scrim — a click at
  // its centre lands on the drawer. Aim at the strip left uncovered beside it.
  const width = page.viewportSize()?.width ?? 390;
  await page.locator('.scrim').click({ position: { x: width - 8, y: 200 } });
  await expect(drawer).toBeHidden();
});

test('focus reading hides the chrome and always leaves a way back', async ({ page }) => {
  await openDoc(page, 'shapes', 'long-document.md');

  await page.getByRole('button', { name: /focus reading mode/i }).click();
  await expect(page.locator('.app-shell--focus')).toBeVisible();
  await expect(page.locator('.topbar')).toBeHidden();
  await expect(page.locator('.prose')).toBeVisible();

  // On a phone there is no Escape key, so the exit control is the only way out and must
  // be a real target, clear of the notch.
  const exit = page.getByRole('button', { name: /exit focus reading/i });
  await expect(exit).toBeVisible();
  const box = await exit.boundingBox();
  expect(box?.width).toBeGreaterThanOrEqual(24);
  expect(box?.height).toBeGreaterThanOrEqual(24);

  await exit.click();
  await expect(page.locator('.topbar')).toBeVisible();
});

test('every interactive control clears the 24px minimum target size', async ({ page }) => {
  await openDoc(page, 'shapes', 'tasks.md');
  const undersized = await page.evaluate(() => {
    const targets = [...document.querySelectorAll('button, a[href], input, summary')];
    return targets
      .filter((node) => {
        const rect = node.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return false; // not rendered
        // Inline links inside prose are exempt under WCAG 2.5.8.
        if (node.closest('.prose') && node.tagName === 'A') return false;
        // A visually-hidden proxy is not the target — the visible control that opens it
        // is, and that one is measured here like anything else. The file input behind the
        // Upload button is the case in point.
        if (node.classList.contains('sr-only')) return false;
        return rect.width < 24 || rect.height < 24;
      })
      .map((node) => `${node.tagName}.${node.className}`);
  });
  expect(undersized).toEqual([]);
});
