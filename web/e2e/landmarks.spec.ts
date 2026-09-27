import { expect, test } from '@playwright/test';

/**
 * Every screen starts the same way for a keyboard.
 *
 * A reader learns the first tab stop once — it is a skip link — and then expects it on
 * every screen. The trash page is rendered outside the shell, so it had neither the link
 * nor a `main` landmark at all, and the home page had the target without ever offering the
 * link: WCAG 2.4.1 on one route each, and a surprise on both. The link only moves focus if
 * its target can take it, so the target's `tabindex` is part of what is checked here.
 */

const SCREENS = [
  { what: 'the home page', at: '/' },
  { what: 'a document', at: '/n/shapes/links.md' },
  { what: 'a folder', at: '/f/shapes' },
  { what: 'the trash', at: '/trash/shapes' },
] as const;

for (const { what, at } of SCREENS) {
  test(`${what} opens onto a skip link that moves focus into its main landmark`, async ({ page }) => {
    await page.goto(at);

    const skip = page.getByRole('link', { name: 'Skip to content' });
    await expect(skip).toHaveAttribute('href', '#main-content');

    // The first Tab from a freshly loaded page has to reach it, or it is decoration.
    await page.keyboard.press('Tab');
    await expect(skip).toBeFocused();

    await page.keyboard.press('Enter');
    const main = page.locator('#main-content');
    await expect(main).toHaveCount(1);
    await expect(main).toBeFocused();
  });
}
