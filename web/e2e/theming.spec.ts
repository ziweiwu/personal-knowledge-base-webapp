import { expect, test, type Page } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * The two places that bridge the palette into code which cannot read CSS.
 *
 * Mermaid lays out its own SVG from resolved colour values, and CodeMirror's theme is built
 * from `var(--…)` references; both arrived with the reading redesign and neither had an
 * oracle. The diagram was only ever asserted to *exist*, so "the nodes stay light in dark
 * mode" was something a person had to notice and report, twice. These are the measurements
 * that make the bridges observable, including the rule-count regression that made every
 * theme switch leave another copy of the editor's styles in the document.
 */

/** system -> light -> dark, so at most three clicks reach any one of them. */
const THEME_CHOICES = 3;
/** Enough switches that a per-toggle leak is unmistakable rather than arguable. */
const THEME_SWITCHES = 10;
/** Mermaid is lazily imported, and the download is on the clock the first time. */
const DIAGRAM_TIMEOUT_MS = 15_000;

/**
 * Drives the real control rather than stamping `data-theme`, because the question these
 * tests ask is whether the *app* repaints — a directly stamped attribute changes the
 * cascade without ever telling React, which is the half that was broken.
 */
async function chooseTheme(page: Page, wanted: 'light' | 'dark'): Promise<void> {
  const toggle = page.getByRole('button', { name: /Switch to/ });
  for (let click = 0; click < THEME_CHOICES; click += 1) {
    if ((await page.evaluate(() => document.documentElement.dataset.theme)) === wanted) return;
    await toggle.click();
  }
  throw new Error(`the theme toggle never reached ${wanted}`);
}

/**
 * A token as the browser resolves it, so it compares equal to a computed colour.
 *
 * Reading the custom property directly returns the text of the declaration; painting it
 * onto a probe and reading it back returns the same `rgb(…)` form `fill` and
 * `background-color` come back in. Comparing against the token rather than a literal is
 * deliberate: this pins the binding, so re-warming the palette does not fail a test.
 */
async function resolvedColour(page: Page, property: string): Promise<string> {
  return page.evaluate((token) => {
    const probe = document.createElement('span');
    probe.style.color = `var(${token})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).color;
    probe.remove();
    return colour;
  }, property);
}

/** Font stacks differ only in quoting between a declaration and its computed value. */
async function resolvedFonts(page: Page, property: string): Promise<string> {
  const declared = await page.evaluate(
    (token) => getComputedStyle(document.documentElement).getPropertyValue(token),
    property,
  );
  return normaliseFonts(declared);
}

function normaliseFonts(stack: string): string {
  return stack
    .split(',')
    .map((family) => family.trim().replace(/['"]/g, ''))
    .filter(Boolean)
    .join(',');
}

test('a mermaid diagram is painted from the palette in both themes', async ({ page }) => {
  await openDoc(page, 'shapes', 'mermaid.md');
  await expect(page.locator('.prose .mermaid svg')).toBeVisible({ timeout: DIAGRAM_TIMEOUT_MS });

  // Whatever shape the node is drawn as, its fill is the question.
  const nodeFill = () =>
    page
      .locator('.prose .mermaid svg g.node')
      .first()
      .evaluate((node) => {
        const shape = node.querySelector('rect, polygon, path');
        return shape ? getComputedStyle(shape).fill : null;
      });

  await chooseTheme(page, 'light');
  await expect.poll(nodeFill).toBe(await resolvedColour(page, '--accent-subtle'));

  // The diagram has to be drawn again to change colour, and it is drawn from tokens read
  // at that moment — so this fails both if it never re-renders and if it reads them early.
  await chooseTheme(page, 'dark');
  await expect.poll(nodeFill).toBe(await resolvedColour(page, '--accent-subtle'));
});

test('a note is edited in the reading serif and a text file in mono', async ({ page }) => {
  await openDoc(page, 'shapes', 'links.md');
  await page.getByRole('button', { name: /edit/i }).click();
  const prose = page.locator('.editor__host--prose .cm-scroller');
  await expect(prose).toBeVisible();
  const proseFont = await prose.evaluate((node) => getComputedStyle(node).fontFamily);
  expect(normaliseFonts(proseFont), 'a note wears the reading serif').toBe(await resolvedFonts(page, '--font-serif'));

  await openDoc(page, 'shapes', 'notes.txt');
  await page.getByRole('button', { name: /edit/i }).click();
  const code = page.locator('.editor__host--code .cm-scroller');
  await expect(code).toBeVisible();
  const codeFont = await code.evaluate((node) => getComputedStyle(node).fontFamily);
  expect(normaliseFonts(codeFont), 'anything that is not a note is code').toBe(
    await resolvedFonts(page, '--font-mono'),
  );
});

test('the open editor is repainted by a theme switch, not left on the old palette', async ({ page }) => {
  await openDoc(page, 'shapes', 'links.md');
  await chooseTheme(page, 'light');
  await page.getByRole('button', { name: /edit/i }).click();
  const editor = page.locator('.cm-editor');
  await expect(editor).toBeVisible();

  await chooseTheme(page, 'dark');
  await expect
    .poll(() => editor.evaluate((node) => getComputedStyle(node).backgroundColor))
    .toBe(await resolvedColour(page, '--bg'));
});

/**
 * `EditorView.theme` mints a new style module per call and reconfiguring the compartment
 * never retracts the old one, so returning a fresh theme left another copy of the editor's
 * rules in the document on every single toggle. Two module constants fixed it.
 *
 * The baseline is taken only after both sides have been shown once, because mounting the
 * second side's module legitimately adds its rules — that is the whole point of there being
 * two. What must never grow again is what happens after that, and counting from a
 * first-paint baseline instead reads that one legitimate mount as the leak.
 */
test('switching theme repeatedly adds no further style rules', async ({ page }) => {
  await openDoc(page, 'shapes', 'links.md');
  await page.getByRole('button', { name: /edit/i }).click();
  await expect(page.locator('.cm-editor')).toBeVisible();

  const ruleCount = () =>
    page.evaluate(() =>
      Array.from(document.styleSheets).reduce((total, sheet) => {
        try {
          return total + sheet.cssRules.length;
        } catch {
          // A cross-origin sheet cannot be counted, and there is none here to miss.
          return total;
        }
      }, 0),
    );

  await chooseTheme(page, 'light');
  await chooseTheme(page, 'dark');
  const bothSidesMounted = await ruleCount();

  const toggle = page.getByRole('button', { name: /Switch to/ });
  for (let click = 0; click < THEME_SWITCHES; click += 1) await toggle.click();

  expect(await ruleCount(), 'the editor theme is built once per side, not once per switch').toBe(
    bothSidesMounted,
  );
});
