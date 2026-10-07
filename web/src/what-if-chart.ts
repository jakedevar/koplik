import type { EnsembleResult } from './generated/v2/EnsembleResult';
import { gaines } from './scenario';

export function ensembleChart(result: EnsembleResult): SVGSVGElement {
  const rows = result.daily.filter((row) => row.geography === gaines);
  const ns = 'http://www.w3.org/2000/svg';
  function node(tag: string, attributes: Record<string, string | number>, text?: string) {
    const element = document.createElementNS(ns, tag);
    for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, String(value));
    if (text) element.textContent = text;
    return element;
  }
  const svg = node('svg', { viewBox: '0 0 640 230', role: 'img', 'aria-label': 'Gaines County simulated cumulative infections: median and equal-tail 50% and 90% predictive bands. Exact values in the table below.' }) as SVGSVGElement;
  svg.append(node('title', {}, 'Simulated cumulative infections, including initially exposed and infectious individuals; excludes vaccine immunity.'));
  const max = Math.max(1, ...rows.map((r) => r.cumulative_infections.upper_90));
  const horizon = Math.max(1, ...rows.map((r) => r.day));
  const x = (day: number) => 60 + day / horizon * 540;
  const y = (value: number) => 175 - value / max * 145;
  for (const value of [0, max]) {
    svg.append(node('line', { x1: 60, x2: 600, y1: y(value), y2: y(value), class: 'gridline' }),
      node('text', { x: 5, y: y(value) + 4, class: 'axis-label' }, String(value)));
  }
  for (const day of [...new Set([0, Math.round(horizon / 2), horizon])]) {
    svg.append(node('text', { x: x(day), y: 197, 'text-anchor': 'middle', class: 'axis-label' }, `Day ${day}`));
  }
  svg.append(node('text', { x: 60, y: 15, class: 'axis-label' }, 'Cumulative infections (simulated)'));
  for (const level of [90, 50] as const) {
    const upper = rows.map((r) => `${x(r.day)},${y(r.cumulative_infections[`upper_${level}`])}`);
    const lower = [...rows].reverse().map((r) => `${x(r.day)},${y(r.cumulative_infections[`lower_${level}`])}`);
    svg.append(node('polygon', { points: [...upper, ...lower].join(' '), class: `ensemble-band-${level}` }));
  }
  svg.append(node('polyline', { points: rows.map((r) => `${x(r.day)},${y(r.cumulative_infections.median)}`).join(' '), class: 'ensemble-median' }));
  return svg;
}
