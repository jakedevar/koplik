import { resolve } from 'node:path';
import type { Plugin, ResolvedConfig } from 'vite';
import { assertNoSyntheticData } from '../scripts/check-production-data.mjs';

/** Guard Vite itself, including direct builds that bypass the npm script. */
export function productionDataGuard(): Plugin {
  let config: ResolvedConfig;
  return {
    name: 'koplik-production-data',
    apply: 'build',
    async configResolved(resolved) {
      config = resolved;
      if (config.publicDir) await assertNoSyntheticData(config.publicDir);
    },
    async closeBundle() {
      await assertNoSyntheticData(resolve(config.root, config.build.outDir));
    },
  };
}
