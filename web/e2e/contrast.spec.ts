import { expect, test } from '@playwright/test';

/**
 * The palette's accessibility floors, checked as arithmetic rather than by eye.
 *
 * Two tokens carry a WCAG minimum and nothing enforced either one: `--border-strong` is the
 * edge of every control and needs 3:1 for 1.4.11, `--fg-faint` is every eyebrow and
 * secondary label and needs 4.5:1 for 1.4.3 because it is small text. Both had shipped
 * failing — the border on the tightest of the three surfaces, the label on all of them in
 * the light theme — and both were fixed by choosing values by measurement. This is the
 * measurement, so the next person to warm up a token hears about it here.
 *
 * A control or a label can sit on any of three surfaces, so each pair is checked against
 * all three; `--bg-subtle` is usually the tightest.
 */

const SURFACES = ['--bg', '--bg-subtle', '--bg-elevated'] as const;

/** Small text under 18.66px, which every one of these labels is. */
const TEXT_MINIMUM = 4.5;
/** Non-text contrast for the boundary of a control. */
const CONTROL_MINIMUM = 3;

interface Pair {
  token: string;
  surface: string;
  ratio: number;
}

async function ratios(page: import('@playwright/test').Page, theme: 'light' | 'dark'): Promise<Pair[]> {
  return page.evaluate(
    ({ surfaces, wanted, pickTheme }) => {
      document.documentElement.dataset.theme = pickTheme;
      const read = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

      const luminance = (hex: string) => {
        const channels = [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16) / 255);
        const [r, g, b] = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
      };
      const ratio = (colour: string, against: string) => {
        const [lighter, darker] = [luminance(colour), luminance(against)].sort(
          (first, second) => second - first,
        );
        return (lighter + 0.05) / (darker + 0.05);
      };

      return wanted.flatMap((token) =>
        surfaces.map((surface) => ({
          token,
          surface,
          ratio: Math.round(ratio(read(token), read(surface)) * 100) / 100,
        })),
      );
    },
    { surfaces: [...SURFACES], wanted: ['--fg-faint', '--fg-muted', '--border-strong'], pickTheme: theme },
  );
}

for (const theme of ['light', 'dark'] as const) {
  test(`every palette token that carries a contrast floor clears it in the ${theme} theme`, async ({ page }) => {
    await page.goto('/');
    const measured = await ratios(page, theme);
    expect(measured.length).toBe(9);

    const failing = measured.filter(
      ({ token, ratio }) => ratio < (token === '--border-strong' ? CONTROL_MINIMUM : TEXT_MINIMUM),
    );
    expect(failing, `measured: ${JSON.stringify(measured)}`).toEqual([]);
  });
}
