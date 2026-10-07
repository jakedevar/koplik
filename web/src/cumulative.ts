import type { CumulativeCaseReport, CumulativeMissingReason } from './generated/v7/CumulativeCaseReport';
import { bindProvenance, provenanceNumber, type ProvenanceInfo } from './provenance';

/**
 * The Texas DSHS cumulative-by-report-date series (contracts v7, #1439): the cumulative count each DSHS
 * report printed for a county, one point per report. It is a different thing from the weekly series:
 * weekly counts are never derived from it, no value is interpolated between reports, and no value is
 * carried forward to a date DSHS did not report.
 */
export const cumulativeHeading = 'Cumulative confirmed cases as reported by Texas DSHS';

/** Why a report gives no usable count for a county, in plain words (never zero, never estimated). */
export const cumulativeMissingWords: Record<CumulativeMissingReason, string> = {
  no_county_table: 'DSHS published no county table in this report',
  not_labelled_confirmed: 'this report’s county table does not say its counts are confirmed cases',
  not_listed: 'the county is not listed in this report, and the report does not show that it lists every county, so its absence is not read as zero',
  ambiguous: 'the county’s count in this report could not be read as one number',
};

export const cumulativeExplanation = 'Each point is one DSHS report: the cumulative number of confirmed cases it printed for this county, on its report date. DSHS published these reports irregularly, so the points are unevenly spaced, they are not joined by a line, and no value is estimated between reports. A later report can print a lower count when DSHS removed or reclassified a case; each point is what that report printed. Weekly counts are not derived from this series.';
export const cumulativeMissingExplanation = '× marks a report date where DSHS gave no usable count for this county. The count is unknown, not zero, and none is estimated; the reason is listed under the chart.';

const NS = 'http://www.w3.org/2000/svg';
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const left = 50;
const right = 610;
const top = 28;
const base = 170;
const missingY = 206;

function svgElement(tag: string, attributes: Record<string, string | number>) {
  const node = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  return node;
}

/** Whole days since 1970-01-01 for a `YYYY-MM-DD` date (UTC, so no time zone can move a date). */
function day(date: string): number {
  return Math.round(Date.parse(`${date}T00:00:00Z`) / 86_400_000);
}

/** One geography's rows in report-date order. */
export function cumulativeSeries(rows: readonly CumulativeCaseReport[], geography: string): CumulativeCaseReport[] {
  return rows.filter((r) => r.geography === geography).sort((a, b) => a.report_date.localeCompare(b.report_date));
}

/** What a row says, in words: the count, or "No data" and its reason. */
export function cumulativeText(row: CumulativeCaseReport): string {
  return row.cases.status === 'reported' ? `${row.cases.count.toLocaleString('en-US')} cumulative confirmed cases` :
    `No data: ${cumulativeMissingWords[row.cases.reason]}`;
}

function rowInfo(row: CumulativeCaseReport, name: string, synthetic: boolean): ProvenanceInfo {
  const where = `${name} · DSHS report of ${row.report_date}`;
  return row.cases.status === 'reported' ?
    { label: `${where} · ${row.cases.count} cumulative confirmed cases`, records: row.provenance, synthetic,
      note: 'The cumulative count of confirmed cases as printed in this DSHS report for this county on its report date. It is not a weekly count, and nothing is estimated or carried between reports. The records are the report snapshot and the Census county file that mapped the county name to its FIPS code.' } :
    { label: `${where} · no data`, records: row.provenance, synthetic,
      note: `No count is shown for this report: ${cumulativeMissingWords[row.cases.reason]}. The value is unknown, not zero, and none is estimated. The records are the report that was examined and the Census county file that keys the county.` };
}

/**
 * The reports with no usable count, as runs of consecutive reports that share a reason, so a long stretch of the same
 * reason is one line. Each run names its dates, how many reports it covers and the reason in words.
 */
export function cumulativeGapRuns(rows: readonly CumulativeCaseReport[]): { reason: CumulativeMissingReason; first: string; last: string; reports: number; text: string }[] {
  const runs: { reason: CumulativeMissingReason; first: string; last: string; reports: number; text: string }[] = [];
  let open: (typeof runs)[number] | undefined;
  for (const row of rows) {
    if (row.cases.status === 'reported') { open = undefined; continue; }
    if (open && open.reason === row.cases.reason) { open.last = row.report_date; open.reports++; }
    else { open = { reason: row.cases.reason, first: row.report_date, last: row.report_date, reports: 1, text: '' }; runs.push(open); }
  }
  for (const run of runs) {
    run.text = `${run.reports === 1 ? run.first : `${run.first} to ${run.last} (${run.reports} reports)`}: ${cumulativeMissingWords[run.reason]}`;
  }
  return runs;
}

/** Month starts across the span, thinned to at most seven labelled ticks. */
function monthTicks(first: number, last: number): { at: number; label: string }[] {
  const start = new Date(first * 86_400_000);
  const months: { at: number; label: string }[] = [];
  for (let y = start.getUTCFullYear(), m = start.getUTCMonth(); ; m++) {
    if (m === 12) { y++; m = 0; }
    const at = Math.round(Date.UTC(y, m, 1) / 86_400_000);
    if (at > last) break;
    if (at >= first) months.push({ at, label: `${MONTHS[m]} ${y}` });
  }
  const step = Math.max(1, Math.ceil(months.length / 7));
  return months.filter((_, i) => i % step === 0);
}

/**
 * Points only: one circle per report that printed a count, one × per report that did not. There is no line,
 * step or fill between reports, because nothing is known between them. Every mark opens its provenance.
 */
export function cumulativeChart(rows: readonly CumulativeCaseReport[], name: string, synthetic = false): SVGSVGElement {
  const reported = rows.flatMap((r) => r.cases.status === 'reported' ? [r.cases.count] : []);
  const max = Math.max(1, ...reported);
  const days = rows.map((r) => day(r.report_date));
  const first = days.length ? Math.min(...days) : 0;
  const last = days.length ? Math.max(...days) : 0;
  const x = (date: string) => last === first ? (left + right) / 2 : left + (day(date) - first) / (last - first) * (right - left);
  const y = (value: number) => base - value / max * (base - top);
  const title = `${cumulativeHeading} · ${name}. One point per report date; points are not joined, and weekly counts are not derived. × marks a report with no usable count.`;
  const svg = svgElement('svg', { viewBox: '0 0 640 222', role: 'group', 'aria-label': title, class: 'cumulative-chart' }) as SVGSVGElement;
  svg.dataset.geography = rows[0]?.geography ?? '';
  const titleNode = svgElement('title', {});
  titleNode.textContent = title;
  svg.append(titleNode);
  for (const [lineY, value] of [[base, 0], [top, max]] as const) {
    svg.append(svgElement('line', { x1: left, x2: right, y1: lineY, y2: lineY, class: 'gridline' }));
    const text = svgElement('text', { x: 5, y: lineY + 4, class: 'axis-label' });
    text.textContent = value.toLocaleString('en-US');
    svg.append(text);
  }
  for (const tick of monthTicks(first, last)) {
    const px = last === first ? x(rows[0].report_date) : left + (tick.at - first) / (last - first) * (right - left);
    svg.append(svgElement('line', { x1: px, x2: px, y1: base, y2: base + 4, class: 'gridline' }));
    const text = svgElement('text', { x: px, y: base + 16, 'text-anchor': 'middle', class: 'axis-label' });
    text.textContent = tick.label;
    svg.append(text);
  }
  // The strip under the month labels where a report with no usable count is marked; its height is not a count.
  const strip = svgElement('text', { x: 5, y: missingY + 4, class: 'axis-label' });
  strip.textContent = 'no count';
  svg.append(strip);
  const legend = svgElement('text', { x: left, y: 14, class: 'axis-label series-legend' });
  legend.textContent = 'Cumulative confirmed cases · ● printed in the report · × no usable count';
  svg.append(legend);
  for (const row of rows) {
    const info = rowInfo(row, name, synthetic);
    const mark = row.cases.status === 'reported' ?
      svgElement('circle', { cx: x(row.report_date), cy: y(row.cases.count), r: 4.5, class: 'cumulative-point', 'data-count': row.cases.count }) :
      svgElement('path', { d: `M${x(row.report_date) - 4} ${missingY - 4}l8 8m-8 0l8-8`, class: 'cumulative-missing', 'data-reason': row.cases.reason });
    mark.setAttribute('data-report-date', row.report_date);
    const markTitle = svgElement('title', {});
    markTitle.textContent = `Report of ${row.report_date}: ${cumulativeText(row)}`;
    mark.append(markTitle);
    bindProvenance(mark as SVGElement, info);
    svg.append(mark);
  }
  return svg;
}

/** The exact values, one row per report date, for readers who cannot or would rather not use the chart. */
export function cumulativeTable(rows: readonly CumulativeCaseReport[], name: string, synthetic = false): HTMLTableElement {
  const table = document.createElement('table');
  table.className = 'cumulative-table';
  const caption = document.createElement('caption');
  caption.textContent = `${name} · ${cumulativeHeading}`;
  const head = document.createElement('thead');
  const headers = document.createElement('tr');
  for (const text of ['DSHS report date', 'Cumulative confirmed cases']) {
    const cell = document.createElement('th');
    cell.scope = 'col';
    cell.textContent = text;
    headers.append(cell);
  }
  head.append(headers);
  const body = document.createElement('tbody');
  for (const row of rows) {
    const tr = document.createElement('tr');
    tr.dataset.reportDate = row.report_date;
    const date = document.createElement('th');
    date.scope = 'row';
    date.textContent = row.report_date;
    const cell = document.createElement('td');
    if (row.cases.status === 'reported') cell.append(provenanceNumber(row.cases.count.toLocaleString('en-US'), rowInfo(row, name, synthetic)));
    else {
      cell.textContent = cumulativeText(row);
      cell.className = 'cumulative-no-data';
    }
    tr.append(date, cell);
    body.append(tr);
  }
  table.append(caption, head, body);
  return table;
}

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

/**
 * The county drill-down block: heading, chart and plain-words notes, or a statement that this county has no
 * series. Only rows under the confirmed-case definition are charted under that heading.
 */
export function cumulativeSection(rows: readonly CumulativeCaseReport[], geography: string, name: string, synthetic = false): HTMLElement[] {
  const series = cumulativeSeries(rows, geography);
  const confirmed = series.filter((r) => r.case_definition === 'confirmed');
  const blocks: HTMLElement[] = [element('h3', cumulativeHeading)];
  if (!confirmed.length) {
    blocks.push(element('p', 'No cumulative series for this county: no Texas DSHS report this site reads names it. That is not a count of zero.', 'notice cumulative-none'));
    return blocks;
  }
  if (confirmed.length < series.length) {
    blocks.push(element('p', `${series.length - confirmed.length} report rows use a different case definition and are not charted here.`, 'notice'));
  }
  blocks.push(cumulativeChart(confirmed, name, synthetic) as unknown as HTMLElement,
    element('p', cumulativeExplanation, 'chart-note cumulative-note'));
  const gaps = cumulativeGapRuns(confirmed);
  if (gaps.length) {
    const list = element('ul', undefined, 'cumulative-gaps');
    list.setAttribute('aria-label', 'Reports with no usable count for this county, with the reason');
    for (const run of gaps) list.append(element('li', run.text));
    blocks.push(element('p', cumulativeMissingExplanation, 'chart-note cumulative-missing-note'), list);
  }
  return blocks;
}
