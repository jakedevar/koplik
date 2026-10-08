import { pageEvent, pageFromHash, type PageId } from './router';

/** Marks a rejected lazy module import so its recovery can differ from a page mount failure. */
export class LazyPageImportError extends Error {
  constructor(error: unknown) {
    super(error instanceof Error ? error.message : 'Unable to load page code', { cause: error });
    this.name = 'LazyPageImportError';
  }
}

/** Marks a rejected dynamic import so callers can distinguish it from a mount failure. */
export function loadLazyModule<T>(load: () => Promise<T>): Promise<T> {
  return load().catch((error: unknown) => { throw new LazyPageImportError(error); });
}

/**
 * Runs `mount` when the page is first on screen: its code, its data and (for the what-if) its WASM engine load only then.
 * If the import or the mount fails, the page says so in an alert (never left as a bare heading), the failure is not left
 * unhandled, and the next time the page is shown, or the Retry button, tries again. A repeatedly rejected import offers
 * a reload because browsers can cache module-fetch failures for the lifetime of the page. A success is never repeated.
 * `container` is a function because the page node is replaced when the dashboard shell is rebuilt.
 */
export function onFirstShow(root: HTMLElement, id: PageId, label: string, container: () => HTMLElement | null, mount: (retry: number) => Promise<() => void>, win: Window = window) {
  let state: 'idle' | 'loading' | 'mounted' = 'idle';
  let retries = 0;
  let importFailures = 0;
  let alert: HTMLElement | undefined;
  const start = () => {
    if (state !== 'idle') return;
    state = 'loading';
    alert?.remove();
    alert = undefined;
    // What a failed mount may have drawn before it threw is removed again, so a retry never doubles it.
    const before = new Set(container()?.children ?? []);
    // A synchronous throw from `mount` is a failure too, not an unhandled exception.
    Promise.resolve().then(() => mount(retries)).then((cleanup) => {
      state = 'mounted';
      root.removeEventListener(pageEvent, onPage);
      win.addEventListener('pagehide', cleanup, { once: true });
    }, (error: unknown) => {
      state = 'idle';
      retries++;
      if (error instanceof LazyPageImportError) importFailures++;
      const page = container();
      if (!page) return;
      for (const child of [...page.children]) if (!before.has(child)) child.remove();
      alert = document.createElement('div');
      alert.className = 'notice load-failed';
      alert.setAttribute('role', 'alert');
      const canReload = error instanceof LazyPageImportError && importFailures >= 2;
      const message = document.createElement('p');
      message.textContent = `${label} is unavailable: it could not be loaded${error instanceof Error && error.message ? ` (${error.message})` : ''}. Nothing is shown in its place; no figures were estimated. ${canReload ? 'Reload the page to try again.' : 'Retry loading the page.'}`;
      const retry = document.createElement('button');
      retry.type = 'button';
      if (canReload) {
        retry.textContent = 'Reload page';
        retry.addEventListener('click', () => win.location.reload());
      } else {
        retry.textContent = `Retry loading ${label}`;
        retry.addEventListener('click', start);
      }
      alert.append(message, retry);
      page.append(alert);
    });
  };
  const onPage = (event: Event) => { if ((event as CustomEvent<{ page: PageId }>).detail.page === id) start(); };
  root.addEventListener(pageEvent, onPage);
  if (pageFromHash(win.location.hash) === id) start();
}
