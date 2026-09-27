import { expect, test, type Page } from '@playwright/test';
import { openDoc, openDrawer, readRootFile } from './helpers';

/**
 * The tree menu closes on any scroll, and a freshly opened document scrolls its active
 * row into view a moment after load, so opening the menu is retried until it holds.
 */
async function chooseMove(page: Page, name: string): Promise<void> {
  await expect(async () => {
    await page.getByRole('button', { name: `Actions for ${name}` }).click();
    await page.getByRole('menuitem', { name: /move…/i }).click({ timeout: 2_000 });
  }).toPass();
}

/**
 * Move reuses the rename route with a different parent. Both the note and the linker are
 * created here so no other spec's fixture is moved out from under it; the rename spec
 * already proves that links are rewritten, this one proves the file itself lands.
 */
test.describe('move', () => {
  test('a note moved from the actions menu lands in the chosen folder and keeps its links', async ({ page }) => {
    await page.request.post('/api/doc/shapes/move-me.md', {
      data: { content: '# Move me\n', baseMtimeMs: 0 },
    });
    await page.request.post('/api/doc/shapes/move-linker.md', {
      data: { content: 'Points at [[move-me]].\n', baseMtimeMs: 0 },
    });
    await openDoc(page, 'shapes', 'move-me.md');
    // The tree reloads once the watcher reports the two new files; wait for that to land.
    await openDrawer(page);
    await expect(
      page
        .locator('.tree')
        .getByRole('link', { name: /move-linker\.md/ })
        .first(),
    ).toBeVisible();
    await chooseMove(page, 'move-me.md');

    const dialog = page.getByRole('dialog', { name: 'Move note' });
    await expect(dialog).toBeVisible();
    // The note already lives at the top level, so that choice cannot be confirmed.
    await expect(dialog.getByRole('button', { name: /^move to/i })).toBeDisabled();
    await dialog.getByRole('radio', { name: 'deep', exact: true }).click();
    await dialog.getByRole('button', { name: 'Move to “deep”' }).click();

    await expect(page).toHaveURL(/\/n\/shapes\/deep\/move-me\.md$/);
    await expect(page.locator('.toast').filter({ hasText: /^Moved to deep\/move-me\.md/ })).toBeVisible();
    // Arriving at the moved note closed the drawer; the tree is proof it landed.
    await openDrawer(page);
    await expect(
      page
        .locator('.tree')
        .getByRole('link', { name: /move-me\.md/ })
        .first(),
    ).toBeVisible();
    await expect.poll(() => readRootFile('content-shapes', 'deep/move-me.md')).toContain('# Move me');
    // The bare name still resolves after the move, so the author's shorthand is kept as is.
    expect(readRootFile('content-shapes', 'move-linker.md')).toContain('[[move-me]]');
  });

  test('the picker can be driven with arrow keys, and a folder cannot be moved into itself', async ({ page }) => {
    await page.request.post('/api/folder/shapes/move-folder', { data: {} });
    await page.goto('/f/shapes/move-folder');
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();

    await openDrawer(page);
    await chooseMove(page, 'move-folder');
    const dialog = page.getByRole('dialog', { name: 'Move folder' });
    await expect(dialog.getByRole('radio', { name: 'move-folder', exact: true })).toBeDisabled();

    await dialog.getByRole('radio', { name: '/ (top level)' }).focus();
    await page.keyboard.press('ArrowDown');
    const chosen = dialog.getByRole('radio', { checked: true });
    await expect(chosen).toBeFocused();
    const name = await chosen.textContent();
    expect(name?.trim()).not.toBe('/ (top level)');
  });
});
