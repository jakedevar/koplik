import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseBoundaries, parseRows, type Dataset } from './data';
import type { RtEstimate } from './generated/RtEstimate';

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
    cumulative: parseRows('cumulative', fixtureJson('cumulative-cases')),
    states: parseBoundaries(fixtureJson('us-states'), 'state'),
    counties: parseBoundaries(fixtureJson('texas-counties'), 'county'),
    synthetic: true,
  };
}

/** Explicitly synthetic paired-level rows, derived from the committed test fixture. */
export function pairedRtRows(): RtEstimate[] {
  return fixtureDataset().rt.filter((row) => row.geography === '48' && [2, 3].includes(row.week.week))
    .flatMap((row) => [0.5, 0.95].map((interval_level) => ({ ...row, interval_level })));
}
