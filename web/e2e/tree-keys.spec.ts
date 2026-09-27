import { expect, test } from '@playwright/test';
import { openDoc, openDrawer } from './helpers';

/**
 * Focus lands on a row's link, so "the focused row" is read back as the active element's
 * text. The `deep` folder is the one fixture folder with a nested child, which is what
 * stepping into and out of a folder needs.
 */
function focusedText(page: Parameters<typeof openDoc>[0]): Promise<string> {
  return page.evaluate(() => (document.activeElement as HTMLElement).innerText);
}

test('down and up arrows move between tree rows', async ({ page }) => {
  await openDoc(page, 'shapes', 'index.md');
  await openDrawer(page);
  const links = page.locator('#sidebar-drawer .tree__link');
  await links.first().focus();
  const first = await focusedText(page);

  await page.keyboard.press('ArrowDown');
  expect(await focusedText(page)).not.toBe(first);
  await expect(links.nth(1)).toBeFocused();

  await page.keyboard.press('ArrowUp');
  await expect(links.first()).toBeFocused();
});

test('right arrow opens a folder and steps in; left steps out and closes it', async ({ page }) => {
  await openDoc(page, 'shapes', 'index.md');
  await openDrawer(page);
  const row = page.locator('#sidebar-drawer .tree__row').filter({ has: page.getByText('deep', { exact: true }) });
  const twisty = row.getByRole('button', { name: /^(expand|collapse) deep$/i });
  if ((await twisty.getAttribute('aria-expanded')) === 'true') await twisty.click();
  await expect(twisty).toHaveAttribute('aria-expanded', 'false');

  await row.locator('.tree__link').focus();
  await page.keyboard.press('ArrowRight');
  await expect(twisty).toHaveAttribute('aria-expanded', 'true');
  await expect(page.locator('#sidebar-drawer').getByText('nested', { exact: true })).toBeVisible();

  await page.keyboard.press('ArrowRight');
  expect(await focusedText(page)).toContain('nested');
  await page.keyboard.press('ArrowLeft');
  expect(await focusedText(page)).toContain('deep');
  await page.keyboard.press('ArrowLeft');
  await expect(twisty).toHaveAttribute('aria-expanded', 'false');
});
