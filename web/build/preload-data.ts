import type { Plugin, ResolvedConfig } from 'vite';

/** The artifacts the Explorer reads on load (src/data.ts loadDataset), fetched with the same URLs. */
export const explorerArtifacts = ['v6/geographies', 'v6/weekly-cases', 'v6/coverage', 'v6/rt', 'v6/us-states', 'v6/texas-counties', 'v8/cumulative-cases'];

/**
 * Production builds only: ask the browser to start fetching the Explorer's data files while the script is still downloading,
 * so the data is usually there when the script wants it. The same URLs and no new data: the script still fetches and validates them.
 */
export function preloadExplorerData(): Plugin {
  let config: ResolvedConfig;
  return {
    name: 'koplik-preload-explorer-data',
    apply: 'build',
    configResolved(resolved) { config = resolved; },
    transformIndexHtml() {
      const base = config.base.replace(/\/?$/, '/');
      return explorerArtifacts.map((name) => ({ tag: 'link', attrs: { rel: 'preload', as: 'fetch', crossorigin: 'anonymous', href: `${base}data/${name}.json` }, injectTo: 'head' as const }));
    },
  };
}
