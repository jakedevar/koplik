import maplibregl, { type GeoJSONSource } from 'maplibre-gl';
import type { CaseDefinition, Dataset, Metric, Boundaries } from './data';
import { mapMetricValue } from './data';

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
const coverageScale: [number, string][] = [[0, '#f3d9a6'], [80, '#ead38a'], [90, '#8db896'], [95, '#357c68'], [100, '#123f3b']];
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

function bounds(source: Boundaries, id?: string): [[number, number], [number, number]] | undefined {
  const coordinates: number[][] = [];
  for (const feature of source.features.filter((f) => !id || f.properties.GEOID === id)) {
    const rings = feature.geometry.type === 'Polygon' ? feature.geometry.coordinates : feature.geometry.coordinates.flat();
    for (const ring of rings) coordinates.push(...ring);
  }
  if (!coordinates.length) return;
  return [[Math.min(...coordinates.map((p) => p[0])), Math.min(...coordinates.map((p) => p[1]))],
    [Math.max(...coordinates.map((p) => p[0])), Math.max(...coordinates.map((p) => p[1]))]];
}

export function createMap(container: HTMLElement, data: Dataset, onSelect: (id: string) => void, onError: () => void): MapView {
  container.dataset.mapState = 'loading';
  const map = new maplibregl.Map({
    container,
    style: { version: 8, sources: {}, layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#f4f7f6' } }] },
    center: [-98, 38], zoom: 3, attributionControl: false,
  });
  map.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'top-right');
  map.addControl(new maplibregl.AttributionControl({ compact: false }));
  let current: ['state' | 'county', Metric, string, CaseDefinition?] = ['state', 'cases-2025', '48'];
  let ready = false;
  let previousLevel = 'state';
  map.on('error', () => { container.dataset.mapState = 'error'; onError(); });
  map.on('idle', () => {
    if (ready && map.isSourceLoaded('regions')) container.dataset.mapState = 'ready';
  });
  map.on('load', () => {
    // Local hatch image: no sprites, glyph services, tile servers or external requests.
    const pixels = new Uint8Array(8 * 8 * 4);
    for (let py = 0; py < 8; py++) for (let px = 0; px < 8; px++) {
      const offset = (py * 8 + px) * 4;
      const shade = (px + py) % 8 < 2 ? 149 : 231;
      pixels.set([shade, shade, shade, 255], offset);
    }
    map.addImage('missing-hatch', { width: 8, height: 8, data: pixels });
    map.addSource('regions', { type: 'geojson', data: mapFeatures(data, ...current), attribution: boundaryAttribution(current[0] === 'state' ? data.states : data.counties) });
    map.addLayer({ id: 'reported', type: 'fill', source: 'regions', filter: ['==', ['get', 'missing'], false],
      paint: { 'fill-color': fillColor(current[1], current[0], current[3]) as never, 'fill-opacity': 0.9 } });
    map.addLayer({ id: 'missing', type: 'fill', source: 'regions', filter: ['==', ['get', 'missing'], true], paint: { 'fill-pattern': 'missing-hatch' } });
    map.addLayer({ id: 'outlines', type: 'line', source: 'regions', paint: { 'line-color': ['case', ['get', 'selected'], '#b84920', '#ffffff'], 'line-width': ['case', ['get', 'selected'], 3, 1] } });
    for (const layer of ['reported', 'missing']) {
      map.on('click', layer, (event) => {
        const id = event.features?.[0]?.properties?.GEOID;
        if (typeof id === 'string') onSelect(id);
      });
      map.on('mouseenter', layer, () => { map.getCanvas().style.cursor = 'pointer'; });
      map.on('mouseleave', layer, () => { map.getCanvas().style.cursor = ''; });
    }
    ready = true;
    update(...current);
    if (data.synthetic) {
      const extent = bounds(data.states);
      if (extent) map.fitBounds(extent, { padding: 35, duration: 0 });
    }
  });
  function update(level: 'state' | 'county', metric: Metric, selected: string, definition?: CaseDefinition) {
    const oldSelection = current[2];
    current = [level, metric, selected, definition];
    if (!ready) return;
    const source = map.getSource('regions') as GeoJSONSource;
    source.attribution = boundaryAttribution(level === 'state' ? data.states : data.counties);
    source.setData(mapFeatures(data, level, metric, selected, definition));
    map.setPaintProperty('reported', 'fill-color', fillColor(metric, level, definition) as never);
    if (level !== previousLevel) {
      const extent = bounds(level === 'state' ? data.states : data.counties);
      if (extent) map.fitBounds(extent, { padding: 35, duration: 0 });
    } else if (selected !== oldSelection) {
      const extent = bounds(level === 'state' ? data.states : data.counties, selected);
      if (extent) map.fitBounds(extent, { padding: 70, maxZoom: level === 'county' ? 9 : 6, duration: 0 });
    }
    previousLevel = level;
  }
  return { update, destroy: () => map.remove(), resize: () => map.resize() };
}
