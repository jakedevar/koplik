import { gaines, loadScenario, scenarioJson, type Scenario } from './scenario';
import { createSimulationWorker, type SimulationWorker } from './simulation';
import type { EnsembleResult } from './generated/v2/EnsembleResult';
import { ensembleChart } from './what-if-chart';

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
interface Options {
  base: string;
  synthetic?: boolean;
  load?: typeof loadScenario;
  worker?: () => SimulationWorker;
  clock?: () => number;
}

export function mountWhatIf(main: HTMLElement, options: Options): () => void {
  const panel = element('section', undefined, 'panel what-if');
  panel.setAttribute('aria-label', 'Gaines County 2025 what-if');
  panel.append(element('h2', 'What if? · Gaines County, Texas · 2025'),
    element('p', 'illustrative model scenario, not a prediction', 'notice'));
  if (options.synthetic) panel.append(element('p', 'SYNTHETIC MODEL INPUTS · Artificial seven-county fixture for development only; coverage, population and seeding are invented.', 'synthetic notice'));
  const status = element('p', 'Loading scenario data…', 'what-if-status');
  status.setAttribute('role', 'status');
  const controls = element('div', undefined, 'what-if-controls');
  const label = element('label', 'Gaines County kindergarten MMR coverage');
  const slider = element('input');
  slider.type = 'range'; slider.id = 'what-if-coverage'; slider.min = '0'; slider.max = '100'; slider.step = 'any'; slider.disabled = true;
  label.htmlFor = slider.id;
  const coverage = element('output');
  coverage.htmlFor = slider.id;
  const reset = element('button', 'Restore baseline coverage');
  reset.type = 'button'; reset.disabled = true;
  controls.append(label, slider, coverage, reset);
  const metadata = element('div', undefined, 'what-if-metadata');
  const output = element('div', undefined, 'what-if-result');
  panel.append(status, controls, metadata, output);
  main.append(panel);

  let disposed = false;
  let scenario: Scenario | undefined;
  let worker: SimulationWorker | undefined;
  let desired = 0;
  let active = 0;
  let started = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let baseline = 0;
  const now = options.clock || (() => performance.now());

  function fail(message: string) {
    output.replaceChildren();
    panel.setAttribute('aria-busy', 'false');
    status.textContent = `Simulation unavailable. ${message}`;
  }
  function render(result: EnsembleResult, elapsed: number) {
    if (result.contract_version !== 2 || result.members.length !== 1000 || result.seed !== scenario!.seed || !/^[0-9a-f]{64}$/.test(result.fingerprint)) {
      fail('Unexpected engine result'); return;
    }
    status.textContent = `1,000 runs complete · update ${(elapsed / 1000).toFixed(3)} s${elapsed >= 3000 ? ' · exceeds the 3 s target' : ''}${options.synthetic ? ' · synthetic fixture' : ''}`;
    panel.dataset.updateMs = String(elapsed);
    panel.setAttribute('aria-busy', 'false');
    const fingerprint = element('code', result.fingerprint);
    fingerprint.className = 'engine-fingerprint';
    const fingerprintLabel = element('p', 'Engine fingerprint (sha256, member 0 full trajectory): ');
    fingerprintLabel.append(fingerprint);
    const replay = element('details');
    replay.append(element('summary', 'Exact scenario JSON for native replay'), element('pre', result.scenario_json));
    const reports = element('details', undefined, 'table-scroll');
    reports.append(element('summary', 'Exact daily ensemble values'));
    const table = element('table');
    table.append(element('caption', 'Gaines County cumulative infections · 1,000 simulated runs'));
    const head = element('thead');
    const header = element('tr');
    for (const text of ['Day', 'Median', '50% lower', '50% upper', '90% lower', '90% upper']) {
      const cell = element('th', text); cell.scope = 'col'; header.append(cell);
    }
    head.append(header);
    const body = element('tbody');
    for (const day of result.daily.filter((r) => r.geography === gaines)) {
      const row = element('tr');
      const dayCell = element('th', String(day.day)); dayCell.scope = 'row'; row.append(dayCell);
      for (const value of ['median', 'lower_50', 'upper_50', 'lower_90', 'upper_90'] as const) row.append(element('td', String(day.cumulative_infections[value])));
      body.append(row);
    }
    table.append(head, body); reports.append(table);
    output.replaceChildren(element('h3', 'Simulated cumulative infections'), ensembleChart(result),
      element('p', 'Line: median · Dark band: equal-tail 50% · Light band: equal-tail 90%. Predictive bands describe the simulated ensemble. Includes initial exposed and infectious individuals, excludes vaccine immunity; these are not reported cases.', 'chart-note'),
      fingerprintLabel, replay, reports);
  }
  function dispatch() {
    timer = undefined;
    if (disposed || !scenario || active) return;
    try {
      if (!worker) {
        worker = (options.worker || createSimulationWorker)();
        worker.onmessage = ({ data }) => {
          if (disposed || data.id !== active) return;
          active = 0;
          if (data.id !== desired) { if (!timer) dispatch(); return; }
          if ('error' in data) fail(data.error);
          else render(data.result, now() - started);
        };
        worker.onerror = () => {
          if (disposed) return;
          worker?.terminate(); worker = undefined; active = 0;
          fail('The simulation worker could not run. Adjust coverage to retry.');
        };
      }
      active = desired;
      worker.postMessage({ id: active, scenarioJson: scenarioJson(scenario, Number(slider.value)) });
    } catch (error) {
      active = 0;
      fail(error instanceof Error ? error.message : String(error));
    }
  }
  function update(immediate = false) {
    desired++;
    started = now();
    coverage.textContent = `${slider.value}%${Number(slider.value) === baseline ? ' · baseline' : ' · what-if override'}`;
    slider.setAttribute('aria-valuetext', coverage.textContent);
    output.replaceChildren(); delete panel.dataset.updateMs;
    panel.setAttribute('aria-busy', 'true');
    status.textContent = 'Running 1,000 simulated trajectories…';
    clearTimeout(timer);
    if (immediate) dispatch();
    else timer = setTimeout(dispatch, 120);
  }
  slider.addEventListener('input', () => update());
  reset.addEventListener('click', () => { slider.value = String(baseline); update(true); });

  (options.load || loadScenario)(options.base, options.synthetic).then((loaded) => {
    if (disposed) return;
    if (!loaded) { status.textContent = 'Scenario data not yet available. Gaines County coverage, population and centroids must come from the pipeline.'; return; }
    scenario = loaded;
    const node = loaded.nodes.find((n) => n.id === gaines)!;
    const measured = node.baseline_coverage;
    if (measured.status !== 'reported') { fail('Measured coverage not yet available'); return; }
    baseline = measured.coverage_pct;
    slider.value = String(baseline); slider.disabled = false; reset.disabled = false;
    metadata.append(element('p', `${options.synthetic ? 'Synthetic fixture baseline' : 'Measured baseline coverage'}: ${baseline}% · MMWR ${loaded.start_week.year} W${loaded.start_week.week} · Seed: ${loaded.seed} · 1,000 runs`));
    const sources = element('details');
    sources.append(element('summary', 'Baseline coverage provenance'));
    for (const provenance of measured.provenance) {
      const line = element('p', `${provenance.source_id} · Retrieved ${provenance.retrieved_at} · Licence/terms: ${provenance.licence_id}`);
      const link = element('a', provenance.url);
      // Source URLs are rendered only as HTTP(S) links, never executable schemes.
      if (/^https?:\/\//i.test(provenance.url)) { link.href = provenance.url; line.append(element('br'), link); }
      line.append(element('br'), element('code', provenance.sha256)); sources.append(line);
    }
    const parameters = element('details');
    parameters.append(element('summary', 'Model parameters (from scenario artifact)'), element('pre', JSON.stringify(loaded.parameters, null, 2)));
    metadata.append(sources, parameters);
    update(true);
  }).catch((error: unknown) => { if (!disposed) fail(error instanceof Error ? error.message : String(error)); });

  return () => { disposed = true; clearTimeout(timer); worker?.terminate(); panel.remove(); };
}
