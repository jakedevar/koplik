import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { RtEstimate } from '../src/generated/RtEstimate';

/** Explicitly synthetic display stress test derived from the committed synthetic fixture, not measured data. */
export function outlierRtRows(): RtEstimate[] {
  const fixture: RtEstimate[] = JSON.parse(readFileSync(resolve('../data/fixtures/web/synthetic-v1/synthetic-rt.json'), 'utf8'));
  const template = fixture.find((row) => row.geography === '48' && row.week.week === 2)!;
  return Array.from({ length: 40 }, (_, i) => ({
    ...template, week: { year: 2025, week: i + 1 },
    ...(i === 39 ? { mean: 12.345, upper: 66.4 } : {}),
  }));
}
