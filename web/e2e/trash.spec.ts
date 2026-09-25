import { expect, test } from '@playwright/test';
import { openDrawer, readRootFile } from './helpers';

const NOTE = 'trash-me.md';
const CONTENT = '# Trash me\n\nA note the e2e suite deletes and then restores.\n';

test.describe('trash', () => {
  /**
   * Delete is a move into `.trash/`, so the undo has to be reachable from the app: a
   * phone has no file manager to dig the note out with.
   */
  test('a deleted note is listed in the trash and restore puts it back on disk', async ({ page }) => {
    // page.request shares the browser context's cookies, so this rides the stored login.
    const created = await page.request.post(`/api/doc/shapes/${NOTE}`, { data: { content: CONTENT, baseMtimeMs: 0 } });
    expect(created.ok()).toBe(true);
    const deleted = await page.request.delete(`/api/doc/shapes/${NOTE}`);
    expect(deleted.ok()).toBe(true);

    await page.goto('/trash/shapes');
    const row = page.getByRole('listitem').filter({ hasText: NOTE });
    await expect(row).toBeVisible();

    await row.getByRole('button', { name: /restore/i }).click();
    await expect(page.getByText(`Restored ${NOTE}`)).toBeVisible();
    await expect(row).toHaveCount(0);
    expect(readRootFile('content-shapes', NOTE)).toContain('deletes and then restores');
  });

  test('a collection with nothing deleted says so', async ({ page }) => {
    await page.goto('/trash/plain');
    await expect(page.getByText('Trash is empty')).toBeVisible();
  });

  test('the sidebar links to the trash', async ({ page }) => {
    await page.goto('/f/plain');
    await openDrawer(page);
    await page.getByRole('button', { name: /open the trash/i }).click();
    await expect(page).toHaveURL(/\/trash\/plain$/);
    await expect(page.getByRole('heading', { name: 'Trash' })).toBeVisible();
  });
});
