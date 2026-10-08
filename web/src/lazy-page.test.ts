import { afterEach, describe, expect, it, vi } from 'vitest';
import { LazyPageImportError, onFirstShow } from './lazy-page';
import { pageEvent } from './router';

function setup(hash: string) {
  const root = document.createElement('div');
  const page = document.createElement('section');
  page.innerHTML = '<h1>Forecast</h1>';
  root.append(page);
  document.body.append(root);
  const win = { location: { hash, reload: vi.fn() }, addEventListener: vi.fn() } as unknown as Window;
  const show = () => root.dispatchEvent(new CustomEvent(pageEvent, { detail: { page: 'forecast' } }));
  return { root, page, win, show };
}
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
afterEach(() => { document.body.replaceChildren(); });

describe('lazy page loading', () => {
  it('shows an accessible unavailable state when the import rejects, without an unhandled rejection', async () => {
    const { root, page, win } = setup('#/forecast');
    const unhandled = vi.fn();
    process.on('unhandledRejection', unhandled);
    onFirstShow(root, 'forecast', 'The forecast', () => page, () => Promise.reject(new LazyPageImportError(new Error('Failed to fetch dynamically imported module'))), win);
    await flush();
    const alert = page.querySelector('[role="alert"]')!;
    expect(alert.textContent).toContain('The forecast is unavailable');
    expect(alert.textContent).toContain('Failed to fetch dynamically imported module');
    expect(page.querySelector('button')?.textContent).toBe('Retry loading The forecast');
    expect(page.querySelector('h1')?.textContent).toBe('Forecast');
    await flush();
    process.off('unhandledRejection', unhandled);
    expect(unhandled).not.toHaveBeenCalled();
  });
  it('offers a reload after an in-place retry also fails to fetch the module', async () => {
    const { root, page, win } = setup('#/forecast');
    const load = vi.fn(() => Promise.reject(new LazyPageImportError(new Error('Failed to fetch dynamically imported module'))));
    onFirstShow(root, 'forecast', 'The forecast', () => page, load, win);
    await flush();
    page.querySelector('button')!.click();
    await flush();
    expect(load).toHaveBeenCalledTimes(2);
    expect(page.querySelector('[role="alert"]')?.textContent).toContain('Reload the page to try again.');
    const button = page.querySelector('button')!;
    expect(button.textContent).toBe('Reload page');
    button.click();
    expect(win.location.reload).toHaveBeenCalledOnce();
  });
  it('retries on the next show, and with the Retry button, and mounts once on success', async () => {
    const { root, page, win, show } = setup('#/explorer');
    const cleanup = vi.fn();
    const mount = vi.fn()
      .mockRejectedValueOnce(new Error('offline'))
      .mockImplementationOnce(async () => { page.append(document.createElement('div')); throw new Error('half drawn'); })
      .mockImplementation(async () => { const panel = document.createElement('div'); panel.className = 'panel'; page.append(panel); return cleanup; });
    onFirstShow(root, 'forecast', 'The forecast', () => page, mount, win);
    expect(mount).not.toHaveBeenCalled();
    show();
    await flush();
    expect(page.querySelectorAll('[role="alert"]')).toHaveLength(1);
    show();
    await flush();
    expect(mount).toHaveBeenCalledTimes(2);
    expect(page.querySelectorAll('[role="alert"]')).toHaveLength(1);
    expect(page.children).toHaveLength(2);
    page.querySelector('button')!.click();
    await flush();
    expect(mount).toHaveBeenCalledTimes(3);
    expect([...page.children].map((c) => c.className)).toEqual(['', 'panel']);
    show();
    await flush();
    expect(mount).toHaveBeenCalledTimes(3);
    expect(win.addEventListener).toHaveBeenCalledWith('pagehide', cleanup, { once: true });
  });
  it('treats a mount that throws synchronously as a failure', async () => {
    const { root, page, win } = setup('#/forecast');
    onFirstShow(root, 'forecast', 'The what-if simulation', () => page, () => { throw new Error('boom'); }, win);
    await flush();
    expect(page.querySelector('[role="alert"]')?.textContent).toContain('The what-if simulation is unavailable');
  });
});
