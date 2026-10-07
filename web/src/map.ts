import maplibregl, { type GeoJSONSource } from 'maplibre-gl';
import 'maplibre-gl/dist/maplibre-gl.css';
import type { CaseDefinition, Dataset, Metric, Boundaries } from './data';
import { fillColor, mapFeatures, boundaryAttribution, type MapView } from './map-scales';

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

export { boundaryAttribution, caseScales, fillColor, mapFeatures, type MapView } from './map-scales';

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
