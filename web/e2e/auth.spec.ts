import { expect, test } from '@playwright/test';
import { EMAIL, PASSWORD } from './helpers';

// This spec is about getting in, so it must start from outside.
test.use({ storageState: { cookies: [], origins: [] } });

test.describe('authentication', () => {
  test('an unauthenticated visitor is asked to sign in, not shown the documents', async ({ page }) => {
    await page.goto('/n/shapes/index.md');
    await expect(page.getByLabel('Password')).toBeVisible();
    await expect(page.locator('.prose')).toHaveCount(0);
  });

  test('file bytes are behind the gate too, because an attachment is document content', async ({ request }) => {
    for (const route of ['/api/roots', '/api/tree?root=shapes', '/api/file/media/logo.png']) {
      expect((await request.get(route)).status(), route).toBe(401);
    }
  });

  test('a wrong password is refused without saying whether the account exists', async ({ page }) => {
    await page.goto('/');
    await page.getByLabel('Email').fill(EMAIL);
    await page.getByLabel('Password').fill('not-the-password');
    await page.getByRole('button', { name: 'Sign in' }).click();
    await expect(page.getByRole('alert')).toBeVisible();
    await expect(page.getByLabel('Password')).toBeVisible();
  });

  test('signing in reaches the documents and survives a reload', async ({ page }) => {
    await page.goto('/');
    await page.getByLabel('Email').fill(EMAIL);
    await page.getByLabel('Password').fill(PASSWORD);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await expect(page.locator('.app-shell')).toBeVisible();

    await page.reload();
    await expect(page.locator('.app-shell')).toBeVisible();
    await expect(page.getByLabel('Password')).toHaveCount(0);
  });
});
