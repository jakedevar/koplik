import { provenanceNumber } from './provenance';
import { formatRetrieved, provisionalWords, weekWords, type ExplorerSummary, type SummaryStat } from './summary';

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

/** What the drawer says about a national figure: what was summed, what was left out, and that nothing is estimated. */
function statNote(stat: SummaryStat): string {
  const left = stat.notIncluded.length ? ` Not included (no figure for it from the source's reports): ${stat.notIncluded.join(', ')}.` : ' Every jurisdiction is included.';
  const scope = `A sum over ${stat.included} of ${stat.total} state-level jurisdictions (states, the District of Columbia and territories)`;
  if (stat.id === 'latest-week') {
    const caveat = stat.provisional
      ? ` ${provisionalWords}: CDC's weekly counts are updated as reports arrive, and this is one of the two newest weeks, which the pipeline marks provisional on its R_t rows too. The count is shown as reported and is not adjusted.`
      : '';
    return `${scope} that have a reported count for this one week.${left} A jurisdiction is left out only when its count for this week is absent or missing, never counted as zero or estimated; one that reported zero is included.${caveat} Lists every source record attached to the summed weekly reports.`;
  }
  return `${scope} that have this figure in their own reports.${left} A jurisdiction with missing or incomplete weekly reports is left out, never counted as zero or estimated. Lists every source record attached to the summed weekly reports; this is not a full-year total.`;
}

/** The block at the top of the Explorer: when the data is from, and the headline numbers with their coverage. */
export function summarySection(summary: ExplorerSummary, synthetic: boolean): HTMLElement {
  const section = element('section', undefined, 'summary');
  section.setAttribute('aria-label', 'National summary');
  const asOf = element('p', undefined, 'summary-asof');
  if (summary.asOf) {
    const { week, retrieved, records } = summary.asOf;
    asOf.append('Data as of ', provenanceNumber(weekWords(week), { label: `Latest report week · ${weekWords(week)}`, records, synthetic,
      note: `The latest MMWR week in which at least one jurisdiction has a reported count. The date shown is the latest retrieval time recorded for the reports of that week; it comes from the source snapshot, not from this page.${summary.asOf.provisional ? ` ${provisionalWords}.` : ''}` }),
    ', the latest week with reports', summary.asOf.provisional ? ' (provisional: still subject to reporting delay)' : '');
    asOf.append(retrieved ? ` · source snapshot retrieved ${formatRetrieved(retrieved)}` : ' · retrieval time not recorded');
  } else asOf.textContent = 'As-of date unavailable: no jurisdiction has a reported weekly count.';
  const stats = element('dl', undefined, 'summary-stats');
  for (const stat of summary.stats) {
    const box = element('div', undefined, 'stat');
    box.dataset.stat = stat.id;
    const value = element('p', undefined, 'stat-value');
    if (stat.value === null) {
      value.textContent = 'No data';
      value.classList.add('missing-value');
    } else value.append(provenanceNumber(stat.value.toLocaleString('en-US'), { label: `United States · ${stat.label} · ${stat.value.toLocaleString('en-US')}`, records: stat.records, synthetic, note: statNote(stat) }));
    const dd = element('dd');
    dd.append(value);
    if (stat.provisional) dd.append(element('p', 'Provisional: this week is still subject to reporting delay and may change.', 'stat-caveat'));
    dd.append(element('p', stat.detail, 'stat-note'));
    box.append(element('dt', stat.label), dd);
    stats.append(box);
  }
  section.append(asOf, stats);
  return section;
}
