import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

test.describe('link preview', () => {
  test('resting the pointer on a wikilink shows the target note without leaving the page', async ({ page }) => {
    await openDoc(page, 'vault', 'Reading List.md');
    const link = page.locator('.prose').getByRole('link', { name: 'Glossary' }).first();
    await link.hover();

    const preview = page.getByRole('tooltip');
    await expect(preview).toContainText('Glossary');
    // The reader is still where they were; only the peek appeared.
    await expect(page).toHaveURL(/Reading%20List\.md$/);

    await page.keyboard.press('Escape');
    await expect(preview).toHaveCount(0);
  });

  test('keyboard focus on a wikilink opens the same preview', async ({ page }) => {
    await openDoc(page, 'vault', 'Reading List.md');
    const link = page.locator('.prose').getByRole('link', { name: 'Glossary' }).first();
    await link.focus();
    await expect(page.getByRole('tooltip')).toContainText('Glossary');
    await expect(link).toHaveAttribute('aria-describedby', /.+/);
  });
});
