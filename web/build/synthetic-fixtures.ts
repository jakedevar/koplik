import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import type { Plugin } from 'vite';

const names = new Set(['geographies', 'weekly-cases', 'coverage', 'rt', 'us-states', 'texas-counties'].map((name) => `synthetic-${name}.json`));

/** Fixtures live outside public/ and are exposed only by the opted-in dev server. */
export function syntheticFixtures(fixtureRoot: string, enabled: boolean): Plugin {
  return {
    name: 'koplik-synthetic-fixtures',
    apply: 'serve',
    configureServer(server) {
      if (!enabled) return;
      const base = `${server.config.base.replace(/\/$/, '')}/data/synthetic-v1/`;
      server.middlewares.use(async (request, response, next) => {
        const path = request.url?.split('?')[0] || '';
        const scenarioBase = `${server.config.base.replace(/\/$/, '')}/data/scenarios/`;
        const scenarioFile = { [`${scenarioBase}synthetic-scenario.json`]: 'synthetic-scenario.json', [`${scenarioBase}synthetic-scenario.provenance.json`]: 'synthetic-scenario.provenance.json' }[path];
        if (scenarioFile) {
          try {
            const content = await readFile(resolve(fixtureRoot, '../../seir', scenarioFile));
            response.setHeader('Content-Type', 'application/json');
            response.end(content);
          } catch (error) { next(error); }
          return;
        }
        const name = path.slice(base.length);
        if (!path.startsWith(base) || !names.has(name)) return next();
        try {
          const content = await readFile(resolve(fixtureRoot, name));
          response.setHeader('Content-Type', 'application/json');
          response.end(content);
        } catch (error) { next(error); }
      });
    },
  };
}
