import { expect, test, type Page } from '@playwright/test';
import { openDoc } from './helpers';

/** The editor buffer as CodeMirror holds it, one string per line joined by newlines. */
async function bufferText(page: Page): Promise<string> {
  return page.locator('.cm-content').evaluate((node) => (node as HTMLElement).innerText);
}

test.describe('formatting toolbar', () => {
  test.beforeEach(async ({ page }) => {
    // A note of our own, so no other spec depends on what the marks do to it.
    await page.request.post('/api/doc/shapes/formatting-playground.md', {
      data: { content: 'plain line\n', baseMtimeMs: 0 },
    });
    await openDoc(page, 'shapes', 'formatting-playground.md');
    await page.getByRole('button', { name: /^edit$/i }).click();
    await expect(page.locator('.cm-content')).toBeVisible();
  });

  test('Mod+B wraps the selection in bold and unwraps it again', async ({ page }) => {
    await page.locator('.cm-content').click();
    await page.keyboard.press('ControlOrMeta+End');
    await page.keyboard.type('word');
    await page.keyboard.press('Shift+Home');
    await page.keyboard.press('ControlOrMeta+b');
    await expect.poll(() => bufferText(page)).toContain('**word**');

    await page.keyboard.press('ControlOrMeta+b');
    await expect.poll(() => bufferText(page)).not.toContain('**');
    await expect.poll(() => bufferText(page)).toContain('word');
  });

  test('the Heading button cycles the current line and keeps the editor focused', async ({ page }) => {
    await page.locator('.cm-content').click();
    await page.keyboard.press('ControlOrMeta+Home');
    const heading = page.getByRole('toolbar', { name: 'Formatting' }).getByRole('button', { name: 'Heading' });
    await heading.click();
    await expect.poll(() => bufferText(page)).toMatch(/^# plain line/);
    await heading.click();
    await expect.poll(() => bufferText(page)).toMatch(/^## plain line/);
    await expect(page.locator('.cm-editor')).toHaveClass(/cm-focused/);
  });

  test('the Link button builds a markdown link around the selection', async ({ page }) => {
    await page.locator('.cm-content').click();
    await page.keyboard.press('ControlOrMeta+Home');
    await page.keyboard.press('Shift+End');
    await page.getByRole('toolbar', { name: 'Formatting' }).getByRole('button', { name: 'Link', exact: true }).click();
    await page.keyboard.type('https://example.test');
    await expect.poll(() => bufferText(page)).toContain('[plain line](https://example.test)');
  });

  test('Mod+Shift+C turns a line into a task and back', async ({ page }) => {
    await page.locator('.cm-content').click();
    await page.keyboard.press('ControlOrMeta+Home');
    await page.keyboard.press('ControlOrMeta+Shift+c');
    await expect.poll(() => bufferText(page)).toContain('- [ ] plain line');
    await page.keyboard.press('ControlOrMeta+Shift+c');
    await expect.poll(() => bufferText(page)).toContain('- [x] plain line');
  });

  test('the toolbar is one tab stop with arrow keys between tools', async ({ page }) => {
    const toolbar = page.getByRole('toolbar', { name: 'Formatting' });
    await toolbar.getByRole('button', { name: 'Bold' }).focus();
    await page.keyboard.press('ArrowRight');
    await expect(toolbar.getByRole('button', { name: 'Italic' })).toBeFocused();
    await page.keyboard.press('End');
    await expect(toolbar.getByRole('button', { name: 'List' })).toBeFocused();
    await expect(toolbar.locator('button[tabindex="0"]')).toHaveCount(1);
  });

  test('a plain text file gets no markdown toolbar', async ({ page }) => {
    await openDoc(page, 'shapes', 'notes.txt');
    await page.getByRole('button', { name: /^edit$/i }).click();
    await expect(page.locator('.cm-content')).toBeVisible();
    await expect(page.getByRole('toolbar', { name: 'Formatting' })).toHaveCount(0);
  });
});
