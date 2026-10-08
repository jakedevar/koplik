import { afterEach, describe, expect, it, vi } from 'vitest';
import { fixtureDataset } from './fixtures.test-utils';
import { createLazyMap } from './map-lazy';

vi.mock('./map', () => { throw new Error('map chunk unavailable'); });

afterEach(() => { document.body.replaceChildren(); });

describe('lazy map loading', () => {
  it('reports a rejected map import through the map error callback', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const onError = vi.fn();
    createLazyMap(container, fixtureDataset(), vi.fn(), onError);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(container.dataset.mapState).toBe('error');
    expect(onError).toHaveBeenCalledOnce();
  });
});
