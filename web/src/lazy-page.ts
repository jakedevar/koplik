import { pageEvent, pageFromHash, type PageId } from './router';

/**
 * Runs `mount` when the page is first on screen: its code, its data and (for the what-if) its WASM engine load only then.
 * If the import or the mount fails, the page says so in an alert (never left as a bare heading), the failure is not left
 * unhandled, and the next time the page is shown, or the Retry button, tries again. A success is never repeated.
 * `container` is a function because the page node is replaced when the dashboard shell is rebuilt.
 */
export function onFirstShow(root: HTMLElement, id: PageId, label: string, container: () => HTMLElement | null, mount: (retry: number) => Promise<() => void>, win: Window = window) {
  let state: 'idle' | 'loading' | 'mounted' = 'idle';
  let retries = 0;
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
      const page = container();
      if (!page) return;
      for (const child of [...page.children]) if (!before.has(child)) child.remove();
      alert = document.createElement('div');
      alert.className = 'notice load-failed';
      alert.setAttribute('role', 'alert');
      const message = document.createElement('p');
      message.textContent = `${label} is unavailable: it could not be loaded${error instanceof Error && error.message ? ` (${error.message})` : ''}. Nothing is shown in its place; no figures were estimated. Retry, or reload the page if it keeps failing.`;
      const retry = document.createElement('button');
      retry.type = 'button';
      retry.textContent = `Retry loading ${label}`;
      retry.addEventListener('click', start);
      alert.append(message, retry);
      page.append(alert);
    });
  };
  const onPage = (event: Event) => { if ((event as CustomEvent<{ page: PageId }>).detail.page === id) start(); };
  root.addEventListener(pageEvent, onPage);
  if (pageFromHash(win.location.hash) === id) start();
}
