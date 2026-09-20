import { expect, test, type Page } from '@playwright/test';
import { openDoc, readRootFile } from './helpers';

const IDLE_MS = 10_000;
const HALF_IDLE_MS = 5_000;

async function openEditor(page: Page): Promise<void> {
  await openDoc(page, 'shapes', 'notes.txt');
  await page.getByRole('button', { name: /edit/i }).click();
  const editor = page.locator('.cm-content');
  await expect(editor).toBeVisible();
  await editor.click();
  await page.keyboard.press('End');
}

/**
 * The editor saves on its own after a quiet spell, so a phone that backgrounds the tab
 * does not lose the buffer. The clock is faked so the idle window costs no wall time.
 */
test.describe('autosave', () => {
  test('a buffer left idle is saved without pressing Save', async ({ page }) => {
    await page.clock.install();
    await openEditor(page);
    await page.keyboard.type('\nAutosaved by the e2e suite.');
    await expect(page.getByRole('status').filter({ hasText: 'Unsaved changes' })).toBeVisible();
    expect(readRootFile('content-shapes', 'notes.txt')).not.toContain('Autosaved by the e2e suite');

    await page.clock.runFor(IDLE_MS + 1);

    await expect(page.getByRole('status').filter({ hasText: /^Saved/ })).toBeVisible();
    await expect.poll(() => readRootFile('content-shapes', 'notes.txt')).toContain('Autosaved by the e2e suite');
  });

  test('typing keeps restarting the idle window, so a busy buffer is not saved mid-thought', async ({ page }) => {
    await page.clock.install();
    await openEditor(page);
    await page.keyboard.type('\nFirst burst.');
    await page.clock.runFor(HALF_IDLE_MS);
    await page.keyboard.type(' Second burst.');
    await page.clock.runFor(HALF_IDLE_MS + 1);

    // Ten seconds have passed since the first keystroke, but only five since the last.
    await expect(page.getByRole('status').filter({ hasText: 'Unsaved changes' })).toBeVisible();
    expect(readRootFile('content-shapes', 'notes.txt')).not.toContain('Second burst');

    await page.clock.runFor(HALF_IDLE_MS + 1);
    await expect.poll(() => readRootFile('content-shapes', 'notes.txt')).toContain('First burst. Second burst.');
  });
});
