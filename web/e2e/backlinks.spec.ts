import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * A backlink quotes the line it links from, with the link that points here marked, so
 * the reader learns why the other note links here without opening it. Glossary is the
 * vault's most linked note: one link sits in a table cell, one in a CJK sentence.
 */
test.describe('backlink context', () => {
  test('each backlink quotes the line it links from and marks the link', async ({ page }) => {
    await openDoc(page, 'vault', 'Glossary.md');
    const backlinks = page.getByRole('region', { name: /^Backlinks/ });

    const readingList = backlinks.getByRole('listitem').filter({ hasText: 'Reading List' });
    await expect(readingList.locator('.linkrefs__context')).toContainText('See [[Glossary]] for terms');
    await expect(readingList.locator('.linkrefs__context mark')).toHaveText('[[Glossary]]');

    const chinese = backlinks.getByRole('listitem').filter({ hasText: '知识管理' });
    await expect(chinese.locator('.linkrefs__context')).toContainText('见 [[index]] 和 [[Glossary]]');
    await expect(chinese.locator('.linkrefs__context mark')).toHaveText('[[Glossary]]');
  });

  test('outlinks carry no quotation, since the reader is already on that line', async ({ page }) => {
    await openDoc(page, 'vault', 'Reading List.md');
    const outlinks = page.getByRole('region', { name: /^Links from this note/ });
    await expect(outlinks.getByRole('link', { name: /Glossary/ })).toBeVisible();
    await expect(outlinks.locator('.linkrefs__context')).toHaveCount(0);
  });
});
