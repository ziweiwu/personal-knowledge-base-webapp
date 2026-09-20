import { expect, type Locator, type Page, test } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * CodeMirror ignores Enter for a short moment after the list opens, so a return typed as the
 * list pops up does not accept a completion the user never saw. The test has to wait it out.
 */
const COMPLETION_INTERACTION_DELAY_MS = 150;

async function openEditorAtEnd(page: Page, rootId: string, path: string): Promise<Locator> {
  await openDoc(page, rootId, path);
  await page.getByRole('button', { name: /^edit$/i }).click();
  const editor = page.locator('.cm-content');
  await expect(editor).toBeVisible();
  await editor.click();
  await page.keyboard.press('ControlOrMeta+End');
  return editor;
}

async function acceptSelected(page: Page, option: Locator): Promise<void> {
  await expect(option).toHaveAttribute('aria-selected', 'true');
  await page.waitForTimeout(COMPLETION_INTERACTION_DELAY_MS);
  await page.keyboard.press('Enter');
}

/**
 * Typing `[[` in the editor offers the collection's note titles, so a link never depends on
 * recalling an exact name. The vault fixture carries a note called "Glossary".
 */
test.describe('wikilink completion', () => {
  test('two brackets and a few letters complete to a closed wikilink', async ({ page }) => {
    const editor = await openEditorAtEnd(page, 'vault', 'index.md');
    await page.keyboard.type('\nSee [[Glo');

    const option = page.locator('.cm-tooltip-autocomplete li', { hasText: 'Glossary' });
    await expect(option).toBeVisible();
    await acceptSelected(page, option);

    await expect(editor).toContainText('See [[Glossary]]');
    await expect(page.locator('.cm-tooltip-autocomplete')).toHaveCount(0);
  });

  /**
   * Two notes in the vault are both called "Meeting Notes". Each is offered with its folder,
   * and picking one links it by path — the only form Obsidian resolves unambiguously.
   */
  test('a shared title matches on a later word and links by path', async ({ page }) => {
    const editor = await openEditorAtEnd(page, 'vault', 'index.md');
    await page.keyboard.type('\n[[notes');

    const options = page.locator('.cm-tooltip-autocomplete li', { hasText: 'Meeting Notes' });
    await expect(options).toHaveCount(2);
    await expect(options.first().locator('.cm-completionDetail')).toHaveText('meetings');

    await expect(options.first()).toHaveAttribute('aria-selected', 'true');
    await page.waitForTimeout(COMPLETION_INTERACTION_DELAY_MS);
    await page.keyboard.press('ArrowDown');
    await acceptSelected(page, options.nth(1));

    await expect(editor).toContainText('[[projects/kbview/notes/Meeting Notes]]');
  });

  test('a plain text file gets no wikilink completion', async ({ page }) => {
    await openEditorAtEnd(page, 'shapes', 'notes.txt');
    await page.keyboard.type('\n[[Glo');

    await expect(page.locator('.cm-tooltip-autocomplete')).toHaveCount(0);
  });
});
