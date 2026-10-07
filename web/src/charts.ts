import type { WeeklyCaseCount } from './generated/WeeklyCaseCount';
import type { RtEstimate } from './generated/RtEstimate';
import { compareWeeks } from './data';

const NS = 'http://www.w3.org/2000/svg';
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
  svg.append(label);
  return svg;
}
const x = (week: number) => 45 + (week - 1) * 10.7;
const y = (value: number, maximum: number) => 170 - value / maximum * 145;

export function caseChart(rows: WeeklyCaseCount[], year: number): SVGSVGElement {
  const selected = rows.filter((r) => r.week.year === year).sort(compareWeeks);
  const max = Math.max(1, ...selected.map((r) => r.confirmed.status === 'reported' ? r.confirmed.count : 0));
  const svg = chart(`Weekly confirmed cases, MMWR ${year}. Gaps mean no data; exact reports are in the table.`, max, 'Confirmed cases');
  for (const row of selected) {
    if (row.confirmed.status !== 'reported') continue;
    const bar = svgElement('rect', { x: x(row.week.week) - 3, y: y(row.confirmed.count, max), width: 6,
      height: 170 - y(row.confirmed.count, max), class: 'case-bar', 'data-week': row.week.week });
    const title = svgElement('title', {});
    title.textContent = `Week ${row.week.week}: ${row.confirmed.count} confirmed cases`;
    bar.append(title);
    svg.append(bar);
  }
  return svg;
}

export function rtLabel(row: RtEstimate): string {
  const status = row.status === 'insufficient_data' ? 'Insufficient data' : 'Estimate available';
  if (row.provisional) return `${status} · Provisional — estimate withheld from chart`;
  if (row.status === 'insufficient_data') return status;
  return `Mean ${row.mean}; ${row.interval_level * 100}% interval ${row.lower}–${row.upper}`;
}

/** Never connect over gaps or publish a provisional/insufficient row as an estimate. */
export function rtChart(rows: RtEstimate[], year: number): SVGSVGElement {
  const selected = rows.filter((r) => r.week.year === year).sort(compareWeeks);
  const drawable = selected.filter((r) => !r.provisional && r.status === 'ok');
  const maximum = Math.max(2, ...drawable.map((r) => Math.max(r.upper!, r.mean!)));
  const svg = chart(`Effective reproduction number, MMWR ${year}. Mean and credible interval; provisional and insufficient data are withheld.`, maximum, 'R_t');
  svg.append(svgElement('line', { x1: 40, x2: 610, y1: y(1, maximum), y2: y(1, maximum), class: 'rt-reference' }));
  const groups: RtEstimate[][] = [];
  for (const row of drawable) {
    const previous = groups.at(-1)?.at(-1);
    if (!previous || previous.week.week + 1 !== row.week.week || previous.interval_level !== row.interval_level) groups.push([]);
    groups.at(-1)!.push(row);
  }
  for (const group of groups) {
    const points = [...group.map((r) => `${x(r.week.week)},${y(r.upper!, maximum)}`),
      ...[...group].reverse().map((r) => `${x(r.week.week)},${y(r.lower!, maximum)}`)].join(' ');
    svg.append(svgElement('polygon', { points, class: 'rt-ribbon' }));
    svg.append(svgElement('polyline', { points: group.map((r) => `${x(r.week.week)},${y(r.mean!, maximum)}`).join(' '), class: 'rt-mean' }));
    for (const row of group) {
      svg.append(svgElement('line', { x1: x(row.week.week), x2: x(row.week.week), y1: y(row.lower!, maximum), y2: y(row.upper!, maximum), class: 'rt-interval', 'data-week': row.week.week }));
      const point = svgElement('circle', { cx: x(row.week.week), cy: y(row.mean!, maximum), r: 3, class: 'rt-point', 'data-week': row.week.week });
      const title = svgElement('title', {});
      title.textContent = `Week ${row.week.week}: ${rtLabel(row)}`;
      point.append(title);
      svg.append(point);
    }
  }
  return svg;
}
