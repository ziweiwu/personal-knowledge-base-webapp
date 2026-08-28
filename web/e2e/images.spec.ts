import { expect, test } from '@playwright/test';
import { awaitLoaded, openDoc } from './helpers';

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
    const image = page.locator('.prose img[src*="wide-screenshot.png"]').first();
    await image.scrollIntoViewIfNeeded();
    await awaitLoaded(image);

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

    // The browser may legitimately have chosen a narrower variant; what must hold is that
    // whatever it fetched fills the column without overflowing or distorting it.
    expect(measured.naturalWidth).toBeGreaterThan(0);
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
    const image = page.locator('.prose img[src*="tall-diagram.png"]').first();
    await image.scrollIntoViewIfNeeded();
    await awaitLoaded(image);
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
    const image = page.locator('.prose img[src*="pixel.png"]').first();
    await image.scrollIntoViewIfNeeded();
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
      const match = /\/api\/file\/.*\/([^/?]+)(?:\?.*)?$/.exec(response.url());
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

  test('an indexed image reserves its space before the bytes arrive', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const image = page.locator('.prose img[src$="wide-screenshot.png"]').first();
    const attrs = await image.evaluate((node) => ({
      width: node.getAttribute('width'),
      height: node.getAttribute('height'),
    }));
    // Without these the browser cannot size the box until the image lands, and a page of
    // screenshots reflows under the reader as each one arrives.
    expect(attrs.width).toBe('1600');
    expect(attrs.height).toBe('400');
  });

  /**
   * The browser chooses from `srcset`, so the assertion is about what it *needed*: where a
   * candidate narrower than the original covers the column at this device pixel ratio, one
   * must be chosen. On a dense landscape phone the column wants more pixels than the file
   * has, and fetching the original is then the correct answer, not a failure.
   */
  test('a screenshot is fetched at no more than the size the column needs', async ({ page }) => {
    const responses = new Map<string, number>();
    page.on('response', (response) => {
      if (/screenshot\.png/.test(response.url())) {
        responses.set(response.url(), Number(response.headers()['content-length'] ?? 0));
      }
    });

    await openDoc(page, 'shapes', 'images.md');
    const image = page.locator('.prose img[src*="screenshot.png"]').first();
    await image.scrollIntoViewIfNeeded();
    await awaitLoaded(image);
    await expect.poll(() => responses.size, { timeout: 15_000 }).toBeGreaterThan(0);

    const needed = await image.evaluate(
      (node) => node.getBoundingClientRect().width * window.devicePixelRatio,
    );
    const [url, bytes] = [...responses.entries()][0];

    // 1600px of incompressible pixels; the source is ~4.6 MB.
    expect(bytes, 'never more than the original').toBeLessThan(4_600_000);
    if (needed < 1600) {
      expect(url, `column needs ${Math.round(needed)}px, so a variant exists`).toContain('?w=');
      expect(bytes).toBeLessThan(1_500_000);
    }
  });

  test('the page still never scrolls sideways with images on it', async ({ page }) => {
    await openDoc(page, 'shapes', 'images.md');
    const overflows = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(overflows).toBe(false);
  });
});
