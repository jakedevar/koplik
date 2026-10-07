/**
 * One chart style for every SVG chart on the site: the weekly case charts, R_t, the Texas cumulative reports, the forecast
 * and the what-if ensemble. They share the plot area, the gridlines, the tick labels (the `.axis-label` class, whose size
 * and colour live in style.css), the number format of an axis, and the legend row. The marks that say "this is not a
 * measurement" (missing, insufficient, provisional) are shared CSS classes, so the same meaning looks the same everywhere.
 */
const NS = 'http://www.w3.org/2000/svg';

export function svgNode(tag: string, attributes: Record<string, string | number> = {}, text?: string): SVGElement {
  const node = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  if (text !== undefined) node.textContent = text;
  return node;
}

/** Plot area in viewBox units: every chart is 640 wide, with the value axis labels to the left of `left`. */
export const plot = { width: 640, left: 50, right: 610, top: 28, base: 170 } as const;

/** The tick-label row sits this far below the baseline; the series label sits at this height above the plot. */
export const tickRow = plot.base + 18;
export const seriesLabelY = 14;

/**
 * An axis tick as text. Whole numbers keep every digit (with thousands separators); a fraction is shortened to a readable
 * length, because a tick is a guide to the scale, not a reported value (the exact values are in the tables and drawers).
 */
export function formatTick(value: number): string {
  if (Number.isInteger(value)) return value.toLocaleString('en-US');
  return Math.abs(value) >= 100 ? Math.round(value).toLocaleString('en-US') : (Math.round(value * 10) / 10).toString();
}

export interface AxisOptions {
  /** Plot area; defaults to the shared one. */
  left?: number;
  right?: number;
  top?: number;
  base?: number;
  maximum: number;
  /** The value axis label, e.g. "New confirmed cases"; carries the case definition where there is one. */
  unit: string;
  ticks: { x: number; label: string }[];
  format?: (value: number) => string;
}

/** Zero and maximum gridlines with right-aligned value labels, the tick labels, and the series label above the plot. */
export function drawAxes(svg: SVGElement, options: AxisOptions): void {
  const left = options.left ?? plot.left;
  const right = options.right ?? plot.right;
  const top = options.top ?? plot.top;
  const base = options.base ?? plot.base;
  const format = options.format ?? formatTick;
  for (const [y, value] of [[base, 0], [top, options.maximum]] as const) {
    svg.append(svgNode('line', { x1: left, x2: right, y1: y, y2: y, class: 'gridline' }),
      svgNode('text', { x: left - 6, y: y + 4, 'text-anchor': 'end', class: 'axis-label' }, format(value)));
  }
  for (const tick of options.ticks) svg.append(svgNode('text', { x: tick.x, y: base + 18, 'text-anchor': 'middle', class: 'axis-label' }, tick.label));
  svg.append(svgNode('text', { x: left, y: seriesLabelY, class: 'axis-label series-legend' }, options.unit));
}

export interface LegendItem {
  /** `swatch` is a filled box (class decides the fill and stroke), `line` a short stroke, `dot` a point, `cross` a ×. */
  kind: 'swatch' | 'line' | 'dot' | 'cross';
  className: string;
  label: string;
  /** For a swatch that needs a pattern fill. */
  fill?: string;
}

// Legend layout assumes the largest tick font (see --chart-font in style.css) and an average glyph of 0.58 em.
const legendFont = 15;
const glyph = legendFont * 0.58;
/** Lays the legend out left to right, wrapping to a new row when the next item would pass the plot edge. Returns its height. */
export function drawLegend(svg: SVGElement, items: LegendItem[], top: number, className: string, label?: string): number {
  const group = svgNode('g', { class: `chart-legend ${className}`, ...(label ? { 'aria-label': label } : {}) });
  const rowHeight = 22;
  let x = plot.left;
  let row = 0;
  for (const item of items) {
    const width = 20 + item.label.length * glyph + 16;
    if (x > plot.left && x + width > plot.right + 20) { x = plot.left; row++; }
    const y = top + row * rowHeight;
    if (item.kind === 'swatch') group.append(svgNode('rect', { x, y: y - 11, width: 12, height: 12, class: item.className, ...(item.fill ? { fill: item.fill } : {}) }));
    // A dot is a fully rounded box and a cross two strokes, so a legend never adds a circle or path to a chart's marks.
    else if (item.kind === 'dot') group.append(svgNode('rect', { x, y: y - 11, width: 12, height: 12, rx: 6, class: item.className }));
    else if (item.kind === 'cross') group.append(svgNode('line', { x1: x + 2, x2: x + 10, y1: y - 9, y2: y - 1, class: item.className }), svgNode('line', { x1: x + 2, x2: x + 10, y1: y - 1, y2: y - 9, class: item.className }));
    else group.append(svgNode('line', { x1: x, x2: x + 14, y1: y - 5, y2: y - 5, class: item.className }));
    group.append(svgNode('text', { x: x + 20, y, class: 'axis-label' }, item.label));
    x += width;
  }
  svg.append(group);
  return (row + 1) * rowHeight;
}
