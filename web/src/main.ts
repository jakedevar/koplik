import 'maplibre-gl/dist/maplibre-gl.css';
import './style.css';
import { mountDashboard, showStatus } from './app';
import { loadDataset } from './data';
import { mountForecast } from './forecast-view';
import { mountWhatIf } from './what-if';

const root = document.querySelector<HTMLElement>('#app')!;
showStatus(root, 'Loading pipeline artifacts…');
const synthetic = import.meta.env.DEV && import.meta.env.VITE_SYNTHETIC_FIXTURES === '1';
const page = (id: string) => root.querySelector<HTMLElement>(`[data-page-view="${id}"]`)!;
// The geography chosen on the Explorer, shared with the Forecast page (which stays mounted while hidden).
let geography: string | undefined;
root.addEventListener('koplik:selection', (event) => { geography = (event as CustomEvent<{ geography?: string }>).detail.geography ?? geography; });
loadDataset(import.meta.env.BASE_URL, synthetic)
  .then((data) => {
    mountDashboard(root, data);
    // The forecast follows the dashboard's selection and is never shown without its provenance.
    const cleanup = mountForecast(page('forecast'), { base: import.meta.env.BASE_URL, synthetic, data, geography, events: root });
    window.addEventListener('pagehide', cleanup, { once: true });
  })
  .catch((error: unknown) => {
    showStatus(root, `Data unavailable. ${error instanceof Error ? error.message : 'Unable to read pipeline artifacts'}. Surveillance reports are unavailable.`, true);
  }).finally(() => {
    const cleanup = mountWhatIf(page('what-if'), { base: import.meta.env.BASE_URL, synthetic });
    window.addEventListener('pagehide', cleanup, { once: true });
  });
