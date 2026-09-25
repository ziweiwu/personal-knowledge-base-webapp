import { expect, test, type Page } from '@playwright/test';
import { openDoc, openDrawer } from './helpers';

/**
 * The shell around a note: a 44px strip that leaves while the reader scrolls down, a
 * document list that is a drawer at every width, and on a wide screen a reading column
 * with the contents in the left margin and the tags and links in the right.
 */

const scrollPane = (page: Page, top: number) =>
  page.locator('.main-pane').evaluate((pane, offset) => {
    pane.scrollTop = offset;
  }, top);

const stripTop = (page: Page) => page.locator('.topbar').evaluate((strip) => strip.getBoundingClientRect().top);

test.describe('top strip', () => {
  test('is 44px, leaves on the way down and returns on the way up', async ({ page }) => {
    await openDoc(page, 'shapes', 'long-document.md');
    const shell = page.locator('.app-shell');
    expect(await page.locator('.topbar').evaluate((strip) => strip.getBoundingClientRect().height)).toBe(44);

    await scrollPane(page, 600);
    await expect(shell).toHaveClass(/app-shell--strip-hidden/);
    await expect.poll(() => stripTop(page)).toBeLessThan(0);

    await scrollPane(page, 500);
    await expect(shell).not.toHaveClass(/app-shell--strip-hidden/);
    await expect.poll(() => stripTop(page)).toBe(0);
  });

  test('never leaves a short page', async ({ page }) => {
    await openDoc(page, 'shapes', 'index.md');
    // Size the window so the note scrolls, but only just: too little range to be worth the room.
    const scrollHeight = await page.locator('.main-pane').evaluate((pane) => pane.scrollHeight);
    await page.setViewportSize({ width: 1280, height: scrollHeight - 120 });
    await scrollPane(page, 100);
    await expect.poll(() => page.locator('.main-pane').evaluate((pane) => pane.scrollTop)).toBe(100);
    await expect(page.locator('.app-shell')).not.toHaveClass(/app-shell--strip-hidden/);
  });

  test('comes back when a control in it takes focus', async ({ page }) => {
    await openDoc(page, 'shapes', 'long-document.md');
    await scrollPane(page, 600);
    await expect(page.locator('.app-shell')).toHaveClass(/app-shell--strip-hidden/);
    await page.getByRole('button', { name: 'Open document list' }).focus();
    await expect.poll(() => stripTop(page)).toBe(0);
  });
});

test.describe('document drawer', () => {
  test('opens from the strip, sits below it, and closes once the reader has gone somewhere', async ({ page }) => {
    await openDoc(page, 'shapes', 'index.md');
    const drawer = page.locator('#sidebar-drawer');
    await expect(drawer).toBeHidden();

    await openDrawer(page);
    expect(await drawer.evaluate((node) => node.getBoundingClientRect().top)).toBe(44);
    expect(await page.locator('.main-pane').evaluate((node) => (node as HTMLElement).inert)).toBe(true);
    await expect(page.getByRole('button', { name: 'Close document list' })).toBeVisible();

    await drawer.locator('.tree__link').filter({ hasText: 'long-document.md' }).first().click();
    await expect(page).toHaveURL(/long-document\.md$/);
    await expect(drawer).toBeHidden();
  });

  test('Escape closes it and hands focus back to the toggle', async ({ page }) => {
    await openDoc(page, 'shapes', 'index.md');
    await openDrawer(page);
    await page.keyboard.press('Escape');
    await expect(page.locator('#sidebar-drawer')).toBeHidden();
    await expect(page.getByRole('button', { name: 'Open document list' })).toBeFocused();
  });

  test('keeps the strip on screen while open', async ({ page }) => {
    await openDoc(page, 'shapes', 'long-document.md');
    await scrollPane(page, 600);
    await expect(page.locator('.app-shell')).toHaveClass(/app-shell--strip-hidden/);
    await page.getByRole('button', { name: 'Open document list' }).focus();
    await page.keyboard.press('Enter');
    await expect(page.locator('#sidebar-drawer')).toBeVisible();
    await expect(page.locator('.app-shell')).not.toHaveClass(/app-shell--strip-hidden/);
  });
});

test.describe('reading column', () => {
  const rects = (page: Page) =>
    page.evaluate(() => {
      const rect = (selector: string) => document.querySelector(selector)?.getBoundingClientRect() ?? null;
      return {
        head: rect('.doc__head'),
        rail: rect('.toc--rail'),
        prose: rect('.doc__layout > .doc__inner'),
        margin: rect('.doc__margin'),
      };
    });

  test('on a wide screen the contents sit in the left margin and the links in the right', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await openDoc(page, 'shapes', 'long-document.md');
    await expect(page.locator('.doc__margin .linkrefs').first()).toBeVisible();
    const { head, rail, prose, margin } = await rects(page);
    expect(rail && prose && margin && head).toBeTruthy();
    expect(rail!.right).toBeLessThanOrEqual(prose!.left);
    expect(margin!.left).toBeGreaterThanOrEqual(prose!.right);
    expect(Math.abs(rail!.top - prose!.top)).toBeLessThanOrEqual(4);
    expect(Math.abs(margin!.top - prose!.top)).toBeLessThanOrEqual(4);
    expect(Math.abs(head!.left - prose!.left)).toBeLessThanOrEqual(2);
  });

  test('the tags move into the margin on a wide screen and stay under the title otherwise', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await openDoc(page, 'shapes', 'frontmatter-callout-tasks.md');
    await expect(page.locator('.doc__margin .doc__tags')).toHaveCount(1);
    await expect(page.locator('.doc__head .doc__tags')).toHaveCount(0);

    await page.setViewportSize({ width: 1000, height: 800 });
    await expect(page.locator('.doc__head .doc__tags')).toHaveCount(1);
    await expect(page.locator('.doc__margin .doc__tags')).toHaveCount(0);
  });

  test('below the wide breakpoint the margins fold: contents above the text, links below it', async ({ page }) => {
    await page.setViewportSize({ width: 1000, height: 800 });
    await openDoc(page, 'shapes', 'long-document.md');
    await expect(page.locator('.toc--rail')).toBeHidden();
    await expect(page.locator('.toc-mobile')).toBeVisible();
    const { prose, margin } = await rects(page);
    expect(margin!.top).toBeGreaterThanOrEqual(prose!.bottom);
  });
});
