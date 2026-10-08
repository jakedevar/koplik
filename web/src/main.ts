import './style.css';
import { mountDashboard, showStatus } from './app';
import { loadDataset } from './data';
import { onFirstShow } from './lazy-page';

const root = document.querySelector<HTMLElement>('#app')!;
// index.html already paints the shell, the navigation and a skeleton of the Explorer, so nothing is drawn here until the data is in.
const synthetic = import.meta.env.DEV && import.meta.env.VITE_SYNTHETIC_FIXTURES === '1';
const page = (id: string) => root.querySelector<HTMLElement>(`[data-page-view="${id}"]`)!;
// The geography chosen on the Explorer, shared with the Forecast page (which is mounted when first shown, then stays mounted while hidden).
let geography: string | undefined;
root.addEventListener('koplik:selection', (event) => { geography = (event as CustomEvent<{ geography?: string }>).detail.geography ?? geography; });

const importForecast = (retry: number) => retry === 0
  ? import('./forecast-view')
  : import(/* @vite-ignore */ `${import.meta.env.BASE_URL}${import.meta.env.DEV ? 'src/forecast-view.ts' : 'assets/forecast-view.js'}?retry=${retry}`);
const importWhatIf = (retry: number) => retry === 0
  ? import('./what-if')
  : import(/* @vite-ignore */ `${import.meta.env.BASE_URL}${import.meta.env.DEV ? 'src/what-if.ts' : 'assets/what-if.js'}?retry=${retry}`);

const mountForecastPage = async (data: Awaited<ReturnType<typeof loadDataset>>, retry: number) => {
  const { mountForecast } = await importForecast(retry);
  // The forecast follows the dashboard's selection and is never shown without its provenance.
  return mountForecast(page('forecast'), { base: import.meta.env.BASE_URL, synthetic, data, geography, events: root });
};
const mountWhatIfPage = async (retry: number) => {
  const { mountWhatIf } = await importWhatIf(retry);
  return mountWhatIf(page('what-if'), { base: import.meta.env.BASE_URL, synthetic });
};

loadDataset(import.meta.env.BASE_URL, synthetic)
  .then((data) => {
    mountDashboard(root, data);
    onFirstShow(root, 'forecast', 'The forecast', () => root.querySelector<HTMLElement>('[data-page-view="forecast"]'), (retry) => mountForecastPage(data, retry));
  })
  .catch((error: unknown) => {
    showStatus(root, `Data unavailable. ${error instanceof Error ? error.message : 'Unable to read pipeline artifacts'}. Surveillance reports are unavailable.`, true);
  }).finally(() => {
    onFirstShow(root, 'what-if', 'The what-if simulation', () => root.querySelector<HTMLElement>('[data-page-view="what-if"]'), mountWhatIfPage);
  });
