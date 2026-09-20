import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

const PATH = 'conflict-diff.md';
const ORIGINAL = '# Conflict diff\n\nfirst line\nsecond line\nthird line\n';

/**
 * A 409 is provoked the way it happens for real: the note is opened in the editor,
 * something else rewrites it on disk, and the editor's save carries a stale base.
 */
test.describe('conflict dialog line diff', () => {
  test('only the lines that differ are highlighted, on both sides', async ({ page }) => {
    await page.request.post(`/api/doc/shapes/${PATH}`, { data: { content: ORIGINAL, baseMtimeMs: 0 } });
    await openDoc(page, 'shapes', PATH);
    await page.getByRole('button', { name: /^edit$/i }).click();
    const editor = page.locator('.cm-content');
    await expect(editor).toBeVisible();

    // A clean buffer would simply adopt the disk change; a dirty one keeps its stale base.
    await editor.click();
    await page.keyboard.press('ControlOrMeta+End');
    // The file ends with a newline, so End already sits on an empty last line.
    await page.keyboard.type('fourth line, typed here');
    // Behind the editor's back: read the current mtime and rewrite one line with it.
    const payload = await (await page.request.get(`/api/doc/shapes/${PATH}`)).json();
    const behind = await page.request.put(`/api/doc/shapes/${PATH}`, {
      data: {
        content: ORIGINAL.replace('second line', 'second line, rewritten on disk'),
        baseMtimeMs: payload.meta.mtimeMs,
      },
    });
    expect(behind.ok()).toBeTruthy();

    await page.getByRole('button', { name: /^save$/i }).click();

    const dialog = page.getByRole('dialog', { name: /changed on disk/i });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('status')).toContainText(/3 lines differ/);

    const panes = dialog.locator('.conflict__pane');
    const mine = panes.nth(0).locator('.conflict__line--changed');
    const disk = panes.nth(1).locator('.conflict__line--changed');
    await expect(mine).toHaveCount(2);
    await expect(mine.nth(0)).toContainText('second line');
    await expect(mine.nth(1)).toContainText('fourth line, typed here');
    await expect(disk).toHaveCount(1);
    await expect(disk).toContainText('rewritten on disk');
    await expect(panes.nth(0).locator('.conflict__line:not(.conflict__line--changed)')).toHaveCount(4);

    await dialog.getByRole('button', { name: /jump to first change/i }).click();
    await expect(page.locator('#conflict-first-change-mine')).toBeFocused();

    await dialog.getByRole('button', { name: /cancel/i }).click();
  });
});
