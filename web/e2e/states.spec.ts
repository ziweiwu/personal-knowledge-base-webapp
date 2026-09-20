import { expect, test } from '@playwright/test';

test.describe('empty, error and not-found states', () => {
  test('an address that matches no screen shows the not-found state with the path and a way home', async ({ page }) => {
    await page.goto('/nowhere/at/all');
    const state = page.locator('.state--neutral');
    await expect(state.getByText('Page not found')).toBeVisible();
    await expect(state.locator('.state__path')).toHaveText('/nowhere/at/all');
    await expect(state.getByRole('link', { name: 'Go home' })).toHaveAttribute('href', '/');
    await expect(state.getByRole('link', { name: 'Open the collection' })).toHaveCount(0);
  });

  test('a missing note is a neutral not-found state, not an alarm', async ({ page }) => {
    await page.goto('/n/shapes/does-not-exist.md');
    const state = page.locator('.state--neutral');
    await expect(state.getByText('Not found')).toBeVisible();
    await expect(state.getByText(/moved or renamed/)).toBeVisible();
    await expect(page.locator('.state--danger')).toHaveCount(0);
  });

  test('an empty folder is a neutral empty state', async ({ page }) => {
    const created = await page.request.post('/api/folder/shapes/states-empty-folder', { data: {} });
    expect(created.ok()).toBe(true);
    await page.goto('/f/shapes/states-empty-folder');
    const state = page.locator('.state--neutral').filter({ hasText: 'This folder is empty' }).first();
    await expect(state).toBeVisible();
    await expect(state.locator('.state__glyph svg')).toBeVisible();
  });

  test('a server failure is a danger state with a retry', async ({ page }) => {
    await page.route('**/api/doc/shapes/currency.md', (route) =>
      route.fulfill({ status: 500, contentType: 'application/json', body: '{"code":"internal","message":"boom"}' }),
    );
    await page.goto('/n/shapes/currency.md');
    const state = page.locator('.state--danger');
    await expect(state).toHaveAttribute('role', 'alert');
    await expect(state.getByRole('button', { name: 'Try again' })).toBeVisible();
  });
});
