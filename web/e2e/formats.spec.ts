import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * One test per document kind. Each asserts the viewer the registry is supposed to pick,
 * because a kind falling through to the download card is silent otherwise.
 */
test.describe('document kinds', () => {
  test('a Word document is converted to HTML rather than offered as a download', async ({ page }) => {
    await page.goto('/n/media/meeting-minutes.docx');
    await expect(page.locator('.prose')).toBeVisible();
    expect(await page.locator('.prose').innerText()).not.toHaveLength(0);
  });

  test('a PDF is shown inline with a way out for browsers that will not embed it', async ({ page }) => {
    await page.goto('/n/media/report.pdf');
    await expect(page.locator('object, iframe').first()).toBeVisible();
    await expect(page.getByRole('link', { name: /open|new tab|download/i }).first()).toBeVisible();
  });

  test('an image is displayed', async ({ page }) => {
    await page.goto('/n/media/logo.png');
    const image = page.locator('img').first();
    await expect(image).toBeVisible();
    expect(await image.evaluate((node: HTMLImageElement) => node.naturalWidth)).toBeGreaterThan(0);
  });

  test('an SVG is displayed rather than treated as source', async ({ page }) => {
    await page.goto('/n/shapes/assets/shape.svg');
    await expect(page.locator('img').first()).toBeVisible();
  });

  test('a CSV becomes a table, including quoted commas and quoted quotes', async ({ page }) => {
    await page.goto('/n/shapes/data/inventory.csv');
    const table = page.locator('table');
    await expect(table).toBeVisible();
    await expect(table.getByText('a name, with a comma')).toBeVisible();
    await expect(table.getByText('a name with "quotes"')).toBeVisible();
  });

  test('JSON and plain text are highlighted as source', async ({ page }) => {
    await page.goto('/n/shapes/data/settings.json');
    await expect(page.locator('pre')).toBeVisible();
    await page.goto('/n/shapes/notes.txt');
    await expect(page.locator('pre')).toBeVisible();
  });

  test('an unknown binary is listed and downloadable, not rendered as mojibake', async ({ page }) => {
    await page.goto('/n/media/opaque.bin');
    await expect(page.getByRole('link', { name: /download/i }).first()).toBeVisible();
    await expect(page.locator('.prose')).toHaveCount(0);
  });
});

test.describe('paths and names', () => {
  test('a filename with spaces and capitals opens from a link and from its URL', async ({ page }) => {
    await openDoc(page, 'shapes', 'Spaces And Caps.md');
    await expect(page.locator('.doc__title')).toContainText('Spaces And Caps');
  });

  test('a non-ASCII filename round-trips through the URL', async ({ page }) => {
    await openDoc(page, 'shapes', 'unicode-标题.md');
    await expect(page.locator('.prose')).toContainText('知识库标题');
  });

  test('a deep path shows its whole trail in the breadcrumbs', async ({ page }) => {
    await openDoc(page, 'shapes', 'deep/nested/folder/leaf.md');
    const crumbs = page.locator('.topbar__crumbs');
    await expect(crumbs).toContainText('deep');
    await expect(crumbs).toContainText('nested');
    await expect(crumbs).toContainText('folder');
  });

  test('a folder with no index file still lists its contents', async ({ page }) => {
    await page.goto('/f/shapes/data');
    await expect(page.getByRole('link', { name: /inventory\.csv/ })).toBeVisible();
  });

  test('a root index file becomes the home page of that root', async ({ page }) => {
    await page.goto('/f/shapes');
    await expect(page.locator('.prose')).toContainText('Content Shapes');
  });
});
