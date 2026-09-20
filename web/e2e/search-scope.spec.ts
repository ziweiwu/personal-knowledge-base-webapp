import { expect, test } from '@playwright/test';

test.describe('search scope', () => {
  test("all collections finds a hit in another root, listed under that root's name", async ({ page }) => {
    await page.goto('/f/shapes');
    await page.getByRole('button', { name: /search this collection/i }).click();

    const palette = page.getByRole('dialog');
    // "glossary" lives only in the Obsidian Vault fixture, so the current collection
    // alone must come up empty and the fan-out must find it.
    await palette.getByRole('combobox').fill('glossary');
    // The sr-only status line says the same thing; the visible empty state is the one to wait on.
    await expect(palette.locator('.state__detail', { hasText: /no matches for/i })).toBeVisible();

    await palette.getByRole('button', { name: 'All collections' }).click();
    await expect(palette.getByRole('button', { name: 'All collections' })).toHaveAttribute('aria-pressed', 'true');

    const group = palette.getByRole('group', { name: 'Obsidian Vault' });
    await expect(group.getByRole('option').first()).toBeVisible();

    // Enter opens the highlighted hit, which now belongs to the other root.
    await palette.getByRole('combobox').press('Enter');
    await expect(page).toHaveURL(/\/n\/vault\//);
  });

  test('the scope survives closing and reopening the palette', async ({ page }) => {
    await page.goto('/f/shapes');
    await page.getByRole('button', { name: /search this collection/i }).click();
    const palette = page.getByRole('dialog');
    await palette.getByRole('button', { name: 'All collections' }).click();
    await page.keyboard.press('Escape');
    await expect(palette).toBeHidden();

    await page.getByRole('button', { name: /search this collection/i }).click();
    await expect(page.getByRole('dialog').getByRole('button', { name: 'All collections' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    // Leave the stored preference as it was found for the specs that follow.
    await page.getByRole('dialog').getByRole('button', { name: 'This collection' }).click();
  });
});
