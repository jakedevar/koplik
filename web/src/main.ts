import 'maplibre-gl/dist/maplibre-gl.css';
import './style.css';
import { mountDashboard, showStatus } from './app';
import { loadDataset } from './data';
import { mountWhatIf } from './what-if';

const root = document.querySelector<HTMLElement>('#app')!;
showStatus(root, 'Loading pipeline artifacts…');
const synthetic = import.meta.env.DEV && import.meta.env.VITE_SYNTHETIC_FIXTURES === '1';
loadDataset(import.meta.env.BASE_URL, synthetic)
  .then((data) => mountDashboard(root, data))
  .catch((error: unknown) => {
    showStatus(root, `Data unavailable. ${error instanceof Error ? error.message : 'Unable to read pipeline artifacts'}. No estimates are shown.`, true);
  }).finally(() => {
    const cleanup = mountWhatIf(root.querySelector('main')!, { base: import.meta.env.BASE_URL, synthetic });
    window.addEventListener('pagehide', cleanup, { once: true });
  });
