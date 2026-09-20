import { expect, test, type Page } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * Text of every node in the app chrome, leaving out rendered markdown: a note may
 * legitimately contain emoji, the chrome around it must not.
 */
async function chromeText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const parts: string[] = [];
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (node.parentElement?.closest('.prose, script, style')) continue;
      parts.push(node.textContent ?? '');
    }
    return parts.join(' ');
  });
}

const EMOJI = /\p{Extended_Pictographic}/u;

test.describe('icon set', () => {
  test('the sidebar, tree rows and theme toggle draw inline SVG icons', async ({ page }) => {
    await openDoc(page, 'shapes', 'currency.md');
    expect(await page.locator('.sidebar svg.icon').count()).toBeGreaterThan(0);
    expect(await page.locator('.tree__row svg.icon').count()).toBeGreaterThan(0);
    const themeToggle = page.getByRole('button', { name: /theme\. Switch to/ });
    await expect(themeToggle.locator('svg.icon')).toHaveCount(1);
  });

  test('no emoji is left in the chrome of the home, folder and document pages', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('region', { name: 'Collections' })).toBeVisible();
    expect(await chromeText(page)).not.toMatch(EMOJI);

    await page.goto('/f/shapes');
    await expect(page.locator('.entries__item').first()).toBeVisible();
    expect(await chromeText(page)).not.toMatch(EMOJI);

    await openDoc(page, 'shapes', 'currency.md');
    expect(await chromeText(page)).not.toMatch(EMOJI);
  });
});
