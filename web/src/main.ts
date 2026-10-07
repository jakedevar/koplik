import './style.css';
import { mountDashboard, showStatus } from './app';
import { loadDataset } from './data';
import { pageEvent, pageFromHash, type PageId } from './router';

const root = document.querySelector<HTMLElement>('#app')!;
// index.html already paints the shell, the navigation and a skeleton of the Explorer, so nothing is drawn here until the data is in.
const synthetic = import.meta.env.DEV && import.meta.env.VITE_SYNTHETIC_FIXTURES === '1';
const page = (id: string) => root.querySelector<HTMLElement>(`[data-page-view="${id}"]`)!;
// The geography chosen on the Explorer, shared with the Forecast page (which is mounted when first shown, then stays mounted while hidden).
let geography: string | undefined;
root.addEventListener('koplik:selection', (event) => { geography = (event as CustomEvent<{ geography?: string }>).detail.geography ?? geography; });

/** Runs `mount` once, when the page is first on screen: its code, its data and (for the what-if) its WASM engine load only then. */
function onFirstShow(id: PageId, mount: () => Promise<() => void>) {
  let started = false;
  const start = () => {
    if (started) return;
    started = true;
    root.removeEventListener(pageEvent, onPage);
    mount().then((cleanup) => window.addEventListener('pagehide', cleanup, { once: true }));
  };
  const onPage = (event: Event) => { if ((event as CustomEvent<{ page: PageId }>).detail.page === id) start(); };
  if (pageFromHash(location.hash) === id) start(); else root.addEventListener(pageEvent, onPage);
}
const mountForecastPage = async (data: Awaited<ReturnType<typeof loadDataset>>) => {
  const { mountForecast } = await import('./forecast-view');
  // The forecast follows the dashboard's selection and is never shown without its provenance.
  return mountForecast(page('forecast'), { base: import.meta.env.BASE_URL, synthetic, data, geography, events: root });
};
const mountWhatIfPage = async () => {
  const { mountWhatIf } = await import('./what-if');
  return mountWhatIf(page('what-if'), { base: import.meta.env.BASE_URL, synthetic });
};

loadDataset(import.meta.env.BASE_URL, synthetic)
  .then((data) => {
    mountDashboard(root, data);
    onFirstShow('forecast', () => mountForecastPage(data));
  })
  .catch((error: unknown) => {
    showStatus(root, `Data unavailable. ${error instanceof Error ? error.message : 'Unable to read pipeline artifacts'}. Surveillance reports are unavailable.`, true);
  }).finally(() => {
    onFirstShow('what-if', mountWhatIfPage);
  });
