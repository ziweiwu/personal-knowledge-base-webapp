import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

test.describe('home page', () => {
  test('lists every collection as a link with its size', async ({ page }) => {
    await page.goto('/');
    const collections = page.getByRole('region', { name: 'Collections' });
    await expect(collections.getByRole('link', { name: /Content Shapes/ })).toBeVisible();
    await expect(collections.getByRole('link', { name: /Obsidian Vault/ })).toBeVisible();
    await expect(collections.getByRole('link', { name: /Content Shapes/ })).toContainText(/\d+ documents/);
  });

  test('a note opened once is offered again under Recently opened, and can be pinned', async ({ page }) => {
    await openDoc(page, 'shapes', 'currency.md');
    const title = (await page.locator('.doc__title').innerText()).trim();

    await page.goto('/');
    const recents = page.getByRole('region', { name: 'Recently opened' });
    await expect(recents.getByRole('link', { name: title })).toBeVisible();

    // Pinning lifts the note out of the recents and into the Pinned index above them.
    await recents.getByRole('button', { name: `Pin ${title}` }).click();
    const pinned = page.getByRole('region', { name: 'Pinned' });
    await expect(pinned.getByRole('button', { name: `Unpin ${title}` })).toHaveAttribute('aria-pressed', 'true');
    await expect(pinned.getByRole('link', { name: title })).toBeVisible();
    await expect(recents.getByRole('link', { name: title })).toHaveCount(0);

    // Unpinning returns it, and an empty Pinned index is not shown at all.
    await pinned.getByRole('button', { name: `Unpin ${title}` }).click();
    await expect(recents.getByRole('link', { name: title })).toBeVisible();
    await expect(page.getByRole('region', { name: 'Pinned' })).toHaveCount(0);
  });

  test('the collections read as a contents list, each row naming its root and its size', async ({ page }) => {
    await page.goto('/');
    const rows = page.getByRole('region', { name: 'Collections' }).getByRole('listitem');
    await expect(rows).toHaveCount(4);
    // The rows stack in one column, a contents page rather than a card grid.
    const boxes = await rows.evaluateAll((items) => items.map((item) => item.getBoundingClientRect()));
    for (let index = 1; index < boxes.length; index += 1) {
      expect(boxes[index].left).toBe(boxes[0].left);
      expect(boxes[index].top).toBeGreaterThan(boxes[index - 1].top);
    }
  });

  test('picking a collection again returns to where the user left it', async ({ page }) => {
    await openDoc(page, 'shapes', 'tables.md');
    await page.goto('/');
    await page.getByRole('region', { name: 'Collections' }).getByRole('link', { name: /Content Shapes/ }).click();
    await expect(page).toHaveURL(/\/shapes\/tables\.md$/);
  });
});
