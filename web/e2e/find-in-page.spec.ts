import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

test.describe('find in page', () => {
  test('Ctrl+F opens a find bar that counts matches and steps through them', async ({ page }) => {
    await openDoc(page, 'shapes', 'long-document.md');
    await page.keyboard.press('ControlOrMeta+f');

    const input = page.getByRole('searchbox', { name: 'Find in document' });
    await expect(input).toBeFocused();
    await input.fill('Filler sentence');

    const count = page.locator('.find-bar__count');
    await expect(count).toHaveText(/^1 of \d+$/);
    const total = Number((await count.innerText()).split(' of ')[1]);
    expect(total).toBeGreaterThan(1);
    await expect(page.locator('.prose mark.find-hit')).toHaveCount(total);

    await page.getByRole('button', { name: 'Next match' }).click();
    await expect(count).toHaveText(`2 of ${total}`);

    // Shift+Enter walks back, and wraps at the start rather than stopping.
    await input.press('Shift+Enter');
    await expect(count).toHaveText(`1 of ${total}`);
    await input.press('Shift+Enter');
    await expect(count).toHaveText(`${total} of ${total}`);
  });

  test('Escape closes the bar and leaves the prose unmarked', async ({ page }) => {
    await openDoc(page, 'shapes', 'long-document.md');
    await page.getByRole('button', { name: 'Find' }).click();
    const input = page.getByRole('searchbox', { name: 'Find in document' });
    await input.fill('Section');
    await expect(page.locator('.prose mark.find-hit').first()).toBeVisible();

    await input.press('Escape');
    await expect(input).toBeHidden();
    await expect(page.locator('.prose mark.find-hit')).toHaveCount(0);
    // Still the reading view, not focus mode: Escape in the bar must not reach the layout.
    await expect(page.locator('.app-shell--focus')).toHaveCount(0);
  });
});
