import { expect, test } from '@playwright/test';
import { openDoc, openDrawer } from './helpers';

/**
 * Two layers open at once.
 *
 * Every other spec sets up exactly one — the shared `openDrawer` helper even says to close
 * the drawer before touching the document — so the whole class of ordering bug was
 * untested: which layer Escape closes, whether a shortcut can summon a third on top of an
 * inert page, and whether the scroll lock survives two holders releasing in one commit.
 */

const bodyOverflow = (page: import('@playwright/test').Page) =>
  page.evaluate(() => getComputedStyle(document.body).overflow);

test.describe('two layers at once', () => {
  test('Escape closes the dialog above the drawer, then the drawer under it', async ({ page }) => {
    await openDoc(page, 'shapes', 'index.md');
    const drawer = await openDrawer(page);

    await page.getByRole('button', { name: 'New note in the current folder' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(drawer).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(drawer).toBeHidden();
  });

  test('dismissing both layers leaves the page scrollable', async ({ page }) => {
    await openDoc(page, 'shapes', 'index.md');
    await openDrawer(page);
    await page.getByRole('button', { name: 'New note in the current folder' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    expect(await bodyOverflow(page)).toBe('hidden');

    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');
    await expect(page.locator('#sidebar-drawer')).toBeHidden();

    // Two holders released in one commit each used to write back its own remembered value,
    // so whichever went second restored `hidden` and the page never scrolled again.
    expect(await bodyOverflow(page)).not.toBe('hidden');
  });

  test('the home page still answers the wheel after a drawer and a dialog have been open together', async ({
    page,
  }) => {
    await openDoc(page, 'shapes', 'index.md');
    await openDrawer(page);
    await page.getByRole('button', { name: 'New note in the current folder' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');

    // The home route is outside the shell, so `body` is its scroller rather than `.main-pane`,
    // and a leaked lock is total there instead of invisible. Sized so there is a little to
    // scroll to, and driven by a real wheel gesture: `overflow: hidden` still permits a
    // scripted `scrollTo`, so only genuine input can tell a locked page from a free one.
    await page.setViewportSize({ width: 800, height: 400 });
    await page.goto('/');
    await expect(page.locator('.home')).toBeVisible();
    await page.mouse.move(400, 200);
    await page.mouse.wheel(0, 300);
    await expect.poll(() => page.evaluate(() => document.body.scrollTop)).toBeGreaterThan(0);
  });

  test('the search shortcut replaces the drawer instead of floating over an inert page', async ({ page }) => {
    await openDoc(page, 'shapes', 'index.md');
    await openDrawer(page);

    await page.keyboard.press('ControlOrMeta+k');
    await expect(page.getByRole('dialog', { name: /search/i })).toBeVisible();
    await expect(page.locator('#sidebar-drawer')).toBeHidden();

    // One Escape, and the reader is back on a page that answers.
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog', { name: /search/i })).toBeHidden();
    expect(await page.locator('.main-pane').evaluate((pane) => (pane as HTMLElement).inert)).toBe(false);
  });

  test('the focus-mode shortcut cannot hide the strip holding the drawer toggle', async ({ page }) => {
    await openDoc(page, 'shapes', 'long-document.md');
    await openDrawer(page);

    await page.keyboard.press('f');
    await expect(page.locator('.app-shell')).not.toHaveClass(/app-shell--focus/);
    await expect(page.getByRole('button', { name: 'Close document list' })).toBeVisible();

    // Still available once the drawer is out of the way, so the guard did not cost the key.
    await page.keyboard.press('Escape');
    await expect(page.locator('#sidebar-drawer')).toBeHidden();
    await page.keyboard.press('f');
    await expect(page.locator('.app-shell')).toHaveClass(/app-shell--focus/);
  });
});
