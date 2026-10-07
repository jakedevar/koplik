import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { mountWhatIf } from './what-if';
import { parseScenario, scenarioJson } from './scenario';
import { parseScenarioProvenance } from './scenario-provenance';
import type { SimulationRequest, SimulationResponse, SimulationWorker } from './simulation';
import type { EnsembleResult } from './generated/v2/EnsembleResult';
import { mountProvenanceDrawer } from './provenance';
import { fixtureDataset } from './fixtures.test-utils';

const fixture = readFileSync(resolve(process.cwd(), '../data/fixtures/seir/synthetic-scenario.json'), 'utf8');
const scenario = parseScenario(fixture, true);
const companion = parseScenarioProvenance(readFileSync(resolve(process.cwd(), '../data/fixtures/seir/synthetic-scenario.provenance.json'), 'utf8'), scenario);
// Exercise rendering with the actual facade output, never hand-authored scientific bands.
const { runEnsemble } = createRequire(resolve(process.cwd(), 'package.json'))('../pkg/node/koplik_wasm.js');
let result: EnsembleResult;
beforeAll(() => { result = JSON.parse(runEnsemble(scenarioJson(scenario, 70))); });
afterEach(() => { vi.useRealTimers(); document.body.replaceChildren(); });

function mount(load = vi.fn().mockResolvedValue(scenario), loadProvenance = vi.fn().mockResolvedValue(companion)) {
  const main = document.createElement('main'); document.body.append(main);
  const worker: SimulationWorker = { onmessage: null, onerror: null, postMessage: vi.fn(), terminate: vi.fn() };
  let time = 0;
  const factory = vi.fn(() => worker);
  const cleanup = mountWhatIf(main, { base: '/', synthetic: true, load, loadProvenance, worker: factory, clock: () => time });
  const respond = (id: number, data: EnsembleResult = result) => worker.onmessage!({ data: { id, result: data } } as MessageEvent<SimulationResponse>);
  const slider = main.querySelector<HTMLInputElement>('input')!;
  function input(value: string) { slider.value = value; slider.dispatchEvent(new Event('input')); }
  const request = (index: number) => vi.mocked(worker.postMessage).mock.calls[index][0] as SimulationRequest;
  return { main, worker, factory, cleanup, respond, slider, input, request, setTime: (value: number) => { time = value; } };
}
// The panel loads the scenario, then its provenance companion: several microtask hops.
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };

describe('what-if panel', () => {
  it('runs 1,000 trajectories at the explicit fixture coverage and shows bands, exact values, replay and fingerprint', async () => {
    const { main, slider, request, respond, cleanup } = mount();
    await flush();
    expect(slider.value).toBe('70');
    expect(JSON.parse(request(0).scenarioJson).run_count).toBe(1000);
    expect(main.textContent).toContain('illustrative model scenario, not a prediction');
    expect(main.textContent).toContain('SYNTHETIC MODEL INPUTS');
    respond(1);
    expect(main.querySelector('.engine-fingerprint')?.textContent).toBe(result.fingerprint);
    expect(main.querySelector('.ensemble-band-50')?.tagName).toBe('polygon');
    expect(main.querySelector('.ensemble-band-90')?.tagName).toBe('polygon');
    expect(main.querySelector('.ensemble-median')?.tagName).toBe('polyline');
    const final = result.daily.filter((r) => r.geography === '48165').at(-1)!;
    expect(main.querySelector('.what-if-result tbody tr:last-child')?.textContent).toBe([final.day, final.cumulative_infections.median, final.cumulative_infections.lower_50, final.cumulative_infections.upper_50, final.cumulative_infections.lower_90, final.cumulative_infections.upper_90].join(''));
    expect(main.textContent).toContain(`Seed: ${result.seed}`);
    expect(main.textContent).toContain(JSON.stringify(result.parameters, null, 2));
    expect(main.textContent).toContain(result.scenario_json);
    expect(main.textContent).toContain(scenario.nodes[0].provenance[0].sha256);
    expect(main.querySelector('[aria-busy]')?.getAttribute('aria-busy')).toBe('false');
    cleanup();
  });
  it('coalesces rapid slider changes and withholds stale results until the latest coverage completes', async () => {
    vi.useFakeTimers();
    const { main, worker, input, request, respond, cleanup } = mount();
    await flush();
    input('80'); input('90'); input('95');
    vi.advanceTimersByTime(120);
    expect(worker.postMessage).toHaveBeenCalledTimes(1);
    respond(1);
    expect(worker.postMessage).toHaveBeenCalledTimes(2);
    expect(JSON.parse(request(1).scenarioJson).coverage_overrides.find((o: { geography: string }) => o.geography === '48165').coverage_pct).toBe(95);
    expect(main.querySelector('.what-if-result')?.childElementCount).toBe(0);
    expect(main.querySelector('[aria-busy]')?.getAttribute('aria-busy')).toBe('true');
    respond(4);
    expect(main.querySelector('.engine-fingerprint')?.textContent).toBe(result.fingerprint);
    cleanup();
  });
  it('clears old results during recomputation and measures time from input to rendering', async () => {
    vi.useFakeTimers();
    const { main, input, respond, setTime, cleanup } = mount(); await flush(); respond(1);
    setTime(100); input('95');
    expect(main.querySelector('.what-if-result')?.childElementCount).toBe(0);
    expect(main.querySelector('output')?.textContent).toBe('95% · what-if override');
    vi.advanceTimersByTime(120); setTime(3600); respond(2);
    expect(main.querySelector('[role="status"]')?.textContent).toContain('3.500 s · exceeds the 3 s target');
    expect(main.querySelector<HTMLElement>('.what-if')?.dataset.updateMs).toBe('3500'); cleanup();
  });
  it('restores exact starting coverage and terminates the worker on cleanup', async () => {
    const { main, slider, request, respond, cleanup, worker } = mount(); await flush(); respond(1);
    slider.value = '95'; main.querySelector<HTMLButtonElement>('.what-if-controls button')!.click();
    expect(slider.value).toBe('70');
    expect(JSON.parse(request(1).scenarioJson).coverage_overrides).toEqual(scenario.coverage_overrides);
    cleanup(); expect(worker.terminate).toHaveBeenCalledOnce();
  });
  it('shows absent and invalid scenarios with disabled controls and no worker', async () => {
    const absent = mount(vi.fn().mockResolvedValue(null)); await flush();
    expect(absent.main.textContent).toContain('Scenario data not yet available');
    expect(absent.slider.disabled).toBe(true); expect(absent.factory).toHaveBeenCalledTimes(0); absent.cleanup();
    const invalid = mount(vi.fn().mockRejectedValue(new Error('Invalid v1 scenario'))); await flush(); await flush();
    expect(invalid.main.textContent).toContain('Simulation unavailable. Invalid v1 scenario');
    expect(invalid.slider.disabled).toBe(true); invalid.cleanup();
  });
  it('handles engine and worker errors visibly and permits retry', async () => {
    const { main, worker, input, respond, cleanup } = mount(); await flush();
    worker.onmessage!({ data: { id: 1, error: 'missing baseline coverage' } } as MessageEvent<SimulationResponse>);
    expect(main.textContent).toContain('Simulation unavailable. missing baseline coverage');
    main.querySelector<HTMLButtonElement>('.what-if-controls button')!.click(); respond(2);
    expect(main.querySelector('.engine-fingerprint')?.textContent).toBe(result.fingerprint);
    input('95'); worker.onerror!({} as ErrorEvent);
    expect(main.textContent).toContain('Adjust coverage to retry');
    expect(main.querySelector('.what-if-result')?.childElementCount).toBe(0); cleanup();
  });
  it('does not start a worker after unmounting a pending scenario load', async () => {
    const mounted = mount(); mounted.cleanup(); await flush();
    expect(mounted.factory).toHaveBeenCalledTimes(0);
  });
  it('lists every county source linked by the replay scenario, keeps the override source missing and cites the parameters', async () => {
    vi.useFakeTimers();
    const mounted = mount();
    const closeDrawer = mountProvenanceDrawer(mounted.main);
    await flush();
    // Test-only metadata variant: borrow a second committed fixture record on a non-Gaines node.
    // Recompute with the real facade so no simulation output or provenance is hand-authored.
    const modified = parseScenario(result.scenario_json, true);
    const extra = fixtureDataset().cases[0].provenance[0];
    modified.nodes[0].provenance.push(extra);
    const withSources = JSON.parse(runEnsemble(scenarioJson(modified, 70)));
    mounted.respond(1, withSources);
    mounted.main.querySelector<HTMLButtonElement>('.what-if-result tbody tr:last-child td button')!.click();
    const dialog = mounted.main.querySelector('dialog')!;
    expect(dialog.querySelectorAll('.provenance-record')).toHaveLength(2);
    expect(dialog.textContent).toContain(extra.sha256);
    expect(dialog.textContent).toContain(scenario.nodes[0].provenance[0].sha256);
    expect(dialog.textContent).toContain('across all its counties');
    expect(dialog.textContent).toContain('SYNTHETIC');
    dialog.querySelector<HTMLButtonElement>('button')!.click();
    mounted.input('95'); mounted.main.querySelector<HTMLOutputElement>('output')!.click();
    expect(dialog.textContent).toContain('User-selected scenario override');
    expect(dialog.textContent).toContain('Source provenance missing');
    dialog.querySelector<HTMLButtonElement>('button')!.click();
    mounted.main.querySelector<HTMLElement>('.what-if-metadata pre')!.click();
    expect(dialog.textContent).toContain('cited to its published source');
    expect(dialog.querySelectorAll('.provenance-citation')).toHaveLength(companion.parameters.length);
    expect(dialog.querySelectorAll('.provenance-record')).toHaveLength(0);
    mounted.cleanup(); closeDrawer();
  });
  it('says in plain words that it is a what-if tool and shows the seeding with its sources and limits', async () => {
    const { main, respond, cleanup } = mount(); await flush(); respond(1);
    expect(main.textContent).toContain('A what-if tool, not a fitted model');
    expect(main.textContent).toContain('never adjusted to match how the outbreak actually unfolded');
    expect(main.querySelector('.what-if-metadata')?.textContent).toContain(`Seeded with ${companion.seeding.initial_infectious} infectious and ${companion.seeding.initial_exposed} exposed in Gaines County`);
    expect(main.querySelector('.what-if-metadata')?.textContent).toContain(companion.seeding.limitation);
    expect(main.querySelector('.what-if-metadata')?.textContent).toContain(companion.seeding.rule);
    expect(main.querySelector('.what-if-metadata')?.textContent).toContain(companion.seeding.provenance[0].sha256);
    cleanup();
  });
  it('opens the seeding number onto its source records and each parameter onto its published citation', async () => {
    const mounted = mount();
    const closeDrawer = mountProvenanceDrawer(mounted.main);
    await flush();
    const dialog = mounted.main.querySelector('dialog')!;
    const seeding = [...mounted.main.querySelectorAll<HTMLButtonElement>('.what-if-metadata .provenance-number')].find((b) => b.textContent === `${companion.seeding.initial_infectious} infectious`)!;
    seeding.click();
    expect(dialog.querySelectorAll('.provenance-record')).toHaveLength(companion.seeding.provenance.length);
    expect(dialog.textContent).toContain(companion.seeding.provenance[0].sha256);
    expect(dialog.textContent).toContain(companion.seeding.limitation);
    dialog.querySelector<HTMLButtonElement>('button')!.click();
    const rows = [...mounted.main.querySelectorAll('.parameter-citations tbody tr')];
    expect(rows.map((r) => r.querySelector('th')?.textContent)).toEqual(companion.parameters.map((p) => p.parameter));
    rows[1].querySelector<HTMLButtonElement>('td button')!.click();
    expect(dialog.textContent).toContain(`Published source: ${companion.parameters[1].source}`);
    expect(dialog.querySelectorAll('.provenance-citation')).toHaveLength(1);
    closeDrawer(); mounted.cleanup();
  });
  it('never shows a scenario without its provenance companion', async () => {
    const mounted = mount(vi.fn().mockResolvedValue(scenario), vi.fn().mockResolvedValue(null)); await flush();
    expect(mounted.main.textContent).toContain('Simulation unavailable. Scenario provenance not yet available');
    expect(mounted.slider.disabled).toBe(true); expect(mounted.factory).toHaveBeenCalledTimes(0); mounted.cleanup();
    const refused = mount(vi.fn().mockResolvedValue(scenario), vi.fn().mockRejectedValue(new Error('Invalid scenario provenance: seed does not match the scenario'))); await flush();
    expect(refused.main.textContent).toContain('Simulation unavailable. Invalid scenario provenance');
    expect(refused.slider.disabled).toBe(true); expect(refused.factory).toHaveBeenCalledTimes(0); refused.cleanup();
  });
});
