import type { EnsembleResult } from './generated/v2/EnsembleResult';
import { gaines } from './scenario';
import { drawAxes, drawLegend, plot, svgNode as node } from './chart-style';

export function ensembleChart(result: EnsembleResult): SVGSVGElement {
  const rows = result.daily.filter((row) => row.geography === gaines);
  const svg = node('svg', { viewBox: `0 0 ${plot.width} 250`, class: 'chart', role: 'img', 'aria-label': 'Gaines County simulated cumulative infections: median and equal-tail 50% and 90% predictive bands. Exact values in the table below.' }) as SVGSVGElement;
  svg.append(node('title', {}, 'Simulated cumulative infections, including initially exposed and infectious individuals; excludes vaccine immunity.'));
  const max = Math.max(1, ...rows.map((r) => r.cumulative_infections.upper_90));
  const horizon = Math.max(1, ...rows.map((r) => r.day));
  const x = (day: number) => plot.left + day / horizon * (plot.right - plot.left);
  const y = (value: number) => plot.base - value / max * (plot.base - plot.top);
  drawAxes(svg, { maximum: max, unit: 'Cumulative infections (simulated)',
    ticks: [...new Set([0, Math.round(horizon / 2), horizon])].map((day) => ({ x: x(day), label: `Day ${day}` })) });
  for (const level of [90, 50] as const) {
    const upper = rows.map((r) => `${x(r.day)},${y(r.cumulative_infections[`upper_${level}`])}`);
    const lower = [...rows].reverse().map((r) => `${x(r.day)},${y(r.cumulative_infections[`lower_${level}`])}`);
    svg.append(node('polygon', { points: [...upper, ...lower].join(' '), class: `ensemble-band-${level}` }));
  }
  svg.append(node('polyline', { points: rows.map((r) => `${x(r.day)},${y(r.cumulative_infections.median)}`).join(' '), class: 'ensemble-median' }));
  drawLegend(svg, [
    { kind: 'line', className: 'key-median', label: 'Median' },
    { kind: 'swatch', className: 'key-band-50', label: '50% band' },
    { kind: 'swatch', className: 'key-band-90', label: '90% band' },
  ], plot.base + 46, 'ensemble-legend');
  return svg;
}
