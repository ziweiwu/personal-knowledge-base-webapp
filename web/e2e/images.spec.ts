import { expect, test } from '@playwright/test';
import { openDoc } from './helpers';

/**
 * A knowledge base is full of screenshots, and a screenshot is wider than a phone. These
 * assert the image scales to the column rather than overflowing it — an overflow is
 * invisible here, because the page hides horizontal scroll, so the excess is simply
 * clipped and unreachable.
 */
test.describe('image responsiveness', () => {
  test('no image overflows the column it sits in', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const overflowing = await page.locator('.prose img').evaluateAll((images) =>
      images
        .map((image) => {
          const container = image.parentElement!.getBoundingClientRect();
          const rect = image.getBoundingClientRect();
          return { src: (image as HTMLImageElement).src.split('/').pop(), rect, container };
        })
        .filter((entry) => entry.rect.right > entry.container.right + 1)
        .map((entry) => `${entry.src} ${Math.round(entry.rect.width)}px`),
    );
    expect(overflowing).toEqual([]);
  });

  test('a 1600px-wide screenshot is scaled down to fit, with its aspect ratio intact', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const image = page.locator('.prose img[src$="wide-screenshot.png"]').first();
    await expect(image).toBeVisible();

    const measured = await image.evaluate((node) => {
      const element = node as HTMLImageElement;
      return {
        width: element.getBoundingClientRect().width,
        height: element.getBoundingClientRect().height,
        naturalWidth: element.naturalWidth,
        naturalHeight: element.naturalHeight,
        proseWidth: element.closest('.prose')!.getBoundingClientRect().width,
      };
    });

    expect(measured.naturalWidth).toBe(1600);
    expect(measured.width).toBeLessThanOrEqual(measured.proseWidth + 1);
    // Squashing is the classic failure when only the width is capped.
    const ratio = measured.width / measured.height;
    expect(ratio).toBeCloseTo(measured.naturalWidth / measured.naturalHeight, 1);
  });

  /**
   * Deliberately not capped by height. `max-height` would scale a 300x2000 diagram down to
   * thumbnail width to make it fit the screen, which is not reading it — a tall image is
   * bounded by the column and then scrolls with the page, like any other tall content.
   */
  test('a tall image fits the column and keeps its proportions rather than being shrunk', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const image = page.locator('.prose img[src$="tall-diagram.png"]').first();
    const measured = await image.evaluate((node) => {
      const element = node as HTMLImageElement;
      const rect = element.getBoundingClientRect();
      return {
        width: rect.width,
        height: rect.height,
        naturalRatio: element.naturalWidth / element.naturalHeight,
        proseWidth: element.closest('.prose')!.getBoundingClientRect().width,
      };
    });
    expect(measured.width).toBeLessThanOrEqual(measured.proseWidth + 1);
    expect(measured.width / measured.height).toBeCloseTo(measured.naturalRatio, 1);
  });

  test('a small image is not stretched up to fill the column', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const image = page.locator('.prose img[src$="pixel.png"]').first();
    const width = (await image.boundingBox())?.width ?? 0;
    expect(width).toBeLessThan(50);
  });

  /**
   * The behaviour, not the attribute. `loading="lazy"` read back as "lazy" even when it
   * was set too late to do anything, so inspecting it proves nothing — only whether the
   * request happens does.
   */
  test('an image far below the fold is not fetched until it is scrolled towards', async ({ page }) => {
    const fetched = new Set<string>();
    page.on('response', (response) => {
      const match = /\/api\/file\/[^?]*\/([^/?]+)$/.exec(response.url());
      if (match) fetched.add(decodeURIComponent(match[1]));
    });

    await openDoc(page, 'shapes', 'far-below-fold.md');
    await expect(page.locator('.prose img').first()).toBeVisible();
    await page.waitForTimeout(1500);

    expect(fetched.has('pixel.png'), 'the image on screen loads').toBe(true);
    expect(fetched.has('wide-screenshot.png'), 'the one 5000px down does not').toBe(false);

    await page.locator('.prose img').last().scrollIntoViewIfNeeded();
    await expect.poll(() => fetched.has('wide-screenshot.png'), { timeout: 10_000 }).toBe(true);
  });

  test('the page still never scrolls sideways with images on it', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const overflows = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(overflows).toBe(false);
  });
});
