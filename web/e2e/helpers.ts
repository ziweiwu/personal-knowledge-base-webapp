import { expect, type Locator, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

export const EMAIL = 'e2e@example.test';
export const PASSWORD = 'e2e-password-not-a-secret';

/** Where run-server.sh put the throwaway copies of the fixture roots. */
export const WORK = process.env.KBVIEW_E2E_DIR ?? join(tmpdir(), 'kbview-e2e');

/** Where the setup project parks the signed-in session. */
export const STORAGE_STATE = 'playwright/.auth/user.json';

/** Read a fixture file as the server sees it on disk — the only proof a write landed. */
export function readRootFile(rootDir: string, relative: string): string {
  return readFileSync(join(WORK, 'roots', rootDir, relative), 'utf8');
}

/**
 * Open a document and wait for the page to have settled on it.
 *
 * Waits on the document shell rather than on `.prose`, because a document with nothing to
 * render — an empty file — legitimately renders an empty state instead, and waiting for
 * prose there would time out on correct behaviour.
 */
export async function openDoc(page: Page, rootId: string, path: string): Promise<void> {
  const segments = path.split('/').map(encodeURIComponent).join('/');
  await page.goto(`/n/${rootId}/${segments}`);
  await expect(page.locator('.doc')).toBeVisible();
}

/**
 * Wait for an image's bytes to have arrived, not merely for its box to exist.
 *
 * Since images carry `width`/`height`, the element is laid out and "visible" long before it
 * has loaded — so `toBeVisible` no longer implies `naturalWidth` is meaningful.
 */
export async function awaitLoaded(image: Locator): Promise<void> {
  await expect
    .poll(async () => image.evaluate((node: HTMLImageElement) => node.naturalWidth), { timeout: 15_000 })
    .toBeGreaterThan(0);
}

/**
 * The app holds an open SSE stream, so the network never goes idle and any wait keyed on
 * that would hang. Wait for the thing itself instead.
 */
export async function proseText(page: Page): Promise<string> {
  return (await page.locator('.prose').innerText()).replace(/\s+/g, ' ');
}
