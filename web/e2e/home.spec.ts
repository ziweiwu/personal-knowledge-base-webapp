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

    await recents.getByRole('button', { name: `Pin ${title}` }).click();
    await expect(recents.getByRole('button', { name: `Unpin ${title}` })).toHaveAttribute('aria-pressed', 'true');
  });

  test('picking a collection again returns to where the user left it', async ({ page }) => {
    await openDoc(page, 'shapes', 'tables.md');
    await page.goto('/');
    await page.getByRole('region', { name: 'Collections' }).getByRole('link', { name: /Content Shapes/ }).click();
    await expect(page).toHaveURL(/\/shapes\/tables\.md$/);
  });
});
