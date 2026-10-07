import { caseDefinitionLabels, type Dataset } from './data';
import {
  backtestReportPath, basisLabel, evaluationScope, insufficientSkillStatus, insufficientWords, loadForecast, noMeasuredSkill, noSkillWords, parameterValue, percent, pooledWords, quantileAt,
  policyWords, seriesBacktestReportPath, seriesEvaluationScope, seriesRows, seriesSkillWords, skillWords, weekLabel, weekOrdinal, withheldNotice, withheldSeriesWords,
  type BacktestSkill, type Forecast, type ForecastProvenance, type ForecastSeries, type PublishedForecast, type SeriesBacktest, type Week,
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
  // First under the heading, before anything else: when forecasts are withheld, why (filled once the forecast loads).
  const withheldBox = element('div');
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
  panel.append(withheldBox, status, notices, controls, result);
  // The backtest has its own section, after the forecasts and not beside any chart; it fills in once the forecast loads.
  const evaluation = element('section', undefined, 'panel forecast-evaluation');
  evaluation.setAttribute('aria-label', 'How we evaluate forecasts');
  evaluation.hidden = true;
  main.append(panel, evaluation);

  let disposed = false;
  let published: PublishedForecast | undefined;
  const names = new Map<string, string>((options.data?.geographies ?? []).map((g) => [g.id, g.name]));
  const nameOf = (id: string) => names.get(id) ?? id;

  /** The notice above a forecast's chart: its own measured skill, or that it has none. Never another series' number. */
  function skillNotice(series: ForecastSeries, name: string): Element {
    const { provenance } = published!;
    const backtest = provenance.series_backtest;
    const entry = backtest?.by_series.find((e) => e.geography === series.geography);
    if (series.skill === 'backtested') return element('p', 'This series is the one the backtest scored: see "How we evaluate forecasts" below.', 'notice forecast-backtested');
    if (series.skill === 'measured' && backtest && entry?.measured) {
      const words = seriesSkillWords(backtest, entry);
      const info: ProvenanceInfo = {
        label: `Measured skill · ${name}`, records: [], synthetic: options.synthetic,
        note: `Measured by koplik-epi on ${backtest.series}, for this series alone. Read exactly from the committed report ${backtest.report_path} (sha256 ${backtest.report_sha256}), run on the source snapshot sha256 ${backtest.input_sha256}. ${backtest.protocol} A measurement of the method on this series' past, not of the forecast shown here.`,
        citations: citationsOf(provenance),
      };
      const box = element('div', undefined, 'notice forecast-measured');
      const headline = element('p', undefined, 'forecast-series-headline');
      headline.append(provenanceNumber(words.headline, info));
      box.append(element('p', 'Measured skill for this series.'), headline, element('p', words.detail));
      if (words.narrow) box.append(element('p', words.narrow, 'forecast-series-narrow'));
      if (words.against) box.append(element('p', words.against));
      box.append(element('p', 'The test and its scope are described under "How we evaluate forecasts" below.'));
      return box;
    }
    if (series.skill === insufficientSkillStatus && backtest && entry) return element('p', noSkillWords(backtest, entry), 'notice forecast-no-skill');
    return element('p', noMeasuredSkill, 'notice forecast-no-skill');
  }

  /** Above a withheld series: that no forecast is published for it, why, its own measured numbers when it has them, and the rule. */
  function withheldNotice_(series: ForecastSeries, name: string): Element {
    const { provenance } = published!;
    const words = withheldSeriesWords(provenance, series, name);
    const backtest = provenance.series_backtest;
    const box = element('div', undefined, 'notice forecast-withheld');
    box.append(element('p', words.headline, 'forecast-withheld-headline'), element('p', words.reason));
    if (words.measured) {
      const info: ProvenanceInfo = {
        label: `Measured skill · ${name}`, records: [], synthetic: options.synthetic,
        note: backtest
          ? `Measured by koplik-epi on ${backtest.series}, for this series alone. Read exactly from the committed report ${backtest.report_path} (sha256 ${backtest.report_sha256}), run on the source snapshot sha256 ${backtest.input_sha256}. ${backtest.protocol} A measurement of the method on this series' past, not of any forecast.`
          : 'Measured by koplik-epi on the series the report-vintage backtest scored; see "How we evaluate forecasts".',
        citations: citationsOf(provenance),
      };
      const measured = element('p', undefined, 'forecast-series-headline');
      measured.append(provenanceNumber(words.measured, info));
      box.append(measured);
    }
    box.append(element('p', words.rule, 'forecast-policy'));
    return box;
  }

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
    // A forecast says first, before the reader sees the chart, what is measured about its own series: its measured
    // skill from the series backtest (in plain words, with the basis), or that it has none.
    if (series.status === 'forecast') blocks.push(skillNotice(series, name));
    if (series.status === 'withheld') blocks.push(withheldNotice_(series, name));
    blocks.push(element('h3', `${name} · weekly ${caseWords}`));
    if (series.status === 'insufficient_data') {
      blocks.push(element('p', insufficientWords(provenance, series), 'notice forecast-insufficient'));
    } else if (series.status === 'withheld') {
      blocks.push(element('p', 'No chart and no forecast values are shown: the forecast the method made for this series is withheld and is not published.', 'chart-note'));
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

  /** The report-vintage backtest on the Texas DSHS outbreak total: what was scored, how it did, what it does not measure. */
  function westTexasBlocks(provenance: ForecastProvenance, skill: BacktestSkill, alongside: boolean): Element[] {
    const words = skillWords(skill);
    const skillInfo: ProvenanceInfo = {
      label: `Backtest skill · ${skill.name}`, records: [], synthetic: options.synthetic,
      note: `Measured by koplik-epi on ${skill.series}. Read exactly from the committed report ${skill.report_path} (sha256 ${skill.report_sha256}), which was run on the report-vintage manifest sha256 ${skill.manifest_sha256}. A measurement of the method on that series, not of any forecast shown above.`,
      citations: citationsOf(provenance),
    };
    const blocks: Element[] = [];
    if (alongside) blocks.push(element('h3', `Test on ${skill.name}: real-time by report vintage`));
    blocks.push(element('p', `We tested this method on ${skill.name}. Each forecast used only the reports available at its forecast date and was then scored against the counts reported for the weeks it predicted.`));
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
    for (const h of skill.by_horizon) body.append(scoreRow(String(h.horizon), h.n, h.mean_crps, h.coverage_50, h.coverage_90));
    body.append(scoreRow('All', skill.targets, skill.mean_crps, skill.coverage_50, skill.coverage_90));
    table.append(head, body);
    const limits = element('ul');
    for (const text of skill.limitations) limits.append(element('li', text));
    horizons.append(table, limits, reportLine(skill.report_path, backtestReportPath, skill.report_sha256, ` · seed ${skill.seed}`));
    blocks.push(horizons);
    return blocks;
  }

  /** One row of a score table: the prose's precision (percent with one decimal plus the exact count), two decimals for scores. */
  function scoreRow(text: string, n: number, crps: number | null | undefined, c50: number | null | undefined, c90: number | null | undefined, extra: string[] = []) {
    const tr = element('tr');
    const weeks = element('th', text); weeks.scope = 'row';
    const number = (value: number | null | undefined, digits: number) => (value == null ? 'no targets' : value.toFixed(digits));
    const coverage = (value: number | null | undefined) => (value == null ? 'no targets' : `${percent(value)} (${Math.round(value * n)} of ${n})`);
    tr.append(weeks, element('td', String(n)), ...extra.map((cell) => element('td', cell)), element('td', number(crps, 2)), element('td', coverage(c50)), element('td', coverage(c90)));
    return tr;
  }

  /** "Exact report: <link> · sha256 <hash>" (named, not linked, for a synthetic fixture). */
  function reportLine(path: string, published_: string, sha256: string, tail: string) {
    const report = element('p', 'Exact report: ');
    if (options.synthetic) report.append(element('code', path));
    else {
      const link = element('a', path);
      link.href = `${options.base.replace(/\/$/, '')}/${published_}`;
      report.append(link);
    }
    report.append(document.createTextNode(' · sha256 '), element('code', sha256), document.createTextNode(tail));
    return report;
  }

  /** The pseudo-real-time test on the CDC NNDSS state series: pooled, then per series, each with its own measured skill or none. */
  function seriesBlocks(provenance: ForecastProvenance, backtest: SeriesBacktest, nameOf_: (id: string) => string): Element[] {
    const info: ProvenanceInfo = {
      label: `Series backtest · ${backtest.name}`, records: [], synthetic: options.synthetic,
      note: `Measured by koplik-epi on ${backtest.series}. Read exactly from the committed report ${backtest.report_path} (sha256 ${backtest.report_sha256}), run on the source snapshot sha256 ${backtest.input_sha256}. ${backtest.protocol} A measurement of the method on those series' past, not of any forecast shown above.`,
      citations: citationsOf(provenance),
    };
    const blocks: Element[] = [element('h3', `Test on ${backtest.name}: ${basisLabel(backtest.basis)}`)];
    blocks.push(element('p', `${backtest.protocol}`, 'forecast-series-basis'));
    blocks.push(element('p', `Series scored: ${backtest.series}. A series has a measured skill only if the test scored at least ${backtest.minimum_targets} forecasts of it from at least ${backtest.minimum_origin_weeks} origin weeks, a floor fixed before any score was computed; below it the series has insufficient data for a skill, whatever its scores would have been.`));
    const pooled = pooledWords(backtest);
    if (pooled) {
      const headline = element('p', undefined, 'forecast-series-headline');
      headline.append(provenanceNumber(pooled.headline, info));
      blocks.push(headline, element('p', pooled.scores));
      if (pooled.narrow) blocks.push(element('p', pooled.narrow, 'notice forecast-series-narrow'));
    } else {
      blocks.push(element('p', 'The pooled result is below the floor for a measured skill: insufficient data.', 'notice'));
    }
    blocks.push(element('p', seriesEvaluationScope(provenance), 'notice forecast-series-scope'));
    blocks.push(element('p', policyWords(provenance.publication_policy), 'forecast-policy'));

    const measured = backtest.by_series.filter((e) => e.measured);
    const scored = backtest.by_series.filter((e) => e.targets > 0).length;
    const details = element('details', undefined, 'table-scroll');
    details.append(element('summary', 'Series with a measured skill, pooled scores by horizon, and what this test does not show'));
    details.append(element('p', `${measured.length} of the ${backtest.by_series.length} state series have a measured skill; ${scored - measured.length} more had forecasts scored but too few for one; the other ${backtest.by_series.length - scored} never had a forecast in the test (the method's minimum-count rule never held for them), so they have no skill.`));
    if (measured.length) {
      const table = element('table', undefined, 'series-skill');
      table.append(element('caption', `Series with a measured skill, ${basisLabel(backtest.basis)}, exactly as measured`));
      const headRow = element('tr');
      for (const text of ['Series', 'Targets', 'Origin weeks', 'Mean CRPS (cases)', '50% interval coverage', '90% interval coverage', 'Carrying the latest count forward (cases)']) { const cell = element('th', text); cell.scope = 'col'; headRow.append(cell); }
      const head = element('thead'); head.append(headRow);
      const body = element('tbody');
      for (const e of [...measured].sort((a, b) => nameOf_(a.geography).localeCompare(nameOf_(b.geography), 'en'))) {
        const m = e.measured!;
        const tr = scoreRow(nameOf_(e.geography), m.targets, m.mean_crps, m.coverage_50, m.coverage_90, [String(e.origin_weeks)]);
        tr.append(element('td', m.mean_persistence_abs_error.toFixed(2)));
        body.append(tr);
      }
      table.append(head, body);
      details.append(table);
    }
    if (backtest.pooled) {
      const table = element('table', undefined, 'series-pooled');
      table.append(element('caption', `All series pooled, by horizon, exactly as measured`));
      const headRow = element('tr');
      for (const text of ['Weeks ahead', 'Targets', 'Mean CRPS (cases)', '50% interval coverage', '90% interval coverage']) { const cell = element('th', text); cell.scope = 'col'; headRow.append(cell); }
      const head = element('thead'); head.append(headRow);
      const body = element('tbody');
      const all = backtest.pooled.scores;
      for (const h of all.by_horizon) body.append(scoreRow(String(h.horizon), h.n, h.mean_crps, h.coverage_50, h.coverage_90));
      body.append(scoreRow('All', all.targets, all.mean_crps, all.coverage_50, all.coverage_90));
      table.append(head, body);
      details.append(table);
    }
    const limits = element('ul');
    for (const text of backtest.limitations) limits.append(element('li', text));
    details.append(limits, reportLine(backtest.report_path, seriesBacktestReportPath, backtest.report_sha256, ` · source snapshot sha256 ${backtest.input_sha256} · seed ${backtest.seed}`));
    blocks.push(details);
    return blocks;
  }

  /** The tests of the method, as evaluations on the series they were run on: what was scored, how it did, what each does not measure. */
  function renderEvaluation() {
    const { provenance } = published!;
    const skill = provenance.backtest;
    const backtest = provenance.series_backtest;
    const blocks: Element[] = [element('h2', 'How we evaluate forecasts')];
    if (options.synthetic) blocks.push(element('p', 'SYNTHETIC EVALUATION · Invented numbers for development only; nothing was backtested.', 'synthetic notice'));
    if (!skill && !backtest) {
      blocks.push(element('p', provenance.scope_note, 'notice'));
    } else {
      if (skill && backtest) blocks.push(element('p', 'We have tested this method twice, on two different things. The two tests are reported separately, each with its own scope and its own basis; neither says anything about the other.'));
      if (backtest) blocks.push(...seriesBlocks(provenance, backtest, nameOf));
      if (skill) blocks.push(...westTexasBlocks(provenance, skill, Boolean(backtest)));
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
    const withheld = provenance.series.filter((s) => s.status === 'withheld');
    const insufficient = provenance.series.filter((s) => s.status === 'insufficient_data');
    select.replaceChildren(
      ...(forecast.length ? [options_(forecast, 'Forecast available')] : []),
      ...(withheld.length ? [options_(withheld, 'Forecast withheld: not published')] : []),
      ...(insufficient.length ? [options_(insufficient, 'Insufficient data: no forecast')] : []),
    );
    const initial = [preferred, forecast[0]?.geography, withheld[0]?.geography, insufficient[0]?.geography].find((id) => id && provenance.series.some((s) => s.geography === id));
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
    const published_ = provenance.series.filter((s) => s.status === 'forecast').length;
    const withheld = provenance.series.filter((s) => s.status === 'withheld').length;
    const insufficient = provenance.series.length - published_ - withheld;
    status.textContent = published_
      ? `${published_} of ${provenance.series.length} series have a forecast that meets our publication rule${withheld ? `; ${withheld} more are withheld` : ''}; the rest have insufficient data. Forecast origin ${weekLabel(provenance.origin_week)}, ${provenance.horizon_weeks} weeks ahead.`
      : withheld
        ? `No forecast is published: the method made forecasts for ${withheld} of ${provenance.series.length} series and our publication rule withheld every one; the other ${insufficient} have insufficient data.`
        : `No series has enough data to forecast: all ${provenance.series.length} are insufficient data.`;
    const top = withheldNotice(provenance);
    withheldBox.replaceChildren(...(top ? [element('p', top, 'notice forecast-withheld-top')] : []));
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
