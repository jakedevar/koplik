import type { Dataset } from './data';
import type { MapView } from './map-scales';
import { pageEvent, type PageId } from './router';

type Update = Parameters<MapView['update']>;

/**
 * The map factory the site uses: MapLibre (the largest dependency) is fetched only when the Explorer is on screen, after the
 * shell and the summary have painted. Updates made before it has loaded are kept and the last one is applied on arrival.
 */
export function createLazyMap(container: HTMLElement, data: Dataset, onSelect: (id: string) => void, onError: () => void): MapView {
  container.dataset.mapState = 'loading';
  let real: MapView | undefined;
  let pending: Update | undefined;
  let requested = false;
  let destroyed = false;
  const doc = container.ownerDocument;
  function load() {
    if (requested || destroyed) return;
    requested = true;
    doc.removeEventListener(pageEvent, onPage);
    import('./map').then(({ createMap }) => {
      if (destroyed) return;
      real = createMap(container, data, onSelect, onError);
      if (pending) real.update(...pending);
    }).catch(() => { container.dataset.mapState = 'error'; onError(); });
  }
  const visible = () => !container.closest('[hidden]');
  const onPage = (event: Event) => { if ((event as CustomEvent<{ page: PageId }>).detail.page === 'explorer' && visible()) load(); };
  if (visible()) load(); else doc.addEventListener(pageEvent, onPage);
  return {
    update: (...args) => { pending = args; real?.update(...args); },
    destroy: () => { destroyed = true; doc.removeEventListener(pageEvent, onPage); real?.destroy(); },
    resize: () => real?.resize?.(),
  };
}
