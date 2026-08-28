import { expect, test } from '@playwright/test';
import { openDoc, proseText } from './helpers';

test.describe('markdown rendering', () => {
  test('currency is prose, not maths — the amounts and the words between them survive', async ({ page }) => {
    await openDoc(page, 'shapes', 'currency.md');
    const text = await proseText(page);
    expect(text).toContain('costs $5 today and $7 tomorrow');
    expect(text).toContain('this sentence must survive intact');
    // The literal case: a `$` followed by a digit never opens maths.
    expect(text).toContain('$5x$');
  });

  test('display maths renders as one expression, not a stack of block boxes', async ({ page }) => {
    await openDoc(page, 'shapes', 'math.md');
    const displays = page.locator('.prose math[display="block"]');
    await expect(displays.first()).toBeVisible();

    // The stacking bug showed as several children of the math root each taking their own
    // line. One row means one expression.
    const distinctTops = await displays.first().evaluate((node) => {
      const tops = [...node.children].map((child) => Math.round(child.getBoundingClientRect().top));
      return new Set(tops).size;
    });
    expect(distinctTops).toBe(1);
  });

  test('an unsupported maths environment degrades in place and never blanks the page', async ({ page }) => {
    await openDoc(page, 'shapes', 'math.md');
    await expect(page.locator('.prose .math-error')).toBeVisible();
    expect(await proseText(page)).toContain('Inline:');
  });

  test('code is highlighted with classes so both themes work from one render', async ({ page }) => {
    await openDoc(page, 'shapes', 'code-and-highlighting.md');
    const highlighted = page.locator('.prose pre [class^="hl-"]').first();
    await expect(highlighted).toBeVisible();
    // Inline colours are what broke dark mode; there must be none.
    expect(await page.locator('.prose pre').first().innerHTML()).not.toContain('style="color');
  });

  test('CJK inside a code fence renders — stepping it byte-wise used to panic the server', async ({ page }) => {
    await openDoc(page, 'shapes', 'code-and-highlighting.md');
    expect(await proseText(page)).toContain('汉字と日本語のテキスト');
  });

  test('a wide table scrolls inside its own box and never scrolls the page sideways', async ({ page }) => {
    await openDoc(page, 'shapes', 'tables.md');
    await expect(page.locator('.prose .scroll-x table')).toHaveCount(2);
    const pageOverflows = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(pageOverflows).toBe(false);
  });

  test('callouts become callouts and a plain blockquote stays a blockquote', async ({ page }) => {
    await openDoc(page, 'shapes', 'callouts.md');
    await expect(page.locator('.prose .callout[data-callout="note"]').first()).toBeVisible();
    await expect(page.locator('.prose .callout[data-callout="warning"]')).toBeVisible();
    const plain = page.locator('.prose blockquote').filter({ hasText: 'An ordinary blockquote' });
    await expect(plain).toBeVisible();
    await expect(plain.locator('.callout')).toHaveCount(0);
  });

  test('an unresolved wikilink is marked as such rather than passed off as a link', async ({ page }) => {
    await openDoc(page, 'shapes', 'links.md');
    await expect(page.locator('.prose .wikilink-unresolved')).toBeVisible();
  });

  test('a mermaid diagram is rendered on the client', async ({ page }) => {
    await openDoc(page, 'shapes', 'mermaid.md');
    await expect(page.locator('.prose .mermaid svg')).toBeVisible({ timeout: 15_000 });
  });

  test('an empty document degrades to an explained empty state, never a blank pane', async ({ page }) => {
    await openDoc(page, 'shapes', 'empty.md');
    await expect(page.locator('.doc__title')).toBeVisible();
    await expect(page.getByText(/nothing to show/i)).toBeVisible();
  });
});
