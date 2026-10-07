import { caseDefinitionLabels, type Dataset } from './data';
import {
  backtestReportPath, evaluationScope, insufficientWords, loadForecast, noMeasuredSkill, parameterValue, percent, quantileAt, seriesRows, skillWords, weekLabel, weekOrdinal,
  type Forecast, type ForecastProvenance, type ForecastSeries, type PublishedForecast, type Week,
} from './forecast';
import type { WeeklyCaseCount } from './generated/v3/WeeklyCaseCount';
import { bindProvenance, provenanceNumber, uniqueProvenance, type ProvenanceInfo } from './provenance';

const NS = 'http://www.w3.org/2000/svg';
function svgNode(tag: string, attributes: Record<string, string | number>, text?: string) {
  const node = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  if (text) node.textContent = text;
  return node;
}
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}
const show = (value: number) => (Number.isInteger(value) ? String(value) : (Math.round(value * 10) / 10).toString());

/** Parameter citations for the provenance drawer: published literature, not stored snapshots. */
function citationsOf(provenance: ForecastProvenance) {
  return provenance.parameters.map((p) => ({ source: `${p.parameter}: ${p.source}`, url: p.url, note: p.note }));
}

interface ChartInput {
  name: string;
  caseWords: string;
  /** Reported weekly counts of the forecast series, under its own case definition only. */
  history: WeeklyCaseCount[];
  rows: Forecast[];
  provenance: ForecastProvenance;
  info: ProvenanceInfo;
  synthetic: boolean;
}

/**
 * Reported weekly counts as bars, then the forecast: median line, 50% and 90% bands. The weeks after
 * the origin that are already reported are provisional and were not used: they are drawn as dashed
 * bars, never as part of the forecast's input. Exact values are in the table beside the chart.
 */
export function forecastChart({ name, caseWords, history, rows, provenance, info, synthetic }: ChartInput): SVGSVGElement {
  const origin = weekOrdinal(provenance.origin_week);
  const first = origin - 15;
  const last = origin + provenance.horizon_weeks;
  const step = 560 / (last - first + 1);
  const x = (ordinal: number) => 50 + (ordinal - first + 0.5) * step;
  const observed = history.filter((r) => weekOrdinal(r.week) >= first && weekOrdinal(r.week) <= weekOrdinal(provenance.latest_data_week));
  const upper = rows.map((r) => quantileAt(r, 0.95)!);
  const counts = observed.map((r) => (r.cases.status === 'reported' ? r.cases.count : 0));
  const max = Math.max(1, ...upper, ...counts);
  const y = (value: number) => 190 - (value / max) * 160;
  const median = rows.map((r) => quantileAt(r, 0.5)!);
  const last90 = rows.at(-1)!;
  const label = `Forecast of weekly ${caseWords} for ${name}: the median runs from ${show(median[0])} in ${weekLabel(rows[0].target_week)} to ${show(median.at(-1)!)} in ${weekLabel(last90.target_week)}; in the last week the 90% interval runs from ${show(quantileAt(last90, 0.05)!)} to ${show(quantileAt(last90, 0.95)!)}. Exact values are in the table below.`;
  const svg = svgNode('svg', { viewBox: '0 0 640 270', role: 'img', 'aria-label': label, class: 'forecast-chart' }) as SVGSVGElement;
  svg.append(svgNode('title', {}, label));

  const known = new Map<number, Week>();
  for (const r of observed) known.set(weekOrdinal(r.week), r.week);
  known.set(origin, provenance.origin_week);
  for (const r of rows) known.set(weekOrdinal(r.target_week), r.target_week);
  for (const value of [0, max]) {
    svg.append(svgNode('line', { x1: 50, x2: 610, y1: y(value), y2: y(value), class: 'gridline' }), svgNode('text', { x: 5, y: y(value) + 4, class: 'axis-label' }, show(value)));
  }
  let previousYear = 0;
  for (const [ordinal, week] of [...known].sort((a, b) => a[0] - b[0])) {
    if ((ordinal - first) % 4 !== 0 && ordinal !== last) continue;
    svg.append(svgNode('text', { x: x(ordinal), y: 207, 'text-anchor': 'middle', class: 'axis-label' }, week.year === previousYear ? `W${week.week}` : weekLabel(week)));
    previousYear = week.year;
  }
  svg.append(svgNode('text', { x: 50, y: 15, class: 'axis-label' }, `Weekly ${caseWords}`));

  // The forecast, drawn first so the provisional bars stay visible over it.
  const bands = svgNode('g', { class: 'forecast-bands' });
  for (const [level, lowQ, highQ] of [[90, 0.05, 0.95], [50, 0.25, 0.75]] as const) {
    const top = rows.map((r) => `${x(weekOrdinal(r.target_week))},${y(quantileAt(r, highQ)!)}`);
    const bottom = [...rows].reverse().map((r) => `${x(weekOrdinal(r.target_week))},${y(quantileAt(r, lowQ)!)}`);
    const band = svgNode('polygon', { points: [...top, ...bottom].join(' '), class: `forecast-band-${level}` });
    band.append(svgNode('title', {}, `${level}% of the simulated runs fall inside this band`));
    bands.append(band);
  }
  bands.append(svgNode('polyline', { points: rows.map((r) => `${x(weekOrdinal(r.target_week))},${y(quantileAt(r, 0.5)!)}`).join(' '), class: 'forecast-median' }));
  bindProvenance(bands as unknown as SVGElement, { ...info, label: `Forecast of weekly ${caseWords} for ${name} · median and 50% and 90% bands` });
  svg.append(bands);

  svg.append(svgNode('line', { x1: x(origin) + step / 2, x2: x(origin) + step / 2, y1: 22, y2: 190, class: 'forecast-origin' }),
    svgNode('text', { x: x(origin) + step / 2 - 4, y: 33, 'text-anchor': 'end', class: 'axis-label' }, `Forecast origin ${weekLabel(provenance.origin_week)}`));

  for (const row of observed) {
    if (row.cases.status !== 'reported') continue;
    const ordinal = weekOrdinal(row.week);
    const provisional = ordinal > origin;
    const text = `${weekLabel(row.week)}: ${row.cases.count} ${caseDefinitionLabels[row.case_definition]}${provisional ? ' (provisional: not used by the forecast)' : ''}`;
    const bar = row.cases.count === 0
      ? svgNode('line', { x1: x(ordinal) - step * 0.3, x2: x(ordinal) + step * 0.3, y1: 190, y2: 190, class: provisional ? 'forecast-provisional-zero' : 'case-zero', 'data-week': row.week.week })
      : svgNode('rect', { x: x(ordinal) - step * 0.3, y: y(row.cases.count), width: step * 0.6, height: 190 - y(row.cases.count), class: provisional ? 'forecast-provisional-bar' : 'case-bar', 'data-week': row.week.week });
    bar.append(svgNode('title', {}, text));
    bindProvenance(bar as SVGElement, { label: text, records: row.provenance, synthetic });
    svg.append(bar);
  }
  const legend = svgNode('g', { class: 'forecast-legend' });
  legend.append(svgNode('rect', { x: 50, y: 226, width: 10, height: 10, class: 'case-bar' }), svgNode('text', { x: 65, y: 235, class: 'axis-label' }, 'Reported'),
    svgNode('rect', { x: 125, y: 226, width: 10, height: 10, class: 'forecast-provisional-bar' }), svgNode('text', { x: 140, y: 235, class: 'axis-label' }, 'Provisional, not used'),
    svgNode('rect', { x: 275, y: 226, width: 10, height: 10, class: 'forecast-band-50' }), svgNode('text', { x: 290, y: 235, class: 'axis-label' }, '50% band'),
    svgNode('rect', { x: 360, y: 226, width: 10, height: 10, class: 'forecast-band-90' }), svgNode('text', { x: 375, y: 235, class: 'axis-label' }, '90% band'),
    svgNode('line', { x1: 445, x2: 461, y1: 231, y2: 231, class: 'forecast-median' }), svgNode('text', { x: 466, y: 235, class: 'axis-label' }, 'Forecast median'));
  svg.append(legend);
  return svg;
}

export interface ForecastOptions {
  base: string;
  synthetic?: boolean;
  /** The surveillance dataset: names and the reported history drawn under the forecast. */
  data?: Dataset;
  /** The geography selected on the dashboard when the panel opens. */
  geography?: string;
  load?: typeof loadForecast;
}

/** The forecast panel: one series at a time, its measured backtest skill in plain words beside it. */
export function mountForecast(main: HTMLElement, options: ForecastOptions): () => void {
  const panel = element('section', undefined, 'panel forecast');
  panel.setAttribute('aria-label', 'Forecast');
  panel.append(element('h2', 'Where next? · forecast'));
  const status = element('p', 'Loading forecast…', 'forecast-status');
  status.setAttribute('role', 'status');
  const notices = element('div');
  const controls = element('div', undefined, 'forecast-controls');
  const label = element('label', 'Forecast for');
  const select = element('select');
  select.id = 'forecast-geography';
  label.htmlFor = select.id;
  controls.append(label, select);
  controls.hidden = true;
  const result = element('div', undefined, 'forecast-result');
  panel.append(status, notices, controls, result);
  // The backtest has its own section, after the forecasts and not beside any chart; it fills in once the forecast loads.
  const evaluation = element('section', undefined, 'panel forecast-evaluation');
  evaluation.setAttribute('aria-label', 'How we evaluate forecasts');
  evaluation.hidden = true;
  main.append(panel, evaluation);

  let disposed = false;
  let published: PublishedForecast | undefined;
  const names = new Map<string, string>((options.data?.geographies ?? []).map((g) => [g.id, g.name]));
  const nameOf = (id: string) => names.get(id) ?? id;

  function renderSeries(id: string) {
    const { provenance } = published!;
    const series = provenance.series.find((s) => s.geography === id) as ForecastSeries;
    const name = nameOf(id);
    const caseWords = caseDefinitionLabels[series.case_definition];
    const blocks: Element[] = [];
    const rows = seriesRows(published!, id);
    const info: ProvenanceInfo = {
      label: `Forecast for ${name}`, records: uniqueProvenance(rows.flatMap((r) => r.provenance)), synthetic: options.synthetic,
      note: `A model projection of weekly ${caseWords}, not a source observation: source records are the weekly reports the forecast was made from (${provenance.input.rows} input rows, sha256 ${provenance.input.sha256}). Method, seed ${provenance.seed}, ${provenance.run_count} members and every parameter with its citation are listed under "Method and parameters".`,
      citations: citationsOf(provenance),
    };
    // A forecast of a series the backtest did not score says so first, before the reader sees the chart.
    if (series.status === 'forecast') {
      blocks.push(series.skill === 'backtested'
        ? element('p', 'This series is the one the backtest scored: see "How we evaluate forecasts" below.', 'notice forecast-backtested')
        : element('p', noMeasuredSkill, 'notice forecast-no-skill'));
    }
    blocks.push(element('h3', `${name} · weekly ${caseWords}`));
    if (series.status === 'insufficient_data') {
      blocks.push(element('p', insufficientWords(provenance, series), 'notice forecast-insufficient'));
    } else {
      const history = (options.data?.cases ?? []).filter((r) => r.geography === id && r.case_definition === series.case_definition);
      const chart = forecastChart({ name, caseWords, history, rows, provenance, info, synthetic: Boolean(options.synthetic) });
      const provisional = Number(parameterValue(provenance, 'provisional_weeks'));
      blocks.push(chart, element('p', `Line: forecast median · dark band: 50% of the ${provenance.run_count} simulated runs · light band: 90% · bars: reported ${caseWords}. Made from data through MMWR ${weekLabel(provenance.latest_data_week)}; the ${provisional} most recent weeks are provisional (still being reported) and were not used, so the forecast starts from ${weekLabel(provenance.origin_week)} and looks ${provenance.horizon_weeks} weeks ahead. Dashed bars are those provisional weeks.`, 'chart-note'));
      const details = element('details', undefined, 'table-scroll');
      details.append(element('summary', 'Exact forecast values'));
      const table = element('table');
      table.append(element('caption', `${name} · forecast weekly ${caseWords} · ${provenance.run_count}-run ensemble`));
      const headRow = element('tr');
      for (const text of ['MMWR week', 'Median', '50% interval', '90% interval']) { const cell = element('th', text); cell.scope = 'col'; headRow.append(cell); }
      const head = element('thead'); head.append(headRow);
      const body = element('tbody');
      for (const row of rows) {
        const tr = element('tr');
        const week = element('th', weekLabel(row.target_week)); week.scope = 'row';
        const cell = (text: string, what: string) => {
          const td = element('td');
          td.append(provenanceNumber(text, { ...info, label: `${name} · ${weekLabel(row.target_week)} · ${what} ${text}`, records: uniqueProvenance(row.provenance) }));
          return td;
        };
        const q = (level: number) => show(quantileAt(row, level)!);
        tr.append(week, cell(q(0.5), 'median'), cell(`${q(0.25)}–${q(0.75)}`, '50% interval'), cell(`${q(0.05)}–${q(0.95)}`, '90% interval'));
        body.append(tr);
      }
      table.append(head, body);
      details.append(table);
      blocks.push(details);
    }

    const method = element('details');
    method.append(element('summary', 'Method and parameters'), element('p', provenance.statement), element('p', provenance.method), element('p', provenance.origin_rule),
      element('p', `Seed ${provenance.seed} · ${provenance.run_count} ensemble members · input ${provenance.input.artifact} (${provenance.input.rows} rows, sha256 ${provenance.input.sha256}). ${provenance.scope_note}`));
    const parameters = element('table', undefined, 'parameter-citations');
    parameters.append(element('caption', 'Forecast parameters and the sources they are cited to'));
    const parameterHead = element('tr');
    for (const heading of ['Parameter', 'Value', 'Published source']) { const cell = element('th', heading); cell.scope = 'col'; parameterHead.append(cell); }
    const parameterHeader = element('thead'); parameterHeader.append(parameterHead);
    const parameterBody = element('tbody');
    for (const parameter of provenance.parameters) {
      const tr = element('tr');
      const name = element('th', parameter.parameter); name.scope = 'row';
      const valueCell = element('td');
      const text = JSON.stringify(parameter.value);
      valueCell.append(provenanceNumber(text, { label: `${parameter.parameter} = ${text}`, records: [], synthetic: options.synthetic, note: 'A forecast parameter, not an observation.',
        citations: [{ source: parameter.source, url: parameter.url, note: parameter.note }] }));
      const sourceCell = element('td', `${parameter.source}. ${parameter.note}`);
      if (parameter.url && /^https?:\/\//i.test(parameter.url)) { const link = element('a', parameter.url); link.href = parameter.url; sourceCell.append(element('br'), link); }
      tr.append(name, valueCell, sourceCell); parameterBody.append(tr);
    }
    parameters.append(parameterHeader, parameterBody);
    method.append(parameters);
    blocks.push(method);
    result.replaceChildren(...blocks);
  }

  /** The backtest, as an evaluation of the method on the series it was run on: what was scored, how it did, what it does not measure. */
  function renderEvaluation() {
    const { provenance } = published!;
    const skill = provenance.backtest;
    const blocks: Element[] = [element('h2', 'How we evaluate forecasts')];
    if (options.synthetic) blocks.push(element('p', 'SYNTHETIC EVALUATION · Invented numbers for development only; nothing was backtested.', 'synthetic notice'));
    if (!skill) {
      blocks.push(element('p', provenance.scope_note, 'notice'));
    } else {
      const words = skillWords(skill);
      const skillInfo: ProvenanceInfo = {
        label: `Backtest skill · ${skill.name}`, records: [], synthetic: options.synthetic,
        note: `Measured by koplik-epi on ${skill.series}. Read exactly from the committed report ${skill.report_path} (sha256 ${skill.report_sha256}), which was run on the report-vintage manifest sha256 ${skill.manifest_sha256}. A measurement of the method on that series, not of any forecast shown above.`,
        citations: citationsOf(provenance),
      };
      blocks.push(element('p', `We have tested this method once, on ${skill.name}. Each forecast used only the reports available at its forecast date and was then scored against the counts reported for the weeks it predicted.`));
      const headline = element('p', undefined, 'forecast-headline');
      headline.append(provenanceNumber(words.headline, skillInfo));
      blocks.push(headline, element('p', words.scores));
      if (words.narrow) blocks.push(element('p', words.narrow, 'notice forecast-narrow'));
      blocks.push(element('p', evaluationScope(provenance), 'notice forecast-evaluation-scope'));
      const horizons = element('details', undefined, 'table-scroll');
      horizons.append(element('summary', 'Backtest scores by horizon, and what the backtest does not show'));
      const table = element('table');
      table.append(element('caption', `Backtest on ${skill.name}, exactly as measured`));
      const headRow = element('tr');
      for (const text of ['Weeks ahead', 'Targets', 'Mean CRPS (cases)', '50% interval coverage', '90% interval coverage']) { const cell = element('th', text); cell.scope = 'col'; headRow.append(cell); }
      const head = element('thead'); head.append(headRow);
      const body = element('tbody');
      const rowOf = (text: string, n: number, crps: number | null | undefined, c50: number | null | undefined, c90: number | null | undefined) => {
        const tr = element('tr');
        const weeks = element('th', text); weeks.scope = 'row';
        const number = (value: number | null | undefined, digits: number) => (value == null ? 'no targets' : value.toFixed(digits));
        // Coverage uses the prose's precision: one-decimal percent plus the exact count (covered of n).
        const coverage = (value: number | null | undefined) => (value == null ? 'no targets' : `${percent(value)} (${Math.round(value * n)} of ${n})`);
        tr.append(weeks, element('td', String(n)), element('td', number(crps, 2)), element('td', coverage(c50)), element('td', coverage(c90)));
        return tr;
      };
      for (const h of skill.by_horizon) body.append(rowOf(String(h.horizon), h.n, h.mean_crps, h.coverage_50, h.coverage_90));
      body.append(rowOf('All', skill.targets, skill.mean_crps, skill.coverage_50, skill.coverage_90));
      table.append(head, body);
      const limits = element('ul');
      for (const text of skill.limitations) limits.append(element('li', text));
      const report = element('p', 'Exact report: ');
      if (options.synthetic) report.append(element('code', skill.report_path));
      else {
        const link = element('a', skill.report_path);
        link.href = `${options.base.replace(/\/$/, '')}/${backtestReportPath}`;
        report.append(link);
      }
      report.append(document.createTextNode(' · sha256 '), element('code', skill.report_sha256), document.createTextNode(` · seed ${skill.seed}`));
      horizons.append(table, limits, report);
      blocks.push(horizons);
    }
    evaluation.replaceChildren(...blocks);
    evaluation.hidden = false;
  }

  function choose(id: string) {
    if (!published) return;
    select.value = id;
    renderSeries(id);
  }
  function fill(preferred?: string) {
    const { provenance } = published!;
    const options_ = (list: ForecastSeries[], group: string) => {
      const optgroup = element('optgroup'); optgroup.label = group;
      for (const s of [...list].sort((a, b) => nameOf(a.geography).localeCompare(nameOf(b.geography), 'en'))) {
        const option = element('option', `${nameOf(s.geography)} · ${caseDefinitionLabels[s.case_definition]}`); option.value = s.geography; optgroup.append(option);
      }
      return optgroup;
    };
    const forecast = provenance.series.filter((s) => s.status === 'forecast');
    const insufficient = provenance.series.filter((s) => s.status === 'insufficient_data');
    select.replaceChildren(...(forecast.length ? [options_(forecast, 'Forecast available')] : []), ...(insufficient.length ? [options_(insufficient, 'Insufficient data: no forecast')] : []));
    const initial = [preferred, forecast[0]?.geography, insufficient[0]?.geography].find((id) => id && provenance.series.some((s) => s.geography === id));
    if (initial) choose(initial);
  }
  select.addEventListener('change', () => choose(select.value));
  const events = main.parentElement ?? main;
  const onSelection = (event: Event) => {
    const geography = (event as CustomEvent<{ geography?: string }>).detail?.geography;
    if (published && geography && published.provenance.series.some((s) => s.geography === geography)) choose(geography);
  };
  events.addEventListener('koplik:selection', onSelection);

  (options.load || loadForecast)(options.base, options.synthetic).then((loaded) => {
    if (disposed) return;
    if (!loaded) { status.textContent = 'Forecast not yet available: the pipeline has not published one. Nothing is shown rather than a guess.'; return; }
    published = loaded;
    const { provenance } = loaded;
    const made = provenance.series.filter((s) => s.status === 'forecast').length;
    status.textContent = made
      ? `${made} of ${provenance.series.length} series have enough data to forecast; the rest are shown as insufficient data. Forecast origin ${weekLabel(provenance.origin_week)}, ${provenance.horizon_weeks} weeks ahead.`
      : `No series has enough data to forecast: all ${provenance.series.length} are insufficient data.`;
    notices.replaceChildren(
      ...(options.synthetic ? [element('p', 'SYNTHETIC FORECAST · Invented values for development only; not a model projection of any observed series.', 'synthetic notice')] : []),
      element('p', 'Model projection from reported counts, not a prediction of what will happen.', 'notice'));
    controls.hidden = false;
    fill(options.geography);
    renderEvaluation();
  }).catch((error: unknown) => {
    if (!disposed) { status.textContent = `Forecast unavailable. ${error instanceof Error ? error.message : String(error)}`; status.setAttribute('role', 'alert'); }
  });

  return () => { disposed = true; events.removeEventListener('koplik:selection', onSelection); panel.remove(); evaluation.remove(); };
}
