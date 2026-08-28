import { test as setup, expect } from '@playwright/test';
import { EMAIL, PASSWORD, STORAGE_STATE } from './helpers';

/**
 * Signs in once and saves the session for every other project to reuse.
 *
 * Signing in per test worked, but it put dozens of round trips through an endpoint that is
 * deliberately rate limited and Argon2-expensive — the two properties that make it the
 * wrong thing to hammer.
 */
setup('sign in once and save the session', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Email').fill(EMAIL);
  await page.getByLabel('Password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.locator('.app-shell')).toBeVisible();
  await page.context().storageState({ path: STORAGE_STATE });
});
