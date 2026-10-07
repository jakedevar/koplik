/** Hash routing for the four pages. Hashes work on GitHub Pages under /koplik/ with no server rewrites. */
export const pageIds = ['explorer', 'forecast', 'what-if', 'sources'] as const;
export type PageId = (typeof pageIds)[number];

export const pageTitles: Record<PageId, string> = { explorer: 'Explorer', forecast: 'Forecast', 'what-if': 'What-if', sources: 'Sources' };

export function pageHash(page: PageId): string {
  return `#/${page}`;
}

/** The page a location hash names. An empty or unknown hash is the Explorer. */
export function pageFromHash(hash: string): PageId {
  const match = /^#\/?([^/?]*)\/?$/.exec(hash);
  return pageIds.find((id) => id === match?.[1]) ?? 'explorer';
}

/** Fired on the app root after every page change, so a view that needs a size (the map) can react. */
export const pageEvent = 'koplik:page';

/**
 * Shows the page named by the hash and hides the others, keeping every page mounted (the map and the what-if
 * ensemble are expensive to rebuild). Marks the active nav link with aria-current and, after a navigation (not the
 * first paint), moves focus to the new page's h1. Returns a cleanup that removes the listener.
 */
export function mountRouter(root: HTMLElement, win: Window = window): () => void {
  const views = new Map(pageIds.map((id) => [id, root.querySelector<HTMLElement>(`[data-page-view="${id}"]`)] as const));
  const links = [...root.querySelectorAll<HTMLAnchorElement>('nav[aria-label="Primary"] a[data-page]')];
  let current: PageId | undefined;
  function show(focus: boolean) {
    const page = pageFromHash(win.location.hash);
    // An unknown hash is rewritten so the address bar and the page agree.
    if (win.location.hash !== '' && win.location.hash !== pageHash(page)) win.history.replaceState(null, '', pageHash(page));
    for (const [id, view] of views) if (view) view.hidden = id !== page;
    for (const link of links) {
      if (link.dataset.page === page) link.setAttribute('aria-current', 'page'); else link.removeAttribute('aria-current');
    }
    win.document.title = page === 'explorer' ? 'Koplik · Measles outbreak intelligence' : `${pageTitles[page]} · Koplik`;
    const changed = page !== current;
    current = page;
    root.dispatchEvent(new CustomEvent(pageEvent, { bubbles: true, detail: { page } }));
    if (focus && changed) {
      const heading = views.get(page)?.querySelector<HTMLElement>('h1');
      if (heading) { heading.tabIndex = -1; heading.focus(); }
      win.scrollTo?.(0, 0);
    }
  }
  const onChange = () => show(true);
  win.addEventListener('hashchange', onChange);
  show(false);
  return () => win.removeEventListener('hashchange', onChange);
}
