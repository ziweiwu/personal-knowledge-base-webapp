import { expect, test } from '@playwright/test';
import { openDoc, readRootFile } from './helpers';

test.describe('task checkboxes', () => {
  /**
   * The regression this whole fixture exists for. Frontmatter is stripped and the callout
   * expands before the parser runs, so a line number taken from the parser addresses the
   * wrong line of the file on disk — and the click returned 200 while corrupting the note.
   */
  test('a click ticks the task it belongs to, not one shifted by the callout above it', async ({ page }) => {
    await openDoc(page, 'shapes', 'frontmatter-callout-tasks.md');

    const first = page.locator('input[data-task-line]').first();
    await expect(first).toBeEnabled();
    await first.check();
    await expect(first).toBeChecked();

    await expect
      .poll(() => readRootFile('content-shapes', 'frontmatter-callout-tasks.md'))
      .toContain('- [x] the first task');

    const source = readRootFile('content-shapes', 'frontmatter-callout-tasks.md');
    expect(source, 'no other task may be touched').toContain('- [ ] the decoy');
    expect(source.match(/- \[x\]/g) ?? []).toHaveLength(1);
  });

  test('the line numbers the page carries are the file\'s own', async ({ page }) => {
    await openDoc(page, 'shapes', 'frontmatter-callout-tasks.md');
    const lines = await page.locator('input[data-task-line]').evaluateAll((boxes) =>
      boxes.map((box) => Number((box as HTMLInputElement).dataset.taskLine)),
    );
    const source = readRootFile('content-shapes', 'frontmatter-callout-tasks.md').split('\n');
    for (const line of lines) {
      expect(source[line - 1], `line ${line}`).toMatch(/^\s*[-*+] \[[ xX]\]/);
    }
  });

  test('a task inside a code fence draws no checkbox at all', async ({ page }) => {
    await openDoc(page, 'shapes', 'tasks.md');
    const labels = await page.locator('input[data-task-line]').evaluateAll((boxes) =>
      boxes.map((box) => box.parentElement?.textContent?.trim() ?? ''),
    );
    expect(labels.some((label) => label.includes('lives inside a code fence'))).toBe(false);
  });

  test('a task inside a blockquote is clickable, and writes', async ({ page }) => {
    await openDoc(page, 'shapes', 'tasks.md');
    const quoted = page.locator('blockquote input[data-task-line]');
    await expect(quoted).toBeEnabled();
    await quoted.check();
    await expect
      .poll(() => readRootFile('content-shapes', 'tasks.md'))
      .toContain('> - [x] a task inside a blockquote');
  });

  /**
   * A literal `<input type="checkbox">` in the document's own HTML gives the page one more
   * checkbox than the source has task lines. A checkbox that does nothing is a small
   * disappointment; one wired to the wrong line edits the wrong task.
   */
  test('a raw-HTML checkbox makes every box in that document read-only', async ({ page }) => {
    await openDoc(page, 'shapes', 'raw-html-checkbox.md');

    // Not one box is wired to a line.
    await expect(page.locator('input[data-task-line]')).toHaveCount(0);

    // The two real task items are rendered read-only. The third checkbox on the page is
    // the document's own raw HTML, which the app renders untouched by design and is
    // therefore deliberately not asserted on here.
    const taskBoxes = page.locator('.prose li input[type="checkbox"]');
    await expect(taskBoxes).toHaveCount(2);
    await expect(taskBoxes.nth(0)).toBeDisabled();
    await expect(taskBoxes.nth(1)).toBeDisabled();
  });

  test('CRLF line endings survive a toggle', async ({ page }) => {
    await openDoc(page, 'shapes', 'crlf.md');
    await page.locator('input[data-task-line]').first().check();
    await expect.poll(() => readRootFile('content-shapes', 'crlf.md')).toContain('- [x] a task in a CRLF document');
    expect(readRootFile('content-shapes', 'crlf.md')).not.toMatch(/[^\r]\n/);
  });

  test('the checkbox is a 24px target and keeps focus through the write', async ({ page }) => {
    await openDoc(page, 'shapes', 'tasks.md');
    const box = page.locator('input[data-task-line]').first();

    const size = await box.boundingBox();
    expect(size?.width, 'WCAG 2.5.8 wants 24px').toBeGreaterThanOrEqual(24);
    expect(size?.height).toBeGreaterThanOrEqual(24);

    await box.focus();
    await page.keyboard.press('Space');
    // Disabling the box for the round trip blurred it, throwing a keyboard user back to
    // the top of the document once per item.
    await expect(box).toBeFocused();
  });

  test('a stale write is refused rather than silently applied', async ({ page }) => {
    await openDoc(page, 'shapes', 'tasks.md');
    const response = await page.request.post('/api/task/shapes/tasks.md', {
      data: { line: 6, checked: true, baseMtimeMs: 1 },
    });
    expect(response.status(), 'the same 409 a save gets').toBe(409);
  });
});
