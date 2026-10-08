import type { WeeklyCaseCount } from './generated/v3/WeeklyCaseCount';
import type { RtEstimate } from './generated/RtEstimate';
import { caseDefinitionLabels, compareWeeks, type CaseDefinition } from './data';
import { bindProvenance, provenanceNumber } from './provenance';

import { drawAxes, drawLegend, formatTick, plot, svgNode as svgElement } from './chart-style';

let chartId = 0;
const chartHeight = 200;
function chart(title: string, maximum: number, unit: string): SVGSVGElement {
  const svg = svgElement('svg', { viewBox: `0 0 ${plot.width} ${chartHeight}`, role: 'img', 'aria-label': title, class: 'chart' }) as SVGSVGElement;
  svg.append(svgElement('title', {}, title));
  drawAxes(svg, { maximum, unit, ticks: [1, 13, 26, 39, 53].map((week) => ({ x: x(week), label: `W${week}` })) });
  return svg;
}
// Weeks 1 to 53 across the plot, inset by half a bar so the first and last bars stay inside it.
const x = (week: number) => plot.left + 6 + (week - 1) * ((plot.right - plot.left - 12) / 52);
const y = (value: number, maximum: number) => plot.base - value / maximum * (plot.base - plot.top);

/** One plotted series: every row shares one case definition. Definitions are never mixed or summed. */
export interface CaseSeries {
  definition: CaseDefinition;
  /** The definition in words, e.g. "confirmed cases". */
  label: string;
  rows: WeeklyCaseCount[];
}

/** The chart model: one series per case definition among the year's rows, in a fixed order. */
export function caseSeries(rows: WeeklyCaseCount[], year: number): CaseSeries[] {
  const selected = rows.filter((r) => r.week.year === year).sort(compareWeeks);
  return (Object.keys(caseDefinitionLabels) as CaseDefinition[])
    .map((definition) => ({ definition, label: caseDefinitionLabels[definition], rows: selected.filter((r) => r.case_definition === definition) }))
    .filter((series) => series.rows.length > 0);
}

function seriesChart(series: CaseSeries, year: number, synthetic: boolean, separate: boolean): SVGSVGElement {
  const selected = series.rows;
  const max = Math.max(1, ...selected.map((r) => r.cases.status === 'reported' ? r.cases.count : 0));
  const words = series.label;
  const apart = separate ? ' Charted separately from other case definitions; they are never added together.' : '';
  const svg = chart(`Weekly ${words}, MMWR ${year}.${apart} Baseline ticks mean reported zero; gaps mean no data; exact reports are in the table.`, max, `New ${words}`);
  svg.dataset.caseDefinition = series.definition;
  bindProvenance(svg, { label: `Weekly ${words} chart · MMWR ${year}`, records: selected.flatMap((r) => r.provenance), synthetic,
    note: 'Sources attached to the plotted weekly reports. Axis ticks are display guides.' });
  for (const row of selected) {
    if (row.cases.status !== 'reported') continue;
    const bar = row.cases.count === 0 ?
      svgElement('line', { x1: x(row.week.week) - 3, x2: x(row.week.week) + 3, y1: plot.base, y2: plot.base, class: 'case-zero', 'data-week': row.week.week }) :
      svgElement('rect', { x: x(row.week.week) - 3, y: y(row.cases.count, max), width: 6,
        height: plot.base - y(row.cases.count, max), class: 'case-bar', 'data-week': row.week.week });
    const title = svgElement('title', {});
    title.textContent = `Week ${row.week.week}: ${row.cases.count} ${caseDefinitionLabels[row.case_definition]}`;
    bindProvenance(bar as SVGElement, { label: title.textContent, records: row.provenance, synthetic });
    bar.append(title);
    svg.append(bar);
  }
  return svg;
}

/** One chart per case definition, each with its own axis and a legend naming the definition; never one mixed series. */
export function caseCharts(rows: WeeklyCaseCount[], year: number, synthetic = false): SVGSVGElement[] {
  const series = caseSeries(rows, year);
  return series.map((one) => seriesChart(one, year, synthetic, series.length > 1));
}

export function rtLabel(row: RtEstimate): string {
  const status = row.status === 'insufficient_data' ? 'Insufficient data' : 'Estimate available';
  if (row.provisional) return `${status} · Provisional — estimate withheld from chart`;
  if (row.status === 'insufficient_data') return status;
  return `Mean ${row.mean}; ${row.interval_level * 100}% interval ${row.lower}–${row.upper}`;
}

export function rtAxisMaximum(rows: RtEstimate[], year: number, fullRange = false): number {
  const published = rows.filter((row) => row.week.year === year && !row.provisional && row.status === 'ok');
  if (fullRange) return Math.max(3, ...published.map((row) => Math.max(row.upper!, row.mean!)));
  // Display choice, not a model parameter: use the lower-order 95th percentile
  // of published upper bounds (at least 3). Rounding the rank down also keeps
  // a single wide interval from flattening ordinary weeks in a short series.
  const bounds = published.map((row) => row.upper!).sort((a, b) => a - b);
  return Math.max(3, bounds[Math.floor((bounds.length - 1) * 0.95)] ?? 0);
}

function offScaleLabel(row: RtEstimate, maximum: number): string | undefined {
  const values = [['lower bound', row.lower!], ['upper bound', row.upper!], ['mean', row.mean!]] as const;
  const exceeded = values.filter(([, value]) => value > maximum);
  return exceeded.length ? `Week ${row.week.week} · ${row.interval_level * 100}%: ${exceeded.map(([label, value]) => `${label} ${formatTick(value)}, off scale`).join('; ')}` : undefined;
}

function offScaleWeekLabel(rows: RtEstimate[], maximum: number): string | undefined {
  const labels = rows.map((row) => offScaleLabel(row, maximum)).filter((label): label is string => label !== undefined);
  if (!labels.length) return undefined;
  const week = rows[0].week.week;
  return `Week ${week} · ${labels.map((label) => label.slice(label.indexOf(' · ') + 3)).join('; ')}`;
}

/** Never connect over gaps or publish a provisional/insufficient row as an estimate. */
export function rtChart(rows: RtEstimate[], year: number, synthetic = false, fullRange = false): SVGSVGElement {
  const selected = rows.filter((r) => r.week.year === year).sort(compareWeeks);
  const drawable = selected.filter((r) => !r.provisional && r.status === 'ok');
  const maximum = rtAxisMaximum(rows, year, fullRange);
  const position = (value: number) => y(Math.min(value, maximum), maximum);
  const svg = chart(`Effective reproduction number, MMWR ${year}. ${fullRange ? 'Full range' : 'Readable range; off-scale values marked at the upper edge'}. Mean and credible interval; provisional and insufficient data are withheld.`, maximum, 'R_t');
  svg.dataset.rtRange = fullRange ? 'full' : 'readable';
  svg.dataset.axisMaximum = String(maximum);
  bindProvenance(svg, { label: `Effective reproduction number chart · MMWR ${year}`, records: selected.flatMap((r) => r.provenance), synthetic,
    note: 'Derived R_t estimates: all source records attached to the chart rows. Axis ticks and R_t = 1 are display references, not source observations.' });
  const hatchId = `rt-insufficient-hatch-${++chartId}`;
  const withheld = selected.filter((r) => r.provisional || r.status === 'insufficient_data');
  const description = svgElement('desc', { id: `${hatchId}-description` });
  description.textContent = `Grey hatch / I: insufficient data. Dashed outline / P: provisional, estimate withheld. IP: both. Blank: no row. Withheld weeks: ${withheld.length ? withheld.map((row) => `Week ${row.week.week}: ${rtLabel(row)}`).join('; ') : 'none'}.`;
  svg.setAttribute('aria-describedby', `${hatchId}-description`);
  svg.append(description);
  const defs = svgElement('defs', {});
  const hatch = svgElement('pattern', { id: hatchId, width: 4, height: 4, patternUnits: 'userSpaceOnUse' });
  hatch.append(svgElement('rect', { width: 4, height: 4, fill: '#edf0ee' }),
    svgElement('path', { d: 'M0 4L4 0', stroke: '#68786f', 'stroke-width': 1 }));
  defs.append(hatch);
  svg.append(defs);
  for (const row of withheld) {
    const label = `Week ${row.week.week}: ${rtLabel(row)}`;
    const marker = svgElement('g', { class: 'rt-withheld', 'data-week': row.week.week,
      'data-status': row.status, 'data-provisional': String(row.provisional), role: 'img', 'aria-label': label });
    const title = svgElement('title', {});
    title.textContent = label;
    marker.append(title);
    bindProvenance(marker as SVGElement, { label, records: row.provenance, synthetic });
    if (row.status === 'insufficient_data') marker.append(svgElement('rect', {
      x: x(row.week.week) - 4, y: plot.top, width: 8, height: plot.base - plot.top, fill: `url(#${hatchId})`, class: 'rt-insufficient-band',
    }));
    if (row.provisional) marker.append(svgElement('rect', {
      x: x(row.week.week) - 4, y: plot.top, width: 8, height: plot.base - plot.top, class: 'rt-provisional-band',
    }));
    const text = svgElement('text', { x: x(row.week.week), y: plot.base + 36, 'text-anchor': 'middle', class: 'axis-label' });
    text.textContent = `${row.status === 'insufficient_data' ? 'I' : ''}${row.provisional ? 'P' : ''}`;
    marker.append(text);
    svg.append(marker);
  }
  const legendHeight = drawLegend(svg, [
    { kind: 'swatch', className: 'key-insufficient', fill: `url(#${hatchId})`, label: 'I: Insufficient data' },
    { kind: 'swatch', className: 'key-provisional-outline', label: 'P: Provisional (withheld)' },
    { kind: 'swatch', className: 'key-blank', label: 'Blank: no row' },
  ], plot.base + 64, 'rt-status-legend', 'I: Insufficient data; P: Provisional, estimate withheld; blank: no row. IP means both statuses.');
  svg.setAttribute('viewBox', `0 0 ${plot.width} ${plot.base + 64 + legendHeight - 12}`);
  // Paired levels are interleaved by week. Build each level's sequence before
  // splitting on unavailable weeks; paint wider credible levels first.
  const groups: RtEstimate[][] = [];
  const levels = [...new Set(drawable.map((row) => row.interval_level))].sort((a, b) => b - a);
  for (const level of levels) {
    const sequence: RtEstimate[][] = [];
    for (const row of drawable.filter((row) => row.interval_level === level)) {
      const previous = sequence.at(-1)?.at(-1);
      if (!previous || previous.week.week + 1 !== row.week.week) sequence.push([]);
      sequence.at(-1)!.push(row);
    }
    groups.push(...sequence);
  }
  for (const group of groups) {
    const points = [...group.map((r) => `${x(r.week.week)},${position(r.upper!)}`),
      ...[...group].reverse().map((r) => `${x(r.week.week)},${position(r.lower!)}`)].join(' ');
    const level = group[0].interval_level;
    const ribbon = svgElement('polygon', { points, class: 'rt-ribbon', 'data-interval-level': level });
    const title = svgElement('title', {});
    title.textContent = `${level * 100}% credible interval · weeks ${group[0].week.week}–${group.at(-1)!.week.week}`;
    const groupInfo = { label: title.textContent, records: group.flatMap((r) => r.provenance), synthetic };
    bindProvenance(ribbon as SVGElement, groupInfo);
    ribbon.append(title);
    svg.append(ribbon);
    const mean = svgElement('polyline', { points: group.map((r) => `${x(r.week.week)},${position(r.mean!)}`).join(' '), class: 'rt-mean', 'data-interval-level': level });
    bindProvenance(mean as SVGElement, groupInfo); svg.append(mean);
    for (const row of group) {
      const interval = svgElement('line', { x1: x(row.week.week), x2: x(row.week.week), y1: position(row.lower!), y2: position(row.upper!), class: 'rt-interval', 'data-week': row.week.week, 'data-interval-level': level });
      const rowInfo = { label: `Week ${row.week.week}: ${rtLabel(row)}`, records: row.provenance, synthetic };
      bindProvenance(interval as SVGElement, rowInfo); svg.append(interval);
      const point = svgElement('circle', { cx: x(row.week.week), cy: position(row.mean!), r: 3, class: 'rt-point', 'data-week': row.week.week, 'data-interval-level': level });
      const title = svgElement('title', {});
      title.textContent = `Week ${row.week.week}: ${rtLabel(row)}`;
      point.append(title);
      bindProvenance(point as SVGElement, rowInfo);
      svg.append(point);
    }
  }
  // Paint the reference and edge markers last so ribbons cannot obscure them.
  svg.append(svgElement('line', { x1: plot.left, x2: plot.right, y1: y(1, maximum), y2: y(1, maximum), class: 'rt-reference' }),
    svgElement('text', { x: plot.left + 4, y: y(1, maximum) - 5, class: 'axis-label rt-reference-label' }, 'R_t = 1'));
  const offScaleByWeek = new Map<number, RtEstimate[]>();
  for (const row of drawable) {
    if (!offScaleLabel(row, maximum)) continue;
    const week = offScaleByWeek.get(row.week.week) ?? [];
    week.push(row);
    offScaleByWeek.set(row.week.week, week);
  }
  for (const [week, rowsForWeek] of offScaleByWeek) {
    const label = offScaleWeekLabel(rowsForWeek, maximum)!;
    if (!label) continue;
    const marker = svgElement('path', { d: `M${x(week) - 5} ${plot.top + 7}L${x(week)} ${plot.top}L${x(week) + 5} ${plot.top + 7}`,
      class: 'rt-off-scale', 'data-week': week });
    marker.append(svgElement('title', {}, label));
    bindProvenance(marker, { label: `${label} · ${rowsForWeek.map(rtLabel).join('; ')}`, records: rowsForWeek.flatMap((row) => row.provenance), synthetic });
    marker.setAttribute('aria-label', `${label}. Open provenance.`);
    svg.append(marker);
  }
  return svg;
}

/** The display range affects coordinates only; every exact value stays in its drawer and report table. */
export function rtChartView(rows: RtEstimate[], year: number, synthetic = false): HTMLDivElement {
  const view = document.createElement('div');
  view.className = 'rt-chart-view';
  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.textContent = 'Show full range';
  const note = document.createElement('p');
  note.className = 'chart-note rt-range-note';
  note.setAttribute('aria-live', 'polite');
  const marks = document.createElement('div');
  const labels = document.createElement('div');
  labels.className = 'rt-off-scale-labels';
  let fullRange = false;
  function render() {
    const maximum = rtAxisMaximum(rows, year, fullRange);
    const readableMaximum = rtAxisMaximum(rows, year);
    const offScaleRows = rows.filter((row) => row.week.year === year && !row.provisional && row.status === 'ok' && offScaleLabel(row, readableMaximum));
    const hasOffScale = offScaleRows.length > 0;
    toggle.hidden = !hasOffScale;
    toggle.setAttribute('aria-pressed', String(fullRange));
    note.textContent = fullRange ? `Full display range: 0–${formatTick(maximum)}. All published means and bounds are shown; select a point for exact values and sources.` :
      hasOffScale ? `Readable display range: 0–${formatTick(maximum)}. ▲ marks off-scale values at the upper edge; select a marker or its label for exact values and sources.` :
        `Readable display range: 0–${formatTick(maximum)}.`;
    marks.replaceChildren(rtChart(rows, year, synthetic, fullRange));
    labels.replaceChildren();
    const grouped = new Map<number, RtEstimate[]>();
    for (const row of offScaleRows.sort(compareWeeks)) {
      const week = grouped.get(row.week.week) ?? [];
      week.push(row);
      grouped.set(row.week.week, week);
    }
    for (const [week, weekRows] of grouped) {
      const label = offScaleWeekLabel(weekRows, maximum);
      if (!label) continue;
      const button = provenanceNumber(`▲ ${label}`, {
        label: `${label} · ${weekRows.map(rtLabel).join('; ')}`, records: weekRows.flatMap((row) => row.provenance), synthetic,
      });
      button.setAttribute('aria-label', `▲ ${label}. Open provenance.`);
      labels.append(button);
    }
  }
  toggle.addEventListener('click', () => { fullRange = !fullRange; render(); });
  view.append(toggle, note, marks, labels);
  render();
  return view;
}
