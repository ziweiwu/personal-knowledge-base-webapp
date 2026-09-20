import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * Runs in every project: on the phones the contents live in a bottom sheet behind a
 * floating button; on the desktop the button must not exist at all.
 */
const isPhone = () => test.info().project.name.startsWith('phone');

test('on a phone the contents open from anywhere and jump to a section', async ({ page }) => {
  test.skip(!isPhone(), 'the sheet only exists below the drawer breakpoint');
  await openDoc(page, 'shapes', 'long-document.md');

  await expect(page.locator('.toc-mobile')).toHaveCount(0);
  const button = page.getByRole('button', { name: 'Table of contents' });
  await expect(button).toBeVisible();

  // Read a little way in first: the button must still be reachable without scrolling back.
  await page.locator('#section-2').evaluate((heading) => heading.scrollIntoView({ block: 'start' }));
  await button.click();

  const sheet = page.getByRole('dialog', { name: 'Contents' });
  await expect(sheet).toBeVisible();
  await expect(sheet.locator('[aria-current="location"]')).toHaveText('Section 2');

  await sheet.getByRole('link', { name: 'Section 3', exact: true }).click();
  await expect(sheet).toBeHidden();
  await expect(page).toHaveURL(/#section-3$/);
  const box = await page.locator('#section-3').boundingBox();
  const height = page.viewportSize()?.height ?? 0;
  expect(box).not.toBeNull();
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.y).toBeLessThan(height);
  await expect(button).toBeFocused();
});

test('on a phone the sheet closes on Escape and on the scrim', async ({ page }) => {
  test.skip(!isPhone(), 'the sheet only exists below the drawer breakpoint');
  await openDoc(page, 'shapes', 'long-document.md');
  const button = page.getByRole('button', { name: 'Table of contents' });

  await button.click();
  const sheet = page.getByRole('dialog', { name: 'Contents' });
  await expect(sheet).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(sheet).toBeHidden();

  await button.click();
  await expect(sheet).toBeVisible();
  await page.locator('.toc-sheet-scrim').click({ position: { x: 20, y: 20 } });
  await expect(sheet).toBeHidden();
});

test('on the desktop the contents stay in the rail and there is no floating button', async ({ page }) => {
  test.skip(isPhone(), 'desktop only');
  await openDoc(page, 'shapes', 'long-document.md');
  await expect(page.locator('.toc--rail')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Table of contents' })).toHaveCount(0);
});
