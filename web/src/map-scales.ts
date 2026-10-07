import type { CaseDefinition, Dataset, Metric, Boundaries } from './data';
import { mapMetricValue } from './data';

/** Everything about the map that needs no map library: the colour scales, the features and the attribution text. */
export interface MapView {
  update(level: 'state' | 'county', metric: Metric, selected: string, definition?: CaseDefinition): void;
  destroy(): void;
  /** Re-measure the container, e.g. when the page it sits on becomes visible again. */
  resize?(): void;
}

/** Boundary attribution uses its own v1 provenance, never a guessed data provider. */
export function boundaryAttribution(boundaries: Boundaries): string {
  const labels = new Set<string>();
  for (const feature of boundaries.features) {
    if (!feature.properties.provenance?.length) labels.add('Boundary source / licence unavailable');
    for (const record of feature.properties.provenance || []) labels.add(`${record.source_id} · ${record.licence_id}`);
  }
  if (!labels.size) labels.add('Boundary source / licence unavailable');
  // MapLibre treats attribution as HTML; source metadata stays plain text.
  return [...labels].sort().map((label) => label.replace(/[&<>"']/g, (character) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]!)).join(' | ');
}

/** One colour scale per case definition (value, colour stops); definitions never share a scale. */
export const caseScales: Record<CaseDefinition, [number, string][]> = {
  confirmed_or_unknown_status: [[0, '#edf4ed'], [1, '#c5ddc3'], [50, '#68a58d'], [100, '#286e66'], [500, '#123f3b']],
  confirmed: [[0, '#eef1f8'], [1, '#c9d3ec'], [50, '#8196cf'], [100, '#46569b'], [500, '#1d2557']],
};
/** Low coverage is a light warm tone and high coverage a dark green, ordered by lightness: the first two stops were nearly the same lightness and indistinguishable to tritanopes (palette.test.ts). */
export const coverageScale: [number, string][] = [[0, '#f7e6bf'], [80, '#e9c97a'], [90, '#8db896'], [95, '#357c68'], [100, '#123f3b']];
/** The fill expression for the map: coverage has its own scale; each case definition has its own. */
export function fillColor(metric: Metric, level: 'state' | 'county', definition?: CaseDefinition) {
  const stops = metric === 'coverage' && level === 'state' ? coverageScale : caseScales[definition ?? 'confirmed_or_unknown_status'];
  return ['interpolate', ['linear'], ['get', 'value'], ...stops.flat()];
}

export function mapFeatures(data: Dataset, level: 'state' | 'county', metric: Metric, selected: string, definition?: CaseDefinition) {
  const source = level === 'state' ? data.states : data.counties;
  return {
    ...source,
    features: source.features.map((feature) => {
      const value = mapMetricValue(data, feature.properties.GEOID, level === 'county' ? 'cases-2025' : metric, definition).value;
      return { ...feature, properties: { ...feature.properties, value, missing: value === null, selected: feature.properties.GEOID === selected } };
    }),
  };
}

