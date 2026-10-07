import type { WeeklyCaseCount } from './generated/v3/WeeklyCaseCount';
import type { RtEstimate } from './generated/RtEstimate';
import { caseDefinitionLabels, compareWeeks, type CaseDefinition } from './data';
import { bindProvenance } from './provenance';

const NS = 'http://www.w3.org/2000/svg';
let chartId = 0;
function svgElement(tag: string, attributes: Record<string, string | number>) {
  const node = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  return node;
}
function chart(title: string, maximum: number, unit: string): SVGSVGElement {
  const svg = svgElement('svg', { viewBox: '0 0 640 210', role: 'img', 'aria-label': title }) as SVGSVGElement;
  const titleNode = svgElement('title', {});
  titleNode.textContent = title;
  svg.append(titleNode);
  for (const [y, value] of [[170, 0], [25, maximum]]) {
    svg.append(svgElement('line', { x1: 40, x2: 610, y1: y, y2: y, class: 'gridline' }));
    const text = svgElement('text', { x: 5, y: y + 4, class: 'axis-label' });
    text.textContent = String(value);
    svg.append(text);
  }
  for (const week of [1, 13, 26, 39, 53]) {
    const text = svgElement('text', { x: x(week), y: 195, 'text-anchor': 'middle', class: 'axis-label' });
    text.textContent = `W${week}`;
    svg.append(text);
  }
  const label = svgElement('text', { x: 40, y: 14, class: 'axis-label' });
  label.textContent = unit;
  label.classList.add('series-legend');
  svg.append(label);
  return svg;
}
const x = (week: number) => 45 + (week - 1) * 10.7;
const y = (value: number, maximum: number) => 170 - value / maximum * 145;

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
      svgElement('line', { x1: x(row.week.week) - 3, x2: x(row.week.week) + 3, y1: 170, y2: 170, class: 'case-zero', 'data-week': row.week.week }) :
      svgElement('rect', { x: x(row.week.week) - 3, y: y(row.cases.count, max), width: 6,
        height: 170 - y(row.cases.count, max), class: 'case-bar', 'data-week': row.week.week });
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

/** Never connect over gaps or publish a provisional/insufficient row as an estimate. */
export function rtChart(rows: RtEstimate[], year: number, synthetic = false): SVGSVGElement {
  const selected = rows.filter((r) => r.week.year === year).sort(compareWeeks);
  const drawable = selected.filter((r) => !r.provisional && r.status === 'ok');
  const maximum = Math.max(2, ...drawable.map((r) => Math.max(r.upper!, r.mean!)));
  const svg = chart(`Effective reproduction number, MMWR ${year}. Mean and credible interval; provisional and insufficient data are withheld.`, maximum, 'R_t');
  bindProvenance(svg, { label: `Effective reproduction number chart · MMWR ${year}`, records: selected.flatMap((r) => r.provenance), synthetic,
    note: 'Derived R_t estimates: all source records attached to the chart rows. Axis ticks and R_t = 1 are display references, not source observations.' });
  svg.setAttribute('viewBox', '0 0 640 240');
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
      x: x(row.week.week) - 4, y: 25, width: 8, height: 145, fill: `url(#${hatchId})`, class: 'rt-insufficient-band',
    }));
    if (row.provisional) marker.append(svgElement('rect', {
      x: x(row.week.week) - 4, y: 25, width: 8, height: 145, class: 'rt-provisional-band',
    }));
    const text = svgElement('text', { x: x(row.week.week), y: 182, 'text-anchor': 'middle', class: 'axis-label' });
    text.textContent = `${row.status === 'insufficient_data' ? 'I' : ''}${row.provisional ? 'P' : ''}`;
    marker.append(text);
    svg.append(marker);
  }
  const legend = svgElement('g', { class: 'rt-status-legend', 'aria-label': 'I: Insufficient data; P: Provisional, estimate withheld; blank: no row. IP means both statuses.' });
  legend.append(svgElement('rect', { x: 40, y: 217, width: 10, height: 12, fill: `url(#${hatchId})` }),
    svgElement('rect', { x: 215, y: 217, width: 10, height: 12, class: 'rt-provisional-band' }));
  for (const [px, label] of [[56, 'I: Insufficient data'], [231, 'P: Provisional (withheld)'], [450, 'Blank: no row']] as const) {
    const text = svgElement('text', { x: px, y: 227, class: 'axis-label' });
    text.textContent = label;
    legend.append(text);
  }
  svg.append(legend);
  svg.append(svgElement('line', { x1: 40, x2: 610, y1: y(1, maximum), y2: y(1, maximum), class: 'rt-reference' }));
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
    const points = [...group.map((r) => `${x(r.week.week)},${y(r.upper!, maximum)}`),
      ...[...group].reverse().map((r) => `${x(r.week.week)},${y(r.lower!, maximum)}`)].join(' ');
    const level = group[0].interval_level;
    const ribbon = svgElement('polygon', { points, class: 'rt-ribbon', 'data-interval-level': level });
    const title = svgElement('title', {});
    title.textContent = `${level * 100}% credible interval · weeks ${group[0].week.week}–${group.at(-1)!.week.week}`;
    const groupInfo = { label: title.textContent, records: group.flatMap((r) => r.provenance), synthetic };
    bindProvenance(ribbon as SVGElement, groupInfo);
    ribbon.append(title);
    svg.append(ribbon);
    const mean = svgElement('polyline', { points: group.map((r) => `${x(r.week.week)},${y(r.mean!, maximum)}`).join(' '), class: 'rt-mean', 'data-interval-level': level });
    bindProvenance(mean as SVGElement, groupInfo); svg.append(mean);
    for (const row of group) {
      const interval = svgElement('line', { x1: x(row.week.week), x2: x(row.week.week), y1: y(row.lower!, maximum), y2: y(row.upper!, maximum), class: 'rt-interval', 'data-week': row.week.week, 'data-interval-level': level });
      const rowInfo = { label: `Week ${row.week.week}: ${rtLabel(row)}`, records: row.provenance, synthetic };
      bindProvenance(interval as SVGElement, rowInfo); svg.append(interval);
      const point = svgElement('circle', { cx: x(row.week.week), cy: y(row.mean!, maximum), r: 3, class: 'rt-point', 'data-week': row.week.week, 'data-interval-level': level });
      const title = svgElement('title', {});
      title.textContent = `Week ${row.week.week}: ${rtLabel(row)}`;
      point.append(title);
      bindProvenance(point as SVGElement, rowInfo);
      svg.append(point);
    }
  }
  return svg;
}
