import { gaines, loadScenario, scenarioJson, type Scenario } from './scenario';
import { createSimulationWorker, type SimulationWorker } from './simulation';
import type { EnsembleResult } from './generated/v2/EnsembleResult';
import { ensembleChart } from './what-if-chart';
import { bindProvenance, provenanceNumber, uniqueProvenance, type ProvenanceInfo } from './provenance';
import { parseScenario } from './scenario';
import { loadScenarioProvenance } from './scenario-provenance';
import type { Provenance } from './generated/Provenance';

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
function sourceLine(provenance: Partial<Provenance>) {
  const line = element('p', `${provenance.source_id} · Retrieved ${provenance.retrieved_at} · Licence/terms: ${provenance.licence_id}`);
  // Source URLs are rendered only as HTTP(S) links, never executable schemes.
  if (provenance.url && /^https?:\/\//i.test(provenance.url)) { const link = element('a', provenance.url); link.href = provenance.url; line.append(element('br'), link); }
  line.append(element('br'), element('code', provenance.sha256 ?? 'Missing'));
  return line;
}
interface Options {
  base: string;
  synthetic?: boolean;
  load?: typeof loadScenario;
  loadProvenance?: typeof loadScenarioProvenance;
  worker?: () => SimulationWorker;
  clock?: () => number;
}

export function mountWhatIf(main: HTMLElement, options: Options): () => void {
  const panel = element('section', undefined, 'panel what-if');
  panel.setAttribute('aria-label', 'Gaines County 2025 what-if');
  panel.append(element('h2', 'What if? · Gaines County, Texas · 2025'),
    element('p', 'illustrative model scenario, not a prediction', 'notice'),
    element('p', 'A what-if tool, not a fitted model: it starts from a reported case count and published parameters, and was never adjusted to match how the outbreak actually unfolded.', 'notice'));
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
  let baselineInfo: ProvenanceInfo | undefined;
  const now = options.clock || (() => performance.now());

  function fail(message: string) {
    output.replaceChildren();
    panel.setAttribute('aria-busy', 'false');
    status.textContent = `Simulation unavailable. ${message}`;
  }
  function render(result: EnsembleResult) {
    if (result.contract_version !== 2 || result.members.length !== 1000 || result.seed !== scenario!.seed || !/^[0-9a-f]{64}$/.test(result.fingerprint)) {
      fail('Unexpected engine result'); return;
    }
    const replayInput = parseScenario(result.scenario_json, options.synthetic);
    const ensembleInfo: ProvenanceInfo = {
      label: 'Gaines County simulated cumulative infections · 1,000-run ensemble', synthetic: options.synthetic,
      records: uniqueProvenance(replayInput.nodes.flatMap((node) => [...node.provenance,
        ...(node.baseline_coverage.status === 'reported' ? node.baseline_coverage.provenance : [])])),
      note: 'Derived from the exact replay scenario, across all its counties. These are all source records linked to node population, centroids and baseline coverage. The initial seeding is sourced in the scenario provenance companion (the DSHS report it came from) and each parameter is cited there to its published source; neither is an observation of what happens next in this county. Coverage overrides are your own what-if choices. The seed, seeding and parameters appear on the panel.',
    };
    const fingerprint = element('code', result.fingerprint);
    fingerprint.className = 'engine-fingerprint';
    bindProvenance(fingerprint, { ...ensembleInfo, label: 'Engine fingerprint · member 0 full trajectory' });
    const fingerprintLabel = element('p', 'Engine fingerprint (sha256, member 0 full trajectory): ');
    fingerprintLabel.append(fingerprint);
    const replay = element('details');
    const replayJson = element('pre', result.scenario_json);
    bindProvenance(replayJson, { ...ensembleInfo, label: 'Exact scenario JSON for native replay' });
    replay.append(element('summary', 'Exact scenario JSON for native replay'), replayJson);
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
      for (const value of ['median', 'lower_50', 'upper_50', 'lower_90', 'upper_90'] as const) {
        const cell = element('td');
        cell.append(provenanceNumber(String(day.cumulative_infections[value]), { ...ensembleInfo, label: `Day ${day.day} · ${value} · ${day.cumulative_infections[value]} simulated cumulative infections` }));
        row.append(cell);
      }
      body.append(row);
    }
    table.append(head, body); reports.append(table);
    const chart = ensembleChart(result);
    bindProvenance(chart, ensembleInfo);
    output.replaceChildren(element('h3', 'Simulated cumulative infections'), chart,
      element('p', 'Line: median · Dark band: equal-tail 50% · Light band: equal-tail 90%. Predictive bands describe the simulated ensemble. Includes initial exposed and infectious individuals, excludes vaccine immunity; these are not reported cases.', 'chart-note'),
      fingerprintLabel, replay, reports);
    const elapsed = now() - started;
    status.textContent = `1,000 runs complete · update ${(elapsed / 1000).toFixed(3)} s${elapsed >= 3000 ? ' · exceeds the 3 s target' : ''}${options.synthetic ? ' · synthetic fixture' : ''}`;
    status.replaceChildren(provenanceNumber(status.textContent, { label: status.textContent, records: [], synthetic: options.synthetic,
      note: 'Locally measured browser update time and configured run count. These are not published surveillance observations; the timer has no source snapshot.' }));
    panel.dataset.updateMs = String(elapsed);
    panel.setAttribute('aria-busy', 'false');
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
          else render(data.result);
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
    if (baselineInfo) bindProvenance(coverage, Number(slider.value) === baseline ? baselineInfo : {
      label: `${slider.value}% · user-selected what-if coverage`, records: [], synthetic: options.synthetic,
      note: 'User-selected scenario override, not measured vaccination coverage. No published source is attached to this setting.',
    });
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

  (options.load || loadScenario)(options.base, options.synthetic).then(async (loaded) => {
    if (disposed) return;
    if (!loaded) { status.textContent = 'Scenario data not yet available. Gaines County coverage, population and centroids must come from the pipeline.'; return; }
    const provenance = await (options.loadProvenance || loadScenarioProvenance)(options.base, loaded, options.synthetic);
    if (disposed) return;
    if (!provenance) { fail('Scenario provenance not yet available: the scenario is never shown without its sources.'); return; }
    scenario = loaded;
    const node = loaded.nodes.find((n) => n.id === gaines)!;
    const measured = node.baseline_coverage;
    if (measured.status !== 'reported' && !options.synthetic) { fail('Measured coverage not yet available'); return; }
    baseline = measured.status === 'reported' ? measured.coverage_pct : loaded.coverage_overrides.find((o) => o.geography === gaines)!.coverage_pct;
    baselineInfo = {
      label: `${options.synthetic ? 'Synthetic starting coverage' : 'Measured baseline coverage'} · ${baseline}%`,
      records: measured.status === 'reported' ? measured.provenance : [], synthetic: options.synthetic,
      note: measured.status === 'reported' ? 'Baseline coverage sources attached to the Gaines County node.' :
        'Explicit synthetic fixture override. Measured coverage is missing. No source records are linked to the override itself; the fixture is retained in exact replay JSON.',
    };
    slider.value = String(baseline); slider.disabled = false; reset.disabled = false;
    const description = element('p', `${options.synthetic ? 'Synthetic fixture baseline' : 'Measured baseline coverage'}: `);
    description.append(provenanceNumber(`${baseline}%`, baselineInfo), document.createTextNode(` · MMWR ${loaded.start_week.year} W${loaded.start_week.week} · Seed: `),
      provenanceNumber(loaded.seed, { label: `Seed: ${loaded.seed}`, records: [], synthetic: options.synthetic,
        note: 'Explicit RNG seed from the scenario artifact. Configuration, not a source observation.' }), document.createTextNode(' · 1,000 runs'));
    metadata.append(description);
    const sources = element('details');
    sources.append(element('summary', measured.status === 'reported' ? 'Baseline coverage provenance' : 'Synthetic scenario input provenance (coverage override)'));
    for (const record of measured.status === 'reported' ? measured.provenance : node.provenance) sources.append(sourceLine(record));

    const seeding = provenance.seeding;
    const seedingInfo: ProvenanceInfo = {
      label: `Initial seeding · ${seeding.initial_infectious} infectious`, records: seeding.provenance, synthetic: options.synthetic,
      note: `${options.synthetic ? 'Synthetic fixture seeding. ' : ''}DSHS report dated ${seeding.report_date} recorded "${seeding.cell_as_printed}" for ${seeding.county_name_as_printed}. ${seeding.confirmed_basis}. Reporting multiplier ${seeding.reporting_multiplier}${seeding.reporting_multiplier === 1 ? ' (no correction for under-reporting)' : ''}. ${seeding.limitation}`,
    };
    const seedingLine = element('p', 'Seeded with ');
    seedingLine.append(provenanceNumber(`${seeding.initial_infectious} infectious`, seedingInfo), document.createTextNode(` and ${seeding.initial_exposed} exposed in Gaines County from the report DSHS dated ${seeding.report_date}, which recorded ${seeding.recorded_confirmed_count} confirmed cases${seeding.reporting_multiplier === 1 ? ' (taken as recorded, no correction for under-reporting)' : ` (multiplied by ${seeding.reporting_multiplier})`}. `));
    seedingLine.append(element('span', seeding.limitation));
    const seedingDetails = element('details');
    seedingDetails.append(element('summary', 'How the scenario is seeded, and its sources'), element('p', provenance.statement), element('p', `Rule: ${seeding.rule}`),
      element('p', `The report's own labelling that makes these counts confirmed cases: ${seeding.confirmed_basis}. First seen ${seeding.report_first_seen_at}.`));
    for (const skipped of seeding.skipped_vintages) seedingDetails.append(element('p', `Earlier report ${skipped.report_date} was not used: ${skipped.reason}.`));
    seedingDetails.append(element('p', provenance.neighbourhood_note));
    for (const excluded of provenance.excluded_nodes) seedingDetails.append(element('p', `${excluded.name} (${excluded.geography}) is not simulated: ${excluded.reason}.`));
    for (const record of seeding.provenance) seedingDetails.append(sourceLine(record));

    const parameters = element('details');
    const parameterJson = element('pre', JSON.stringify(loaded.parameters, null, 2));
    bindProvenance(parameterJson, { label: 'Model parameters (from scenario artifact)', records: [], synthetic: options.synthetic,
      note: 'The exact values this scenario runs with. Each is configuration, not an observation, and is cited to its published source in the table below and in the scenario provenance companion.',
      citations: provenance.parameters.map((p) => ({ source: `${p.parameter}: ${p.source}`, url: p.url, note: p.note })) });
    const citations = element('table', undefined, 'parameter-citations');
    citations.append(element('caption', 'Model parameters and the sources they are cited to'));
    const citationHead = element('tr');
    for (const heading of ['Parameter', 'Value', 'Published source']) { const cell = element('th', heading); cell.scope = 'col'; citationHead.append(cell); }
    const citationBody = element('tbody');
    for (const parameter of provenance.parameters) {
      const row = element('tr');
      const name = element('th', parameter.parameter); name.scope = 'row';
      const valueCell = element('td');
      const valueText = JSON.stringify(parameter.value);
      valueCell.append(provenanceNumber(valueText, { label: `${parameter.parameter} = ${valueText}`, records: [], synthetic: options.synthetic,
        note: 'A model parameter, not an observation.', citations: [{ source: parameter.source, url: parameter.url, note: parameter.note }] }));
      const sourceCell = element('td', `${parameter.source}. ${parameter.note}`);
      if (parameter.url && /^https?:\/\//i.test(parameter.url)) { const link = element('a', parameter.url); link.href = parameter.url; sourceCell.append(element('br'), link); }
      row.append(name, valueCell, sourceCell); citationBody.append(row);
    }
    const citationHeader = element('thead'); citationHeader.append(citationHead);
    citations.append(citationHeader, citationBody);
    parameters.append(element('summary', 'Model parameters and their sources'), parameterJson, citations);
    metadata.append(seedingLine, sources, seedingDetails, parameters);
    update(true);
  }).catch((error: unknown) => { if (!disposed) fail(error instanceof Error ? error.message : String(error)); });

  return () => { disposed = true; clearTimeout(timer); worker?.terminate(); panel.remove(); };
}
