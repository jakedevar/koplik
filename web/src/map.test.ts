import { describe, expect, it, vi } from 'vitest';
import { boundaryAttribution, createMap } from './map';
import { fixtureDataset } from './fixtures.test-utils';
import { parseBoundaries } from './data';

const mapState = vi.hoisted(() => ({ events: new Map<string, () => void>(), source: { attribution: '', setData: vi.fn() } }));
vi.mock('maplibre-gl', () => ({ default: {
  Map: class {
    addControl() {}
    on(event: string, callback: () => void) { if (typeof callback === 'function') mapState.events.set(event, callback); }
    addImage() {}
    addSource(_id: string, source: { attribution: string }) { mapState.source.attribution = source.attribution; }
    addLayer() {}
    getSource() { return mapState.source; }
    setPaintProperty() {}
    fitBounds() {}
    remove() {}
  },
  NavigationControl: class {},
  AttributionControl: class {},
} }));

describe('boundary attribution', () => {
  it('uses only supplied boundary source and licence, escapes metadata and labels missing provenance', () => {
    const data = fixtureDataset();
    expect(boundaryAttribution(data.states)).toBe('synthetic-web-test · synthetic-test-only');
    data.states.features[0].properties.provenance = [{ ...data.geographies[0].provenance[0], source_id: 'synthetic-<boundary>', licence_id: 'synthetic-"terms"' }];
    expect(boundaryAttribution(data.states)).toContain('synthetic-&lt;boundary&gt; · synthetic-&quot;terms&quot;');
    delete data.states.features[0].properties.provenance;
    expect(boundaryAttribution(data.states)).toContain('Boundary source / licence unavailable');
    const invalid = structuredClone(data.states);
    invalid.features[0].properties.provenance = [];
    expect(() => parseBoundaries(invalid, 'state')).toThrow('invalid boundary provenance');
  });
  it('updates attribution with the displayed state or county boundary source', () => {
    const data = fixtureDataset();
    data.counties.features.forEach((feature) => { feature.properties.provenance = [{ ...data.geographies[0].provenance[0], source_id: 'synthetic-county-boundaries', licence_id: 'synthetic-county-terms' }]; });
    const view = createMap(document.createElement('div'), data, vi.fn(), vi.fn());
    mapState.events.get('load')!();
    expect(mapState.source.attribution).toBe('synthetic-web-test · synthetic-test-only');
    view.update('county', 'cases-2025', '48165');
    expect(mapState.source.attribution).toBe('synthetic-county-boundaries · synthetic-county-terms');
    view.update('state', 'coverage', '48');
    expect(mapState.source.attribution).toBe('synthetic-web-test · synthetic-test-only');
  });
});
