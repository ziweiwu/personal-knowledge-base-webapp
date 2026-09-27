/**
 * The first tab stop on every screen, jumping past the chrome to `#main-content`.
 *
 * Shared rather than written per layout because a keyboard user learns it from the first
 * page and then expects it everywhere: the trash page, rendered outside the shell, had no
 * skip link at all, which is WCAG 2.4.1 on that one route and a surprise on every route.
 * Any screen that renders this must carry a `main` with that id and `tabIndex={-1}`, so
 * activating it moves focus rather than only scrolling.
 */
export function SkipLink() {
  return (
    <a className="skip-link" href="#main-content">
      Skip to content
    </a>
  );
}
