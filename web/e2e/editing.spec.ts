import { expect, test } from '@playwright/test';
import { openDoc, readRootFile } from './helpers';

test.describe('editing', () => {
  test('an edit is saved to the file on disk', async ({ page }) => {
    await openDoc(page, 'shapes', 'notes.txt');
    await page.getByRole('button', { name: /edit/i }).click();

    const editor = page.locator('.cm-content');
    await expect(editor).toBeVisible();
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.type('\nA line added by the e2e suite.');
    await page.getByRole('button', { name: /save/i }).click();

    await expect.poll(() => readRootFile('content-shapes', 'notes.txt')).toContain('added by the e2e suite');
  });

  /**
   * The precondition that stops last-write-wins. Obsidian may have the same file open, and
   * without this a save silently destroys whatever it did not know about.
   */
  test('a save against a stale base mtime is a 409 carrying both versions', async ({ page }) => {
    // page.request shares the browser context's cookies, so this rides the login above.
    const response = await page.request.put('/api/doc/shapes/notes.txt', {
      data: { content: 'clobbered', baseMtimeMs: 1 },
    });
    expect(response.status()).toBe(409);
    const body = await response.json();
    expect(body).toHaveProperty('diskContent');
    expect(body).toHaveProperty('yourContent');
  });

  test('a document that has no source is not offered an editor', async ({ page }) => {
    await page.goto('/n/media/report.pdf');
    await expect(page.getByRole('button', { name: /^edit$/i })).toHaveCount(0);
  });

  /**
   * Renaming without rewriting inbound links corrupts a folder, so this is the behaviour
   * that matters most in the whole write path. Both documents are created here rather than
   * taken from the corpus: the specs share one server, and a test that moves a fixture out
   * from under a later test is a flake waiting to happen.
   */
  test('renaming a note rewrites the links that point at it, and leaves prose alone', async ({ page }) => {
    await page.request.post('/api/doc/shapes/rename-target.md', {
      data: { content: '# Rename target\n', baseMtimeMs: 0 },
    });
    await page.request.post('/api/doc/shapes/rename-linker.md', {
      data: {
        content: 'A link: [[rename-target]].\n\nProse that merely says rename-target must not change.\n',
        baseMtimeMs: 0,
      },
    });

    const response = await page.request.post('/api/rename?root=shapes', {
      data: { from: 'rename-target.md', to: 'renamed-destination.md' },
    });
    expect(response.status()).toBe(200);

    await expect.poll(() => readRootFile('content-shapes', 'rename-linker.md')).toContain('[[renamed-destination]]');
    const linker = readRootFile('content-shapes', 'rename-linker.md');
    expect(linker, 'prose is not a link').toContain('merely says rename-target must not change');
  });

  test('a delete moves the file to the trash rather than destroying it', async ({ page }) => {
    const create = await page.request.post('/api/doc/shapes/disposable.md', {
      data: { content: '# Disposable\n', baseMtimeMs: 0 },
    });
    expect(create.ok()).toBeTruthy();

    const removed = await page.request.delete('/api/doc/shapes/disposable.md');
    expect(removed.ok()).toBeTruthy();
    expect(() => readRootFile('content-shapes', 'disposable.md')).toThrow();
    expect(readRootFile('content-shapes', '.trash/disposable.md')).toContain('Disposable');
  });
});

test.describe('search', () => {
  test('search finds a phrase and navigates to the document holding it', async ({ page }) => {
    await page.goto('/f/shapes');
    // The visible trigger rather than the shortcut: the shortcut is bound at the document
    // and needs the page to already hold focus, which is a property of the harness rather
    // than of the app.
    await page.getByRole('button', { name: /search this collection/i }).click();

    const palette = page.getByRole('dialog');
    await palette.getByRole('combobox').fill('swallows');
    // Results are listbox options, not links, so the keyboard is how one is taken.
    await expect(palette.getByRole('option').first()).toBeVisible();
    await palette.getByRole('combobox').press('Enter');

    await expect(page.locator('.prose')).toContainText('swallows the prose');
  });
});
