import { expect, test } from '@playwright/test';
import { EMAIL, PASSWORD } from './helpers';

// The page under test is the one a visitor sees before signing in.
test.use({ storageState: { cookies: [], origins: [] } });

const LIGHT_BACKGROUND = 'rgb(255, 255, 255)';

test.describe('login page', () => {
  test('says what the app is before asking for a password', async ({ page }) => {
    await page.goto('/login');
    await expect(page.getByText('Your Obsidian vault, readable and editable from any browser.')).toBeVisible();
    await expect(page.getByText('Wikilinks · Search · Edits')).toBeVisible();
    await expect(page.getByText('Self-hosted · nothing leaves your NAS')).toBeVisible();
    await expect(page.locator('.login-scene')).toHaveCount(1);
    await expect(page.getByLabel('Email')).toBeFocused();
  });

  test('still signs in through the same form', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Email').fill(EMAIL);
    await page.getByLabel('Password').fill(PASSWORD);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await expect(page.getByRole('region', { name: 'Collections' })).toBeVisible();
  });

  test('follows the dark theme instead of a fixed light scene', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.goto('/login');
    const background = await page.locator('.login').evaluate((element) => getComputedStyle(element).backgroundColor);
    expect(background).not.toBe(LIGHT_BACKGROUND);
  });
});
