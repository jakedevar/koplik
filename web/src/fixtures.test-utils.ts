import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseBoundaries, parseRows, type Dataset } from './data';

export const fixtureRoot = resolve(process.cwd(), '../data/fixtures/web');
export function fixtureJson(name: string): unknown {
  return JSON.parse(readFileSync(resolve(fixtureRoot, `synthetic-v1/synthetic-${name}.json`), 'utf8'));
}
export function fixtureDataset(): Dataset {
  return {
    geographies: parseRows('geographies', fixtureJson('geographies')),
    cases: parseRows('cases', fixtureJson('weekly-cases')),
    coverage: parseRows('coverage', fixtureJson('coverage')),
    rt: parseRows('rt', fixtureJson('rt')),
    states: parseBoundaries(fixtureJson('us-states'), 'state'),
    counties: parseBoundaries(fixtureJson('texas-counties'), 'county'),
    synthetic: true,
  };
}
